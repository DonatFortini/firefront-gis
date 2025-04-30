use crate::dependency::{DependencyError, check_dependencies};
use crate::gis_operation::regions::build_regions_graph;
use crate::utils::create_directory_if_not_exists;
use std::fmt;
use std::path::PathBuf;

// TODO: implement through the app to replace hardcoded paths
pub struct Config {
    pub cache_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub resource_dir: PathBuf,
    pub resolution: f64,
    pub slice_factor: u32,
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
        }
    }
}

/// Vérifie si les dépendances sont installées et crée les répertoires nécessaires.
///
/// # Returns
/// - Result<(), DependencyError>
pub fn setup_check() -> Result<(), String> {
    create_directory_if_not_exists("projects/cache").map_err(|e| e.to_string())?;
    create_directory_if_not_exists("tmp").map_err(|e| e.to_string())?;
    check_dependencies().map_err(|e| e.to_string())?;
    build_regions_graph(Some("resources/regions_graph.json")).map_err(|e| e.to_string())?;
    Ok(())
}

impl fmt::Display for DependencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyError::GDALNotInstalled => write!(f, "GDAL is not installed"),
            DependencyError::PythonNotInstalled => write!(f, "Python is not installed"),
            DependencyError::SevenZipNotInstalled => write!(f, "7zip is not installed"),
        }
    }
}
