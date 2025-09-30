use geo::{Geometry, Intersects, Relate};
use geojson::GeoJson;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self};
use std::path::Path;

use crate::types::{BoundingBox, Driver, GTiff, Region, get_region};
use crate::utils::{execute_sidecar, resolution, resource_dir};

pub mod merger;
pub mod overlay;

pub use merger::{MergeOptions, Merger};
pub use overlay::Overlay;

/// Crée un fichier raster de référence avec une résolution donnée (10m/pixel)
/// et calcule la taille de l'image en fonction de la boîte englobante
///
/// # Arguments
///
/// * `output_raster_path` - chemin du fichier raster de sortie
/// * `project_bb` - coordonnées de la boîte englobante du projet
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si la création a réussi ou échoué
///
///
/// # Example
///
/// ```rust
///
/// use crate::gis_processing::create_reference_raster;
/// use crate::utils::BoundingBox;
///
///
/// let output_raster_path = "path/to/reference.tif";
///
/// let project_bb = BoundingBox {
///     xmin: 1210000.0,
///     ymin: 6070000.0,
///     xmax: 1235000.0,
///     ymax: 6095000.0,
/// };
///
/// create_reference_raster(output_raster_path, &project_bb).unwrap();
///
///```
///
///
pub async fn create_reference_raster(
    project_file_path: &str,
    project_bb: &BoundingBox,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolution = resolution();
    let width = (project_bb.width() / resolution).ceil() as usize;
    let height = (project_bb.height() / resolution).ceil() as usize;

    if !width.is_multiple_of(500) || !height.is_multiple_of(500) {
        return Err("Width and height must be multiples of 500".into());
    }

    let args = [
        "-ot",
        "Byte",
        "-outsize",
        &width.to_string(),
        &height.to_string(),
        "-bands",
        "4",
        "-burn",
        "0",
        "-burn",
        "0",
        "-burn",
        "0",
        "-burn",
        "255",
        "-a_srs",
        "EPSG:2154",
        "-a_ullr",
        &project_bb.xmin.to_string(),
        &project_bb.ymax.to_string(),
        &project_bb.xmax.to_string(),
        &project_bb.ymin.to_string(),
        "-co",
        "TILED=YES",
        "-co",
        "COMPRESS=LZW",
        "-co",
        "BIGTIFF=IF_SAFER",
        project_file_path,
    ];

    Driver::<GTiff>::new().create(&args).await?;

    Ok(())
}

/// Convertit un fichier en format GeoPackage (GPKG) en utilisant ogr2ogr
///
/// # Arguments
///
/// * `input_file` - chemin du fichier d'entrée
/// * `output_gpkg` - chemin du fichier GeoPackage de sortie
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si la conversion a réussi ou échoué
pub async fn convert_to_gpkg(
    input_file: &str,
    output_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    let input_file_path = current_dir.join(input_file);
    let output_gpkg_path = current_dir.join(output_gpkg);

    let args = [
        "-f",
        "GPKG",
        output_gpkg_path.to_str().unwrap(),
        input_file_path.to_str().unwrap(),
        "-t_srs",
        "EPSG:2154",
        "-nlt",
        "PROMOTE_TO_MULTI",
        "--config",
        "OGR_GEOMETRY_ACCEPT_UNCLOSED_RING",
        "NO",
        "-dim",
        "XY",
        "--config",
        "OGR_ARC_STEPSIZE",
        "0.1",
        "--config",
        "OGR_GEOMETRY_CORRECT_UNCLOSED_RINGS",
        "YES",
    ];

    execute_sidecar("ogr2ogr", &args).await?;

    Ok(())
}

/// Fusionne plusieurs fichiers GeoPackage en un seul
///
/// # Arguments
///
/// * `datasets` - une liste de chemins vers les fichiers GeoPackage à fusionner
/// * `output_gpkg` - chemin du fichier GeoPackage de sortie fusionné
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si la fusion a réussi ou échoué
pub async fn fusion_datasets(
    datasets: &[String],
    output_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if datasets.is_empty() {
        return Err("No datasets provided for fusion".into());
    }

    if std::path::Path::new(output_gpkg).exists() {
        std::fs::remove_file(output_gpkg)?;
    }

    let first_dataset = &datasets[0];

    execute_sidecar("ogr2ogr", &["-f", "GPKG", output_gpkg, first_dataset]).await?;

    for dataset in datasets.iter().skip(1) {
        execute_sidecar(
            "ogr2ogr",
            &["-f", "GPKG", "-append", "-update", output_gpkg, dataset],
        )
        .await?;
    }

    Ok(())
}

/// Découpe un GeoPackage en fonction d'une boîte englobante, afin de le réduire à la zone d'intérêt
///
/// # Arguments
///
/// * `input_gpkg` - chemin du fichier GeoPackage d'entrée
/// * `output_gpkg` - chemin du fichier GeoPackage de sortie
/// * `project_bb` - coordonnées de la boîte englobante du projet
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si le découpage a réussi ou échoué
pub async fn clip_to_bb(
    input_gpkg: &str,
    output_gpkg: &str,
    project_bb: &BoundingBox,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    let input_gpkg = current_dir.join(input_gpkg);
    let output_gpkg = current_dir.join(output_gpkg);

    let args = [
        "-f",
        "GPKG",
        output_gpkg.to_str().unwrap(),
        input_gpkg.to_str().unwrap(),
        "-clipsrc",
        &project_bb.xmin.to_string(),
        &project_bb.ymin.to_string(),
        &project_bb.xmax.to_string(),
        &project_bb.ymax.to_string(),
        "-nlt",
        "PROMOTE_TO_MULTI",
        "--config",
        "OGR_GEOMETRY_ACCEPT_UNCLOSED_RING",
        "NO",
        "-skipfailures",
        "--config",
        "OGR_ENABLE_PARTIAL_REPROJECTION",
        "YES",
        "--config",
        "OGR_GEOMETRY_CORRECT_UNCLOSED_RINGS",
        "YES",
    ];

    execute_sidecar("ogr2ogr", &args).await?;

    Ok(())
}

/// Crée un fichier GeoJSON pour une région donnée
///
/// # Arguments
///
/// * `region_id` - code départemental de la région
/// * `output_path` - chemin du fichier GeoJSON de sortie
///
/// # Returns
///
/// * `Result<(), Box<dyn Error>>` - un résultat indiquant si la création du fichier a réussi ou échoué
pub fn create_region_geojson(region_id: &str, output_path: &str) -> Result<(), Box<dyn Error>> {
    let region = get_region(region_id)?;
    let geometry: geojson::Geometry = region.extent().into();

    let properties = serde_json::json!({
        "code": region.code(),
        "name": region.name(),
        "neighbors": region.neighbors()
    });

    let feature = geojson::Feature {
        bbox: None,
        geometry: Some(geometry),
        id: None,
        properties: Some(properties.as_object().unwrap().clone()),
        foreign_members: None,
    };

    let feature_collection = geojson::FeatureCollection {
        bbox: None,
        features: vec![feature],
        foreign_members: Some(
            serde_json::json!({
                "crs": {
                    "type": "name",
                    "properties": {
                        "name": "EPSG:2154"
                    }
                }
            })
            .as_object()
            .unwrap()
            .clone(),
        ),
    };

    let geojson_string = geojson::GeoJson::FeatureCollection(feature_collection).to_string();
    fs::write(output_path, geojson_string)?;

    Ok(())
}

/// Construit un graphe de dépendances entre les régions à partir d'un fichier GeoJSON.
/// Le graphe est sauvegardé dans un fichier JSON pour une utilisation ultérieure.
/// Si le fichier de sortie existe déjà, il est chargé à partir de ce fichier.
///
/// # Arguments
///
/// * `output_file` - Le chemin vers le fichier de sortie où le graphe sera sauvegardé.
///
/// # Returns
///
/// * `Result<bool, Box<dyn Error>>` - Retourne `true` si le graphe a été construit ou chargé avec succès.
pub async fn build_regions_graph(output_file: Option<&str>) -> Result<bool, Box<dyn Error>> {
    if let Some(path) = output_file
        && Path::new(path).exists()
    {
        println!("Loading regions graph from cache file: {path}");
        let json_str = fs::read_to_string(path)?;
        let _: HashMap<String, Region> = serde_json::from_str(&json_str)?;
        return Ok(true);
    }

    let regional_geojson_path = resource_dir().join("regions.geojson");
    let geojson_str = fs::read_to_string(&regional_geojson_path)?;
    let geojson: GeoJson = geojson_str.parse()?;

    let feature_collection = match geojson {
        GeoJson::FeatureCollection(fc) => fc,
        _ => return Err("GeoJSON is not a FeatureCollection".into()),
    };

    let mut regions_info: HashMap<String, Region> = HashMap::new();
    let total_features = feature_collection.features.len();
    println!("Parsing {} features...", total_features);

    for (index, feature) in feature_collection.features.iter().enumerate() {
        if index % 100 == 0 {
            print!(
                "\rProgress: {}/{} features parsed ({:.1}%)",
                index,
                total_features,
                (index as f64 / total_features as f64) * 100.0
            );
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
        }

        let Some(code) = feature.property("code").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = feature
            .property("nom")
            .and_then(|v| v.as_str())
            .unwrap_or(code);
        let Some(geometry) = &feature.geometry else {
            continue;
        };

        let geojson_value = serde_json::to_value(geometry)?;
        let gdal_geom: Geometry = serde_json::to_string(&geojson_value)?
            .parse::<geojson::Geometry>()?
            .try_into()?;

        regions_info.insert(
            code.to_string(),
            Region::new(code.to_string(), name.to_string(), gdal_geom),
        );
    }

    let region_codes: Vec<String> = regions_info.keys().cloned().collect();
    let total_comparisons = (region_codes.len() * (region_codes.len() - 1)) / 2;
    let mut comparisons_done = 0;

    for i in 0..region_codes.len() {
        let code_i = &region_codes[i];
        let geom_i = regions_info[code_i].extent().clone();

        for code_j in &region_codes[i + 1..] {
            let geom_j = regions_info[code_j].extent().clone();

            if geom_i.intersects(&geom_j) || geom_i.relate(&geom_j).is_touches() {
                regions_info
                    .get_mut(code_i)
                    .unwrap()
                    .add_neighbor(code_j.clone());
                regions_info
                    .get_mut(code_j)
                    .unwrap()
                    .add_neighbor(code_i.clone());
            }

            comparisons_done += 1;
            if comparisons_done % 1000 == 0 {
                print!(
                    "\rProgress: {}/{} comparisons ({:.1}%)",
                    comparisons_done,
                    total_comparisons,
                    (comparisons_done as f64 / total_comparisons as f64) * 100.0
                );
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
                tokio::task::yield_now().await;
            }
        }
    }

    if let Some(path) = output_file {
        let json_str = serde_json::to_string_pretty(&regions_info)?;
        fs::write(path, json_str)?;
        println!("Regions graph saved to: {path}");
    }

    Ok(true)
}
