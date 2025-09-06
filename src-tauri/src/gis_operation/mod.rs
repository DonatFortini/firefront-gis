use tauri_plugin_shell::ShellExt;

use crate::utils::{BoundingBox, get_handle, resolution};

pub mod layers;
pub mod processing;
pub mod regions;
pub mod slicing;

/// Crée un projet de carte avec une résolution donnée (10m/pixel)
/// et calcule la taille de l'image en fonction de la boîte englobante
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
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
/// use crate::gis_processing::create_project;
/// use crate::utils::BoundingBox;
///
///
/// let project_file_path = "path/to/project.tif";
///
/// let project_bb = BoundingBox {
///     xmin: 1210000.0,
///     ymin: 6070000.0,
///     xmax: 1235000.0,
///     ymax: 6095000.0,
/// };
///
/// create_project(project_file_path, &project_bb).unwrap();
///
///```
///
///
pub async fn create_project(
    project_file_path: &str,
    project_bb: &BoundingBox,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolution = resolution();
    let width = (project_bb.width() / resolution).ceil() as usize;
    let height = (project_bb.height() / resolution).ceil() as usize;

    if width % 500 != 0 || height % 500 != 0 {
        return Err("Width and height must be multiples of 500".into());
    }

    let status = get_handle()
        .unwrap()
        .shell()
        .sidecar("gdal_create")?
        .args([
            "-of",
            "GTiff",
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
        ])
        .status()
        .await?;

    if status.success() {
        Ok(())
    } else {
        Err("Failed to create project raster".into())
    }
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

    let handle = get_handle().unwrap();
    let status = handle
        .shell()
        .sidecar("ogr2ogr")?
        .args([
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
        ])
        .status()
        .await?;

    if !status.success() {
        return Err("Failed to convert to GeoPackage".into());
    }

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

    let handle = get_handle().unwrap();
    let mut status = handle
        .shell()
        .sidecar("ogr2ogr")?
        .args(["-f", "GPKG", output_gpkg, first_dataset])
        .status()
        .await?;

    if !status.success() {
        return Err(format!("Failed to process first dataset: {first_dataset}").into());
    }

    for dataset in datasets.iter().skip(1) {
        status = handle
            .shell()
            .sidecar("ogr2ogr")?
            .args(["-f", "GPKG", "-append", "-update", output_gpkg, dataset])
            .status()
            .await?;

        if !status.success() {
            return Err(format!("Failed to append dataset: {dataset}").into());
        }
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

    let handle = get_handle().unwrap();
    let status = handle
        .shell()
        .sidecar("ogr2ogr")?
        .args([
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
        ])
        .status()
        .await?;

    if !status.success() {
        return Err("Failed to clip GeoPackage".into());
    }

    Ok(())
}
