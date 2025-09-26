pub mod config_utils;
pub mod progress_handler;

use lazy_static::lazy_static;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};
use tauri_plugin_shell::ShellExt;

use crate::gis_operation::slicing::slice_images;
use crate::types::BoundingBox;

pub use config_utils::prelude::*;
pub use progress_handler::prelude::*;

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
        let output_dir = directories::UserDirs::new()
            .unwrap()
            .download_dir()
            .expect("Failed to get download directory")
            .to_path_buf();

        std::sync::Mutex::new(output_dir)
    };
    pub static ref VulcainColors: HashMap<&'static str, [&'static str; 3]> = HashMap::from([
        ("Chêne", ["80", "200", "120"]),
        ("Pin", ["50", "200", "80"]),
        ("Brousaille", ["25", "50", "60"]),
        ("Chaume", ["4", "25", "30"]),
        ("Incombustible", ["0", "0", "0"]),
    ]);
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

/// Exécute une commande en tant que sidecar et retourne la sortie standard et le statut de sortie.
///
/// Note : Assurez-vous que la commande est incluse dans les sidecars de Tauri (dans `tauri.conf.json`).
///
/// # Arguments
/// * `command` - Le nom de la commande à exécuter.
/// * `args` - Les arguments à passer à la commande.
///
/// # Returns
/// Un résultat contenant un tuple avec la sortie standard et le statut de sortie, ou une erreur.
pub async fn executor(
    command: &str,
    args: &[&str],
) -> Result<(String, tauri_plugin_shell::process::ExitStatus), Box<dyn Error>> {
    let app_handle = get_handle().unwrap();
    let output = app_handle
        .shell()
        .sidecar(command)?
        .args(args)
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed to execute {command} command. Status: {:?}\nStdout: {}\nStderr: {}",
            output.status, stdout, stderr
        )
        .into());
    }

    Ok((stdout, output.status))
}

pub async fn compress_folder(
    source_folder_path: &str,
    output_zip_name: &str,
    destination_directory: &str,
) -> Result<(), Box<dyn Error>> {
    let output_zip_path = format!("{destination_directory}/{output_zip_name}.zip");
    executor(
        "_7z",
        &["a", &output_zip_path, &format!("{}/*", source_folder_path)],
    )
    .await?;
    println!("Successfully compressed folder '{source_folder_path}' to '{output_zip_path}'");
    Ok(())
}

pub async fn extract_files_by_name(
    archive_path: &str,
    target_filename: &str,
    output_dir: &str,
) -> Result<(), Box<dyn Error>> {
    let output_path = Path::new(output_dir);
    let temp_extract_dir = output_path.join("temp_extract");

    fs::create_dir_all(output_path)?;
    fs::create_dir_all(&temp_extract_dir)?;

    executor(
        "_7z",
        &[
            "x",
            archive_path,
            &format!("-o{}", temp_extract_dir.display()),
        ],
    )
    .await?;

    let mut found_files = Vec::new();
    find_files_by_basename(&temp_extract_dir, target_filename, &mut found_files)?;

    if found_files.is_empty() {
        fs::remove_dir_all(&temp_extract_dir)?;
        return Err(format!("No files matching '{target_filename}' found in archive").into());
    }

    let destination = output_path.join(target_filename);
    fs::create_dir_all(&destination)?;

    for file_path in found_files {
        if let Some(file_name) = file_path.file_name() {
            fs::copy(&file_path, destination.join(file_name))?;
        }
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
                if let Some(file_stem) = path.file_stem()
                    && file_stem.to_string_lossy() == target_basename
                {
                    result.push(path);
                }
            } else if path.is_dir() {
                find_files_by_basename(&path, target_basename, result)?;
            }
        }
    }

    Ok(())
}

pub fn get_previous_projects() -> Result<HashMap<String, Vec<String>>, Box<dyn Error>> {
    let projects_path = projects_dir();
    let mut projects = HashMap::new();

    if !projects_path.exists() {
        return Ok(projects);
    }

    for entry in fs::read_dir(&projects_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir()
            && let Some(project_name) = path.file_name().and_then(|n| n.to_str())
        {
            let project_path = projects_dir().join(project_name);
            let preview_image_path = project_path.join(format!("{project_name}_ORTHO.jpeg"));
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

/// Exporte un projet ainsi que l'ensemble de ses ressources
/// (images, fichiers de configuration, etc.) dans un format compressé.
///
/// # Arguments
///
/// * `project_name` - Le nom du projet à exporter.
///
/// # Returns
///
pub async fn export_project(project_name: &str) -> Result<(), Box<dyn Error>> {
    let project_path = format!("{}/{}", projects_dir().to_string_lossy(), project_name);
    let slice_factor_value = slice_factor();
    let output_dir = output_location().to_string_lossy().to_string();

    println!("Exporting project: {project_name}");
    println!("Project path: {project_path}");
    println!("Output directory: {output_dir}");

    let date = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match slice_images(project_name, slice_factor_value).await {
        Ok(_) => {
            compress_folder(
                &project_path,
                &format!("export_{project_name}_{date}"),
                &output_dir,
            )
            .await?;
            Ok(())
        }
        Err(e) => Err(format!("Echec découpage: {project_name}: {e}").into()),
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
pub async fn export_to_jpg(
    project_file_path: &str,
    output_jpg_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    executor(
        "magick",
        &[
            "convert",
            project_file_path,
            "-strip",
            "-quality",
            "100",
            output_jpg_path,
        ],
    )
    .await?;

    Ok(())
}

pub async fn get_project_bounding_box(
    project_name: &str,
) -> Result<BoundingBox, Box<dyn std::error::Error>> {
    let project_path = format!("{}/{}/", projects_dir().to_string_lossy(), project_name);

    let tiff_path = format!("{project_path}{project_name}.tiff");
    let output = executor("gdalinfo", &[&tiff_path, "-json"]).await?;

    let json_output: Value =
        serde_json::from_str(&output.0).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let corner_coordinates = json_output["cornerCoordinates"].as_object().unwrap();

    Ok(BoundingBox {
        xmin: corner_coordinates["lowerLeft"][0].as_f64().unwrap(),
        ymin: corner_coordinates["lowerLeft"][1].as_f64().unwrap(),
        xmax: corner_coordinates["upperRight"][0].as_f64().unwrap(),
        ymax: corner_coordinates["upperRight"][1].as_f64().unwrap(),
    })
}

pub async fn get_geojson_bounding_box(
    file_path: &str,
) -> Result<BoundingBox, Box<dyn std::error::Error>> {
    let output = executor("ogrinfo", &["-so", "-al", file_path]).await?;

    let info_str = String::from_utf8(output.0.into())?;

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
