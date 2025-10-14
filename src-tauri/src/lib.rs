use std::fs::create_dir_all;

use commands::*;
use tauri::{AppHandle, async_runtime::spawn};
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
        [
            &config.cache_dir,
            &config.wms_cache_dir,
            &config.temp_dir,
            &config.projects_dir,
        ]
        .iter()
        .try_for_each(create_dir_all)?;
        Ok(())
    })?)
}

async fn check_for_updates(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let updater = app.updater_builder().build()?;
    let timeout_duration = std::time::Duration::from_secs(15);

    match tokio::time::timeout(timeout_duration, updater.check()).await {
        Ok(Ok(Some(update))) => {
            println!(
                "Update available: {} -> {}",
                update.current_version, update.version
            );
            println!("Downloading update...");

            let download_timeout = std::time::Duration::from_secs(300);

            match tokio::time::timeout(
                download_timeout,
                update.download_and_install(
                    |chunk_length, content_length| {
                        if let Some(total) = content_length {
                            let progress = (chunk_length as f64 / total as f64) * 100.0;
                            println!("Download progress: {:.1}%", progress);
                        } else {
                            println!("Downloaded: {} bytes", chunk_length);
                        }
                    },
                    || println!("Download finished"),
                ),
            )
            .await
            {
                Ok(Ok(_)) => {
                    println!("Update installed successfully\nRestarting application...");
                    app.restart();
                }
                Ok(Err(e)) => eprintln!("Failed to download/install update: {}", e),
                Err(_) => eprintln!("Update download timed out after 5 minutes"),
            }
        }
        Ok(Ok(None)) => println!("No updates available - you're on the latest version"),
        Ok(Err(e)) => eprintln!("Update check failed: {}", e),
        Err(_) => eprintln!("Update check timed out after 15 seconds - continuing startup"),
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
            initialize_app(app_handle).map_err(|e| {
                eprintln!("Application setup failed: {e:?}");
                Box::<dyn std::error::Error>::from(format!("Application setup failed: {e:?}"))
            })?;

            let handle = app_handle.clone();
            spawn(async move {
                println!("Application started - checking for updates in background...");
                if let Err(e) = check_for_updates(handle).await {
                    eprintln!("Update check encountered an error: {}", e);
                }
                println!("Update check completed");
            });

            Ok(())
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
            get_app_version,
            check_for_updates_manual,
            get_cache_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
