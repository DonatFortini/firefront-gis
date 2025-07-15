use crate::gis_operation::regions::build_regions_graph;
use crate::utils::{OUTPUT_DIR, create_directory_if_not_exists};
use futures_util::future::ok;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::str;
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
pub struct AppConfig {
    pub cache_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub resource_dir: PathBuf,
    pub resolution: f64,
    pub slice_factor: u32,
    pub output_location: PathBuf,
    pub gdal_path: Option<PathBuf>,
    #[serde(skip)]
    pub handle: Option<AppHandle>,
}

// Global configuration instance, initialized at application startup.
lazy_static! {
    pub static ref CONFIG: Mutex<AppConfig> = Mutex::new(AppConfig::load().unwrap_or_default());
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::with_resource_dir(PathBuf::from(RESOURCES_DIR))
    }
}

impl AppConfig {
    pub fn new(app_handle: &AppHandle) -> Self {
        let resource_dir = resolve_resource_dir(app_handle, RESOURCES_DIR)
            .unwrap_or_else(|_| PathBuf::from(RESOURCES_DIR));
        Self::with_resource_dir(resource_dir);
        Self {
            handle: Some(app_handle.clone()),
            ..Self::default()
        }
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
            handle: None,
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let config_json = serde_json::to_string_pretty(self)?;
        File::create(CONFIG_FILE)?.write_all(config_json.as_bytes())?;
        Ok(())
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        if !PathBuf::from(CONFIG_FILE).exists() {
            let default_config = AppConfig::default();
            default_config.save()?;
            return Ok(default_config);
        }
        let mut contents = String::new();
        File::open(CONFIG_FILE)?.read_to_string(&mut contents)?;
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn update(
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

    pub fn required_dirs(&self) -> Vec<&PathBuf> {
        vec![&self.cache_dir, &self.temp_dir]
    }
}

fn resolve_resource_dir(app_handle: &AppHandle, resource_path: &str) -> Result<PathBuf, String> {
    app_handle
        .path()
        .resolve(resource_path, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve resource path '{resource_path}': {e}"))
}

/// Initialise l'application en créant les répertoires nécessaires et en vérifiant les dépendances.
///
/// # Arguments
/// * `app_handle` - Un handle vers l'application Tauri.
///
/// # Returns
/// * `Result<(), String>` - Un résultat indiquant si l'initialisation a réussi ou non.
pub fn initialize_app(app_handle: &AppHandle) -> Result<(), String> {
    let config = AppConfig::new(app_handle);
    {
        let mut config_guard = CONFIG
            .lock()
            .map_err(|e| format!("Failed to lock CONFIG: {e}"))?;
        *config_guard = config.clone();
    }

    for dir in config.required_dirs() {
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
            .map_err(|e| format!("Failed to lock CONFIG: {e}"))?;
        verify_dependency(&mut config_guard).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug)]
pub enum DependencyError {
    GdalNotInstalled,
}

impl fmt::Display for DependencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyError::GdalNotInstalled => write!(f, "GDAL is not installed"),
        }
    }
}

fn verify_command_exists(
    command: &str,
    arg: &str,
    error: DependencyError,
) -> Result<(), DependencyError> {
    if Command::new(command).arg(arg).output().is_err() {
        Err(error)
    } else {
        println!("{command} is found");
        Ok(())
    }
}

pub fn verify_dependency(config: &mut AppConfig) -> Result<(), DependencyError> {
    let (gdal_cmd, path_cmd) = if cfg!(target_os = "windows") {
        ("gdalinfo.exe", "where")
    } else {
        ("gdalinfo", "which")
    };

    verify_command_exists(gdal_cmd, "--version", DependencyError::GdalNotInstalled)?;
    if let Ok(path_output) = Command::new(path_cmd).arg(gdal_cmd).output() {
        let path = str::from_utf8(&path_output.stdout)
            .unwrap_or_default()
            .trim();
        config.gdal_path = Some(path.into());
        println!("{gdal_cmd} path set to: {path}");
    }

    Ok(())
}

pub fn install_gdal_unix() -> Result<(), String> {
    let installers = [
        (
            "apt-get",
            &[
                "sudo",
                "apt-get",
                "install",
                "-y",
                "gdal-bin",
                "libgdal-dev",
            ][..],
        ),
        ("dnf", &["sudo", "dnf", "install", "-y", "gdal"][..]),
        ("pacman", &["sudo", "pacman", "-S", "gdal"][..]),
        ("brew", &["brew", "install", "gdal"][..]),
    ];

    for (pm, cmd) in installers.iter() {
        if Command::new("which")
            .arg(pm)
            .output()
            .is_ok_and(|o| o.status.success())
        {
            let output = Command::new(cmd[0])
                .args(&cmd[1..])
                .output()
                .map_err(|e| format!("Failed to install GDAL with {pm}: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "Failed to install GDAL with {pm}: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            return Ok(());
        }
    }

    Err("No supported package manager found (apt-get, dnf, pacman, brew). Please install GDAL manually.".to_string())
}

fn install_gdal_windows() -> Result<(), String> {
    let installer_url = "
https://download.osgeo.org/gdal/win64/v3.4.0/gdal-3.4.0-x64-core.msi";
    Ok(())
}
