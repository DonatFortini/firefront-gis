use crate::utils::executor;

pub trait DriverFormat {
    const NAME: &'static str;
    const EXTENSION: &'static str;
}

pub struct GTiff;
pub struct JPEG;
pub struct PNG;
pub struct GPKG;
pub struct ENVI;
pub struct Shapefile;
pub struct GeoJSON;

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

impl DriverFormat for ENVI {
    const NAME: &'static str = "ENVI";
    const EXTENSION: &'static str = "dat";
}

impl DriverFormat for Shapefile {
    const NAME: &'static str = "ESRI Shapefile";
    const EXTENSION: &'static str = "shp";
}

impl DriverFormat for GeoJSON {
    const NAME: &'static str = "GeoJSON";
    const EXTENSION: &'static str = "geojson";
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

pub mod prelude {
    pub use super::{Driver, DriverFormat, ENVI, GPKG, GTiff, GeoJSON, JPEG, PNG, Shapefile};
}
