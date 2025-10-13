use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::error::{GisError, GisResult};
use crate::services::data::ArchiveService;
use crate::services::gis::{LayerService, Overlay, VectorService};
use crate::types::{BoundingBox, Dataset};
use crate::utils::{LayerProgress, PathBuilder, VulcainColors, clean_tmp, execute_sidecar};

struct LayerConfig {
    archive_name: &'static str,
    layer_type: &'static str,
    files: Vec<&'static str>,
    order: i8,
}

impl LayerConfig {
    fn get_configs() -> Vec<Self> {
        vec![
            Self {
                archive_name: "BDFORET",
                layer_type: "Végétation",
                files: vec!["FORMATION_VEGETALE"],
                order: 1,
            },
            Self {
                archive_name: "RPG",
                layer_type: "Parcelles agricoles",
                files: vec!["PARCELLES_GRAPHIQUES"],
                order: 2,
            },
            Self {
                archive_name: "BDTOPO",
                layer_type: "Topographie",
                files: vec![
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
        ]
    }

    fn by_order() -> BTreeMap<i8, Vec<&'static str>> {
        let mut result = BTreeMap::new();
        for config in Self::get_configs() {
            result.insert(config.order, config.files);
        }
        result
    }
}

pub struct ProcessingService;

impl ProcessingService {
    pub async fn prepare_layers(
        project_bb: &BoundingBox,
        code: &str,
    ) -> GisResult<(String, String, String, HashMap<String, Vec<String>>)> {
        let path_builder = PathBuilder::new();
        let mut layer_progress = LayerProgress::new("Préparation des Couches", 4);

        layer_progress.next_layer("étendue régionale");
        let regional_gpkg = Self::prepare_regional_layer(&path_builder, code, project_bb).await?;

        layer_progress.next_layer("extraction des données");
        let extracted_files = Self::extract_all_required_files(&path_builder, code).await?;

        let mut vegetation_gpkg = String::new();
        let mut rpg_gpkg = String::new();
        let mut topo_gpkgs: HashMap<String, Vec<String>> = HashMap::new();

        for config in LayerConfig::get_configs() {
            layer_progress.next_layer(&format!("traitement {}", config.layer_type));

            for (file_idx, file) in config.files.iter().enumerate() {
                layer_progress.layer_operation(
                    file,
                    "Traitement",
                    file_idx + 1,
                    config.files.len(),
                );

                if let Some(extracted_path) =
                    extracted_files.get(&format!("{}_{}", config.archive_name, file))
                {
                    let output_gpkg = Self::process_extracted_file(
                        &path_builder,
                        extracted_path,
                        file,
                        code,
                        project_bb,
                    )
                    .await?;

                    match config.order {
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
        }

        Ok((regional_gpkg, vegetation_gpkg, rpg_gpkg, topo_gpkgs))
    }

    async fn extract_all_required_files(
        path_builder: &PathBuilder,
        code: &str,
    ) -> GisResult<HashMap<String, String>> {
        let mut all_extracted_files = HashMap::new();

        for config in LayerConfig::get_configs() {
            let archive_path =
                path_builder.cache_file(&format!("{}_{}.7z", config.archive_name, code));

            if !Path::new(&archive_path).exists() {
                println!("Archive non trouvée: {}", archive_path);
                continue;
            }

            let file_names: Vec<&str> = config.files.to_vec();

            match ArchiveService::extract_multiple_files(
                &archive_path,
                &file_names,
                &path_builder.temp_dir,
            )
            .await
            {
                Ok(extracted) => {
                    for (file_name, file_path) in extracted {
                        let key = format!("{}_{}", config.archive_name, file_name);
                        all_extracted_files.insert(key, file_path);
                    }
                }
                Err(e) => {
                    println!(
                        "Erreur lors de l'extraction de {}: {}",
                        config.archive_name, e
                    );
                }
            }
        }

        Ok(all_extracted_files)
    }

    async fn prepare_regional_layer(
        path_builder: &PathBuilder,
        code: &str,
        project_bb: &BoundingBox,
    ) -> GisResult<String> {
        let temp_gpkg = path_builder.temp_file(code, "gpkg");
        let output_gpkg = path_builder.temp_file(&format!("{}_region", code), "gpkg");
        println!("Préparation de la couche régionale...");
        VectorService::clip_to_bb(&temp_gpkg, &output_gpkg, project_bb).await?;

        Ok(output_gpkg)
    }

    async fn process_extracted_file(
        path_builder: &PathBuilder,
        extracted_dir: &str,
        file_name: &str,
        code: &str,
        project_bb: &BoundingBox,
    ) -> GisResult<String> {
        let exts = ["gpkg", "shp", "geojson"];
        let source_file = exts
            .iter()
            .map(|ext| format!("{}/{}.{}", extracted_dir, file_name, ext))
            .find(|f| Path::new(f).exists())
            .ok_or_else(|| {
                GisError::Dataset(format!("No supported file found for {}", file_name))
            })?;

        let temp_gpkg = if source_file.ends_with(".gpkg") {
            source_file.clone()
        } else {
            let temp_gpkg = path_builder.temp_file(file_name, "gpkg");
            VectorService::convert_to_gpkg(&source_file, &temp_gpkg).await?;
            temp_gpkg
        };

        let output_gpkg = path_builder.temp_file(&format!("{}_{}", code, file_name), "gpkg");
        VectorService::clip_to_bb(&temp_gpkg, &output_gpkg, project_bb).await?;

        Ok(output_gpkg)
    }

    pub async fn add_all_layers(project_file_path: &str) -> GisResult<()> {
        let project_folder = Path::new(project_file_path)
            .parent()
            .ok_or_else(|| GisError::InvalidGeometry("Invalid project path".to_string()))?
            .to_string_lossy()
            .to_string();

        let project_name = Path::new(project_file_path)
            .file_stem()
            .ok_or_else(|| GisError::InvalidGeometry("Invalid project name".to_string()))?
            .to_string_lossy()
            .to_string();

        let path_builder = PathBuilder::new();
        let mut processor = LayerProcessor::new();
        let mut layer_progress = LayerProgress::new("Ajout des Couches", 4);

        layer_progress.next_layer("couche régionale");
        let regional_path = path_builder.project_resource(&project_folder, &project_name);
        processor
            .apply_black_layer(project_file_path, &regional_path, "regional")
            .await?;

        for (order, files) in LayerConfig::by_order() {
            let layer_type = LayerConfig::get_configs()
                .iter()
                .find(|l| l.order == order)
                .map(|l| l.layer_type)
                .unwrap_or("Inconnu");

            layer_progress.next_layer(&format!("couches {}", layer_type));

            for (idx, file) in files.iter().enumerate() {
                layer_progress.layer_operation(
                    &format!("couches {}", layer_type),
                    &format!("Ajout de {}", file),
                    idx + 1,
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
                        if Path::new(&layer_path).exists() {
                            let dataset = Dataset::open(&layer_path)
                                .await
                                .map_err(|e| GisError::Dataset(e.to_string()))?;
                            if let Some(count) = dataset
                                .feature_count(0)
                                .map_err(|e| GisError::Dataset(e.to_string()))?
                                && count > 0
                            {
                                processor
                                    .apply_black_layer(project_file_path, &layer_path, "topo")
                                    .await?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Self::integrity_check(project_file_path).await?;
        Ok(())
    }

    async fn integrity_check(project_file_path: &str) -> GisResult<()> {
        let (stdout, _) = execute_sidecar(
            "magick",
            &[project_file_path, "-format", "%c", "histogram:info:"],
        )
        .await
        .map_err(|e| GisError::Dataset(e.to_string()))?;

        let mut corrupted = false;
        for line in stdout.lines() {
            if let Some(start) = line.find('(')
                && let Some(end) = line[start..].find(')')
            {
                let rgb_str = &line[start + 1..start + end];
                let rgb_parts: Vec<&str> = rgb_str.split(',').collect();
                if rgb_parts.len() >= 3 {
                    let rgb = [
                        rgb_parts[0].trim(),
                        rgb_parts[1].trim(),
                        rgb_parts[2].trim(),
                    ];
                    if !VulcainColors.values().any(|c| c == &rgb) {
                        corrupted = true;
                        break;
                    }
                }
            }
        }

        if corrupted {
            println!("Warning: some colors are not in VulcainColors");
        } else {
            println!("All layer colors are valid");
        }

        Ok(())
    }
}

struct LayerProcessor {
    path_builder: PathBuilder,
}

impl LayerProcessor {
    fn new() -> Self {
        Self {
            path_builder: PathBuilder::new(),
        }
    }

    async fn apply_black_layer(
        &mut self,
        project_file: &str,
        gpkg_path: &str,
        prefix: &str,
    ) -> GisResult<()> {
        let temp_layer = self
            .path_builder
            .temp_file(&format!("temp_{}", prefix), "tif");
        let layer_name = Dataset::open(gpkg_path)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .layer_name(0)
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        LayerService::rasterize_layer(
            project_file,
            gpkg_path,
            &layer_name,
            &temp_layer,
            ["255", "255", "255"],
            None,
        )
        .await?;

        let mut overlay = Overlay::new();
        overlay
            .apply_overlay(project_file, &temp_layer, |&v| v > 0, Some([0, 0, 0]))
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        std::fs::remove_file(temp_layer)?;
        Ok(())
    }

    async fn apply_colored_layer(
        &mut self,
        project_file: &str,
        gpkg_path: &str,
        color: [&str; 3],
        prefix: &str,
    ) -> GisResult<()> {
        let temp_layer = self
            .path_builder
            .temp_file(&format!("temp_{}", prefix), "tif");
        let layer_name = Dataset::open(gpkg_path)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .layer_name(0)
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        LayerService::rasterize_layer(
            project_file,
            gpkg_path,
            &layer_name,
            &temp_layer,
            color,
            None,
        )
        .await?;

        let mut overlay = Overlay::new();
        overlay
            .apply_overlay(project_file, &temp_layer, |&v| v > 0, None)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        std::fs::remove_file(temp_layer)?;
        Ok(())
    }

    async fn apply_vegetation_layer(
        &mut self,
        project_file: &str,
        veg_gpkg: &str,
    ) -> GisResult<()> {
        let vegetation_types = vec![
            (
                vec![
                    "Feuillus",
                    "Châtaignier",
                    "Chênes sempervirents",
                    "Chênes décidus",
                    "Hêtre",
                ],
                VulcainColors["Chêne"],
                "feuillus",
            ),
            (vec!["NC", "NR"], VulcainColors["Brousaille"], "undefined"),
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

            let temp_layer = self.path_builder.temp_file(prefix, "tif");
            let layer_name = Dataset::open(veg_gpkg)
                .await
                .map_err(|e| GisError::Dataset(e.to_string()))?
                .layer_name(0)
                .map_err(|e| GisError::Dataset(e.to_string()))?;

            LayerService::rasterize_layer(
                project_file,
                veg_gpkg,
                &layer_name,
                &temp_layer,
                color,
                Some(&where_clause),
            )
            .await?;

            let mut overlay = Overlay::new();
            overlay
                .apply_overlay(project_file, &temp_layer, |&v| v > 0, None)
                .await
                .map_err(|e| GisError::Dataset(e.to_string()))?;

            std::fs::remove_file(temp_layer)?;
        }

        let all_defined: Vec<String> = [
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

        let other_where = format!("ESSENCE NOT IN ({})", all_defined.join(", "));
        let temp_layer = self.path_builder.temp_file("other", "tif");
        let layer_name = Dataset::open(veg_gpkg)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .layer_name(0)
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        LayerService::rasterize_layer(
            project_file,
            veg_gpkg,
            &layer_name,
            &temp_layer,
            VulcainColors["Pin"],
            Some(&other_where),
        )
        .await?;

        let mut overlay = Overlay::new();
        overlay
            .apply_overlay(project_file, &temp_layer, |&v| v > 0, None)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        std::fs::remove_file(temp_layer)?;
        clean_tmp(None).map_err(|e| GisError::Dataset(e.to_string()))?;

        Ok(())
    }
}
