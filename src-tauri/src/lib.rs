use app_setup::setup_check;
use commands::{
    clear_cache, create_project_com, delete_project, export, get_os, get_projects, get_settings,
    save_settings,
};

pub mod app_setup;
pub mod archive_utils;
pub mod commands;
pub mod dependency;
pub mod gis_operation;
pub mod utils;
pub mod web_request;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle();
            match setup_check(app_handle) {
                Ok(_) => {
                    println!("Application setup completed successfully");
                    Ok(())
                }
                Err(_) => {
                    eprintln!("Application setup failed");
                    Err(Box::<dyn std::error::Error>::from(
                        "Application setup failed",
                    ))
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            create_project_com,
            get_projects,
            get_os,
            export,
            delete_project,
            get_settings,
            save_settings,
            clear_cache
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
