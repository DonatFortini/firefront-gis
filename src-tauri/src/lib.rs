use app_setup::initialize_app;
use commands::{
    clear_cache, create_project_com, delete_project, export, get_os, get_projects, get_settings,
    save_settings,
};

pub mod app_setup;
pub mod commands;
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
            match initialize_app(app_handle) {
                Ok(_) => {
                    println!("Application setup completed successfully");
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Application setup failed: {e:?}");
                    Err(Box::<dyn std::error::Error>::from(format!(
                        "Application setup failed: {e:?}"
                    )))
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
