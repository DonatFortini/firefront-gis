use tauri::command;

use crate::{
    config::{self, get_config},
    utils::{cache_dir, create_directory_if_not_exists, get_operating_system},
};

#[command]
pub fn get_os() -> String {
    get_operating_system().to_string()
}

#[command]
/// Récupère les paramètres de configuration de l'application.
///
/// # Retourne
/// - `Result<serde_json::Value, String>` : Un objet JSON contenant les paramètres de configuration ou une erreur.
pub fn get_settings() -> Result<serde_json::Value, String> {
    let output_location = get_config(|config| config.output_location.to_string_lossy().to_string());

    Ok(serde_json::json!({
        "output_location": output_location,
    }))
}

#[command(rename_all = "snake_case")]
/// Enregistre les paramètres de configuration de l'application.
///     
/// # Arguments
///
/// * `output_location` - Option<String> : L'emplacement de sortie.
///
/// # Retourne
///
/// * `String` : Un message de succès ou d'erreur.
pub fn save_settings(output_location: Option<String>) -> String {
    match config::update_config(|config| {
        if let Some(output_location) = output_location {
            config.output_location = std::path::PathBuf::from(output_location);
        }
        Ok(())
    }) {
        Ok(_) => "Paramètres sauvegardés avec succès".to_string(),
        Err(e) => {
            format!("Échec de sauvegarde des paramètres: {e}")
        }
    }
}

#[command]
/// Vide le cache des projets.
///
/// # Retourne
///
/// * `Result<String, String>` : Un message de succès ou d'erreur.
pub fn clear_cache() -> Result<String, String> {
    match std::fs::remove_dir_all(cache_dir()) {
        Ok(_) => {
            create_directory_if_not_exists(cache_dir().to_string_lossy().as_ref())
                .map_err(|e| e.to_string())?;
            Ok("Cache vidé avec succès".to_string())
        }
        Err(e) => Err(format!("Échec du vidage du cache: {e}")),
    }
}

pub mod prelude {
    pub use super::{clear_cache, get_os, get_settings, save_settings};
}
