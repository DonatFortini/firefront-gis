use std::collections::HashMap;
use std::path::Path;
use tauri::command;

use crate::config::get_config;
use crate::services::{ProjectService, RegionService};
use crate::types::BoundingBox;
use crate::utils::{
    cache_dir, create_directory_if_not_exists, get_operating_system, wms_cache_dir,
};

#[command]
pub fn get_projects() -> Result<HashMap<String, Vec<String>>, String> {
    ProjectService::list_projects().map_err(|e| e.to_string())
}

#[command]
pub fn get_project_data(name: String, data: String) -> Result<String, String> {
    ProjectService::get_project_data(&name, &data).map_err(|e| e.to_string())
}

#[command(rename_all = "snake_case")]
pub async fn export(project_name: &str) -> Result<String, String> {
    ProjectService::export_project(project_name)
        .await
        .map(|_| "success".to_string())
        .map_err(|e| e.to_string())
}

#[command(rename_all = "snake_case")]
pub async fn delete_project(project_name: &str) -> Result<String, String> {
    ProjectService::delete_project(project_name)
        .await
        .map(|_| "success".to_string())
        .map_err(|e| e.to_string())
}

#[command(rename_all = "snake_case")]
pub async fn create_project(name: String, project_bb: BoundingBox) -> Result<(), String> {
    ProjectService::create_project(name, project_bb)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub fn get_os() -> String {
    get_operating_system().to_string()
}

#[command]
pub fn get_settings() -> Result<serde_json::Value, String> {
    let output_location = get_config(|config| config.output_location.to_string_lossy().to_string());

    Ok(serde_json::json!({
        "output_location": output_location,
    }))
}

#[command(rename_all = "snake_case")]
pub fn save_settings(output_location: Option<String>) -> String {
    match crate::config::update_config(|config| {
        if let Some(location) = output_location {
            config.output_location = std::path::PathBuf::from(location);
        }
        Ok(())
    }) {
        Ok(_) => "Paramètres sauvegardés avec succès".to_string(),
        Err(e) => format!("Échec de sauvegarde des paramètres: {}", e),
    }
}

#[command]
pub fn clear_cache() -> Result<String, String> {
    let cache_path = cache_dir();

    std::fs::remove_dir_all(&cache_path).map_err(|e| format!("Échec du vidage du cache: {}", e))?;

    create_directory_if_not_exists(&cache_path.to_string_lossy()).map_err(|e| e.to_string())?;
    create_directory_if_not_exists(&wms_cache_dir().to_string_lossy())
        .map_err(|e| e.to_string())?;

    Ok("Cache vidé avec succès".to_string())
}

#[command]
pub async fn load_regions_graph() -> Result<(), String> {
    let graph_path = get_config(|config| config.regions_graph_path());

    if !Path::new(&graph_path).exists() {
        println!("Regions graph file not found, building...");
        RegionService::build_regions_graph(graph_path.to_str())
            .await
            .map_err(|e| format!("Failed to build regions graph: {}", e))?;
    }

    println!("Regions graph loaded successfully");
    Ok(())
}

#[command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub mod prelude {
    pub use super::*;
}
