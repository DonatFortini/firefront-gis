use crate::app_setup::{CONFIG, Config};
use gdal::vector::Geometry;
use image_convert;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::MutexGuard;
use xdg_user;

use crate::gis_operation::slicing::slice_images;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Copy)]
pub struct BoundingBox {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

impl BoundingBox {
    pub fn new(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Self {
        BoundingBox {
            xmin,
            ymin,
            xmax,
            ymax,
        }
    }

    pub fn width(&self) -> f64 {
        self.xmax - self.xmin
    }

    pub fn height(&self) -> f64 {
        self.ymax - self.ymin
    }

    pub fn to_wkt(&self) -> String {
        format!(
            "POLYGON(({} {}, {} {}, {} {}, {} {}, {} {}))",
            self.xmin,
            self.ymin,
            self.xmax,
            self.ymin,
            self.xmax,
            self.ymax,
            self.xmin,
            self.ymax,
            self.xmin,
            self.ymin
        )
    }

    pub fn to_geometry(&self) -> Result<Geometry, gdal::errors::GdalError> {
        Geometry::from_wkt(&self.to_wkt())
    }
}

lazy_static! {
    pub static ref RPG_DEP: HashMap<&'static str, Vec<&'static str>> = HashMap::from([
        (
            "84",
            vec![
                "1", "3", "7", "15", "26", "38", "42", "43", "63", "69", "73", "74"
            ]
        ),
        ("27", vec!["21", "25", "39", "58", "70", "71", "89", "90"]),
        ("53", vec!["22", "29", "35", "56"]),
        ("24", vec!["18", "28", "36", "37", "41", "45"]),
        ("94", vec!["2A", "2B"]),
        (
            "44",
            vec!["8", "10", "51", "52", "54", "55", "57", "67", "68", "88"]
        ),
        ("32", vec!["2", "59", "60", "62", "80"]),
        ("11", vec!["75", "77", "78", "91", "92", "93", "94", "95"]),
        ("28", vec!["14", "27", "50", "61", "76"]),
        (
            "75",
            vec![
                "16", "17", "19", "23", "24", "33", "40", "47", "64", "79", "86", "87"
            ]
        ),
        (
            "76",
            vec![
                "9", "11", "12", "30", "31", "32", "34", "46", "48", "65", "66", "81", "82"
            ]
        ),
        ("52", vec!["44", "49", "53", "72", "85"]),
        ("93", vec!["4", "5", "6", "13", "83", "84"]),
        ("01", vec!["971"]),
        ("02", vec!["972"]),
        ("03", vec!["973"]),
        ("04", vec!["974"]),
        ("06", vec!["976"]),
    ]);
    pub static ref OUTPUT_DIR: std::sync::Mutex<PathBuf> = {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        let output_dir = directories::UserDirs::new()
            .unwrap()
            .download_dir()
            .expect("Failed to get download directory")
            .to_path_buf();
        #[cfg(target_os = "linux")]
        let output_dir = xdg_user::UserDirs::new()
            .unwrap()
            .downloads()
            .expect("Failed to get downloads directory")
            .to_path_buf();
        std::sync::Mutex::new(output_dir)
    };
}

pub fn get_rpg_for_dep_code(code: &str) -> Option<&str> {
    RPG_DEP
        .iter()
        .find_map(|(rpg, deps)| {
            if deps.contains(&code) {
                Some(rpg)
            } else {
                None
            }
        })
        .map(|v| &**v)
}

pub fn create_directory_if_not_exists(path: &str) -> Result<(), Box<dyn Error>> {
    if !Path::new(path).exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

pub fn compress_folder(
    source_folder_path: &str,
    output_zip_name: &str,
    destination_directory: &str,
) -> Result<(), Box<dyn Error>> {
    let output_zip_path = format!("{}/{}.zip", destination_directory, output_zip_name);
    // FIXME : add the cross-platform support
    let mut command = Command::new("7z");
    command.args(["a", &output_zip_path]);
    command.current_dir(source_folder_path);
    command.arg(".");
    let output = command.output()?;

    if !output.status.success() {
        return Err(format!("Failed to execute 7z command: {:?}", output).into());
    }

    Ok(())
}

pub fn extract_files_by_name(
    archive_path: &str,
    target_filename: &str,
    output_dir: &str,
) -> Result<(), Box<dyn Error>> {
    create_directory_if_not_exists(output_dir)?;
    let temp_extract_dir = Path::new(output_dir).join("temp_extract");
    create_directory_if_not_exists(temp_extract_dir.to_str().unwrap())?;
    // FIXME : add the cross-platform support
    let extract_output = Command::new("7z")
        .args([
            "x",
            archive_path,
            &format!("-o{}", temp_extract_dir.to_str().unwrap()),
        ])
        .output()?;

    if !extract_output.status.success() {
        return Err("Archive extraction failed".into());
    }

    let destination = Path::new(output_dir).join(target_filename);
    create_directory_if_not_exists(destination.to_str().unwrap())?;

    let mut found_files = Vec::new();
    find_files_by_basename(&temp_extract_dir, target_filename, &mut found_files)?;

    if found_files.is_empty() {
        return Err(format!("No files matching '{}' found in archive", target_filename).into());
    }

    for file_path in &found_files {
        let file_name = file_path.file_name().unwrap();
        let dest_path = destination.join(file_name);
        fs::copy(file_path, dest_path)?;
    }

    fs::remove_dir_all(temp_extract_dir)?;

    Ok(())
}

fn find_files_by_basename(
    dir: &Path,
    target_basename: &str,
    result: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();

            if path.is_file() {
                if let Some(file_stem) = path.file_stem() {
                    if file_stem.to_string_lossy() == target_basename {
                        result.push(path);
                    }
                }
            } else if path.is_dir() {
                find_files_by_basename(&path, target_basename, result)?;
            }
        }
    }

    Ok(())
}
// FIXME : add the cross-platform support
pub fn get_previous_projects() -> Result<HashMap<String, Vec<String>>, Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd")
        .args(&["/C", "dir", &projects_dir().to_string_lossy(), "/b", "/a:d"])
        .output()?;
    #[cfg(not(target_os = "windows"))]
    let output = Command::new("ls")
        .args([projects_dir().to_string_lossy().as_ref()])
        .output()?;
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut projects = HashMap::new();
    for line in output_str.lines() {
        let project_name = line.trim();
        if project_name != "cache" {
            let project_path = project_dir(project_name);
            let preview_image_path = project_path.join(format!("{}_ORTHO.jpeg", project_name));
            projects.insert(
                project_name.to_string(),
                vec![
                    preview_image_path.to_string_lossy().to_string(),
                    project_path.to_string_lossy().to_string(),
                ],
            );
        }
    }
    Ok(projects)
}

pub fn get_operating_system() -> &'static str {
    std::env::consts::OS
}

/// Exporte un projet ainsi que l'ensemble de ses ressources
/// (images, fichiers de configuration, etc.) dans un format compressé.
///
/// # Arguments
///
/// * `project_name` - Le nom du projet à exporter.
///
/// # Returns
///
/// * `Result<(), Box<dyn Error>>` - Un résultat indiquant si l'exportation a réussi ou échoué.
pub fn export_project(project_name: &str) -> Result<(), Box<dyn Error>> {
    let project_path = format!("{}/{}", projects_dir().to_string_lossy(), project_name);
    let slice_factor_value = slice_factor();
    let output_dir = output_location().to_string_lossy().to_string();

    let date = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match slice_images(project_name, slice_factor_value) {
        Ok(_) => {
            compress_folder(
                &project_path,
                &format!("export_{}_{}", project_name, date),
                &output_dir,
            )?;
            Ok(())
        }
        Err(e) => Err(format!("Echec découpage: {}: {}", project_name, e).into()),
    }
}

/// Exporte un projet en format JPEG
/// Cette fonction est utilisée pour créer une image JPEG à partir d'un projet GDAL.
/// Utilise ImageMagick pour exporter un projet en JPEG. (Compatibilité avec le simulateur)
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet à exporter (format GTiff)
/// * `output_jpg_path` - chemin du fichier JPEG de sortie
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'exportation a réussi ou échoué
pub fn export_to_jpg(
    project_file_path: &str,
    output_jpg_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let input = image_convert::ImageResource::from_path(project_file_path);
    let mut output = image_convert::ImageResource::from_path(output_jpg_path);
    let config = image_convert::JPGConfig::default();
    let status = image_convert::to_jpg(&mut output, &input, &config);

    if let Err(e) = status {
        return Err(Box::new(e));
    }

    Ok(())
}

pub fn get_project_bounding_box(project_name: &str) -> Result<BoundingBox, String> {
    let project_path = format!("{}/{}/", projects_dir().to_string_lossy(), project_name);
    // FIXME : add the cross-platform support
    let output = Command::new("gdalinfo")
        .args([
            format!("{}{}.tiff", project_path, project_name),
            "-json".to_owned(),
        ])
        .output();

    let json_output: Value = serde_json::from_slice(&output.unwrap().stdout)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let corner_coordinates = json_output["cornerCoordinates"].as_object().unwrap();

    Ok(BoundingBox {
        xmin: corner_coordinates["lowerLeft"][0].as_f64().unwrap(),
        ymin: corner_coordinates["lowerLeft"][1].as_f64().unwrap(),
        xmax: corner_coordinates["upperRight"][0].as_f64().unwrap(),
        ymax: corner_coordinates["upperRight"][1].as_f64().unwrap(),
    })
}

pub fn get_geojson_bounding_box(
    file_path: &str,
) -> Result<BoundingBox, Box<dyn std::error::Error>> {
    // FIXME : add the cross-platform support
    let output = Command::new("ogrinfo")
        .args(["-so", "-al", file_path])
        .output()?;
    let info_str = String::from_utf8(output.stdout)?;

    let extent_pattern = r"Extent:\s*\(([\d.-]+),\s*([\d.-]+)\)\s*-\s*\(([\d.-]+),\s*([\d.-]+)\)";
    let caps = regex::Regex::new(extent_pattern)?
        .captures(&info_str)
        .ok_or("Could not find extent in ogrinfo output")?;

    Ok(BoundingBox {
        xmin: caps[1].parse()?,
        ymin: caps[2].parse()?,
        xmax: caps[3].parse()?,
        ymax: caps[4].parse()?,
    })
}

/// Nettoie le dossier tmp en conservant uniquement les fichiers GPKG
/// Cette fonction est utilisée pour nettoyer les fichiers entre les traitements
/// de différentes régions dans le processus de création de projet
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - Un résultat indiquant le succès ou l'échec
pub fn clean_tmp_except_gpkg() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = temp_dir();

    if !tmp_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(tmp_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
            continue;
        }

        if let Some(extension) = path.extension() {
            if extension != "gpkg" {
                std::fs::remove_file(&path)?;
            }
        } else {
            std::fs::remove_file(&path)?;
        }
    }

    Ok(())
}

pub fn get_config() -> MutexGuard<'static, Config> {
    CONFIG.lock().unwrap()
}

pub fn get_config_mut() -> MutexGuard<'static, Config> {
    CONFIG.lock().unwrap()
}

pub fn cache_dir() -> PathBuf {
    get_config().cache_dir.clone()
}

pub fn projects_dir() -> PathBuf {
    get_config().projects_dir.clone()
}

pub fn temp_dir() -> PathBuf {
    get_config().temp_dir.clone()
}

pub fn resource_dir() -> PathBuf {
    get_config().resource_dir.clone()
}

pub fn output_location() -> PathBuf {
    get_config().output_location.clone()
}

pub fn resolution() -> f64 {
    get_config().resolution
}

pub fn slice_factor() -> u32 {
    get_config().slice_factor
}

pub fn in_cache_dir<P: AsRef<Path>>(path: P) -> PathBuf {
    cache_dir().join(path)
}

pub fn in_projects_dir<P: AsRef<Path>>(path: P) -> PathBuf {
    projects_dir().join(path)
}

pub fn in_temp_dir<P: AsRef<Path>>(path: P) -> PathBuf {
    temp_dir().join(path)
}

pub fn in_resource_dir<P: AsRef<Path>>(path: P) -> PathBuf {
    resource_dir().join(path)
}

pub fn project_dir(project_name: &str) -> PathBuf {
    in_projects_dir(project_name)
}

pub fn in_project_dir(project_name: &str, path: &str) -> PathBuf {
    project_dir(project_name).join(path)
}

pub fn save_config() -> Result<(), Box<dyn std::error::Error>> {
    get_config().save()
}
