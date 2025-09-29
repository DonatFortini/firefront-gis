use crate::{types::BoundingBox, utils::executor};
use serde_json::Value;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct RasterBand {
    pub index: usize,
    pub data_type: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub common_name: Option<String>,
}

impl RasterBand {
    pub fn new(index: usize, data_type: String) -> Self {
        Self {
            index,
            data_type,
            name: None,
            description: None,
            common_name: None,
        }
    }

    pub fn with_metadata(
        mut self,
        name: Option<String>,
        description: Option<String>,
        common_name: Option<String>,
    ) -> Self {
        self.name = name;
        self.description = description;
        self.common_name = common_name;
        self
    }
}

pub trait DriverFormat {
    const NAME: &'static str;
    const EXTENSION: &'static str;
}

macro_rules! define_driver {
    ($name:ident, $driver:expr, $ext:expr) => {
        pub struct $name;
        impl DriverFormat for $name {
            const NAME: &'static str = $driver;
            const EXTENSION: &'static str = $ext;
        }
    };
}

define_driver!(GTiff, "GTiff", "tif");
define_driver!(JPEG, "JPEG", "jpeg");
define_driver!(PNG, "PNG", "png");
define_driver!(GPKG, "GPKG", "gpkg");
define_driver!(ENVI, "ENVI", "dat");
define_driver!(Shapefile, "ESRI Shapefile", "shp");
define_driver!(GeoJSON, "GeoJSON", "geojson");

#[derive(Debug, Default)]
/// Représente un pilote GDAL pour la création de nouveaux ensembles de données.
/// # Type Paramètre
/// - `T`: Le format du pilote, qui doit implémenter le trait `DriverFormat`.
/// # Méthodes
/// - `new()`: Crée une nouvelle instance du pilote.
/// - `create(args: &[&str])`: Crée un nouvel ensemble de données en utilisant le pilote GDAL spécifié.
pub struct Driver<T: DriverFormat> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: DriverFormat> Driver<T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }

    /// Crée un nouvel ensemble de données en utilisant le pilote GDAL spécifié.
    /// # Arguments
    /// - `args`: Un tableau de chaînes représentant les arguments supplémentaires pour la création
    ///   de l'ensemble de données (par exemple, le chemin du fichier, la taille, etc.).
    /// # Retourne
    /// - `Result<(), Box<dyn Error>>`: Un résultat indiquant si la création a réussi ou échoué.
    /// # Exemple
    /// ```rust
    /// use your_crate::types::dataset::{Driver, GTiff};
    /// let driver = Driver::<GTiff>::new();
    /// driver.create(&["output.tif", "512", "512"]).await.unwrap();
    /// ```
    pub async fn create(&self, args: &[&str]) -> Result<(), Box<dyn Error>> {
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

impl GeoTransform {
    pub fn new(
        x_origin: f64,
        pixel_width: f64,
        x_rotation: f64,
        y_origin: f64,
        y_rotation: f64,
        pixel_height: f64,
    ) -> Self {
        Self {
            x_origin,
            pixel_width,
            x_rotation,
            y_origin,
            y_rotation,
            pixel_height,
        }
    }

    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.x_origin,
            self.pixel_width,
            self.x_rotation,
            self.y_origin,
            self.y_rotation,
            self.pixel_height,
        ]
    }

    pub fn from_vec(vec: Vec<f64>) -> Option<Self> {
        if vec.len() != 6 {
            return None;
        }
        Some(Self {
            x_origin: vec[0],
            pixel_width: vec[1],
            x_rotation: vec[2],
            y_origin: vec[3],
            y_rotation: vec[4],
            pixel_height: vec[5],
        })
    }
}

#[derive(Debug, Clone)]
pub struct RasterInfo {
    pub width: usize,
    pub height: usize,
    pub geo_transform: GeoTransform,
    pub projection: String,
    pub bands: Vec<RasterBand>,
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub name: String,
    pub geometry_type: String,
    pub feature_count: Option<usize>,
    pub projection: String,
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub field_type: String,
    pub width: Option<usize>,
    pub precision: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum DatasetType {
    Raster(RasterInfo),
    Vector(Vec<LayerInfo>),
}

#[derive(Debug, Clone)]
/// Représente un ensemble de données géospatiales, qu'il soit raster ou vectoriel.
///
/// # Champs
/// - `filename`: Le chemin du fichier de l'ensemble de données.
/// - `dataset_type`: Le type de l'ensemble de données (raster ou vectoriel avec les informations associées).
/// - `bbox`: La boîte englobante de l'ensemble de données.
/// - `driver`: Le nom du pilote GDAL utilisé pour lire l'ensemble de données.
/// ```rust
/// let dataset = Dataset::open("path/to/dataset.tif").await?;
/// if dataset.is_raster() {
///     let (width, height) = dataset.raster_size()?;
///     println!("Raster size: {}x{}", width, height);
/// } else if dataset.is_vector() {
///     let layer_count = dataset.layer_count();
///     println!("Number of layers: {}", layer_count);
/// }
/// let bbox = dataset.bbox();
/// println!("Bounding box: {:?}", bbox);
/// let projection = dataset.projection();
/// println!("Projection: {}", projection);
/// ```
pub struct Dataset {
    pub filename: String,
    pub dataset_type: DatasetType,
    pub bbox: BoundingBox,
    pub driver: String,
}

impl Dataset {
    pub async fn open(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (is_raster, driver_name) = Self::detect_format(&extension)?;

        let output = if is_raster {
            executor("gdalinfo", &["-json", file_path]).await?.0
        } else {
            executor("ogrinfo", &["-json", file_path]).await?.0
        };

        let info: Value = serde_json::from_str(&output)?;

        let (dataset_type, bbox) = if is_raster {
            (
                Self::parse_raster_info(&info)?,
                Self::parse_raster_bbox(&info)?,
            )
        } else {
            (
                Self::parse_vector_info(&info)?,
                Self::parse_vector_bbox(&info).unwrap_or_default(),
            )
        };

        Ok(Self {
            filename: file_path.to_string(),
            dataset_type,
            bbox,
            driver: driver_name.to_string(),
        })
    }

    fn detect_format(ext: &str) -> Result<(bool, &'static str), Box<dyn Error>> {
        match ext {
            "tif" | "tiff" => Ok((true, GTiff::NAME)),
            "jpeg" | "jpg" => Ok((true, JPEG::NAME)),
            "png" => Ok((true, PNG::NAME)),
            "dat" => Ok((true, ENVI::NAME)),
            "gpkg" => Ok((false, GPKG::NAME)),
            "shp" => Ok((false, Shapefile::NAME)),
            "geojson" | "json" => Ok((false, GeoJSON::NAME)),
            _ => Err(format!("Unsupported extension: {}", ext).into()),
        }
    }

    fn get_array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
        value[key].as_array().map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn parse_raster_info(info: &Value) -> Result<DatasetType, Box<dyn Error>> {
        let size = info["size"].as_array().ok_or("No size info")?;
        let width = size[0].as_u64().ok_or("Invalid width")? as usize;
        let height = size[1].as_u64().ok_or("Invalid height")? as usize;

        let geo_transform: Vec<f64> = Self::get_array(info, "geoTransform")
            .iter()
            .filter_map(|v| v.as_f64())
            .collect();

        let projection = info["coordinateSystem"]["wkt"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let eo_bands = Self::get_array(info, "eo:bands");

        let bands: Vec<RasterBand> = Self::get_array(info, "bands")
            .iter()
            .enumerate()
            .map(|(i, band)| {
                let data_type = band["type"].as_str().unwrap_or("Unknown").to_string();
                let mut raster_band = RasterBand::new(i + 1, data_type);

                if let Some(eo_band) = eo_bands.get(i) {
                    raster_band.name = eo_band["name"].as_str().map(String::from);
                    raster_band.description = eo_band["description"].as_str().map(String::from);
                    raster_band.common_name = eo_band["common_name"].as_str().map(String::from);
                }

                raster_band
            })
            .collect();

        Ok(DatasetType::Raster(RasterInfo {
            width,
            height,
            geo_transform: GeoTransform::from_vec(geo_transform).ok_or("Invalid geotransform")?,
            projection,
            bands,
        }))
    }

    fn parse_vector_info(info: &Value) -> Result<DatasetType, Box<dyn Error>> {
        let layers: Vec<LayerInfo> = Self::get_array(info, "layers")
            .iter()
            .map(|layer| {
                let fields: Vec<FieldInfo> = Self::get_array(layer, "fields")
                    .iter()
                    .map(|field| FieldInfo {
                        name: field["name"].as_str().unwrap_or("Unnamed").to_string(),
                        field_type: field["type"].as_str().unwrap_or("Unknown").to_string(),
                        width: field["width"].as_u64().map(|w| w as usize),
                        precision: field["precision"].as_u64().map(|p| p as usize),
                    })
                    .collect();

                LayerInfo {
                    name: layer["name"].as_str().unwrap_or("Unnamed").to_string(),
                    geometry_type: layer["geometryType"]
                        .as_str()
                        .unwrap_or("Unknown")
                        .to_string(),
                    feature_count: layer["featureCount"].as_u64().map(|c| c as usize),
                    projection: layer["coordinateSystem"]["wkt"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    fields,
                }
            })
            .collect();

        Ok(DatasetType::Vector(layers))
    }

    fn parse_raster_bbox(info: &Value) -> Result<BoundingBox, Box<dyn Error>> {
        let coords = info["cornerCoordinates"]
            .as_object()
            .ok_or("No corner coordinates")?;

        Ok(BoundingBox::new(
            coords["lowerLeft"][0].as_f64().ok_or("Invalid xmin")?,
            coords["lowerLeft"][1].as_f64().ok_or("Invalid ymin")?,
            coords["upperRight"][0].as_f64().ok_or("Invalid xmax")?,
            coords["upperRight"][1].as_f64().ok_or("Invalid ymax")?,
        ))
    }

    fn parse_vector_bbox(info: &Value) -> Result<BoundingBox, Box<dyn Error>> {
        let parse_extent = |extent: &Value| -> Option<BoundingBox> {
            let arr = extent.as_array()?;
            if arr.len() == 4 {
                Some(BoundingBox::new(
                    arr[0].as_f64()?,
                    arr[1].as_f64()?,
                    arr[2].as_f64()?,
                    arr[3].as_f64()?,
                ))
            } else {
                None
            }
        };

        if let Some(bbox) = parse_extent(&info["extent"]) {
            return Ok(bbox);
        }

        if let Some(layers) = info["layers"].as_array() {
            for layer in layers {
                if let Some(bbox) = parse_extent(&layer["extent"]) {
                    return Ok(bbox);
                }

                if let Some(geom_fields) = layer["geometryFields"].as_array() {
                    for geom_field in geom_fields {
                        if let Some(bbox) = parse_extent(&geom_field["extent"]) {
                            return Ok(bbox);
                        }
                    }
                }
            }
        }

        Err("No extent information found".into())
    }

    // ==================== RASTER-SPECIFIC METHODS ====================

    pub fn raster_size(&self) -> Result<(usize, usize), Box<dyn Error>> {
        match &self.dataset_type {
            DatasetType::Raster(info) => Ok((info.width, info.height)),
            DatasetType::Vector(_) => Err("Not a raster dataset".into()),
        }
    }

    pub fn geo_transform(&self) -> Result<Vec<f64>, Box<dyn Error>> {
        match &self.dataset_type {
            DatasetType::Raster(info) => Ok(info.geo_transform.to_vec()),
            DatasetType::Vector(_) => Err("Not a raster dataset".into()),
        }
    }

    pub fn rasterband(&self, band_index: usize) -> Result<RasterBand, Box<dyn Error>> {
        match &self.dataset_type {
            DatasetType::Raster(info) => {
                if band_index == 0 || band_index > info.bands.len() {
                    return Err("Band index out of range".into());
                }
                Ok(info.bands[band_index - 1].clone())
            }
            DatasetType::Vector(_) => Err("Not a raster dataset".into()),
        }
    }

    pub fn raster_count(&self) -> usize {
        match &self.dataset_type {
            DatasetType::Raster(info) => info.bands.len(),
            DatasetType::Vector(_) => 0,
        }
    }

    // ==================== VECTOR-SPECIFIC METHODS ====================

    pub fn layer_count(&self) -> usize {
        match &self.dataset_type {
            DatasetType::Vector(layers) => layers.len(),
            DatasetType::Raster(_) => 0,
        }
    }

    pub fn layer(&self, layer_index: usize) -> Result<&LayerInfo, Box<dyn Error>> {
        match &self.dataset_type {
            DatasetType::Vector(layers) => layers
                .get(layer_index)
                .ok_or("Layer index out of range".into()),
            DatasetType::Raster(_) => Err("Not a vector dataset".into()),
        }
    }

    pub fn layer_by_name(&self, name: &str) -> Result<&LayerInfo, Box<dyn Error>> {
        match &self.dataset_type {
            DatasetType::Vector(layers) => layers
                .iter()
                .find(|layer| layer.name == name)
                .ok_or("Layer not found".into()),
            DatasetType::Raster(_) => Err("Not a vector dataset".into()),
        }
    }

    pub fn layer_name(&self, layer_index: usize) -> Result<String, Box<dyn Error>> {
        self.layer(layer_index).map(|layer| layer.name.clone())
    }

    pub fn geometry_type(&self, layer_index: usize) -> Result<String, Box<dyn Error>> {
        self.layer(layer_index)
            .map(|layer| layer.geometry_type.clone())
    }

    pub fn feature_count(&self, layer_index: usize) -> Result<Option<usize>, Box<dyn Error>> {
        self.layer(layer_index).map(|layer| layer.feature_count)
    }

    // ==================== COMMON METHODS ====================

    pub fn projection(&self) -> String {
        match &self.dataset_type {
            DatasetType::Raster(info) => info.projection.clone(),
            DatasetType::Vector(layers) => layers
                .first()
                .map(|layer| layer.projection.clone())
                .unwrap_or_default(),
        }
    }

    pub fn bbox(&self) -> BoundingBox {
        self.bbox
    }

    pub fn driver_name(&self) -> &str {
        &self.driver
    }

    pub fn is_raster(&self) -> bool {
        matches!(self.dataset_type, DatasetType::Raster(_))
    }

    pub fn is_vector(&self) -> bool {
        matches!(self.dataset_type, DatasetType::Vector(_))
    }
}

pub mod prelude {
    pub use super::{
        Dataset, DatasetType, Driver, DriverFormat, ENVI, FieldInfo, GPKG, GTiff, GeoJSON,
        GeoTransform, JPEG, LayerInfo, PNG, RasterBand, RasterInfo, Shapefile,
    };
}
