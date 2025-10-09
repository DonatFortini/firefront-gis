use crate::error::{GisError, GisResult};
use crate::services::ArchiveService;
use crate::types::BoundingBox;
use crate::utils::{cache_dir, execute_sidecar, temp_dir};
use std::path::{Path, PathBuf};

pub struct ElevationService;

impl ElevationService {
    pub async fn process_elevation_tiles(
        project_bb: &BoundingBox,
        code: &str,
        output_path: &str,
        project_folder: &str,
    ) -> GisResult<()> {
        let archive_path = cache_dir().join(format!("RGEALTI_{}.7z", code));
        if !archive_path.exists() {
            return Err(GisError::Dataset(format!(
                "RGEALTI archive not found: {}",
                archive_path.display()
            )));
        }

        let extract_dir = temp_dir().join(format!("rgealti_{}", code));

        if extract_dir.exists() {
            std::fs::remove_dir_all(&extract_dir).ok();
        }
        std::fs::create_dir_all(&extract_dir)?;

        println!("Extracting RGEALTI archive for code {}...", code);
        ArchiveService::extract_all(
            archive_path.to_str().unwrap(),
            extract_dir.to_str().unwrap(),
        )
        .await
        .map_err(|e| GisError::Dataset(format!("Failed to extract RGEALTI: {}", e)))?;

        let all_tiles = Self::find_all_asc_files(&extract_dir)?;

        if all_tiles.is_empty() {
            std::fs::remove_dir_all(&extract_dir).ok();
            return Err(GisError::Dataset(
                "No .asc elevation files found in archive".to_string(),
            ));
        }

        println!("Found {} total elevation tiles", all_tiles.len());

        let intersecting_tiles = Self::filter_intersecting_tiles(&all_tiles, project_bb)?;

        if intersecting_tiles.is_empty() {
            std::fs::remove_dir_all(&extract_dir).ok();
            return Err(GisError::Dataset(
                "No elevation tiles intersect project area".to_string(),
            ));
        }

        println!(
            "Found {} intersecting tiles for processing",
            intersecting_tiles.len()
        );
        let vrt_path = temp_dir().join(format!("elevation_{}.vrt", code));
        Self::create_vrt(&intersecting_tiles, &vrt_path).await?;
        Self::warp_to_project(&vrt_path, output_path, project_bb).await?;

        let (min_elev, max_elev) = Self::get_elevation_range(output_path).await?;
        Self::create_color_ramp(project_folder, min_elev, max_elev)?;

        std::fs::remove_file(&vrt_path).ok();
        std::fs::remove_dir_all(&extract_dir).ok();

        println!("Elevation processing complete for code {}", code);

        Ok(())
    }

    pub async fn get_elevation_range(raster_path: &str) -> GisResult<(f64, f64)> {
        let (output, _) = execute_sidecar("gdalinfo", &["-stats", "-json", raster_path])
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "gdalinfo".to_string(),
                message: e.to_string(),
            })?;

        let info: serde_json::Value = serde_json::from_str(&output).map_err(GisError::JsonParse)?;

        let bands = info["bands"]
            .as_array()
            .ok_or_else(|| GisError::Dataset("No bands found".to_string()))?;

        if let Some(band) = bands.first() {
            let min = band["minimum"]
                .as_f64()
                .ok_or_else(|| GisError::Dataset("No minimum value found".to_string()))?;
            let max = band["maximum"]
                .as_f64()
                .ok_or_else(|| GisError::Dataset("No maximum value found".to_string()))?;

            println!("Elevation range: {:.2}m to {:.2}m", min, max);
            Ok((min, max))
        } else {
            Err(GisError::Dataset("No band information found".to_string()))
        }
    }

    pub fn create_color_ramp(project_folder: &str, min_elev: f64, max_elev: f64) -> GisResult<()> {
        let resources_dir = format!("{}/resources", project_folder);
        std::fs::create_dir_all(&resources_dir)?;

        let color_ramp_path = format!("{}/color_ramp.txt", resources_dir);

        let range = max_elev - min_elev;
        let step1 = min_elev + (range * 0.25);
        let step2 = min_elev + (range * 0.50);
        let step3 = min_elev + (range * 0.75);

        let color_ramp = format!(
            "nv 255 255 255\n{:.2} 220 230 255\n{:.2} 150 180 230\n{:.2} 80 120 200\n{:.2} 40 70 150\n{:.2} 0 20 100\n",
            min_elev, step1, step2, step3, max_elev
        );

        std::fs::write(&color_ramp_path, color_ramp)?;
        println!("Color ramp created at: {}", color_ramp_path);

        Ok(())
    }

    fn find_all_asc_files(dir: &Path) -> GisResult<Vec<PathBuf>> {
        let mut asc_files = Vec::new();
        Self::find_asc_recursive(dir, &mut asc_files)?;
        Ok(asc_files)
    }

    fn find_asc_recursive(dir: &Path, result: &mut Vec<PathBuf>) -> GisResult<()> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();

            if path.is_dir() {
                Self::find_asc_recursive(&path, result)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("asc") {
                result.push(path);
            }
        }

        Ok(())
    }

    fn filter_intersecting_tiles(tiles: &[PathBuf], bbox: &BoundingBox) -> GisResult<Vec<String>> {
        let mut intersecting = Vec::new();
        let tile_size = 5000.0;

        for tile_path in tiles {
            let filename = tile_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            let parts: Vec<&str> = filename.split('_').collect();

            if parts.len() < 4 {
                continue;
            }

            let mut x_km: Option<f64> = None;
            let mut y_km: Option<f64> = None;

            for i in 0..parts.len() - 1 {
                if let Ok(x) = parts[i].parse::<f64>()
                    && let Ok(y) = parts[i + 1].parse::<f64>()
                    && (1000.0..=1400.0).contains(&x)
                    && (6000.0..=7200.0).contains(&y)
                {
                    x_km = Some(x);
                    y_km = Some(y);
                    break;
                }
            }

            if let (Some(x), Some(y)) = (x_km, y_km) {
                let tile_xmin = x * 1000.0;
                let tile_ymin = y * 1000.0;
                let tile_xmax = tile_xmin + tile_size;
                let tile_ymax = tile_ymin + tile_size;

                if tile_xmax >= bbox.xmin
                    && tile_xmin <= bbox.xmax
                    && tile_ymax >= bbox.ymin
                    && tile_ymin <= bbox.ymax
                {
                    intersecting.push(tile_path.to_string_lossy().to_string());
                }
            }
        }

        Ok(intersecting)
    }

    async fn create_vrt(input_files: &[String], output_vrt: &Path) -> GisResult<()> {
        let mut args = vec!["-overwrite"];
        args.push(output_vrt.to_str().unwrap());
        args.extend(input_files.iter().map(|s| s.as_str()));

        execute_sidecar("gdalbuildvrt", &args)
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "gdalbuildvrt".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    async fn warp_to_project(
        vrt_path: &Path,
        output_path: &str,
        bbox: &BoundingBox,
    ) -> GisResult<()> {
        execute_sidecar(
            "gdalwarp",
            &[
                "-tr",
                "10",
                "10",
                "-te",
                &bbox.xmin.to_string(),
                &bbox.ymin.to_string(),
                &bbox.xmax.to_string(),
                &bbox.ymax.to_string(),
                "-r",
                "average",
                "-dstnodata",
                "-9999",
                "-co",
                "COMPRESS=LZW",
                "-co",
                "TILED=YES",
                "-of",
                "GTiff",
                vrt_path.to_str().unwrap(),
                output_path,
            ],
        )
        .await
        .map_err(|e| GisError::GdalOperation {
            operation: "gdalwarp".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }
}
