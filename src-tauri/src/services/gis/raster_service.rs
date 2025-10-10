use image::{DynamicImage, GenericImageView};
use std::path::Path;

use crate::error::{GisError, GisResult};
use crate::services::ProjectService;
use crate::types::{BoundingBox, Driver, GTiff};
use crate::utils::{
    create_directory_if_not_exists, execute_sidecar, projects_dir, resolution, slice_factor,
};

struct TileCoordinates {
    x: u32,
    y: u32,
}

impl TileCoordinates {
    fn from_meters(x_meters: f64, y_meters: f64) -> Self {
        Self {
            x: (x_meters / 1000.0) as u32,
            y: (y_meters / 1000.0) as u32,
        }
    }

    fn filename(&self, suffix: &str) -> String {
        format!("{}_{}{}", self.x, self.y, suffix)
    }
}

pub struct RasterService;

impl RasterService {
    fn validate_dimensions(width: usize, height: usize) -> GisResult<()> {
        let multiple = slice_factor() as usize;
        if !width.is_multiple_of(multiple) || !height.is_multiple_of(multiple) {
            return Err(GisError::InvalidGeometry(format!(
                "Width and height must be multiples of {}",
                slice_factor()
            )));
        }
        Ok(())
    }

    fn calculate_tile_bounds(
        tile_x: usize,
        tile_y: usize,
        tile_size: f64,
        project_bb: &BoundingBox,
    ) -> BoundingBox {
        let xmin = project_bb.xmin + (tile_x as f64 * tile_size);
        let ymax = project_bb.ymax - (tile_y as f64 * tile_size);

        BoundingBox {
            xmin,
            ymax,
            xmax: (xmin + tile_size).min(project_bb.xmax),
            ymin: (ymax - tile_size).max(project_bb.ymin),
        }
    }

    fn cleanup_aux_xml(base_path: &Path) {
        let extension = base_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let aux_path = base_path.with_extension(format!("{}.aux.xml", extension));
        std::fs::remove_file(aux_path).ok();
    }

    pub async fn create_reference_raster(
        output_path: &str,
        project_bb: &BoundingBox,
    ) -> GisResult<()> {
        let resolution = resolution();
        let width = (project_bb.width() / resolution).ceil() as usize;
        let height = (project_bb.height() / resolution).ceil() as usize;

        Self::validate_dimensions(width, height)?;

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
                let tile_bb =
                    Self::calculate_tile_bounds(tile_x, tile_y, tile_size_meters, project_bb);

                let coords = TileCoordinates::from_meters(tile_bb.xmin, tile_bb.ymin);

                let output_grd = slice_dir.join(coords.filename("_alti.grd"));
                let output_bmp = slice_dir.join(coords.filename("_altiImage.bmp"));

                execute_sidecar(
                    "gdal_translate",
                    &[
                        "-of",
                        "AAIGrid",
                        "-projwin",
                        &tile_bb.xmin.to_string(),
                        &tile_bb.ymax.to_string(),
                        &tile_bb.xmax.to_string(),
                        &tile_bb.ymin.to_string(),
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

                Self::cleanup_aux_xml(&output_bmp);
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

        let base_coords = TileCoordinates::from_meters(project_bb.xmin, project_bb.ymin);

        Self::process_slices(
            &veget_image,
            &ortho_image,
            &slice_dir,
            slice_factor,
            base_coords.x,
            base_coords.y,
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
                    slice_dir.join(format!("{}_{}_veget_{}.jpg", coord_x, coord_y, factor));
                let ortho_file = slice_dir.join(format!("{}_{}_{}.jpg", coord_x, coord_y, factor));

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
