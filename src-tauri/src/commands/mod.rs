use std::path::Path;

use crate::{config::get_config, gis_operation::build_regions_graph};

pub mod projects;
pub mod systems;

pub use projects::prelude::*;
pub use systems::prelude::*;

/// Charge le graphe des régions. Si le fichier du graphe n'existe pas,
/// il le construit à partir du fichier GeoJSON des régions.
///
/// Note : Cette fonction est appelée au démarrage de l'application et uniquement dans cet usecase.
///
/// # Retourne
/// * `Result<(), String>` - Ok si le graphe est chargé ou construit avec succès
#[tauri::command]
pub async fn load_regions_graph() -> Result<(), String> {
    let graph_path = get_config(|config| config.regions_graph_path());
    if !Path::new(&graph_path).exists() {
        println!("Regions graph file not found, building a new one...");
        build_regions_graph(graph_path.to_str())
            .await
            .map_err(|e| format!("Failed to build regions graph: {e}"))?;
    }
    println!("Regions graph loaded successfully.");

    Ok(())
}
