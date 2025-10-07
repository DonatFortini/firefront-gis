use image::{DynamicImage, GenericImageView};
use std::path::Path;

use crate::error::{GisError, GisResult};
use crate::services::ProjectService;
use crate::types::{BoundingBox, Driver, GTiff};
use crate::utils::{create_directory_if_not_exists, execute_sidecar, projects_dir, resolution};

pub struct RasterService;

impl RasterService {
    pub async fn create_reference_raster(
        output_path: &str,
        project_bb: &BoundingBox,
    ) -> GisResult<()> {
        let resolution = resolution();
        let width = (project_bb.width() / resolution).ceil() as usize;
        let height = (project_bb.height() / resolution).ceil() as usize;

        if !width.is_multiple_of(500) || !height.is_multiple_of(500) {
            return Err(GisError::InvalidGeometry(
                "Width and height must be multiples of 500".to_string(),
            ));
        }

        let args = [
            "-ot",
            "Byte",
            "-outsize",
            &width.to_string(),
            &height.to_string(),
            "-bands",
            "4",
            "-burn",
            "0",
            "-burn",
            "0",
            "-burn",
            "0",
            "-burn",
            "255",
            "-a_srs",
            "EPSG:2154",
            "-a_ullr",
            &project_bb.xmin.to_string(),
            &project_bb.ymax.to_string(),
            &project_bb.xmax.to_string(),
            &project_bb.ymin.to_string(),
            "-co",
            "TILED=YES",
            "-co",
            "COMPRESS=LZW",
            "-co",
            "BIGTIFF=IF_SAFER",
            output_path,
        ];

        Driver::<GTiff>::new()
            .create(&args)
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "create_reference_raster".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    async fn slice_elevation(
        elevation_path: &Path,
        slice_dir: &Path,
        project_bb: &BoundingBox,
        project_folder: &str,
        factor: u32,
    ) -> GisResult<()> {
        let resolution = resolution();
        let tile_size_meters = (factor as f64) * resolution;

        let width = project_bb.width();
        let height = project_bb.height();

        let tiles_x = (width / tile_size_meters).ceil() as usize;
        let tiles_y = (height / tile_size_meters).ceil() as usize;

        let color_ramp_path = format!("{}/resources/color_ramp.txt", project_folder);

        for tile_y in 0..tiles_y {
            for tile_x in 0..tiles_x {
                let xmin = project_bb.xmin + (tile_x as f64 * tile_size_meters);
                let ymax = project_bb.ymax - (tile_y as f64 * tile_size_meters);
                let xmax = (xmin + tile_size_meters).min(project_bb.xmax);
                let ymin = (ymax - tile_size_meters).max(project_bb.ymin);

                let coord_x = (xmin / 1000.0) as u32;
                let coord_y = (ymin / 1000.0) as u32;

                let output_grd = slice_dir.join(format!("{}_{}_alti.grd", coord_x, coord_y));
                let output_bmp = slice_dir.join(format!("{}_{}_{}.bmp", coord_x, coord_y, factor));

                execute_sidecar(
                    "gdal_translate",
                    &[
                        "-of",
                        "AAIGrid",
                        "-projwin",
                        &xmin.to_string(),
                        &ymax.to_string(),
                        &xmax.to_string(),
                        &ymin.to_string(),
                        "-co",
                        "FORCE_CELLSIZE=YES",
                        "-co",
                        "DECIMAL_PRECISION=2",
                        "--config",
                        "GDAL_PAM_ENABLED",
                        "NO",
                        elevation_path.to_str().unwrap(),
                        output_grd.to_str().unwrap(),
                    ],
                )
                .await
                .map_err(|e| GisError::SliceFailed(format!("Failed to slice elevation: {}", e)))?;

                Self::fix_asc_header(&output_grd)?;

                execute_sidecar(
                    "gdaldem",
                    &[
                        "color-relief",
                        output_grd.to_str().unwrap(),
                        &color_ramp_path,
                        output_bmp.to_str().unwrap(),
                        "-of",
                        "BMP",
                        "--config",
                        "GDAL_PAM_ENABLED",
                        "NO",
                    ],
                )
                .await
                .map_err(|e| GisError::SliceFailed(format!("Failed to create BMP: {}", e)))?;

                let aux_xml = output_bmp.with_extension("bmp.aux.xml");
                if aux_xml.exists() {
                    std::fs::remove_file(aux_xml).ok();
                }
            }
        }

        Ok(())
    }

    pub async fn slice_project(project_name: &str, slice_factor: u32) -> GisResult<String> {
        let project_dir = projects_dir().join(project_name);
        let slice_dir = project_dir.join("slices");

        create_directory_if_not_exists(&slice_dir.to_string_lossy())
            .map_err(|e| GisError::SliceFailed(e.to_string()))?;

        let veget_path = project_dir.join(format!("{}_VEGET.jpeg", project_name));
        let ortho_path = project_dir.join(format!("{}_ORTHO.jpeg", project_name));
        let elevation_path = project_dir.join("resources/elevation.tif");

        let veget_image = Self::load_image(&veget_path)?;
        let ortho_image = Self::load_image(&ortho_path)?;

        let project_bb = ProjectService::get_project_bounding_box(project_name).await?;
        let (base_x, base_y) = (
            (project_bb.xmin / 1000.0) as u32,
            (project_bb.ymin / 1000.0) as u32,
        );

        Self::process_slices(
            &veget_image,
            &ortho_image,
            &slice_dir,
            slice_factor,
            base_x,
            base_y,
        )?;

        if elevation_path.exists() {
            Self::slice_elevation(
                &elevation_path,
                &slice_dir,
                &project_bb,
                &project_dir.to_string_lossy(),
                slice_factor,
            )
            .await?;
        }

        Ok("Slicing completed".to_string())
    }

    fn process_slices(
        veget: &DynamicImage,
        ortho: &DynamicImage,
        slice_dir: &Path,
        factor: u32,
        base_x: u32,
        base_y: u32,
    ) -> GisResult<()> {
        let (width, height) = veget.dimensions();

        for img_y in (0..height).step_by(factor as usize).rev() {
            for img_x in (0..width).step_by(factor as usize) {
                if img_x + factor > width || img_y + factor > height {
                    continue;
                }

                let cropped_veget = veget.crop_imm(img_x, img_y, factor, factor);
                let cropped_ortho = ortho.crop_imm(img_x, img_y, factor, factor);

                let coord_x = base_x + img_x / 100;
                let coord_y = base_y + (height - img_y - factor) / 100;

                let veget_file =
                    slice_dir.join(format!("{}_{}_{}_veget.jpg", coord_x, coord_y, factor));
                let ortho_file =
                    slice_dir.join(format!("{}_{}_{}_ortho.jpg", coord_x, coord_y, factor));

                cropped_ortho
                    .save(&ortho_file)
                    .map_err(|e| GisError::SliceFailed(e.to_string()))?;
                cropped_veget
                    .save(&veget_file)
                    .map_err(|e| GisError::SliceFailed(e.to_string()))?;
            }
        }

        Ok(())
    }

    fn fix_asc_header(file_path: &Path) -> GisResult<()> {
        let content = std::fs::read_to_string(file_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let mut fixed_lines = Vec::new();

        for line in lines {
            let trimmed = line.trim();

            if trimmed.starts_with("ncols")
                || trimmed.starts_with("nrows")
                || trimmed.starts_with("xllcorner")
                || trimmed.starts_with("yllcorner")
                || trimmed.starts_with("cellsize")
                || trimmed.starts_with("NODATA_value")
            {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() == 2
                    && let Ok(value) = parts[1].parse::<f64>()
                {
                    fixed_lines.push(format!("{} {}", parts[0], value as i64));
                    continue;
                }
            }

            fixed_lines.push(line.to_string());
        }

        std::fs::write(file_path, fixed_lines.join("\n"))?;
        Ok(())
    }
    fn load_image(path: &Path) -> GisResult<DynamicImage> {
        image::ImageReader::open(path)
            .map_err(|e| GisError::ImageProcessing(format!("Failed to open: {}", e)))?
            .decode()
            .map_err(|e| GisError::ImageProcessing(format!("Failed to decode: {}", e)))
    }
}
