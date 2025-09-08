use std::fs::create_dir_all;

use commands::*;

use tauri::AppHandle;
use utils::resolve_resource_dir;

use crate::{
    commands::{load_regions_graph, projects::get_projects},
    config::AppConfig,
};

pub mod commands;
pub mod config;
pub mod fetch_resources;
pub mod gis_operation;
pub mod types;
pub mod utils;

fn initialize_app(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    AppConfig::init(app_handle.clone())?;
    Ok(AppConfig::with_write(|config| {
        for dir_path in [&config.cache_dir, &config.temp_dir, &config.projects_dir] {
            create_dir_all(dir_path)?;
        }
        Ok(())
    })?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle();
            let res_dir = resolve_resource_dir(app_handle, "resources")?;
            unsafe {
                std::env::set_var("PROJ_LIB", res_dir.join("proj").to_str().unwrap());
                std::env::set_var("GDAL_DATA", res_dir.join("gdal").to_str().unwrap());
            }
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
            create_project,
            get_projects,
            get_os,
            export,
            delete_project,
            get_project_data,
            get_settings,
            save_settings,
            clear_cache,
            load_regions_graph
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
