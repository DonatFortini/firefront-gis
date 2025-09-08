use std::collections::HashMap;

use tauri::command;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tokio::fs;

use crate::{
    fetch_resources::{download_shp_file, get_shp_file_urls},
    gis_operation::{
        create_reference_raster, fetch_orthophoto_wms, fusion_datasets,
        layers::{add_layers, prepare_layers},
    },
    types::{BoundingBox, regions::find_intersecting_regions},
    utils::{
        DownloadProgress, Progress, ProgressTracker, cache_dir, clean_tmp, export_project,
        export_to_jpg, get_handle, get_previous_projects, in_cache_dir, projects_dir,
    },
};

#[command]
/// Obtient la liste des projets précédents.
///
/// # Retourne
/// - HashMap<String, Vec<String>> : Une hashmap contenant le nom du projet et la liste des fichiers associés.
pub fn get_projects() -> HashMap<String, Vec<String>> {
    get_previous_projects().unwrap()
}

/// Récupère les données d'un projet existant.(permet au front de display les images du projet)
///
/// # Arguments
/// * `name` - Le nom du projet.
/// * `data` - Le nom du fichier de données.
///
/// # Retourne
/// - Result<String, String> : Le chemin du fichier de données ou une erreur.
#[tauri::command]
pub fn get_project_data(name: String, data: String) -> Result<String, String> {
    let project_folder = format!("{}/{name}", projects_dir().to_string_lossy());
    let project_file_path = format!("{project_folder}/{data}");

    if !std::path::Path::new(&project_file_path).exists() {
        return Err(format!("Le projet '{name}' n'existe pas"));
    }

    Ok(project_file_path)
}

#[command(rename_all = "snake_case")]
/// Exporte un projet, fais la decoupe puis le zip
///
/// # Paramètres
/// - project_name: &str : Le nom du projet à exporter.
///
/// # Retourne
/// - Result<String, String> : Un résultat contenant le message de succès ou l'erreur.
pub async fn export(project_name: &str) -> Result<String, String> {
    match export_project(project_name).await {
        Ok(_) => {
            println!("Exportation réussie");
            Ok("success".to_string())
        }
        Err(e) => {
            println!("Erreur lors de l'exportation: {e:?}");
            Err("error".to_string())
        }
    }
}

#[command(rename_all = "snake_case")]
/// Supprime un projet existant.
///
/// # Arguments
///
/// * `project_name` - Le nom du projet à supprimer.
///
/// # Retourne
///
/// * `Ok(String)` - "success" si la suppression a réussi.
/// * `Err(String)` - Un message d'erreur descriptif en cas de problème.
pub async fn delete_project(project_name: &str) -> Result<String, String> {
    let project_folder = format!("{}/{}", projects_dir().to_string_lossy(), project_name);
    if !std::path::Path::new(&project_folder).exists() {
        return Err(format!("Le projet '{project_name}' n'existe pas"));
    }

    match tokio::fs::remove_dir_all(&project_folder).await {
        Ok(_) => {
            println!("Projet '{project_name}' supprimé avec succès");
            Ok("success".to_string())
        }
        Err(e) => {
            println!("Erreur lors de la suppression du projet '{project_name}': {e:?}");
            Err(format!("Erreur lors de la suppression du projet: {e}"))
        }
    }
}

#[command(rename_all = "snake_case")]
/// Crée un projet avec les fichiers SHP associés.
/// Télécharge les fichiers SHP nécessaires, crée un projet de carte,
/// fusionne les couches et ajoute les couches au projet.
/// Télécharge également une image satellite et l'exporte en JPEG.
/// Nettoie les fichiers temporaires après la création du projet.
///
/// # Arguments
///
/// * `name` - Nom du projet.
/// * `project_bb` - Boîte englobante du projet.
///
/// # Retourne
///
/// * `Result<String, String>` - Chemin du dossier du projet créé ou un message d'erreur.
pub async fn create_project(name: String, project_bb: BoundingBox) -> Result<String, String> {
    Progress::status("Recherche des fichiers");

    let mut region_codes: Vec<String> = Vec::new();
    match find_intersecting_regions(&project_bb) {
        Ok(result) => {
            if result.is_empty() {
                return Err("La surface de travail est incorrecte".to_string());
            } else {
                for region in result {
                    region_codes.push(region.code);
                }
            }
        }
        Err(_) => return Err("La surface de travail est incorrecte".to_string()),
    }

    let urls = get_shp_file_urls(&region_codes)
        .await
        .map_err(|e| e.to_string())?;

    // Download phase
    Progress::status("Téléchargement des données");
    let download = DownloadProgress::new();

    let file_types = ["BDTOPO", "BDFORET", "RPG"];
    let total_downloads = urls.len();
    let mut download_count = 0;

    for (code_index, code) in region_codes.iter().enumerate() {
        for (file_type_index, file_type) in file_types.iter().enumerate() {
            let url_index = code_index * 3 + file_type_index;
            if url_index >= urls.len() {
                break;
            }

            let url = &urls[url_index];
            download_count += 1;

            download.file_progress(file_type, download_count, total_downloads);

            let cache_path = format!(
                "{}/{}_{}.7z",
                cache_dir().to_string_lossy(),
                file_type,
                code
            );
            if !in_cache_dir(&cache_path) {
                download_shp_file(url, code).await.map_err(|e| {
                    format!("Erreur lors du téléchargement du fichier SHP depuis {url}: {e:?}")
                })?;
            }
        }
    }

    Progress::status("Initialisation du projet");

    let project_folder = format!("{}/{name}", projects_dir().to_string_lossy());
    let project_file_path = format!("{project_folder}/{name}.tiff");
    if std::path::Path::new(&project_file_path).exists() {
        let should_overwrite = get_handle()
            .unwrap()
            .dialog()
            .message("Voulez-vous écraser le projet existant ?")
            .title("Projet dejà existant")
            .buttons(MessageDialogButtons::YesNo)
            .blocking_show();

        if !should_overwrite {
            return Ok("Project creation cancelled".to_string());
        }

        std::fs::remove_dir_all(&project_folder).unwrap();
    }

    // Project initialization with tracker
    let mut init_tracker = ProgressTracker::new("Initialisation du projet", 2);

    init_tracker.set_step(1, "Création des dossiers");
    std::fs::create_dir_all(&project_folder).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(format!("{project_folder}/resources")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(format!("{project_folder}/slices")).map_err(|e| e.to_string())?;

    init_tracker.set_step(2, "Configuration du projet");

    if let Err(e) = create_reference_raster(&project_file_path, &project_bb).await {
        return Err(format!("Erreur lors de la création du projet: {e:?}"));
    }

    Progress::status("Préparation des Couches");

    let mut regional_gpkgs: Vec<String> = Vec::new();
    let mut vegetation_gpkgs: Vec<String> = Vec::new();
    let mut rpg_gpkgs: Vec<String> = Vec::new();
    let mut topo_gpkgs: HashMap<String, Vec<String>> = HashMap::new();

    let total_regions = region_codes.len();
    for (idx, code) in region_codes.iter().enumerate() {
        Progress::full(
            "Préparation des Couches",
            format!("Traitement de la région {code}"),
            idx + 1,
            total_regions,
        );

        if let Err(e) = clean_tmp(Some(".gpkg")) {
            return Err(format!(
                "Erreur lors du nettoyage des fichiers temporaires: {e:?}"
            ));
        }

        let (r_gpkg, v_gpkg, rp_gpkg, t_gpkg) = prepare_layers(&project_bb, code).await?;

        regional_gpkgs.push(r_gpkg);
        vegetation_gpkgs.push(v_gpkg);
        rpg_gpkgs.push(rp_gpkg);

        for (layer_name, paths) in t_gpkg {
            topo_gpkgs.entry(layer_name).or_default().extend(paths);
        }

        if let Err(e) = clean_tmp(Some(".gpkg")) {
            return Err(format!(
                "Erreur lors du nettoyage des fichiers temporaires: {e:?}"
            ));
        }
    }

    // Fusion phase
    Progress::full("Fusion des données", "Fusion des régions", 1, 4);

    let regional_merged_gpkg = format!("{project_folder}/resources/{name}.gpkg");
    let vegetation_merged_gpkg = format!("{project_folder}/resources/FORMATION_VEGETALE.gpkg");
    let rpg_merged_gpkg = format!("{project_folder}/resources/PARCELLES_GRAPHIQUES.gpkg");

    if region_codes.len() > 1 {
        let mut fusion_tracker = ProgressTracker::new("Fusion des données", 4);

        fusion_tracker.set_step(1, "Fusion des couches régionales");
        if let Err(e) = fusion_datasets(&regional_gpkgs, &regional_merged_gpkg).await {
            return Err(format!(
                "Erreur lors de la fusion des couches régionales: {e:?}"
            ));
        }

        fusion_tracker.set_step(2, "Fusion des couches de végétation");
        if let Err(e) = fusion_datasets(&vegetation_gpkgs, &vegetation_merged_gpkg).await {
            return Err(format!(
                "Erreur lors de la fusion des couches de végétation: {e:?}"
            ));
        }

        fusion_tracker.set_step(3, "Fusion des couches RPG");
        if let Err(e) = fusion_datasets(&rpg_gpkgs, &rpg_merged_gpkg).await {
            return Err(format!("Erreur lors de la fusion des couches RPG: {e:?}"));
        }

        fusion_tracker.set_step(4, "Fusion des couches topographiques");

        let total_topo_layers = topo_gpkgs.len();
        let mut topo_count = 1;
        for (layer_name, paths) in &topo_gpkgs {
            Progress::full(
                "Fusion des données",
                format!("Fusion de {layer_name}"),
                topo_count,
                total_topo_layers,
            );
            let topo_merged_path = format!("{project_folder}/resources/{layer_name}.gpkg");
            if let Err(e) = fusion_datasets(paths, &topo_merged_path).await {
                return Err(format!(
                    "Erreur lors de la fusion des couches topo {layer_name}: {e:?}"
                ));
            }
            topo_count += 1;
        }
    } else {
        Progress::full(
            "Fusion des données",
            "Copie des fichiers (une seule région)",
            1,
            1,
        );

        if let Err(e) = fs::rename(&regional_gpkgs[0], &regional_merged_gpkg).await {
            return Err(format!(
                "Erreur lors du renommage de la couche régionale: {e:?}"
            ));
        }

        if let Err(e) = fs::rename(&vegetation_gpkgs[0], &vegetation_merged_gpkg).await {
            return Err(format!(
                "Erreur lors du renommage de la couche de végétation: {e:?}"
            ));
        }

        if let Err(e) = fs::rename(&rpg_gpkgs[0], &rpg_merged_gpkg).await {
            return Err(format!("Erreur lors du renommage de la couche RPG: {e:?}"));
        }

        for (layer_name, paths) in &topo_gpkgs {
            if !paths.is_empty() {
                let topo_merged_path = format!("{project_folder}/resources/{layer_name}.gpkg");
                if let Err(e) = fs::rename(&paths[0], &topo_merged_path).await {
                    return Err(format!(
                        "Erreur lors du renommage de la couche topo {layer_name}: {e:?}"
                    ));
                }
            }
        }
    }

    if let Err(e) = clean_tmp(Some(".gpkg")) {
        return Err(format!(
            "Erreur lors du nettoyage des fichiers temporaires: {e:?}"
        ));
    }

    Progress::status("Ajout des Couches");
    if let Err(e) = add_layers(&project_file_path).await {
        return Err(format!("Erreur lors de l'ajout des couches: {e:?}"));
    }

    // Finalization phase
    Progress::status("Finalisation");
    let mut final_tracker = ProgressTracker::new("Finalisation", 2);

    final_tracker.set_step(1, "Export en JPEG");
    if let Err(e) = export_to_jpg(
        &project_file_path,
        format!("{project_folder}/{name}_VEGET.jpeg").as_str(),
    )
    .await
    {
        return Err(format!("Erreur lors de l'exportation de l'image: {e:?}"));
    }

    final_tracker.set_step(2, "Téléchargement d'orthophoto");
    if let Err(e) = fetch_orthophoto_wms(
        format!("{project_folder}/{name}_ORTHO.jpeg").as_str(),
        &project_bb,
    )
    .await
    {
        return Err(format!(
            "Erreur lors du téléchargement de l'image satellite: {e:?}"
        ));
    }

    Progress::status("Nettoyage");

    if let Err(e) = clean_tmp(None) {
        return Err(format!(
            "Erreur lors du nettoyage des fichiers temporaires: {e:?}"
        ));
    }

    Progress::status("Projet créé avec succès");

    Ok(project_folder)
}

pub mod prelude {
    pub use super::{create_project, delete_project, export, get_project_data, get_projects};
}
