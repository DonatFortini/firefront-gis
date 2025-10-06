use image::{DynamicImage, GenericImageView};
use std::path::Path;

use crate::error::{GisError, GisResult};
use crate::services::ProjectService;
use crate::types::{BoundingBox, Driver, GTiff};
use crate::utils::{create_directory_if_not_exists, projects_dir, resolution};

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

    pub async fn slice_project(project_name: &str, slice_factor: u32) -> GisResult<String> {
        let project_dir = projects_dir().join(project_name);
        let slice_dir = project_dir.join("slices");

        create_directory_if_not_exists(&slice_dir.to_string_lossy())
            .map_err(|e| GisError::SliceFailed(e.to_string()))?;

        let veget_path = project_dir.join(format!("{}_VEGET.jpeg", project_name));
        let ortho_path = project_dir.join(format!("{}_ORTHO.jpeg", project_name));

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

        Ok("Slicing completed".to_string())
    }

    fn load_image(path: &Path) -> GisResult<DynamicImage> {
        image::ImageReader::open(path)
            .map_err(|e| GisError::ImageProcessing(format!("Failed to open: {}", e)))?
            .decode()
            .map_err(|e| GisError::ImageProcessing(format!("Failed to decode: {}", e)))
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
                let ortho_file = slice_dir.join(format!(
                    "{}_{}_{}_{}.jpg",
                    coord_x, coord_y, factor, "ortho"
                ));

                cropped_veget
                    .save(&veget_file)
                    .map_err(|e| GisError::SliceFailed(e.to_string()))?;
                cropped_ortho
                    .save(&ortho_file)
                    .map_err(|e| GisError::SliceFailed(e.to_string()))?;
            }
        }

        Ok(())
    }
}
