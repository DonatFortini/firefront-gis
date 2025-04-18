use crate::dependency::{DependencyError, check_dependencies};
use crate::utils::create_directory_if_not_exists;
use std::fmt;

/// Vérifie si les dépendances sont installées et crée les répertoires nécessaires.
///
/// # Returns
/// - Result<(), DependencyError>
pub fn setup_check() -> Result<(), String> {
    create_directory_if_not_exists("projects/cache").map_err(|e| e.to_string())?;
    create_directory_if_not_exists("tmp").map_err(|e| e.to_string())?;
    check_dependencies().map_err(|e| e.to_string())?;
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
