use std::collections::HashMap;

use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tokio::fs;

use crate::error::{GisError, GisResult, ProjectError, ProjectResult};
use crate::services::{
    ArchiveService, FetchService, ProcessingService, RasterService, VectorService,
};
use crate::types::BoundingBox;
use crate::types::regions::find_intersecting_regions;
use crate::utils::{
    Progress, ProgressTracker, clean_tmp, execute_sidecar, get_handle, output_location,
    projects_dir, slice_factor, temp_dir,
};

pub struct ProjectService;
//TODO: ajout metadata projet
impl ProjectService {
    pub fn list_projects() -> ProjectResult<HashMap<String, Vec<String>>> {
        let projects_path = projects_dir();
        let mut projects = HashMap::new();

        if !projects_path.exists() {
            return Ok(projects);
        }

        for entry in std::fs::read_dir(&projects_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir()
                && let Some(project_name) = path.file_name().and_then(|n| n.to_str())
            {
                let project_path = projects_dir().join(project_name);
                let preview_image = project_path.join(format!("{}_ORTHO.jpeg", project_name));

                projects.insert(
                    project_name.to_string(),
                    vec![
                        preview_image.to_string_lossy().to_string(),
                        project_path.to_string_lossy().to_string(),
                    ],
                );
            }
        }

        Ok(projects)
    }

    pub async fn get_project_bounding_box(project_name: &str) -> GisResult<BoundingBox> {
        let tiff_path = format!(
            "{}/{}/{}.tiff",
            projects_dir().to_string_lossy(),
            project_name,
            project_name
        );

        let (output, _) = execute_sidecar("gdalinfo", &[&tiff_path, "-json"])
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "gdalinfo".to_string(),
                message: e.to_string(),
            })?;

        let json: serde_json::Value = serde_json::from_str(&output)?;

        let coords = json["cornerCoordinates"]
            .as_object()
            .ok_or_else(|| GisError::Dataset("Invalid gdalinfo output".to_string()))?;

        Ok(BoundingBox {
            xmin: coords["lowerLeft"][0].as_f64().unwrap(),
            ymin: coords["lowerLeft"][1].as_f64().unwrap(),
            xmax: coords["upperRight"][0].as_f64().unwrap(),
            ymax: coords["upperRight"][1].as_f64().unwrap(),
        })
    }

    pub fn get_project_data(name: &str, data_file: &str) -> ProjectResult<String> {
        let project_folder = projects_dir().join(name);
        let data_path = project_folder.join(data_file);

        if !data_path.exists() {
            return Err(ProjectError::NotFound {
                name: name.to_string(),
            });
        }

        Ok(data_path.to_string_lossy().to_string())
    }

    pub async fn delete_project(name: &str) -> ProjectResult<()> {
        let project_folder = projects_dir().join(name);

        if !project_folder.exists() {
            return Err(ProjectError::NotFound {
                name: name.to_string(),
            });
        }

        tokio::fs::remove_dir_all(&project_folder).await?;
        println!("Project '{}' deleted successfully", name);
        Ok(())
    }

    pub async fn export_project(name: &str) -> ProjectResult<()> {
        let project_path = projects_dir().join(name);

        if !project_path.exists() {
            return Err(ProjectError::NotFound {
                name: name.to_string(),
            });
        }

        let slice_factor_value = slice_factor();
        let output_dir = output_location();

        println!("Exporting project: {}", name);

        RasterService::slice_project(name, slice_factor_value)
            .await
            .map_err(|e| ProjectError::ExportFailed {
                project: name.to_string(),
                message: e.to_string(),
            })?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        ArchiveService::compress_folder(
            &project_path.to_string_lossy(),
            &format!("export_{}_{}", name, timestamp),
            &output_dir.to_string_lossy(),
        )
        .await
        .map_err(|e| ProjectError::ExportFailed {
            project: name.to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }

    pub async fn create_project(name: String, project_bb: BoundingBox) -> ProjectResult<()> {
        Progress::status("Recherche des régions");
        let regions = find_intersecting_regions(&project_bb)
            .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

        if regions.is_empty() {
            return Err(ProjectError::NoIntersectingRegions);
        }

        let region_codes: Vec<&str> = regions.iter().map(|r| r.code.as_str()).collect();

        Self::download_data_phase(&region_codes).await?;
        let project_folder = Self::initialize_project(&name, &project_bb).await?;
        Self::prepare_layers_phase(&name, &region_codes, &project_bb, &project_folder).await?;

        Self::process_elevation_phase(&region_codes, &project_bb, &project_folder).await?;

        let project_file_path = format!("{}/{}.tiff", project_folder, name);
        Progress::status("Ajout des couches");
        ProcessingService::add_all_layers(&project_file_path)
            .await
            .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

        Self::finalize_project(&name, &project_folder, &project_file_path, &project_bb).await?;

        Progress::status("Projet créé avec succès");
        Ok(())
    }

    async fn process_elevation_phase(
        region_codes: &[&str],
        project_bb: &BoundingBox,
        project_folder: &str,
    ) -> ProjectResult<()> {
        use crate::services::ElevationService;

        Progress::status("Traitement de l'élévation");

        let elevation_output = format!("{}/resources/elevation.tif", project_folder);

        let mut elevation_tiles = Vec::new();

        for (idx, code) in region_codes.iter().enumerate() {
            Progress::full(
                "Traitement de l'élévation",
                format!("Région {}", code),
                idx + 1,
                region_codes.len(),
            );

            let temp_elevation = format!("{}/elevation_{}.tif", temp_dir().display(), code);

            ElevationService::process_elevation_tiles(project_bb, code, &temp_elevation)
                .await
                .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

            elevation_tiles.push(temp_elevation);
        }

        if elevation_tiles.len() > 1 {
            Progress::status("Fusion des tuiles d'élévation");
            Self::merge_elevation_tiles(&elevation_tiles, &elevation_output).await?;

            for tile in &elevation_tiles {
                tokio::fs::remove_file(tile).await.ok();
            }
        } else if !elevation_tiles.is_empty() {
            tokio::fs::rename(&elevation_tiles[0], &elevation_output).await?;
        }

        Ok(())
    }

    async fn merge_elevation_tiles(tiles: &[String], output: &str) -> ProjectResult<()> {
        let vrt_path = format!("{}/merged_elevation.vrt", temp_dir().display());

        let mut args = vec!["-overwrite", &vrt_path];
        args.extend(tiles.iter().map(|s| s.as_str()));

        execute_sidecar("gdalbuildvrt", &args)
            .await
            .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

        execute_sidecar(
            "gdal_translate",
            &[
                "-of",
                "GTiff",
                "-co",
                "COMPRESS=LZW",
                "-co",
                "TILED=YES",
                &vrt_path,
                output,
            ],
        )
        .await
        .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

        tokio::fs::remove_file(&vrt_path).await.ok();

        Ok(())
    }

    async fn download_data_phase(region_codes: &[&str]) -> ProjectResult<()> {
        Progress::status("Téléchargement des données");

        FetchService::fetch_data_sources(region_codes)
            .await
            .map_err(|e| {
                ProjectError::CreationFailed(format!("Échec du téléchargement des données: {}", e))
            })?;

        Ok(())
    }

    async fn initialize_project(name: &str, project_bb: &BoundingBox) -> ProjectResult<String> {
        Progress::status("Initialisation du projet");

        let project_folder = projects_dir().join(name);
        let project_file = project_folder.join(format!("{}.tiff", name));

        if project_file.exists() {
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

        let mut tracker = ProgressTracker::new("Initialisation du projet", 2);

        tracker.set_step(1, "Création des dossiers");
        std::fs::create_dir_all(project_folder.join("resources"))?;
        std::fs::create_dir_all(project_folder.join("slices"))?;

        tracker.set_step(2, "Configuration du projet");
        RasterService::create_reference_raster(&project_file.to_string_lossy(), project_bb)
            .await
            .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

        Ok(project_folder.to_string_lossy().to_string())
    }

    async fn prepare_layers_phase(
        name: &str,
        region_codes: &[&str],
        project_bb: &BoundingBox,
        project_folder: &str,
    ) -> ProjectResult<()> {
        Progress::status("Préparation des couches");

        let mut regional_gpkgs = Vec::new();
        let mut vegetation_gpkgs = Vec::new();
        let mut rpg_gpkgs = Vec::new();
        let mut topo_gpkgs: HashMap<String, Vec<String>> = HashMap::new();

        for (idx, code) in region_codes.iter().enumerate() {
            Progress::full(
                "Préparation des couches",
                format!("Traitement de la région {}", code),
                idx + 1,
                region_codes.len(),
            );

            clean_tmp(Some(".gpkg")).map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

            let (r, v, rp, t) = ProcessingService::prepare_layers(project_bb, code)
                .await
                .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

            regional_gpkgs.push(r);
            vegetation_gpkgs.push(v);
            rpg_gpkgs.push(rp);

            for (layer, paths) in t {
                topo_gpkgs.entry(layer).or_default().extend(paths);
            }

            clean_tmp(Some(".gpkg")).map_err(|e| ProjectError::CreationFailed(e.to_string()))?;
        }

        Self::merge_layers(
            name,
            project_folder,
            regional_gpkgs,
            vegetation_gpkgs,
            rpg_gpkgs,
            topo_gpkgs,
            region_codes.len() > 1,
        )
        .await?;

        clean_tmp(Some(".gpkg")).map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

        Ok(())
    }

    async fn merge_layers(
        name: &str,
        project_folder: &str,
        regional: Vec<String>,
        vegetation: Vec<String>,
        rpg: Vec<String>,
        topo: HashMap<String, Vec<String>>,
        should_merge: bool,
    ) -> ProjectResult<()> {
        Progress::status("Fusion des données");

        let regional_out = format!("{}/resources/{}.gpkg", project_folder, name);
        let vegetation_out = format!("{}/resources/FORMATION_VEGETALE.gpkg", project_folder);
        let rpg_out = format!("{}/resources/PARCELLES_GRAPHIQUES.gpkg", project_folder);

        if should_merge {
            let mut tracker = ProgressTracker::new("Fusion des données", 4);

            tracker.set_step(1, "Fusion des couches régionales");
            VectorService::merge_datasets(&regional, &regional_out)
                .await
                .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

            tracker.set_step(2, "Fusion des couches de végétation");
            VectorService::merge_datasets(&vegetation, &vegetation_out)
                .await
                .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

            tracker.set_step(3, "Fusion des couches RPG");
            VectorService::merge_datasets(&rpg, &rpg_out)
                .await
                .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

            tracker.set_step(4, "Fusion des couches topographiques");
            for (layer_name, paths) in &topo {
                let topo_out = format!("{}/resources/{}.gpkg", project_folder, layer_name);
                VectorService::merge_datasets(paths, &topo_out)
                    .await
                    .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;
            }
        } else {
            tokio::fs::rename(&regional[0], &regional_out).await?;
            tokio::fs::rename(&vegetation[0], &vegetation_out).await?;
            tokio::fs::rename(&rpg[0], &rpg_out).await?;

            for (layer_name, paths) in &topo {
                if !paths.is_empty() {
                    let topo_out = format!("{}/resources/{}.gpkg", project_folder, layer_name);
                    tokio::fs::rename(&paths[0], &topo_out).await?;
                }
            }
        }

        Ok(())
    }

    async fn finalize_project(
        name: &str,
        project_folder: &str,
        project_file: &str,
        project_bb: &BoundingBox,
    ) -> ProjectResult<()> {
        Progress::status("Finalisation");
        let mut tracker = ProgressTracker::new("Finalisation", 2);

        tracker.set_step(1, "Export en JPEG");
        Self::export_to_jpg(
            project_file,
            &format!("{}/{}_VEGET.jpeg", project_folder, name),
        )
        .await?;

        tracker.set_step(2, "Téléchargement d'orthophoto");
        let ortho_path = FetchService::fetch_orthophoto(project_bb)
            .await
            .map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

        fs::copy(
            ortho_path,
            format!("{}/{}_ORTHO.jpeg", project_folder, name),
        )
        .await?;

        Progress::status("Nettoyage");
        clean_tmp(None).map_err(|e| ProjectError::CreationFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn export_to_jpg(input: &str, output: &str) -> ProjectResult<()> {
        execute_sidecar(
            "magick",
            &["convert", input, "-strip", "-quality", "100", output],
        )
        .await
        .map_err(|e| ProjectError::ExportFailed {
            project: input.to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }
}
