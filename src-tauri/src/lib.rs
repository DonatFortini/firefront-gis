use app_setup::setup_check;
use commands::{create_project_com, delete_project, export, get_os, get_projects};

pub mod app_setup;
pub mod commands;
pub mod dependency;
pub mod gis_operation;
pub mod utils;
pub mod web_request;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup_check().expect("Setup check failed");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            create_project_com,
            get_projects,
            get_os,
            export,
            delete_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
