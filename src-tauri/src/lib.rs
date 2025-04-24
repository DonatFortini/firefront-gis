pub mod app_setup;
pub mod dependency;
pub mod gis_processing;
pub mod slicing;
pub mod utils;
pub mod web_request;

use app_setup::setup_check;
use gis_processing::{
    add_regional_layer, add_rpg_layer, add_topo_layer, add_vegetation_layer, clip_to_extent,
    convert_to_gpkg, create_project, download_satellite_jpeg, export_to_jpg, get_regional_extent,
};

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use tauri::Emitter;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tokio::fs;
use utils::{
    export_project, extract_files_by_name, get_departement_list, get_previous_projects,
    get_rpg_for_dep_code,
};
use web_request::{download_shp_file, get_departement_shp_file_url};

//---------------------------------------------------------tauri commands---------------------------------------------------------

#[tauri::command]
/// Crée un nouveau projet en suivant plusieurs étapes : téléchargement de fichiers,
/// initialisation du projet, préparation et ajout de couches, exportation d'images,
/// et nettoyage des fichiers temporaires. Émet également des événements de progression
/// pour informer l'utilisateur de l'état d'avancement.
///
/// # Arguments
///
/// * `app_handle` - Une instance de `tauri::AppHandle` utilisée pour interagir avec
///                  l'application (émission d'événements, affichage de dialogues, etc.).
/// * `code` - Un code unique utilisé pour identifier les fichiers à télécharger
///            (par exemple, un code géographique ou un identifiant de projet).
/// * `name` - Le nom du projet à créer. Ce nom sera utilisé pour nommer le dossier
///            du projet et les fichiers associés.
/// * `xmin` - La coordonnée minimale en X (longitude) de la zone géographique du projet.
/// * `ymin` - La coordonnée minimale en Y (latitude) de la zone géographique du projet.
/// * `xmax` - La coordonnée maximale en X (longitude) de la zone géographique du projet.
/// * `ymax` - La coordonnée maximale en Y (latitude) de la zone géographique du projet).
///
/// # Retourne
///
/// * `Ok(String)` - Le chemin du dossier du projet créé.
/// * `Err(String)` - Un message d'erreur descriptif en cas de problème.
async fn open_new_project(
    app_handle: tauri::AppHandle,
    code: String,
    name: String,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
) -> Result<String, String> {
    let _ = app_handle.emit("progress-update", "Recherche des fichiers");
    let urls = get_shp_file_urls(&code).await.map_err(|e| e.to_string())?;
    let _ = app_handle.emit("progress-update", "Téléchargement des données");
    let file_types = ["BDTOPO", "BDFORET", "RPG"];
    for (i, (url, file_type)) in urls.iter().zip(file_types.iter()).enumerate() {
        let _ = app_handle.emit(
            "progress-update",
            format!(
                "Téléchargement des données|{}|{}/{}",
                file_type,
                i + 1,
                file_types.len()
            ),
        );

        if !Path::new(format!("projects/cache/{}_{}.7z", file_type, code).as_str()).exists() {
            download_shp_file(url, &code).await.map_err(|e| {
                format!(
                    "Erreur lors du téléchargement du fichier SHP depuis {}: {:?}",
                    url, e
                )
            })?;
        }
    }

    let _ = app_handle.emit("progress-update", "Initialisation du projet");
    let project_folder = format!("projects/{}", name);
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
    if let Err(e) = create_project(&project_file_path, xmin, ymin, xmax, ymax) {
        return Err(format!("Erreur lors de la création du projet: {:?}", e));
    }

    let _ = app_handle.emit("progress-update", "Préparation des Couches");
    prepare_layers(
        &app_handle,
        &project_folder,
        &name,
        xmin,
        xmax,
        ymin,
        ymax,
        &code,
    )
    .await?;

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
        xmin,
        ymin,
        xmax,
        ymax,
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

#[tauri::command]
/// Obtient la liste des départements.
///
/// # Retourne
/// - HashMap<String, String> : Une hashmap contenant le code et le nom des départements.
fn get_dpts_list() -> HashMap<String, String> {
    get_departement_list()
}

#[tauri::command]
/// Obtient la liste des projets précédents.
///
/// # Retourne
/// - HashMap<String, Vec<String>> : Une hashmap contenant le nom du projet et la liste des fichiers associés.
fn get_projects() -> HashMap<String, Vec<String>> {
    get_previous_projects().unwrap()
}

#[tauri::command]
fn get_os() -> String {
    utils::get_operating_system().to_string()
}

#[tauri::command(rename_all = "snake_case")]
/// Exporte un projet, fais la decoupe puis le zip
///
/// # Paramètres
/// - project_name: &str : Le nom du projet à exporter.
///
/// # Retourne
/// - Result<String, String> : Un résultat contenant le message de succès ou l'erreur.
fn export(project_name: &str) -> Result<String, String> {
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

#[tauri::command(rename_all = "snake_case")]
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
async fn delete_project(project_name: &str) -> Result<String, String> {
    let project_folder = format!("projects/{}", project_name);
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

//---------------------------------------------------------main---------------------------------------------------------
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup_check().expect("Setup check failed");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            open_new_project,
            get_dpts_list,
            get_projects,
            get_os,
            export,
            delete_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

//---------------------------------------------------------functions---------------------------------------------------------

/// Obtients les URLs des fichiers SHP pour le code de département donné.
/// # Paramètres
/// - `code`: Une tranche de chaîne qui contient le code du département.
/// # Retourne
/// - Result<Vec<String>, String> : Un vecteur contenant les URLs des fichiers SHP.
async fn get_shp_file_urls(code: &str) -> Result<Vec<String>, String> {
    let url1 = get_departement_shp_file_url(
        code,
        "https://geoservices.ign.fr/bdtopo#telechargementgpkgreg",
    )
    .await
    .map_err(|e| {
        format!(
            "Erreur lors de la récupération de l'URL du fichier Topo : {:?}",
            e
        )
    })?;
    let url2 =
        get_departement_shp_file_url(code, "https://geoservices.ign.fr/bdforet#telechargementv2")
            .await
            .map_err(|e| {
                format!(
                    "Erreur lors de la récupération de l'URL du fichier Foret : {:?}",
                    e
                )
            })?;

    let rpg_code = get_rpg_for_dep_code(code).unwrap();
    let url3 = get_departement_shp_file_url(
        rpg_code,
        "https://geoservices.ign.fr/rpg#telechargementrpg2023",
    )
    .await
    .map_err(|e| {
        format!(
            "Erreur lors de la récupération de l'URL du fichier RPG : {:?}",
            e
        )
    })?;

    Ok(vec![url1, url2, url3])
}

/// Prépare les couches pour le projet, en les convertissant au format GPKG et en les découpant à l'extent régional.
/// # Paramètres
/// - `project_folder`: Le dossier du projet.
/// - `project_name`: Le nom du projet.
/// - `xmin`: La coordonnée X minimale.
/// - `xmax`: La coordonnée X maximale.
/// - `ymin`: La coordonnée Y minimale.
/// - `ymax`: La coordonnée Y maximale.
/// - `code`: Le code du département.
/// # Retourne
/// - Result<(), String> : Un résultat vide ou un message d'erreur.
async fn prepare_layers(
    app_handle: &tauri::AppHandle,
    project_folder: &str,
    project_name: &str,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    code: &str,
) -> Result<(), String> {
    let cache_folder_path = "projects/cache".to_string();

    let _ = app_handle.emit(
        "progress-update",
        "Préparation des Couches|Préparation de l'étendue régionale|1/4",
    );
    let regional_geojson = format!("{}/{}.geojson", cache_folder_path, code);
    if !Path::new(&regional_geojson).exists() {
        let _ = get_regional_extent(code);
    }

    let temp_regional_gpkg = format!("/tmp/{}.gpkg", code);
    let regional_gpkg = format!("{}/resources/{}.gpkg", project_folder, project_name);

    let _ = convert_to_gpkg(&regional_geojson, &temp_regional_gpkg);
    let _ = clip_to_extent(&temp_regional_gpkg, &regional_gpkg, xmin, ymin, xmax, ymax);

    let mut layers: HashMap<String, Vec<&str>> = HashMap::new();
    layers.insert(format!("BDFORET_{}.7z", code), vec!["FORMATION_VEGETALE"]);
    layers.insert(format!("RPG_{}.7z", code), vec!["PARCELLES_GRAPHIQUES"]);
    layers.insert(
        format!("BDTOPO_{}.7z", code),
        vec![
            "AERODROME",
            "CONSTRUCTION_SURFACIQUE",
            "EQUIPEMENT_DE_TRANSPORT",
            "RESERVOIR",
            "TERRAIN_DE_SPORT",
            "TRONCON_DE_VOIE_FERREE",
            "ZONE_D_ESTRAN",
            "BATIMENT",
            "COURS_D_EAU",
            "PLAN_D_EAU",
            "SURFACE_HYDROGRAPHIQUE",
            "TRONCON_DE_ROUTE",
            "VOIE_NOMMEE",
        ],
    );

    let mut layer_index = 2;
    let total_archives = layers.len();

    for (archive, files) in layers {
        let layer_type = if archive.contains("BDFORET") {
            "Végétation"
        } else if archive.contains("RPG") {
            "Parcelles agricoles"
        } else if archive.contains("BDTOPO") {
            "Topographie"
        } else {
            "Inconnu"
        };

        let _ = app_handle.emit(
            "progress-update",
            format!(
                "Préparation des Couches|Préparation des couches {}|{}/{}",
                layer_type,
                layer_index,
                total_archives + 1
            ),
        );

        let archive_path = format!("{}/{}", cache_folder_path, archive);

        let total_files = files.len();
        for (file_index, file) in files.iter().enumerate() {
            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Préparation des Couches|Extraction de {}|{}/{}",
                    file,
                    file_index + 1,
                    total_files
                ),
            );

            extract_files_by_name(&archive_path, file, "tmp").map_err(|e| {
                format!(
                    "Erreur lors de l'extraction du fichier {} depuis l'archive {}: {:?}",
                    file, archive, e
                )
            })?;

            let temp_file = format!("tmp/{}/{}.shp", file, file);
            let temp_gpkg = format!("tmp/{}.gpkg", file);
            let output_gpkg = format!("{}/resources/{}.gpkg", project_folder, file);

            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Préparation des Couches|Conversion de {}|{}/{}",
                    file,
                    file_index + 1,
                    total_files
                ),
            );

            if let Err(e) = convert_to_gpkg(&temp_file, &temp_gpkg) {
                return Err(format!(
                    "Erreur lors de la conversion du fichier {} en GPKG: {:?}",
                    temp_file, e
                ));
            }

            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Préparation des Couches|Découpage de {}|{}/{}",
                    file,
                    file_index + 1,
                    total_files
                ),
            );

            if let Err(e) = clip_to_extent(&temp_gpkg, &output_gpkg, xmin, ymin, xmax, ymax) {
                return Err(format!(
                    "Erreur lors du découpage du fichier {}: {:?}",
                    temp_gpkg, e
                ));
            }
        }

        layer_index += 1;
    }

    Ok(())
}

/// Ajoute les couches au projet.
/// # Paramètres
/// - `project_folder`: Le dossier du projet.
/// - `project_file_path`: Le chemin du fichier du projet.
/// - `project_name`: Le nom du projet.
/// # Retourne
/// - Result<(), Box<dyn std::error::Error>> : Un résultat vide ou une erreur.
fn add_layers(
    app_handle: &tauri::AppHandle,
    project_folder: &str,
    project_file_path: &str,
    project_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = app_handle.emit(
        "progress-update",
        "Ajout des Couches|Ajout de la couche régionale|1/4",
    );

    if let Err(e) = add_regional_layer(
        project_file_path,
        &format!("{}/resources/{}.gpkg", project_folder, project_name),
    ) {
        println!("Failed to add regional layer: {:?}", e);
        return Err(e);
    }

    let mut layers: BTreeMap<i8, Vec<&str>> = BTreeMap::new();
    layers.insert(1, vec!["FORMATION_VEGETALE"]);
    layers.insert(2, vec!["PARCELLES_GRAPHIQUES"]);
    layers.insert(
        3,
        vec![
            "AERODROME",
            "CONSTRUCTION_SURFACIQUE",
            "EQUIPEMENT_DE_TRANSPORT",
            "RESERVOIR",
            "TERRAIN_DE_SPORT",
            "TRONCON_DE_VOIE_FERREE",
            "ZONE_D_ESTRAN",
            "BATIMENT",
            "COURS_D_EAU",
            "PLAN_D_EAU",
            "SURFACE_HYDROGRAPHIQUE",
            "TRONCON_DE_ROUTE",
            "VOIE_NOMMEE",
        ],
    );

    let mut layer_index = 2;
    let total_layer_types = layers.len() + 1;

    for (key, value) in layers {
        let layer_type = match key {
            1 => "Végétation",
            2 => "Parcelles agricoles",
            3 => "Topographie",
            _ => "Inconnu",
        };

        let _ = app_handle.emit(
            "progress-update",
            format!(
                "Ajout des Couches|Ajout des couches {}|{}/{}",
                layer_type, layer_index, total_layer_types
            ),
        );

        let total_files = value.len();
        for (file_index, file) in value.iter().enumerate() {
            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Ajout des Couches|Ajout de {}|{}/{}",
                    file,
                    file_index + 1,
                    total_files
                ),
            );

            let layer_path = format!("{}/resources/{}.gpkg", project_folder, file);
            match key {
                1 => add_vegetation_layer(project_file_path, &layer_path),
                2 => add_rpg_layer(project_file_path, &layer_path),
                3 => add_topo_layer(project_file_path, &layer_path),
                _ => {
                    println!("Unknown layer type");
                    return Err(Box::new(std::io::Error::other("Unknown layer type")));
                }
            }?
        }

        layer_index += 1;
    }

    Ok(())
}
