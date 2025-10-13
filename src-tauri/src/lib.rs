use std::fs::create_dir_all;

use commands::*;

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;
use utils::resolve_resource_dir;

use crate::config::AppConfig;

pub mod commands;
pub mod config;
pub mod error;
pub mod services;
pub mod types;
pub mod utils;

fn initialize_app(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    AppConfig::init(app_handle.clone())?;
    Ok(AppConfig::with_write(|config| {
        for dir_path in [
            &config.cache_dir,
            &config.wms_cache_dir,
            &config.temp_dir,
            &config.projects_dir,
        ] {
            create_dir_all(dir_path)?;
        }
        Ok(())
    })?)
}

async fn check_for_updates(app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let updater = app.updater_builder().build()?;

    if let Some(update) = updater.check().await? {
        println!(
            "Update available: {} -> {}",
            update.current_version, update.version
        );

        update
            .download_and_install(
                |chunk_length, content_length| {
                    println!("Downloaded {} of {:?}", chunk_length, content_length);
                },
                || {
                    println!("Download finished");
                },
            )
            .await?;

        println!("Update installed, restarting...");
        app.restart();
    } else {
        println!("No updates available");
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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

                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = check_for_updates(handle).await {
                            eprintln!("Failed to check for updates: {}", e);
                        }
                    });

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
            check_regions_database,
            get_app_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
