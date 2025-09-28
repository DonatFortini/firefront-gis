use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use super::{
    create_region_geojson, {Overlay, clip_to_bb, convert_to_gpkg, integrity_check, rasterize_layer},
};

use crate::{
    types::{BoundingBox, Dataset},
    utils::{LayerProgress, PathBuilder, VulcainColors, clean_tmp, extract_files_by_name},
};

struct LayerConfig {
    pub archive_name: &'static str,
    pub layer_type: &'static str,
    pub files: &'static [&'static str],
    pub order: i8,
}

impl LayerConfig {
    const LAYERS: &'static [LayerConfig] = &[
        LayerConfig {
            archive_name: "BDFORET",
            layer_type: "Végétation",
            files: &["FORMATION_VEGETALE"],
            order: 1,
        },
        LayerConfig {
            archive_name: "RPG",
            layer_type: "Parcelles agricoles",
            files: &["PARCELLES_GRAPHIQUES"],
            order: 2,
        },
        LayerConfig {
            archive_name: "BDTOPO",
            layer_type: "Topographie",
            files: &[
                "AERODROME",
                "CONSTRUCTION_SURFACIQUE",
                "EQUIPEMENT_DE_TRANSPORT",
                "RESERVOIR",
                "TERRAIN_DE_SPORT",
                "TRONCON_DE_VOIE_FERREE",
                "ZONE_D_ESTRAN",
                "BATIMENT",
                "COURS_D_EAU",
                "PLAN_D_EAU",
                "SURFACE_HYDROGRAPHIQUE",
                "TRONCON_DE_ROUTE",
                "VOIE_NOMMEE",
            ],
            order: 3,
        },
    ];

    fn get_archive_path(&self, code: &str) -> String {
        format!("{}_{}.7z", self.archive_name, code)
    }

    fn by_order() -> BTreeMap<i8, Vec<&'static str>> {
        let mut result = BTreeMap::new();
        for layer in Self::LAYERS {
            result.insert(layer.order, layer.files.to_vec());
        }
        result
    }
}

struct LayerProcessor {
    overlay: Overlay,
    path_builder: PathBuilder,
}

impl LayerProcessor {
    fn new() -> Self {
        Self {
            overlay: Overlay::new(),
            path_builder: PathBuilder::new(),
        }
    }

    async fn apply_layer(
        &mut self,
        project_file_path: &str,
        gpkg_path: &str,
        color: [&str; 3],
        fixed_color: Option<[u8; 3]>,
        temp_prefix: &str,
        where_clause: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let layer_name = Dataset::open(gpkg_path).await?.layer_name(0)?;
        let temp_layer = self
            .path_builder
            .temp_file(&format!("temp_{}", temp_prefix), "tif");

        rasterize_layer(
            project_file_path,
            gpkg_path,
            &layer_name,
            &temp_layer,
            color,
            where_clause,
            None,
        )
        .await?;

        self.overlay
            .apply_overlay(
                project_file_path,
                &temp_layer,
                |&value| value > 0,
                fixed_color,
            )
            .await?;

        std::fs::remove_file(temp_layer)?;
        Ok(())
    }

    async fn apply_colored_layer(
        &mut self,
        project_file_path: &str,
        gpkg_path: &str,
        color: [&str; 3],
        temp_prefix: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_layer(project_file_path, gpkg_path, color, None, temp_prefix, None)
            .await
    }

    async fn apply_black_layer(
        &mut self,
        project_file_path: &str,
        gpkg_path: &str,
        temp_prefix: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_layer(
            project_file_path,
            gpkg_path,
            ["255", "255", "255"],
            Some([0, 0, 0]),
            temp_prefix,
            None,
        )
        .await
    }

    async fn apply_vegetation_layer(
        &mut self,
        project_file_path: &str,
        vegetation_gpkg: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let vegetation_types = vec![
            (
                &[
                    "Feuillus",
                    "Châtaignier",
                    "Chênes sempervirents",
                    "Chênes décidus",
                    "Hêtre",
                ][..],
                VulcainColors["Chêne"],
                "feuillus",
            ),
            (&["NC", "NR"][..], VulcainColors["Brousaille"], "undefined"),
        ];

        for (types, color, prefix) in vegetation_types {
            let where_clause = format!(
                "ESSENCE IN ({})",
                types
                    .iter()
                    .map(|t| format!("'{}'", t))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            self.apply_layer(
                project_file_path,
                vegetation_gpkg,
                color,
                None,
                prefix,
                Some(&where_clause),
            )
            .await?;
        }

        let all_defined_types: Vec<String> = [
            "Feuillus",
            "Châtaignier",
            "Chênes sempervirents",
            "Chênes décidus",
            "Hêtre",
            "NC",
            "NR",
        ]
        .iter()
        .map(|t| format!("'{}'", t))
        .collect();

        let other_where = format!("ESSENCE NOT IN ({})", all_defined_types.join(", "));

        self.apply_layer(
            project_file_path,
            vegetation_gpkg,
            VulcainColors["Pin"],
            None,
            "other",
            Some(&other_where),
        )
        .await?;

        clean_tmp(None)?;
        Ok(())
    }
}

pub async fn prepare_layers(
    project_bb: &BoundingBox,
    code: &str,
) -> Result<(String, String, String, HashMap<String, Vec<String>>), String> {
    let path_builder = PathBuilder::new();
    let mut layer_progress = LayerProgress::new("Préparation des Couches", 4);

    layer_progress.next_layer("étendue régionale");
    let regional_gpkg = prepare_regional_layer(&path_builder, code, project_bb).await?;

    let mut vegetation_gpkg = String::new();
    let mut rpg_gpkg = String::new();
    let mut topo_gpkgs: HashMap<String, Vec<String>> = HashMap::new();

    for layer_config in LayerConfig::LAYERS {
        layer_progress.next_layer(&format!("couches {}", layer_config.layer_type));

        let archive_path = path_builder.cache_file(&layer_config.get_archive_path(code));

        for (file_index, file) in layer_config.files.iter().enumerate() {
            let params = ProcessLayerParams {
                path_builder: &path_builder,
                archive_path: &archive_path,
                file,
                code,
                project_bb,
                layer_progress: &mut layer_progress,
                file_index,
                total_files: layer_config.files.len(),
            };
            let output_gpkg = process_layer_file(params).await?;

            match layer_config.order {
                1 => vegetation_gpkg = output_gpkg,
                2 => rpg_gpkg = output_gpkg,
                3 => topo_gpkgs
                    .entry(file.to_string())
                    .or_default()
                    .push(output_gpkg),
                _ => {}
            }
        }
    }

    Ok((regional_gpkg, vegetation_gpkg, rpg_gpkg, topo_gpkgs))
}

async fn prepare_regional_layer(
    path_builder: &PathBuilder,
    code: &str,
    project_bb: &BoundingBox,
) -> Result<String, String> {
    let regional_geojson_path = path_builder.temp_file(code, "geojson");
    let temp_regional_gpkg = path_builder.temp_file(code, "gpkg");
    let regional_gpkg = path_builder.temp_file(&format!("{}_region", code), "gpkg");

    create_region_geojson(code, &regional_geojson_path)
        .map_err(|e| format!("Erreur création géojson régional: {:?}", e))?;

    convert_to_gpkg(&regional_geojson_path, &temp_regional_gpkg)
        .await
        .map_err(|e| format!("Erreur conversion régionale: {:?}", e))?;

    clip_to_bb(&temp_regional_gpkg, &regional_gpkg, project_bb)
        .await
        .map_err(|e| format!("Erreur découpage régional: {:?}", e))?;

    Ok(regional_gpkg)
}

struct ProcessLayerParams<'a> {
    path_builder: &'a PathBuilder,
    archive_path: &'a str,
    file: &'a str,
    code: &'a str,
    project_bb: &'a BoundingBox,
    layer_progress: &'a mut LayerProgress,
    file_index: usize,
    total_files: usize,
}

async fn process_layer_file(params: ProcessLayerParams<'_>) -> Result<String, String> {
    params.layer_progress.layer_operation(
        params.file,
        "Extraction",
        params.file_index + 1,
        params.total_files,
    );
    extract_files_by_name(
        params.archive_path,
        params.file,
        &params.path_builder.temp_dir,
    )
    .await
    .map_err(|e| format!("Erreur extraction {}: {:?}", params.file, e))?;

    params.layer_progress.layer_operation(
        params.file,
        "Conversion",
        params.file_index + 1,
        params.total_files,
    );
    let temp_file = format!(
        "{}/{}/{}.shp",
        params.path_builder.temp_dir, params.file, params.file
    );
    let temp_gpkg = params.path_builder.temp_file(params.file, "gpkg");
    convert_to_gpkg(&temp_file, &temp_gpkg)
        .await
        .map_err(|e| format!("Erreur conversion {}: {:?}", params.file, e))?;

    params.layer_progress.layer_operation(
        params.file,
        "Découpage",
        params.file_index + 1,
        params.total_files,
    );
    let output_gpkg = params
        .path_builder
        .temp_file(&format!("{}_{}", params.code, params.file), "gpkg");
    clip_to_bb(&temp_gpkg, &output_gpkg, params.project_bb)
        .await
        .map_err(|e| format!("Erreur découpage {}: {:?}", params.file, e))?;

    Ok(output_gpkg)
}

pub async fn add_layers(project_file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let project_path = Path::new(project_file_path);
    let project_folder = project_path
        .parent()
        .ok_or("Invalid project_file_path: no parent directory")?
        .to_string_lossy()
        .to_string();
    let project_name = project_path
        .file_stem()
        .ok_or("Invalid project_file_path: no file stem")?
        .to_string_lossy()
        .to_string();

    let path_builder = PathBuilder::new();
    let mut layer_progress = LayerProgress::new("Ajout des Couches", 4);
    let mut processor = LayerProcessor::new();

    layer_progress.next_layer("couche régionale");
    processor
        .apply_black_layer(
            project_file_path,
            &path_builder.project_resource(&project_folder, &project_name),
            "regional",
        )
        .await?;

    for (order, files) in LayerConfig::by_order() {
        let layer_type = LayerConfig::LAYERS
            .iter()
            .find(|l| l.order == order)
            .map(|l| l.layer_type)
            .unwrap_or("Inconnu");

        layer_progress.next_layer(&format!("couches {}", layer_type));

        for (file_index, file) in files.iter().enumerate() {
            layer_progress.layer_operation(
                &format!("couches {}", layer_type),
                &format!("Ajout de {}", file),
                file_index + 1,
                files.len(),
            );

            let layer_path = path_builder.project_resource(&project_folder, file);

            match order {
                1 => {
                    processor
                        .apply_vegetation_layer(project_file_path, &layer_path)
                        .await?
                }
                2 => {
                    processor
                        .apply_colored_layer(
                            project_file_path,
                            &layer_path,
                            VulcainColors["Brousaille"],
                            "rpg",
                        )
                        .await?
                }
                3 => {
                    let dataset = Dataset::open(&layer_path).await?;
                    if dataset.feature_count(0)? != Some(0) {
                        processor
                            .apply_black_layer(project_file_path, &layer_path, "topo")
                            .await?;
                    }
                }
                _ => return Err("Type de couche inconnu".into()),
            }
        }
    }

    integrity_check(project_file_path).await?;
    Ok(())
}

pub async fn add_regional_layer(
    project_file_path: &str,
    regional_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    LayerProcessor::new()
        .apply_black_layer(project_file_path, regional_gpkg, "regional")
        .await
}

pub async fn add_rpg_layer(
    project_file_path: &str,
    rpg_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    LayerProcessor::new()
        .apply_colored_layer(
            project_file_path,
            rpg_gpkg,
            VulcainColors["Brousaille"],
            "rpg",
        )
        .await
}

pub async fn add_vegetation_layer(
    project_file_path: &str,
    vegetation_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    LayerProcessor::new()
        .apply_vegetation_layer(project_file_path, vegetation_gpkg)
        .await
}

pub async fn add_topo_layer(
    project_file_path: &str,
    topo_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dataset = Dataset::open(topo_gpkg).await?;
    if dataset.feature_count(0)? == Some(0) {
        return Ok(());
    }
    LayerProcessor::new()
        .apply_black_layer(project_file_path, topo_gpkg, "topo")
        .await
}

pub mod prelude {
    pub use super::{
        add_layers, add_regional_layer, add_rpg_layer, add_topo_layer, add_vegetation_layer,
        prepare_layers,
    };
}
