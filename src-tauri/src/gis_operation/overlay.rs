use crate::types::Dataset;
use crate::utils::{clean_tmp, executor, temp_dir};
use std::path::{Path, PathBuf};

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

        let output_bands = self
            .combine_bands(
                &project_bands,
                &overlay_bands,
                &mask,
                width,
                height,
                temp_dir,
            )
            .await?;

        let output_file = temp_dir.join("output.tif");
        self.create_final_output(&output_bands, &output_file, &project)
            .await?;

        #[cfg(not(target_os = "windows"))]
        {
            std::fs::rename(&output_file, project_file_path)?;
        }
        #[cfg(target_os = "windows")]
        {
            std::fs::copy(&output_file, project_file_path)?;
            std::fs::remove_file(&output_file)?;
        }

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

            executor("gdal_translate", &args).await?;
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

    async fn combine_bands(
        &self,
        project_bands: &[PathBuf],
        overlay_bands: &[PathBuf],
        mask: &[bool],
        width: usize,
        height: usize,
        temp_dir: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut output_bands = Vec::new();

        for (i, project_band_file) in project_bands.iter().enumerate() {
            let mut project_data = match std::fs::read(project_band_file) {
                Ok(data) => data,
                Err(e) => {
                    return Err(format!("Failed to read project band {}: {}", i + 1, e).into());
                }
            };

            if i < overlay_bands.len() {
                let overlay_data = match std::fs::read(&overlay_bands[i]) {
                    Ok(data) => data,
                    Err(e) => {
                        return Err(format!("Failed to read overlay band {}: {}", i + 1, e).into());
                    }
                };

                for (j, (&mask_value, overlay_value)) in
                    mask.iter().zip(overlay_data.iter()).enumerate()
                {
                    if j < project_data.len() && mask_value {
                        project_data[j] = *overlay_value;
                    }
                }
            }

            let output_band_file = temp_dir.join(format!("output_band_{}.dat", i + 1));
            let output_header_file = temp_dir.join(format!("output_band_{}.hdr", i + 1));

            std::fs::write(&output_band_file, &project_data)?;

            let header_content = format!(
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
                i + 1,
                width,
                height
            );

            std::fs::write(&output_header_file, header_content)?;

            output_bands.push(output_band_file);
        }

        for (i, output_file) in output_bands.iter().enumerate() {
            if !output_file.exists() {
                return Err(format!(
                    "Output band file {} was not created: {}",
                    i + 1,
                    output_file.display()
                )
                .into());
            }

            let header_file = output_file.with_extension("hdr");
            if !header_file.exists() {
                return Err(format!(
                    "Output header file {} was not created: {}",
                    i + 1,
                    header_file.display()
                )
                .into());
            }
        }

        Ok(output_bands)
    }

    async fn create_final_output(
        &self,
        band_files: &[PathBuf],
        output_file: &Path,
        reference_dataset: &Dataset,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let vrt_file = output_file.with_extension("vrt");

        let mut args = vec!["-separate"];
        let vrt_file_str = vrt_file.to_string_lossy().to_string();
        args.push(&vrt_file_str);

        let band_file_strs: Vec<String> = band_files
            .iter()
            .map(|band_file| band_file.to_string_lossy().to_string())
            .collect();

        for band_file_str in &band_file_strs {
            args.push(band_file_str);
        }

        executor("gdalbuildvrt", &args).await?;

        let bbox = reference_dataset.bbox();

        let xmin_str = bbox.xmin.to_string();
        let ymax_str = bbox.ymax.to_string();
        let xmax_str = bbox.xmax.to_string();
        let ymin_str = bbox.ymin.to_string();
        let projection = reference_dataset.projection();

        let mut final_args = vec![
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
            final_args.extend_from_slice(&[
                "-colorinterp",
                "red,green,blue,alpha",
                "-co",
                "PHOTOMETRIC=RGB",
            ]);
        } else if band_files.len() == 3 {
            final_args.extend_from_slice(&[
                "-colorinterp",
                "red,green,blue",
                "-co",
                "PHOTOMETRIC=RGB",
            ]);
        }

        let vrt_file_lossy = vrt_file.to_string_lossy();
        let output_file_lossy = output_file.to_string_lossy();
        final_args.push(&vrt_file_lossy);
        final_args.push(&output_file_lossy);

        executor("gdal_translate", &final_args).await?;

        std::fs::remove_file(&vrt_file)?;

        Ok(())
    }

    pub async fn apply_overlay_with_fixed_color<F>(
        &mut self,
        project_file_path: &str,
        mask_raster_path: &str,
        mask_condition: F,
        fixed_rgb: [u8; 3],
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
        let mask_bands = self
            .extract_bands(mask_raster_path, temp_dir, "mask", 3)
            .await?;

        let mask = self
            .create_mask(&mask_bands, width, height, mask_condition)
            .await?;

        let output_bands = self
            .combine_bands_with_fixed_color(
                &project_bands,
                &mask,
                fixed_rgb,
                width,
                height,
                temp_dir,
            )
            .await?;

        let output_file = temp_dir.join("output.tif");
        self.create_final_output(&output_bands, &output_file, &project)
            .await?;

        #[cfg(not(target_os = "windows"))]
        {
            std::fs::rename(&output_file, project_file_path)?;
        }
        #[cfg(target_os = "windows")]
        {
            std::fs::copy(&output_file, project_file_path)?;
            std::fs::remove_file(&output_file)?;
        }

        self.cleanup();
        Ok(())
    }

    async fn combine_bands_with_fixed_color(
        &self,
        project_bands: &[PathBuf],
        mask: &[bool],
        fixed_rgb: [u8; 3],
        width: usize,
        height: usize,
        temp_dir: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut output_bands = Vec::new();

        for (i, project_band_file) in project_bands.iter().enumerate() {
            let mut project_data = match std::fs::read(project_band_file) {
                Ok(data) => data,
                Err(e) => {
                    return Err(format!("Failed to read project band {}: {}", i + 1, e).into());
                }
            };

            for (j, &mask_value) in mask.iter().enumerate() {
                if j < project_data.len() && mask_value {
                    if i < 3 {
                        project_data[j] = fixed_rgb[i];
                    } else if i == 3 {
                        project_data[j] = 255;
                    }
                }
            }

            let output_band_file = temp_dir.join(format!("output_band_{}.dat", i + 1));
            let output_header_file = temp_dir.join(format!("output_band_{}.hdr", i + 1));

            std::fs::write(&output_band_file, &project_data)?;

            let header_content = format!(
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
                i + 1,
                width,
                height
            );

            std::fs::write(&output_header_file, header_content)?;
            output_bands.push(output_band_file);
        }

        Ok(output_bands)
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
