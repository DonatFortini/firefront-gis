use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use super::{
    create_region_geojson,
    processing::{apply_overlay, rasterize_layer},
    {clip_to_bb, convert_to_gpkg},
};

use crate::{
    types::{BoundingBox, Dataset, Driver, GTiff},
    utils::{
        LayerProgress, VulcainColors, cache_dir, clean_tmp, executor, extract_files_by_name,
        temp_dir,
    },
};

/// Prépare les couches pour le projet, en les convertissant au format GPKG et en les découpant à l'extent régional.
/// Retourne les chemins vers les fichiers GPKG pour chaque type de couche
///
/// # Arguments
///
/// * `app_handle` - Handle de l'application Tauri
/// * `project_bb` - BoundingBox du projet
/// * `code` - Code départemental de la région traitée
///
/// # Returns
///
/// * `Result<(String, String, String, HashMap<String, Vec<String>>), String>` - Un tuple contenant les chemins vers les fichiers GPKG pour la région, la végétation, le RPG et les couches topographiques
pub async fn prepare_layers(
    project_bb: &BoundingBox,
    code: &str,
) -> Result<(String, String, String, HashMap<String, Vec<String>>), String> {
    let cache_folder_path = cache_dir().to_string_lossy().to_string();
    let temp_dir = temp_dir().to_string_lossy().to_string();

    let mut layer_progress = LayerProgress::new("Préparation des Couches", 4);
    layer_progress.next_layer("étendue régionale");

    let regional_geojson_path = format!("{temp_dir}/{code}.geojson");
    create_region_geojson(code, &regional_geojson_path).unwrap();

    let temp_regional_gpkg = format!("{temp_dir}/{code}.gpkg");
    let regional_gpkg = format!("{temp_dir}/{code}_region.gpkg");

    convert_to_gpkg(&regional_geojson_path, &temp_regional_gpkg)
        .await
        .unwrap();
    clip_to_bb(&temp_regional_gpkg, &regional_gpkg, project_bb)
        .await
        .unwrap();

    let mut layers: HashMap<String, Vec<&str>> = HashMap::new();
    layers.insert(format!("BDFORET_{code}.7z"), vec!["FORMATION_VEGETALE"]);
    layers.insert(format!("RPG_{code}.7z"), vec!["PARCELLES_GRAPHIQUES"]);
    layers.insert(
        format!("BDTOPO_{code}.7z"),
        vec![
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
    );

    let mut vegetation_gpkg = String::new();
    let mut rpg_gpkg = String::new();
    let mut topo_gpkgs: HashMap<String, Vec<String>> = HashMap::new();

    for (archive, files) in layers {
        let layer_type = if archive.contains("BDFORET") {
            "Végétation"
        } else if archive.contains("RPG") {
            "Parcelles agricoles"
        } else if archive.contains("BDTOPO") {
            "Topographie"
        } else {
            "Inconnu"
        };

        layer_progress.next_layer(&format!("couches {layer_type}"));

        let archive_path = format!("{cache_folder_path}/{archive}");
        let total_files = files.len();

        for (file_index, file) in files.iter().enumerate() {
            layer_progress.layer_operation(file, "Extraction", file_index + 1, total_files);
            extract_files_by_name(&archive_path, file, &temp_dir).await.map_err(|e| {
                format!(
                    "Erreur lors de l'extraction du fichier {file} depuis l'archive {archive}: {e:?}"
                )
            })?;

            let temp_file = format!("{temp_dir}/{file}/{file}.shp");
            let temp_gpkg = format!("{temp_dir}/{file}.gpkg");
            let output_gpkg = format!("{temp_dir}/{code}_{file}.gpkg");

            layer_progress.layer_operation(file, "Conversion", file_index + 1, total_files);

            if let Err(e) = convert_to_gpkg(&temp_file, &temp_gpkg).await {
                return Err(format!(
                    "Erreur lors de la conversion du fichier {temp_file} en GPKG: {e:?}"
                ));
            }

            layer_progress.layer_operation(file, "Découpage", file_index + 1, total_files);

            if let Err(e) = clip_to_bb(&temp_gpkg, &output_gpkg, project_bb).await {
                return Err(format!(
                    "Erreur lors du découpage du fichier {temp_gpkg}: {e:?}"
                ));
            }

            if file == &"FORMATION_VEGETALE" {
                vegetation_gpkg = output_gpkg.clone();
            } else if file == &"PARCELLES_GRAPHIQUES" {
                rpg_gpkg = output_gpkg.clone();
            } else {
                topo_gpkgs
                    .entry(file.to_string())
                    .or_default()
                    .push(output_gpkg.clone());
            }
        }
    }

    Ok((regional_gpkg, vegetation_gpkg, rpg_gpkg, topo_gpkgs))
}

/// Ajoute les couches au projet.
/// Cette fonction est responsable de l'ajout des couches régionales, de végétation, de RPG et topographiques
/// au projet en utilisant les chemins fournis.
/// Elle émet également des événements de mise à jour de progression pour informer l'utilisateur
/// de l'état d'avancement de l'ajout des couches.
///
/// # Arguments
///
/// * `app_handle` - Handle de l'application Tauri
/// * `project_folder` - chemin du dossier du projet
/// * `project_file_path` - chemin du fichier projet
/// * `project_name` - nom du projet
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
pub async fn add_layers(project_file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let project_file_path_obj = Path::new(project_file_path);
    let project_folder = project_file_path_obj
        .parent()
        .ok_or("Invalid project_file_path: no parent directory")?
        .to_string_lossy()
        .to_string();
    let project_name = project_file_path_obj
        .file_stem()
        .ok_or("Invalid project_file_path: no file stem")?
        .to_string_lossy()
        .to_string();

    let mut layer_progress = LayerProgress::new("Ajout des Couches", 4);

    layer_progress.next_layer("couche régionale");

    if let Err(e) = add_regional_layer(
        project_file_path,
        &format!("{project_folder}/resources/{project_name}.gpkg"),
    )
    .await
    {
        println!("Failed to add regional layer: {e:?}");
        return Err(e);
    }

    let mut layers: BTreeMap<i8, Vec<&str>> = BTreeMap::new();
    layers.insert(1, vec!["FORMATION_VEGETALE"]);
    layers.insert(2, vec!["PARCELLES_GRAPHIQUES"]);
    layers.insert(
        3,
        vec![
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
    );

    for (key, value) in layers {
        let layer_type = match key {
            1 => "Végétation",
            2 => "Parcelles agricoles",
            3 => "Topographie",
            _ => "Inconnu",
        };

        layer_progress.next_layer(&format!("couches {layer_type}"));

        let total_files = value.len();
        for (file_index, file) in value.iter().enumerate() {
            layer_progress.layer_operation(
                &format!("couches {layer_type}"),
                &format!("Ajout de {file}"),
                file_index + 1,
                total_files,
            );

            let layer_path = format!("{project_folder}/resources/{file}.gpkg");
            match key {
                1 => add_vegetation_layer(project_file_path, &layer_path).await,
                2 => add_rpg_layer(project_file_path, &layer_path).await,
                3 => add_topo_layer(project_file_path, &layer_path).await,
                _ => {
                    println!("Unknown layer type");
                    return Err(Box::new(std::io::Error::other("Unknown layer type")));
                }
            }?
        }
    }

    Ok(())
}

/// Ajoute une couche simple à un projet avec une couleur donnée
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `gpkg_path` - chemin du fichier GeoPackage contenant les données
/// * `color` - couleur RGB à appliquer [R, G, B]
/// * `temp_file_prefix` - préfixe pour le fichier temporaire
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
async fn add_simple_layer(
    project_file_path: &str,
    gpkg_path: &str,
    color: [&str; 3],
    temp_file_prefix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let layer_name = Dataset::open(gpkg_path).await?.layer_name(0)?;
    let temp_layer = format!(
        "{}/temp_{}.tif",
        temp_dir().to_string_lossy(),
        temp_file_prefix
    );

    rasterize_layer(
        project_file_path,
        gpkg_path,
        &layer_name,
        &temp_layer,
        color,
        None,
        None,
    )
    .await?;

    apply_overlay(project_file_path, &temp_layer, |&value| value > 0).await?;

    std::fs::remove_file(temp_layer)?;

    Ok(())
}

/// Ajoute une couche départementale à un projet
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `regional_gpkg` - chemin du fichier GeoPackage contenant les données départementales
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
pub async fn add_regional_layer(
    project_file_path: &str,
    regional_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    add_simple_layer(
        project_file_path,
        regional_gpkg,
        VulcainColors["Incombustible"],
        "regional",
    )
    .await
}

/// Ajoute une couche RPG (Registre Parcellaire Graphique) à un projet
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `rpg_gpkg` - chemin du fichier GeoPackage contenant les données RPG
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
pub async fn add_rpg_layer(
    project_file_path: &str,
    rpg_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    add_simple_layer(
        project_file_path,
        rpg_gpkg,
        VulcainColors["Brousaille"],
        "rpg",
    )
    .await
}

/// Ajoute une couche de végétation à un projet en distinguant différents types
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `vegetation_gpkg` - chemin du fichier GeoPackage contenant les données de végétation
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
pub async fn add_vegetation_layer(
    project_file_path: &str,
    vegetation_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let vegetation_layer_name = Dataset::open(vegetation_gpkg).await?.layer_name(0)?;

    let feuillus_types = [
        "Feuillus",
        "Châtaignier",
        "Chênes sempervirents",
        "Chênes décidus",
        "Hêtre",
    ];
    let undefined_types = ["NC", "NR"];

    let feuillus_where = format!(
        "ESSENCE IN ('{}', '{}', '{}', '{}', '{}')",
        feuillus_types[0],
        feuillus_types[1],
        feuillus_types[2],
        feuillus_types[3],
        feuillus_types[4]
    );

    let undefined_where = format!(
        "ESSENCE IN ('{}', '{}')",
        undefined_types[0], undefined_types[1]
    );

    let all_types = feuillus_types
        .iter()
        .chain(undefined_types.iter())
        .map(|t| format!("'{t}'"))
        .collect::<Vec<String>>()
        .join(", ");
    let other_where = format!("ESSENCE NOT IN ({all_types})");

    let temp_path = temp_dir().to_string_lossy().to_string();
    let temp_feuillus = format!("{temp_path}/temp_feuillus.tif");
    let temp_undefined = format!("{temp_path}/temp_undefined.tif");
    let temp_other = format!("{temp_path}/temp_other.tif");

    rasterize_layer(
        project_file_path,
        vegetation_gpkg,
        &vegetation_layer_name,
        &temp_feuillus,
        VulcainColors["Chêne"],
        Some(&feuillus_where),
        None,
    )
    .await?;

    rasterize_layer(
        project_file_path,
        vegetation_gpkg,
        &vegetation_layer_name,
        &temp_undefined,
        VulcainColors["Brousaille"],
        Some(&undefined_where),
        None,
    )
    .await?;

    rasterize_layer(
        project_file_path,
        vegetation_gpkg,
        &vegetation_layer_name,
        &temp_other,
        VulcainColors["Pin"],
        Some(&other_where),
        None,
    )
    .await?;

    apply_overlay(project_file_path, &temp_feuillus, |&value| value > 0).await?;
    apply_overlay(project_file_path, &temp_undefined, |&value| value > 0).await?;
    apply_overlay(project_file_path, &temp_other, |&value| value > 0).await?;

    clean_tmp(None)?;
    Ok(())
}

/// Ajoute une couche topographique à un projet
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `topo_gpkg` - chemin du fichier GeoPackage contenant les données topographiques
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
pub async fn add_topo_layer(
    project_file_path: &str,
    topo_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_info = Dataset::open(project_file_path).await?;
    let (width, height) = project_info.raster_size()?;
    let bbox = project_info.bbox();

    let gpkg_info = Dataset::open(topo_gpkg).await?;
    let layer_name = gpkg_info.layer_name(0)?;

    if gpkg_info.feature_count(0)? == Some(0) {
        println!("Layer has no features");
        return Ok(());
    }

    let geom_type_name = gpkg_info.geometry_type(0)?;

    let geom_type = match geom_type_name.to_uppercase().as_str() {
        "LINESTRING" | "MULTILINESTRING" => "LineString",
        "POLYGON" | "MULTIPOLYGON" => "Polygon",
        "POINT" | "MULTIPOINT" => "Point",
        _ => "Unknown",
    };

    let temp_topo_layer = format!("{}/temp_topo_layer.tif", temp_dir().to_string_lossy());

    Driver::<GTiff>::new()
        .create(&[
            "-ot",
            "Byte",
            "-outsize",
            &width.to_string(),
            &height.to_string(),
            "-bands",
            "3",
            "-a_srs",
            &project_info.projection(),
            "-a_ullr",
            &bbox.xmin.to_string(),
            &bbox.ymax.to_string(),
            &bbox.xmax.to_string(),
            &bbox.ymin.to_string(),
            &temp_topo_layer,
        ])
        .await?;

    let mut args = vec!["-burn", "1", "-burn", "1", "-burn", "1", "-l", &layer_name];
    if geom_type == "LineString" {
        args.push("-at");
    }
    args.extend_from_slice(&[topo_gpkg, &temp_topo_layer]);

    executor("gdal_rasterize", &args).await?;

    apply_overlay(project_file_path, &temp_topo_layer, |&value| value > 0).await?;

    std::fs::remove_file(&temp_topo_layer)?;
    Ok(())
}

pub mod prelude {
    pub use super::{
        add_layers, add_regional_layer, add_rpg_layer, add_topo_layer, add_vegetation_layer,
        prepare_layers,
    };
}
