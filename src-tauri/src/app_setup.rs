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

#[derive(Debug, Serialize, Deserialize)]
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
        Self {
            cache_dir: PathBuf::from("projects/cache"),
            projects_dir: PathBuf::from("projects"),
            temp_dir: PathBuf::from("tmp"),
            resource_dir: PathBuf::from("resources"),
            resolution: 10.0,
            slice_factor: 500,
            output_location: OUTPUT_DIR.lock().unwrap().clone(),
            gdal_path: None,
        }
    }
}

impl Config {
    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let config_path = PathBuf::from("config.json");
        let config_json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(config_path)?;
        file.write_all(config_json.as_bytes())?;
        Ok(())
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let config_path = PathBuf::from("config.json");
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
}

/// Vérifie si les dépendances sont installées et crée les répertoires nécessaires.
///
/// # Returns
/// - Result<(), DependencyError>
pub fn setup_check() -> Result<(), String> {
    let mut config = CONFIG.lock().unwrap();

    create_directory_if_not_exists(&config.cache_dir.to_string_lossy())
        .map_err(|e| e.to_string())?;
    create_directory_if_not_exists(&config.temp_dir.to_string_lossy())
        .map_err(|e| e.to_string())?;

    check_dependencies(&mut config).map_err(|e| e.to_string())?;
    build_regions_graph(Some("resources/regions_graph.json")).map_err(|e| e.to_string())?;
    Ok(())
}

impl fmt::Display for DependencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyError::GDALNotInstalled => write!(f, "GDAL is not installed"),
            DependencyError::SevenZipNotInstalled => write!(f, "7zip is not installed"),
        }
    }
}
