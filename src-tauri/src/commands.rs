use std::{collections::HashMap, path::Path};

use tauri::{Emitter, command};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tokio::fs;

use crate::{
    app_setup,
    gis_operation::{
        create_project, fusion_datasets,
        layers::{add_layers, download_satellite_jpeg, prepare_layers},
        regions::find_intersecting_regions,
    },
    utils::{
        BoundingBox, cache_dir, clean_tmp_except_gpkg, create_directory_if_not_exists,
        export_project, export_to_jpg, get_operating_system, get_previous_projects, projects_dir,
    },
    web_request::{download_shp_file, get_shp_file_urls},
};

//TODO : modify to adapt to 3.11 of gdal

#[command(rename_all = "snake_case")]
/// Crée un projet avec les fichiers SHP associés.
/// Télécharge les fichiers SHP nécessaires, crée un projet de carte,
/// fusionne les couches et ajoute les couches au projet.
/// Télécharge également une image satellite et l'exporte en JPEG.
/// Nettoie les fichiers temporaires après la création du projet.
///
/// # Arguments
///
/// * `app_handle` - Handle de l'application Tauri.
/// * `name` - Nom du projet.
/// * `project_bb` - Boîte englobante du projet.
///
/// # Retourne
///
/// * `Result<String, String>` - Chemin du dossier du projet créé ou un message d'erreur.
pub async fn create_project_com(
    app_handle: tauri::AppHandle,
    name: String,
    project_bb: BoundingBox,
) -> Result<String, String> {
    let _ = app_handle.emit("progress-update", "Recherche des fichiers");

    create_directory_if_not_exists("tmp")
        .map_err(|e| format!("Erreur lors de la création du dossier tmp: {:?}", e))?;

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

    let _ = app_handle.emit("progress-update", "Téléchargement des données");

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

            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Téléchargement des données|{}|{}/{}",
                    file_type, download_count, total_downloads
                ),
            );

            let cache_path = format!(
                "{}/{}_{}.7z",
                cache_dir().to_string_lossy(),
                file_type,
                code
            );
            if !Path::new(&cache_path).exists() {
                download_shp_file(url, code).await.map_err(|e| {
                    format!(
                        "Erreur lors du téléchargement du fichier SHP depuis {}: {:?}",
                        url, e
                    )
                })?;
            }
        }
    }

    let _ = app_handle.emit("progress-update", "Initialisation du projet");
    let project_folder = format!("{}/{}", projects_dir().to_string_lossy(), name);
    let project_file_path = format!("{}/{}.tiff", project_folder, name);

    if std::path::Path::new(&project_file_path).exists() {
        let should_overwrite = app_handle
            .dialog()
            .message("project_exists")
            .title("Project already exists")
            .buttons(MessageDialogButtons::YesNo)
            .blocking_show();

        if !should_overwrite {
            return Ok("Project creation cancelled".to_string());
        }

        std::fs::remove_dir_all(&project_folder).unwrap();
    }

    let _ = app_handle.emit(
        "progress-update",
        "Initialisation du projet|Création des dossiers|1/2",
    );
    std::fs::create_dir_all(&project_folder).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(format!("{}/resources", project_folder)).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(format!("{}/slices", project_folder)).map_err(|e| e.to_string())?;

    let _ = app_handle.emit(
        "progress-update",
        "Initialisation du projet|Configuration du projet|2/2",
    );
    if let Err(e) = create_project(&project_file_path, &project_bb) {
        return Err(format!("Erreur lors de la création du projet: {:?}", e));
    }

    let _ = app_handle.emit("progress-update", "Préparation des Couches");

    let mut regional_gpkgs: Vec<String> = Vec::new();
    let mut vegetation_gpkgs: Vec<String> = Vec::new();
    let mut rpg_gpkgs: Vec<String> = Vec::new();
    let mut topo_gpkgs: HashMap<String, Vec<String>> = HashMap::new();

    let total_regions = region_codes.len();
    for (idx, code) in region_codes.iter().enumerate() {
        let _ = app_handle.emit(
            "progress-update",
            format!(
                "Préparation des Couches|Traitement de la région {}|{}/{}",
                code,
                idx + 1,
                total_regions
            ),
        );

        if idx > 0 {
            if let Err(e) = clean_tmp_except_gpkg() {
                return Err(format!(
                    "Erreur lors du nettoyage des fichiers temporaires: {:?}",
                    e
                ));
            }
        }

        let (r_gpkg, v_gpkg, rp_gpkg, t_gpkg) =
            prepare_layers(&app_handle, &project_bb, code).await?;

        regional_gpkgs.push(r_gpkg);
        vegetation_gpkgs.push(v_gpkg);
        rpg_gpkgs.push(rp_gpkg);

        for (layer_name, paths) in t_gpkg {
            topo_gpkgs.entry(layer_name).or_default().extend(paths);
        }

        if let Err(e) = clean_tmp_except_gpkg() {
            return Err(format!(
                "Erreur lors du nettoyage des fichiers temporaires: {:?}",
                e
            ));
        }
    }

    create_directory_if_not_exists("tmp")
        .map_err(|e| format!("Erreur lors de la création du dossier tmp: {:?}", e))?;

    let _ = app_handle.emit(
        "progress-update",
        "Fusion des données|Fusion des régions|1/4",
    );

    let regional_merged_gpkg = format!("{}/resources/{}.gpkg", project_folder, name);
    let vegetation_merged_gpkg = format!("{}/resources/FORMATION_VEGETALE.gpkg", project_folder);
    let rpg_merged_gpkg = format!("{}/resources/PARCELLES_GRAPHIQUES.gpkg", project_folder);

    if region_codes.len() > 1 {
        let _ = app_handle.emit(
            "progress-update",
            "Fusion des données|Fusion des couches régionales|1/4",
        );
        if let Err(e) = fusion_datasets(&regional_gpkgs, &regional_merged_gpkg) {
            return Err(format!(
                "Erreur lors de la fusion des couches régionales: {:?}",
                e
            ));
        }

        let _ = app_handle.emit(
            "progress-update",
            "Fusion des données|Fusion des couches de végétation|2/4",
        );
        if let Err(e) = fusion_datasets(&vegetation_gpkgs, &vegetation_merged_gpkg) {
            return Err(format!(
                "Erreur lors de la fusion des couches de végétation: {:?}",
                e
            ));
        }

        let _ = app_handle.emit(
            "progress-update",
            "Fusion des données|Fusion des couches RPG|3/4",
        );
        if let Err(e) = fusion_datasets(&rpg_gpkgs, &rpg_merged_gpkg) {
            return Err(format!("Erreur lors de la fusion des couches RPG: {:?}", e));
        }

        let _ = app_handle.emit(
            "progress-update",
            "Fusion des données|Fusion des couches topographiques|4/4",
        );

        let total_topo_layers = topo_gpkgs.len();
        let mut topo_count = 1;
        for (layer_name, paths) in &topo_gpkgs {
            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Fusion des données|Fusion de {}|{}/{}",
                    layer_name, topo_count, total_topo_layers
                ),
            );
            let topo_merged_path = format!("{}/resources/{}.gpkg", project_folder, layer_name);
            if let Err(e) = fusion_datasets(paths, &topo_merged_path) {
                return Err(format!(
                    "Erreur lors de la fusion des couches topo {}: {:?}",
                    layer_name, e
                ));
            }
            topo_count += 1;
        }
    } else {
        let _ = app_handle.emit(
            "progress-update",
            "Fusion des données|Copie des fichiers (une seule région)|1/1",
        );

        if let Err(e) = fs::rename(&regional_gpkgs[0], &regional_merged_gpkg).await {
            return Err(format!(
                "Erreur lors du renommage de la couche régionale: {:?}",
                e
            ));
        }

        if let Err(e) = fs::rename(&vegetation_gpkgs[0], &vegetation_merged_gpkg).await {
            return Err(format!(
                "Erreur lors du renommage de la couche de végétation: {:?}",
                e
            ));
        }

        if let Err(e) = fs::rename(&rpg_gpkgs[0], &rpg_merged_gpkg).await {
            return Err(format!(
                "Erreur lors du renommage de la couche RPG: {:?}",
                e
            ));
        }

        for (layer_name, paths) in &topo_gpkgs {
            if !paths.is_empty() {
                let topo_merged_path = format!("{}/resources/{}.gpkg", project_folder, layer_name);
                if let Err(e) = fs::rename(&paths[0], &topo_merged_path).await {
                    return Err(format!(
                        "Erreur lors du renommage de la couche topo {}: {:?}",
                        layer_name, e
                    ));
                }
            }
        }
    }

    if let Err(e) = clean_tmp_except_gpkg() {
        return Err(format!(
            "Erreur lors du nettoyage des fichiers temporaires: {:?}",
            e
        ));
    }

    let _ = app_handle.emit("progress-update", "Ajout des Couches");
    if let Err(e) = add_layers(&app_handle, &project_folder, &project_file_path, &name) {
        return Err(format!("Erreur lors de l'ajout des couches: {:?}", e));
    }

    let _ = app_handle.emit("progress-update", "Finalisation");
    let _ = app_handle.emit("progress-update", "Finalisation|Export en JPEG|1/2");
    if let Err(e) = export_to_jpg(
        &project_file_path,
        format!("{}/{}_VEGET.jpeg", project_folder, name).as_str(),
    ) {
        return Err(format!("Erreur lors de l'exportation de l'image: {:?}", e));
    }

    let _ = app_handle.emit(
        "progress-update",
        "Finalisation|Téléchargement d'orthophoto|2/2",
    );
    if let Err(e) = download_satellite_jpeg(
        format!("{}/{}_ORTHO.jpeg", project_folder, name).as_str(),
        &project_bb,
    ) {
        return Err(format!(
            "Erreur lors du téléchargement de l'image satellite: {:?}",
            e
        ));
    }

    let _ = app_handle.emit("progress-update", "Nettoyage");
    fs::remove_dir_all("tmp")
        .await
        .map_err(|e| format!("Erreur lors de la suppression du dossier tmp: {:?}", e))?;

    fs::create_dir("tmp")
        .await
        .map_err(|e| format!("Erreur lors de la création du dossier tmp: {:?}", e))?;

    let _ = app_handle.emit("progress-update", "Projet créé avec succès");

    Ok(project_folder)
}

#[command]
/// Obtient la liste des projets précédents.
///
/// # Retourne
/// - HashMap<String, Vec<String>> : Une hashmap contenant le nom du projet et la liste des fichiers associés.
pub fn get_projects() -> HashMap<String, Vec<String>> {
    get_previous_projects().unwrap()
}

#[command]
pub fn get_os() -> String {
    get_operating_system().to_string()
}

#[command(rename_all = "snake_case")]
/// Exporte un projet, fais la decoupe puis le zip
///
/// # Paramètres
/// - project_name: &str : Le nom du projet à exporter.
///
/// # Retourne
/// - Result<String, String> : Un résultat contenant le message de succès ou l'erreur.
pub fn export(project_name: &str) -> Result<String, String> {
    match export_project(project_name) {
        Ok(_) => {
            println!("Exportation réussie");
            Ok("success".to_string())
        }
        Err(e) => {
            println!("Erreur lors de l'exportation: {:?}", e);
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
        return Err(format!("Le projet '{}' n'existe pas", project_name));
    }

    match tokio::fs::remove_dir_all(&project_folder).await {
        Ok(_) => {
            println!("Projet '{}' supprimé avec succès", project_name);
            Ok("success".to_string())
        }
        Err(e) => {
            println!(
                "Erreur lors de la suppression du projet '{}': {:?}",
                project_name, e
            );
            Err(format!("Erreur lors de la suppression du projet: {}", e))
        }
    }
}

#[command]
/// Récupère les paramètres de configuration de l'application.
///
/// # Retourne
/// - `Result<serde_json::Value, String>` : Un objet JSON contenant les paramètres de configuration ou une erreur.
pub fn get_settings() -> Result<serde_json::Value, String> {
    let config = app_setup::CONFIG.lock().unwrap();
    let output_location = config.output_location.to_string_lossy().to_string();
    let gdal_path = config
        .gdal_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    Ok(serde_json::json!({
        "output_location": output_location,
        "gdal_path": gdal_path,
    }))
}

#[command(rename_all = "snake_case")]
/// Enregistre les paramètres de configuration de l'application.
///     
/// # Arguments
///
/// * `output_location` - Option<String> : L'emplacement de sortie.
/// * `gdal_path` - Option<String> : Le chemin vers GDAL.
///
/// # Retourne
///
/// * `String` : Un message de succès ou d'erreur.
pub fn save_settings(output_location: Option<String>, gdal_path: Option<String>) -> String {
    let mut config = app_setup::CONFIG.lock().unwrap();
    match config.update_settings(output_location, gdal_path) {
        Ok(_) => "Paramètres sauvegardés avec succès".to_string(),
        Err(e) => {
            format!("Échec de sauvegarde des paramètres: {}", e)
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
        Err(e) => Err(format!("Échec du vidage du cache: {}", e)),
    }
}
