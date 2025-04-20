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
    extract_files_by_name, get_departement_list, get_previous_projects, get_rpg_for_dep_code,
};
use web_request::{download_shp_file, get_departement_shp_file_url};

//---------------------------------------------------------tauri commands---------------------------------------------------------

//TODO : refactor file at the end when everything is working and remake the doc

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

    download_shp_files(&urls, &code)
        .await
        .map_err(|e| e.to_string())?;

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

    std::fs::create_dir_all(&project_folder).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(format!("{}/resources", project_folder)).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(format!("{}/slices", project_folder)).map_err(|e| e.to_string())?;

    if let Err(e) = create_project(&project_file_path, xmin, ymin, xmax, ymax) {
        return Err(format!("Erreur lors de la création du projet: {:?}", e));
    }

    let _ = app_handle.emit("progress-update", "Préparation des Couches");

    prepare_layers(&project_folder, &name, xmin, xmax, ymin, ymax, &code).await?;

    let _ = app_handle.emit("progress-update", "Ajout des Couches");

    if let Err(e) = add_layers(&project_folder, &project_file_path, &name) {
        return Err(format!("Erreur lors de l'ajout des couches: {:?}", e));
    }

    let _ = app_handle.emit("progress-update", "Finalisation");

    if let Err(e) = export_to_jpg(
        &project_file_path,
        format!("{}/{}_VEGET.jpeg", project_folder, name).as_str(),
    ) {
        return Err(format!("Erreur lors de l'exportation de l'image: {:?}", e));
    }

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
fn get_projects() -> HashMap<String, Vec<String>> {
    get_previous_projects().unwrap()
}

#[tauri::command]
fn get_os() -> String {
    utils::get_operating_system().to_string()
}

// TODO : reimplement with gdalinfo
#[tauri::command]
fn get_project_layers(
    _project_path: &str,
) -> Result<HashMap<String, HashMap<String, String>>, String> {
    // Mock implementation
    let mut layers = HashMap::new();
    let mut layer_details = HashMap::new();
    layer_details.insert("type".to_string(), "vegetation".to_string());
    layer_details.insert("status".to_string(), "loaded".to_string());
    layers.insert("layer1".to_string(), layer_details);

    Ok(layers)
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
            get_project_layers
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
        None,
    )
    .await
    .map_err(|e| {
        format!(
            "Erreur lors de la récupération de l'URL du fichier Topo : {:?}",
            e
        )
    })?;
    let url2 = get_departement_shp_file_url(
        code,
        "https://geoservices.ign.fr/bdforet#telechargementv2",
        None,
    )
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
        Some("R"),
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

/// Télécharge le fichier SHP depuis l'URL donnée.
/// # Paramètres
/// - `url`: Une tranche de chaîne qui contient l'URL du fichier SHP.
/// - `code`: Une tranche de chaîne qui contient le code du département.
/// # Retourne
/// - Result<(), String> : Un résultat vide ou un message d'erreur.
async fn download_shp_files(urls: &[String], code: &str) -> Result<(), String> {
    println!("Téléchargement des fichiers SHP");
    if Path::new(format!("projects/cache/BDTOPO_{}.7z", code).as_str()).exists()
        && Path::new(format!("projects/cache/BDFORET_{}.7z", code).as_str()).exists()
        && Path::new(format!("projects/cache/RPG_{}.7z", code).as_str()).exists()
    {
        return Ok(());
    }

    for url in urls {
        download_shp_file(url, code).await.map_err(|e| {
            format!(
                "Erreur lors du téléchargement du fichier SHP depuis {}: {:?}",
                url, e
            )
        })?;
        println!("Fichier SHP téléchargé depuis {}", url);
    }
    println!("Téléchargement des fichiers SHP terminé");
    Ok(())
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
    project_folder: &str,
    project_name: &str,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    code: &str,
) -> Result<(), String> {
    let cache_folder_path = "projects/cache".to_string();

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

    for (archive, files) in layers {
        let archive_path = format!("{}/{}", cache_folder_path, archive);
        for file in files {
            extract_files_by_name(&archive_path, file, "tmp").map_err(|e| {
                format!(
                    "Erreur lors de l'extraction du fichier {} depuis l'archive {}: {:?}",
                    file, archive, e
                )
            })?;

            let temp_file = format!("tmp/{}/{}.shp", file, file);
            let temp_gpkg = format!("tmp/{}.gpkg", file);
            let output_gpkg = format!("{}/resources/{}.gpkg", project_folder, file);
            if let Err(e) = convert_to_gpkg(&temp_file, &temp_gpkg) {
                return Err(format!(
                    "Erreur lors de la conversion du fichier {} en GPKG: {:?}",
                    temp_file, e
                ));
            }
            if let Err(e) = clip_to_extent(&temp_gpkg, &output_gpkg, xmin, ymin, xmax, ymax) {
                return Err(format!(
                    "Erreur lors du découpage du fichier {}: {:?}",
                    temp_gpkg, e
                ));
            }
        }
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
    project_folder: &str,
    project_file_path: &str,
    project_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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

    for (key, value) in layers {
        for file in value {
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
    }

    Ok(())
}
