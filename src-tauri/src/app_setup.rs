use crate::dependency::{DependencyError, check_dependencies};
use crate::gis_operation::regions::build_regions_graph;
use crate::utils::{OUTPUT_DIR, create_directory_if_not_exists};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

const CACHE_DIR: &str = "projects/cache";
const PROJECTS_DIR: &str = "projects";
const TEMP_DIR: &str = "tmp";
const RESOURCES_DIR: &str = "resources";
const CONFIG_FILE: &str = "config.json";
const REGIONS_GRAPH_FILE: &str = "regions_graph.json";
const DEFAULT_RESOLUTION: f64 = 10.0;
const DEFAULT_SLICE_FACTOR: u32 = 500;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub cache_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub resource_dir: PathBuf,
    pub resolution: f64,
    pub slice_factor: u32,
    // User configurable settings
    pub output_location: PathBuf,
    pub gdal_path: Option<PathBuf>,
}

lazy_static! {
    pub static ref CONFIG: Mutex<Config> = Mutex::new(Config::load().unwrap_or_default());
}

impl Default for Config {
    fn default() -> Self {
        Self::with_resource_dir(PathBuf::from(RESOURCES_DIR))
    }
}

impl Config {
    pub fn new(handle: &AppHandle) -> Self {
        let resource_dir = resolve_resource_path(handle, RESOURCES_DIR)
            .unwrap_or_else(|_| PathBuf::from(RESOURCES_DIR));
        Self::with_resource_dir(resource_dir)
    }

    fn with_resource_dir(resource_dir: PathBuf) -> Self {
        Self {
            cache_dir: PathBuf::from(CACHE_DIR),
            projects_dir: PathBuf::from(PROJECTS_DIR),
            temp_dir: PathBuf::from(TEMP_DIR),
            resource_dir,
            resolution: DEFAULT_RESOLUTION,
            slice_factor: DEFAULT_SLICE_FACTOR,
            output_location: OUTPUT_DIR.lock().unwrap().clone(),
            gdal_path: None,
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let config_path = PathBuf::from(CONFIG_FILE);
        let config_json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(config_path)?;
        file.write_all(config_json.as_bytes())?;
        Ok(())
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let config_path = PathBuf::from(CONFIG_FILE);
        if !config_path.exists() {
            let default_config = Config::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let mut file = File::open(config_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn update_settings(
        &mut self,
        output_location: Option<String>,
        gdal_path: Option<String>,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(output) = output_location {
            self.output_location = PathBuf::from(output);
        }

        self.gdal_path = gdal_path.map(PathBuf::from);
        self.save()?;
        Ok(())
    }

    pub fn regions_graph_path(&self) -> PathBuf {
        self.resource_dir.join(REGIONS_GRAPH_FILE)
    }

    pub fn required_directories(&self) -> Vec<&PathBuf> {
        vec![&self.cache_dir, &self.temp_dir]
    }
}

fn resolve_resource_path(handle: &AppHandle, resource_path: &str) -> Result<PathBuf, String> {
    handle
        .path()
        .resolve(resource_path, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve resource path '{}': {}", resource_path, e))
}

/// Initialise la configuration originale de l'application
/// et vérifie les dépendances nécessaires.
///
/// # Arguments
/// * `handle` - L'handle de l'application Tauri.
///
/// # Returns
/// * `Ok(())` si la configuration et les dépendances sont correctement initialisées.
pub fn setup_check(handle: &AppHandle) -> Result<(), String> {
    let config = Config::new(handle);
    {
        let mut config_guard = CONFIG
            .lock()
            .map_err(|e| format!("Failed to lock CONFIG: {}", e))?;
        *config_guard = config.clone();
    }

    for dir in [&config.cache_dir, &config.temp_dir] {
        create_directory_if_not_exists(&dir.to_string_lossy()).map_err(|e| e.to_string())?;
    }

    let regions_graph_path = config.regions_graph_path();
    let regions_graph_str = regions_graph_path
        .to_str()
        .ok_or_else(|| "Invalid UTF-8 in regions graph path".to_string())?;

    build_regions_graph(Some(regions_graph_str)).map_err(|e| e.to_string())?;

    {
        let mut config_guard = CONFIG
            .lock()
            .map_err(|e| format!("Failed to lock CONFIG: {}", e))?;
        check_dependencies(&mut config_guard).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl fmt::Display for DependencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyError::GDALNotInstalled => write!(f, "GDAL is not installed"),
        }
    }
}
