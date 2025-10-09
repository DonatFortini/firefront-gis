use std::path::{Path, PathBuf};

use crate::error::{GisError, GisResult};
use crate::types::Dataset;
use crate::utils::{clean_tmp, execute_sidecar, temp_dir};

pub struct LayerService;

impl LayerService {
    pub async fn rasterize_layer(
        project_path: &str,
        vector_gpkg: &str,
        layer_name: &str,
        output_raster: &str,
        burn_values: [&str; 3],
        where_clause: Option<&str>,
    ) -> GisResult<()> {
        let project = Dataset::open(project_path)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        let bbox = project.bbox();
        let (width, height) = project
            .raster_size()
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        let width_str = width.to_string();
        let height_str = height.to_string();
        let xmin_str = bbox.xmin.to_string();
        let ymin_str = bbox.ymin.to_string();
        let xmax_str = bbox.xmax.to_string();
        let ymax_str = bbox.ymax.to_string();

        let mut args = vec![
            "-burn",
            burn_values[0],
            "-burn",
            burn_values[1],
            "-burn",
            burn_values[2],
            "-l",
            layer_name,
            "-ts",
            &width_str,
            &height_str,
            "-te",
            &xmin_str,
            &ymin_str,
            &xmax_str,
            &ymax_str,
        ];

        if let Some(clause) = where_clause {
            args.extend_from_slice(&["-where", clause]);
        }

        args.extend_from_slice(&[vector_gpkg, output_raster]);

        execute_sidecar("gdal_rasterize", &args)
            .await
            .map_err(|e| GisError::RasterizationFailed(e.to_string()))?;

        Ok(())
    }
}

struct OverlayContext<'a> {
    width: usize,
    height: usize,
    temp_dir: &'a Path,
    project_bands: Vec<PathBuf>,
    overlay_bands: Vec<PathBuf>,
    mask: Vec<bool>,
    fixed_color: Option<[u8; 3]>,
}

#[derive(Default)]
pub struct Overlay {}

impl Overlay {
    pub fn new() -> Self {
        Overlay {}
    }

    pub async fn apply_overlay<F>(
        &mut self,
        project_file_path: &str,
        overlay_raster_path: &str,
        mask_condition: F,
        fixed_color: Option<[u8; 3]>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(&u8) -> bool,
    {
        let temp_dir_buf = temp_dir();
        let temp_dir = temp_dir_buf.as_path();

        let project = Dataset::open(project_file_path).await?;
        let (width, height) = project.raster_size()?;

        let project_bands = self
            .extract_bands(project_file_path, temp_dir, "project", 4)
            .await?;
        let overlay_bands = self
            .extract_bands(overlay_raster_path, temp_dir, "overlay", 3)
            .await?;
        let mask = self
            .create_mask(&overlay_bands, width, height, mask_condition)
            .await?;

        let context = OverlayContext {
            width,
            height,
            temp_dir,
            project_bands,
            overlay_bands,
            mask,
            fixed_color,
        };

        let output_bands = self.combine_bands(&context)?;

        let output_file = temp_dir.join("output.tif");
        self.create_output(&output_bands, &output_file, &project)
            .await?;
        self.finalize_output(&output_file, project_file_path)?;
        self.cleanup();

        Ok(())
    }

    async fn extract_bands(
        &self,
        input_file: &str,
        temp_dir: &Path,
        prefix: &str,
        num_bands: usize,
    ) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut band_files = Vec::new();

        for band_num in 1..=num_bands {
            let band_file = temp_dir.join(format!("{}_band_{}.dat", prefix, band_num));
            let band_num_str = band_num.to_string();
            let band_file_str = band_file.to_string_lossy().to_string();
            let args = vec![
                "-of",
                "ENVI",
                "-ot",
                "Byte",
                "-b",
                &band_num_str,
                input_file,
                &band_file_str,
            ];

            execute_sidecar("gdal_translate", &args).await?;
            band_files.push(band_file);
        }

        Ok(band_files)
    }

    async fn create_mask<F>(
        &self,
        overlay_bands: &[PathBuf],
        width: usize,
        height: usize,
        mask_condition: F,
    ) -> Result<Vec<bool>, Box<dyn std::error::Error>>
    where
        F: Fn(&u8) -> bool,
    {
        let size = width * height;
        let mut mask = vec![false; size];

        for band_file in overlay_bands {
            let band_data = std::fs::read(band_file)?;
            for (i, &value) in band_data.iter().enumerate() {
                if i < size && mask_condition(&value) {
                    mask[i] = true;
                }
            }
        }

        Ok(mask)
    }

    fn combine_bands(
        &self,
        context: &OverlayContext,
    ) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut output_bands = Vec::new();

        for (i, project_band_file) in context.project_bands.iter().enumerate() {
            let mut project_data = std::fs::read(project_band_file)
                .map_err(|e| format!("Failed to read project band {}: {}", i + 1, e))?;

            match context.fixed_color {
                None => {
                    if i < context.overlay_bands.len() {
                        let overlay_data = std::fs::read(&context.overlay_bands[i])
                            .map_err(|e| format!("Failed to read overlay band {}: {}", i + 1, e))?;

                        for (j, (&mask_value, &overlay_value)) in
                            context.mask.iter().zip(overlay_data.iter()).enumerate()
                        {
                            if j < project_data.len() && mask_value {
                                project_data[j] = overlay_value;
                            }
                        }
                    }
                }
                Some(rgb) => {
                    for (j, &mask_value) in context.mask.iter().enumerate() {
                        if j < project_data.len() && mask_value {
                            project_data[j] = match i {
                                0..=2 => rgb[i],
                                3 => 255,
                                _ => project_data[j],
                            };
                        }
                    }
                }
            }

            let output_band_file = self.write_band(&project_data, i + 1, context)?;
            output_bands.push(output_band_file);
        }

        Ok(output_bands)
    }

    fn write_band(
        &self,
        data: &[u8],
        band_num: usize,
        context: &OverlayContext,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let band_file = context
            .temp_dir
            .join(format!("output_band_{}.dat", band_num));
        let header_file = context
            .temp_dir
            .join(format!("output_band_{}.hdr", band_num));

        std::fs::write(&band_file, data)?;

        let header = format!(
            "ENVI\n\
            description = {{ Combined band {} }}\n\
            samples = {}\n\
            lines = {}\n\
            bands = 1\n\
            header offset = 0\n\
            file type = ENVI Standard\n\
            data type = 1\n\
            interleave = bsq\n\
            byte order = 0\n",
            band_num, context.width, context.height
        );

        std::fs::write(&header_file, header)?;
        Ok(band_file)
    }

    async fn create_output(
        &self,
        band_files: &[PathBuf],
        output_file: &Path,
        reference: &Dataset,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let vrt_file = output_file.with_extension("vrt");
        let vrt_file_lossy = vrt_file.to_string_lossy().to_string();
        let mut vrt_args = vec!["-separate", vrt_file_lossy.as_str()];
        let band_strs: Vec<String> = band_files
            .iter()
            .map(|f| f.to_string_lossy().to_string())
            .collect();
        vrt_args.extend(band_strs.iter().map(|s| s.as_str()));

        execute_sidecar("gdalbuildvrt", &vrt_args).await?;

        let bbox = reference.bbox();
        let xmin_str = bbox.xmin.to_string();
        let ymax_str = bbox.ymax.to_string();
        let xmax_str = bbox.xmax.to_string();
        let ymin_str = bbox.ymin.to_string();

        let projection = reference.projection();
        let mut args = vec![
            "-of",
            "GTiff",
            "-co",
            "TILED=YES",
            "-co",
            "BIGTIFF=IF_SAFER",
            "-a_srs",
            &projection,
            "-a_ullr",
            &xmin_str,
            &ymax_str,
            &xmax_str,
            &ymin_str,
        ];

        if band_files.len() >= 4 {
            args.extend(&[
                "-colorinterp",
                "red,green,blue,alpha",
                "-co",
                "PHOTOMETRIC=RGB",
            ]);
        } else if band_files.len() == 3 {
            args.extend(&["-colorinterp", "red,green,blue", "-co", "PHOTOMETRIC=RGB"]);
        }

        let vrt_file_str = vrt_file.to_string_lossy().to_string();
        let output_file_str = output_file.to_string_lossy().to_string();
        args.extend(&[vrt_file_str.as_str(), output_file_str.as_str()]);
        execute_sidecar("gdal_translate", &args).await?;

        std::fs::remove_file(&vrt_file)?;
        Ok(())
    }

    fn finalize_output(
        &self,
        output_file: &Path,
        target: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(not(target_os = "windows"))]
        std::fs::rename(output_file, target)?;

        #[cfg(target_os = "windows")]
        {
            std::fs::copy(output_file, target)?;
            std::fs::remove_file(output_file)?;
        }

        Ok(())
    }

    fn cleanup(&self) {
        clean_tmp(Some(".tif")).unwrap();
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.cleanup();
    }
}
