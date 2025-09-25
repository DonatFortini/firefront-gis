use crate::utils::{clean_tmp, executor};
use std::env::temp_dir;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MergeOptions {
    pub input_files: Vec<String>,
    pub output_file: String,
    pub output_format: String,
    pub creation_options: Vec<String>,
    pub pixel_size: Option<(f64, f64)>,
    pub output_bounds: Option<(f64, f64, f64, f64)>,
    pub target_srs: Option<String>,
    pub resampling_method: String,
    pub nodata_value: Option<f64>,
    pub separate_bands: bool,
    pub use_warp_method: bool,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            input_files: Vec::new(),
            output_file: String::new(),
            output_format: "GTiff".to_string(),
            creation_options: Vec::new(),
            pixel_size: None,
            output_bounds: None,
            target_srs: None,
            resampling_method: "near".to_string(),
            nodata_value: None,
            separate_bands: false,
            use_warp_method: false,
        }
    }
}

pub struct Merger {
    options: MergeOptions,
}

impl Merger {
    pub fn new(options: MergeOptions) -> Self {
        Self { options }
    }

    pub fn with_files(input_files: Vec<String>, output_file: String) -> Self {
        let options = MergeOptions {
            input_files,
            output_file,
            ..MergeOptions::default()
        };
        Self::new(options)
    }

    pub fn format(mut self, format: &str) -> Self {
        self.options.output_format = format.to_string();
        self
    }

    pub fn creation_option(mut self, option: &str) -> Self {
        self.options.creation_options.push(option.to_string());
        self
    }

    pub fn pixel_size(mut self, x_res: f64, y_res: f64) -> Self {
        self.options.pixel_size = Some((x_res, y_res));
        self
    }

    pub fn bounds(mut self, xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Self {
        self.options.output_bounds = Some((xmin, ymin, xmax, ymax));
        self
    }

    pub fn target_srs(mut self, srs: &str) -> Self {
        self.options.target_srs = Some(srs.to_string());
        self
    }

    pub fn resampling(mut self, method: &str) -> Self {
        self.options.resampling_method = method.to_string();
        self
    }

    pub fn nodata(mut self, value: f64) -> Self {
        self.options.nodata_value = Some(value);
        self
    }

    pub fn separate_bands(mut self) -> Self {
        self.options.separate_bands = true;
        self
    }

    pub fn use_warp_method(mut self) -> Self {
        self.options.use_warp_method = true;
        self
    }

    pub async fn merge(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.options.input_files.is_empty() {
            return Err("No input files specified".into());
        }

        if self.options.output_file.is_empty() {
            return Err("No output file specified".into());
        }

        if self.options.use_warp_method {
            self.merge_with_warp().await
        } else {
            self.merge_with_vrt().await
        }
    }

    async fn merge_with_vrt(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let vrt_path = self.create_vrt().await?;
        self.vrt_to_raster(&vrt_path).await?;
        self.cleanup();
        Ok(())
    }

    async fn merge_with_warp(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut args = vec!["-of", &self.options.output_format];

        for co in &self.options.creation_options {
            args.extend_from_slice(&["-co", co]);
        }

        if let Some(ref srs) = self.options.target_srs {
            args.extend_from_slice(&["-t_srs", srs]);
        }

        let xres_str;
        let yres_str;
        if let Some((xres, yres)) = self.options.pixel_size {
            xres_str = xres.to_string();
            yres_str = yres.to_string();
            args.extend_from_slice(&["-tr", &xres_str, &yres_str]);
        }

        let xmin_str;
        let ymin_str;
        let xmax_str;
        let ymax_str;
        if let Some((xmin, ymin, xmax, ymax)) = self.options.output_bounds {
            xmin_str = xmin.to_string();
            ymin_str = ymin.to_string();
            xmax_str = xmax.to_string();
            ymax_str = ymax.to_string();
            args.extend_from_slice(&["-te", &xmin_str, &ymin_str, &xmax_str, &ymax_str]);
        }

        args.extend_from_slice(&["-r", &self.options.resampling_method]);

        let nodata_str;
        if let Some(nodata) = self.options.nodata_value {
            nodata_str = nodata.to_string();
            args.extend_from_slice(&["-dstnodata", &nodata_str]);
        }

        for input_file in &self.options.input_files {
            args.push(input_file);
        }

        args.push(&self.options.output_file);

        executor("gdalwarp", &args).await?;

        Ok(())
    }

    async fn create_vrt(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let vrt_path = temp_dir().join("merged.vrt");

        let mut args = vec![];

        if self.options.separate_bands {
            args.push("-separate");
        }

        if let Some(ref srs) = self.options.target_srs {
            args.extend_from_slice(&["-a_srs", srs]);
        }

        let xres_str;
        let yres_str;
        if let Some((xres, yres)) = self.options.pixel_size {
            args.push("-resolution");
            args.push("user");
            xres_str = xres.to_string();
            yres_str = yres.to_string();
            args.extend_from_slice(&["-tr", &xres_str, &yres_str]);
        }

        let xmin_str;
        let ymin_str;
        let xmax_str;
        let ymax_str;
        if let Some((xmin, ymin, xmax, ymax)) = self.options.output_bounds {
            xmin_str = xmin.to_string();
            ymin_str = ymin.to_string();
            xmax_str = xmax.to_string();
            ymax_str = ymax.to_string();
            args.extend_from_slice(&["-te", &xmin_str, &ymin_str, &xmax_str, &ymax_str]);
        }

        args.extend_from_slice(&["-r", &self.options.resampling_method]);

        let nodata_str;
        if let Some(nodata) = self.options.nodata_value {
            nodata_str = nodata.to_string();
            args.extend_from_slice(&["-vrtnodata", &nodata_str]);
        }

        let vrt_path_str = vrt_path.to_string_lossy().to_string();
        args.push(&vrt_path_str);

        for input_file in &self.options.input_files {
            args.push(input_file);
        }

        executor("gdalbuildvrt", &args).await?;

        Ok(vrt_path)
    }

    async fn vrt_to_raster(&self, vrt_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut args = vec!["-of", &self.options.output_format];

        for co in &self.options.creation_options {
            args.extend_from_slice(&["-co", co]);
        }

        let vrt_path_str = vrt_path.to_string_lossy().to_string();
        args.push(&vrt_path_str);
        args.push(&self.options.output_file);

        executor("gdal_translate", &args).await?;

        Ok(())
    }

    fn cleanup(&self) {
        clean_tmp(Some(".tif")).unwrap();
    }
}

impl Drop for Merger {
    fn drop(&mut self) {
        self.cleanup();
    }
}
