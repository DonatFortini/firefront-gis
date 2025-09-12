use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager};

use crate::config::{ConfigError, get_config};

// ============================================================================
// Configuration Getters
// ============================================================================

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

pub fn output_location() -> PathBuf {
    get_config(|config| config.output_location.clone())
}

pub fn resolution() -> f64 {
    get_config(|config| config.resolution)
}

pub fn slice_factor() -> u32 {
    get_config(|config| config.slice_factor)
}

pub fn get_handle() -> Option<tauri::AppHandle> {
    get_config(|config| config.handle.clone())
}

// ============================================================================
// Path Existence Checkers
// ============================================================================

pub fn in_cache_dir<P: AsRef<Path>>(path: P) -> bool {
    cache_dir().join(path).exists()
}

pub fn in_projects_dir<P: AsRef<Path>>(path: P) -> bool {
    projects_dir().join(path).exists()
}

pub fn in_temp_dir<P: AsRef<Path>>(path: P) -> bool {
    temp_dir().join(path).exists()
}

pub fn in_resource_dir<P: AsRef<Path>>(path: P) -> bool {
    resource_dir().join(path).exists()
}

pub fn in_project_dir<P: AsRef<Path>>(path: P) -> bool {
    projects_dir().join(path).exists()
}

// ============================================================================
// Path Resolution & Directory Operations
// ============================================================================

pub fn resolve_resource_dir(
    app_handle: &AppHandle,
    resource_path: &str,
) -> Result<PathBuf, ConfigError> {
    app_handle
        .path()
        .resolve(resource_path, tauri::path::BaseDirectory::Resource)
        .map_err(|e| ConfigError::ResourcePathResolution {
            path: resource_path.to_string(),
            source: Box::new(e),
        })
}

pub fn create_directory_if_not_exists(path: &str) -> Result<(), Box<dyn Error>> {
    if !Path::new(path).exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

// ============================================================================
// File System Operations
// ============================================================================

/// Nettoie le dossier tmp en conservant optionnellement les fichiers d'une extension spécifique
/// Cette fonction est utilisée pour nettoyer les fichiers entre les traitements
/// de différentes régions dans le processus de création de projet
///
/// # Arguments
///
/// * `ignore_extension` - Extension des fichiers à conserver (ex: "gpkg"). Si None, supprime tout.
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - Un résultat indiquant le succès ou l'échec
/// ```rust
/// // Exemple d'utilisation
/// clean_tmp(Some(".gpkg")).expect("Failed to clean tmp directory");
/// ```
pub fn clean_tmp(ignore_extension: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = temp_dir();

    if !tmp_dir.exists() {
        return Ok(());
    }

    match ignore_extension {
        Some(ext) => {
            for entry in std::fs::read_dir(&tmp_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    std::fs::remove_dir_all(&path)?;
                    continue;
                }

                if let Some(extension) = path.extension() {
                    let extension_str = extension.to_string_lossy();
                    let target_ext = ext.trim_start_matches('.');
                    if extension_str != target_ext {
                        std::fs::remove_file(&path)?;
                    }
                } else {
                    std::fs::remove_file(&path)?;
                }
            }
        }
        None => {
            std::fs::remove_dir_all(&tmp_dir)?;
            std::fs::create_dir(&tmp_dir)?;
        }
    }

    Ok(())
}

// ============================================================================
// System Information
// ============================================================================

pub fn get_operating_system() -> &'static str {
    std::env::consts::OS
}

pub mod prelude {
    pub use super::{
        cache_dir, clean_tmp, create_directory_if_not_exists, get_handle, get_operating_system,
        in_cache_dir, in_project_dir, in_projects_dir, in_resource_dir, in_temp_dir,
        output_location, projects_dir, resolution, resolve_resource_dir, resource_dir,
        slice_factor, temp_dir,
    };
}
