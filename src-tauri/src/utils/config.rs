use lazy_static::lazy_static;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::config::get_config;
use crate::error::{self, ConfigError, Result};

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
    pub static ref VulcainColors: HashMap<&'static str, [&'static str; 3]> = HashMap::from([
        ("Chêne", ["80", "200", "120"]),
        ("Pin", ["50", "200", "80"]),
        ("Brousaille", ["25", "50", "60"]),
        ("Chaume", ["4", "25", "30"]),
        ("Incombustible", ["0", "0", "0"]),
    ]);
    pub static ref OUTPUT_DIR: std::sync::Mutex<PathBuf> = {
        let output_dir = directories::UserDirs::new()
            .unwrap()
            .download_dir()
            .expect("Failed to get download directory")
            .to_path_buf();
        std::sync::Mutex::new(output_dir)
    };
}

pub fn cache_dir() -> PathBuf {
    get_config(|config| config.cache_dir.clone())
}

pub fn projects_dir() -> PathBuf {
    get_config(|config| config.projects_dir.clone())
}

pub fn temp_dir() -> PathBuf {
    get_config(|config| config.temp_dir.clone())
}

pub fn resource_dir() -> PathBuf {
    get_config(|config| config.resource_dir.clone())
}

pub fn wms_cache_dir() -> PathBuf {
    get_config(|config| config.wms_cache_dir.clone())
}

pub fn output_location() -> PathBuf {
    get_config(|config| config.output_location.clone())
}

pub fn resolution() -> f64 {
    get_config(|config| config.resolution)
}

pub fn slice_factor() -> u32 {
    get_config(|config| config.slice_factor)
}

pub fn get_handle() -> Option<AppHandle> {
    get_config(|config| config.handle.clone())
}

pub fn in_cache_dir<P: AsRef<std::path::Path>>(path: P) -> bool {
    cache_dir().join(path).exists()
}

pub fn in_projects_dir<P: AsRef<std::path::Path>>(path: P) -> bool {
    projects_dir().join(path).exists()
}

pub fn in_temp_dir<P: AsRef<std::path::Path>>(path: P) -> bool {
    temp_dir().join(path).exists()
}

pub fn in_resource_dir<P: AsRef<std::path::Path>>(path: P) -> bool {
    resource_dir().join(path).exists()
}

pub fn in_wms_cache_dir<P: AsRef<std::path::Path>>(path: P) -> bool {
    wms_cache_dir().join(path).exists()
}

pub fn get_data_sources() -> crate::config::DataSources {
    get_config(|config| config.data_sources.clone())
}

pub fn resolve_resource_dir(app_handle: &AppHandle, resource_path: &str) -> Result<PathBuf> {
    app_handle
        .path()
        .resolve(resource_path, tauri::path::BaseDirectory::Resource)
        .map_err(|e| {
            error::AppError::from(ConfigError::ResourcePathResolution {
                path: resource_path.to_string(),
                source: Box::new(e),
            })
        })
}

pub fn get_rpg_for_dep_code(code: &str) -> Option<&str> {
    RPG_DEP
        .iter()
        .find_map(|(rpg, deps)| deps.contains(&code).then_some(*rpg))
}

pub fn get_operating_system() -> &'static str {
    std::env::consts::OS
}

pub struct PathBuilder {
    pub temp_dir: String,
    pub cache_dir: String,
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self {
            temp_dir: temp_dir().to_string_lossy().to_string(),
            cache_dir: cache_dir().to_string_lossy().to_string(),
        }
    }
}

impl PathBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn temp_file(&self, name: &str, extension: &str) -> String {
        format!("{}/{}.{}", self.temp_dir, name, extension)
    }

    pub fn cache_file(&self, name: &str) -> String {
        format!("{}/{}", self.cache_dir, name)
    }

    pub fn project_folder(&self, project_name: &str) -> String {
        format!("{}/{}", projects_dir().to_string_lossy(), project_name)
    }

    pub fn project_resource(&self, project_folder: &str, name: &str) -> String {
        format!("{}/resources/{}.gpkg", project_folder, name)
    }
}
