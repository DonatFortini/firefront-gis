use crate::utils::executor;

pub trait DriverFormat {
    const NAME: &'static str;
    const EXTENSION: &'static str;
}

pub struct GTiff;
pub struct JPEG;
pub struct PNG;
pub struct GPKG;

impl DriverFormat for GTiff {
    const NAME: &'static str = "GTiff";
    const EXTENSION: &'static str = "tif";
}

impl DriverFormat for JPEG {
    const NAME: &'static str = "JPEG";
    const EXTENSION: &'static str = "jpeg";
}

impl DriverFormat for PNG {
    const NAME: &'static str = "PNG";
    const EXTENSION: &'static str = "png";
}

impl DriverFormat for GPKG {
    const NAME: &'static str = "GPKG";
    const EXTENSION: &'static str = "gpkg";
}

#[derive(Debug, Default)]
pub struct Driver<T: DriverFormat> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: DriverFormat> Driver<T> {
    pub fn new() -> Self {
        Driver {
            _marker: std::marker::PhantomData,
        }
    }

    pub async fn create(&self, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
        let mut pref_args = vec!["-of", T::NAME];
        pref_args.extend_from_slice(args);
        executor("gdal_create", &pref_args).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct GeoTransform {
    pub x_origin: f64,
    pub pixel_width: f64,
    pub x_rotation: f64,
    pub y_origin: f64,
    pub y_rotation: f64,
    pub pixel_height: f64,
}

pub struct Dataset {
    pub filename: String,
    pub geo_transform: GeoTransform,
    pub width: usize,
    pub height: usize,
    pub bands: usize,
}

pub mod prelude {
    pub use super::{Dataset, Driver, DriverFormat, GPKG, GTiff, GeoTransform, JPEG, PNG};
}
