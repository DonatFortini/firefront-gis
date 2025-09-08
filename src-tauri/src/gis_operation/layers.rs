use rusqlite::Connection;
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
    types::{BoundingBox, Driver, GTiff},
    utils::{
        LayerProgress, VulcainColors, cache_dir, clean_tmp, executor, extract_files_by_name,
        temp_dir,
    },
};

fn get_layer_name(
    gpkg_path: &str,
    layer_index: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    let conn = Connection::open(gpkg_path)?;

    let mut stmt = conn.prepare(
        "SELECT table_name FROM gpkg_contents WHERE data_type = 'features' ORDER BY table_name LIMIT 1 OFFSET ?"
    )?;

    let layer_name: String = stmt.query_row([layer_index], |row| row.get::<_, String>(0))?;

    Ok(layer_name)
}

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
    let layer_name = get_layer_name(gpkg_path, 0)?;
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

    apply_overlay(project_file_path, &temp_layer, |&value| value > 0)?;

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
    let vegetation_layer_name = get_layer_name(vegetation_gpkg, 0)?;

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

    // Rastériser chaque type exactement comme l'original
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

    // Appliquer EXACTEMENT dans l'ordre de priorité de l'original :
    // 1. Feuillus d'abord (priorité 1)
    apply_overlay(project_file_path, &temp_feuillus, |&value| value > 0)?;

    // 2. Undefined ensuite (priorité 2) - seulement sur les zones encore transparentes
    apply_overlay(project_file_path, &temp_undefined, |&value| value > 0)?;

    // 3. Other en dernier (priorité 3) - seulement sur les zones encore transparentes
    apply_overlay(project_file_path, &temp_other, |&value| value > 0)?;

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
    let project_info = get_raster_info(project_file_path).await?;
    let layer_name = get_layer_name(topo_gpkg, 0)?;

    // Vérifier s'il y a des features
    let conn = Connection::open(topo_gpkg)?;
    let feature_count: i64 =
        conn.query_row(&format!("SELECT COUNT(*) FROM {}", layer_name), [], |row| {
            row.get(0)
        })?;

    if feature_count == 0 {
        println!("Layer has no features");
        conn.close().unwrap();
        return Ok(());
    }

    let geom_type_name: String = conn.query_row(
        "SELECT geometry_type_name FROM gpkg_geometry_columns WHERE table_name = ?1",
        [&layer_name],
        |row| row.get(0),
    )?;

    let geom_type = match geom_type_name.to_uppercase().as_str() {
        "LINESTRING" | "MULTILINESTRING" => "LineString",
        "POLYGON" | "MULTIPOLYGON" => "Polygon",
        "POINT" | "MULTIPOINT" => "Point",
        _ => "Unknown",
    };
    conn.close().unwrap();

    let temp_topo_layer = format!("{}/temp_topo_layer.tif", temp_dir().to_string_lossy());

    // Créer un raster vide avec les bonnes dimensions
    Driver::<GTiff>::new()
        .create(&[
            "-ot",
            "Byte",
            "-outsize",
            &project_info.width.to_string(),
            &project_info.height.to_string(),
            "-bands",
            "3",
            "-a_srs",
            &project_info.projection,
            "-a_ullr",
            &project_info.geo_transform[0].to_string(),
            &project_info.geo_transform[3].to_string(),
            &(project_info.geo_transform[0]
                + (project_info.geo_transform[1] * project_info.width as f64))
                .to_string(),
            &(project_info.geo_transform[3]
                + (project_info.geo_transform[5] * project_info.height as f64))
                .to_string(),
            &temp_topo_layer,
        ])
        .await?;

    // Rastériser directement les géométries avec la valeur 1 (pour créer un masque)
    let mut args = vec!["-burn", "1", "-burn", "1", "-burn", "1", "-l", &layer_name];
    if geom_type == "LineString" {
        args.push("-at");
    }
    args.extend_from_slice(&[topo_gpkg, &temp_topo_layer]);

    executor("gdal_rasterize", &args).await?;

    // Appliquer EXACTEMENT comme l'original : où topo > 0, mettre le projet à noir (0)
    // Cette logique correspond à : if mask_value { 0 } else { base_value }
    apply_overlay(project_file_path, &temp_topo_layer, |&value| value > 0)?;

    std::fs::remove_file(&temp_topo_layer)?;
    Ok(())
}

pub mod prelude {
    pub use super::{
        RasterInfo, add_layers, add_regional_layer, add_rpg_layer, add_topo_layer,
        add_vegetation_layer, get_raster_info, prepare_layers,
    };
}

pub async fn get_raster_info(file_path: &str) -> Result<RasterInfo, Box<dyn std::error::Error>> {
    let output = executor("gdalinfo", &["-json", file_path]).await?.0;
    let info: serde_json::Value = serde_json::from_str(&output)?;

    let size = info["size"].as_array().ok_or("No size info")?;
    let width = size[0].as_u64().ok_or("Invalid width")? as usize;
    let height = size[1].as_u64().ok_or("Invalid height")? as usize;

    let geo_transform = info["geoTransform"]
        .as_array()
        .ok_or("No geotransform")?
        .iter()
        .map(|v| v.as_f64().ok_or("Invalid geotransform value"))
        .collect::<Result<Vec<f64>, _>>()?;

    let projection = info["coordinateSystem"]["wkt"]
        .as_str()
        .ok_or("No projection info")?
        .to_string();

    Ok(RasterInfo {
        width,
        height,
        geo_transform,
        projection,
    })
}

#[derive(Debug)]
pub struct RasterInfo {
    pub width: usize,
    pub height: usize,
    pub geo_transform: Vec<f64>,
    pub projection: String,
}
