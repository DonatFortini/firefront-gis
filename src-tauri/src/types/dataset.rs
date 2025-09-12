use crate::{
    types::{
        BoundingBox, DriverFormat, ENVI, GPKG, GTiff, GeoJSON, JPEG, PNG, RasterBand, Shapefile,
    },
    utils::executor,
};
use serde_json::Value;

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
        Some(GeoTransform {
            x_origin: vec[0],
            pixel_width: vec[1],
            x_rotation: vec[2],
            y_origin: vec[3],
            y_rotation: vec[4],
            pixel_height: vec[5],
        })
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum DatasetType {
    Raster(RasterInfo),
    Vector(Vec<LayerInfo>),
}

pub struct Dataset {
    pub filename: String,
    pub dataset_type: DatasetType,
    pub bbox: BoundingBox,
    pub driver: String,
}

impl Dataset {
    pub async fn open(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (is_raster, driver_name) = match extension.as_str() {
            "tif" | "tiff" => (true, GTiff::NAME),
            "jpeg" | "jpg" => (true, JPEG::NAME),
            "png" => (true, PNG::NAME),
            "dat" => (true, ENVI::NAME),
            "gpkg" => (false, GPKG::NAME),
            "shp" => (false, Shapefile::NAME),
            "geojson" | "json" => (false, GeoJSON::NAME),
            _ => return Err(format!("Unsupported file extension: {}", extension).into()),
        };

        let output = if is_raster {
            executor("gdalinfo", &["-json", file_path]).await?.0
        } else {
            executor("ogrinfo", &["-json", file_path]).await?.0
        };

        let info: serde_json::Value = serde_json::from_str(&output)?;
        let driver = driver_name.to_string();

        if is_raster && info["size"].as_array().is_some() {
            let dataset_type = Self::parse_raster_info(&info)?;
            let bbox = Self::parse_raster_bbox(&info)?;

            Ok(Dataset {
                filename: file_path.to_string(),
                dataset_type,
                bbox,
                driver,
            })
        } else if info["layers"].is_array() {
            let dataset_type = Self::parse_vector_info(&info).await?;
            let bbox = Self::parse_vector_bbox(&info).unwrap_or_default();

            Ok(Dataset {
                filename: file_path.to_string(),
                dataset_type,
                bbox,
                driver,
            })
        } else {
            Err("Unable to determine dataset type (neither raster nor vector)".into())
        }
    }

    fn parse_raster_info(info: &Value) -> Result<DatasetType, Box<dyn std::error::Error>> {
        let size = info["size"].as_array().ok_or("No size info")?;
        let width = size[0].as_u64().ok_or("Invalid width")? as usize;
        let height = size[1].as_u64().ok_or("Invalid height")? as usize;

        let geo_transform = info["geoTransform"]
            .as_array()
            .ok_or("No geotransform")?
            .iter()
            .map(|v| v.as_f64().ok_or("Invalid geotransform value"))
            .collect::<Result<Vec<f64>, _>>()?;

        let projection = info["coordinateSystem"]["wkt"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let binding = vec![];
        let raster_bands = info["bands"].as_array().unwrap_or(&binding);

        let empty_array = vec![];
        let eo_bands = info["eo:bands"].as_array().unwrap_or(&empty_array);

        let mut bands = Vec::new();
        for (i, raster_band) in raster_bands.iter().enumerate() {
            let data_type = raster_band["type"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string();

            let mut band = RasterBand::new(i + 1, data_type);

            if let Some(eo_band) = eo_bands.get(i) {
                band.name = eo_band["name"].as_str().map(|s| s.to_string());
                band.description = eo_band["description"].as_str().map(|s| s.to_string());
                band.common_name = eo_band["common_name"].as_str().map(|s| s.to_string());
            }

            bands.push(band);
        }

        Ok(DatasetType::Raster(RasterInfo {
            width,
            height,
            geo_transform: GeoTransform::from_vec(geo_transform)
                .ok_or("Invalid geotransform format")?,
            projection,
            bands,
        }))
    }

    async fn parse_vector_info(info: &Value) -> Result<DatasetType, Box<dyn std::error::Error>> {
        let layers_json = info["layers"].as_array().ok_or("No layers info")?;
        let mut layers = Vec::new();

        for layer_json in layers_json {
            let name = layer_json["name"].as_str().unwrap_or("Unnamed").to_string();

            let geometry_type = layer_json["geometryType"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string();

            let feature_count = layer_json["featureCount"].as_u64().map(|c| c as usize);

            let projection = layer_json["coordinateSystem"]["wkt"]
                .as_str()
                .unwrap_or("")
                .to_string();

            let mut fields = Vec::new();
            if let Some(fields_json) = layer_json["fields"].as_array() {
                for field_json in fields_json {
                    let field_name = field_json["name"].as_str().unwrap_or("Unnamed").to_string();

                    let field_type = field_json["type"].as_str().unwrap_or("Unknown").to_string();

                    let width = field_json["width"].as_u64().map(|w| w as usize);
                    let precision = field_json["precision"].as_u64().map(|p| p as usize);

                    fields.push(FieldInfo {
                        name: field_name,
                        field_type,
                        width,
                        precision,
                    });
                }
            }

            layers.push(LayerInfo {
                name,
                geometry_type,
                feature_count,
                projection,
                fields,
            });
        }

        Ok(DatasetType::Vector(layers))
    }

    fn parse_raster_bbox(info: &Value) -> Result<BoundingBox, Box<dyn std::error::Error>> {
        let corner_coordinates = info["cornerCoordinates"]
            .as_object()
            .ok_or("No corner coordinates")?;

        Ok(BoundingBox {
            xmin: corner_coordinates["lowerLeft"][0].as_f64().unwrap(),
            ymin: corner_coordinates["lowerLeft"][1].as_f64().unwrap(),
            xmax: corner_coordinates["upperRight"][0].as_f64().unwrap(),
            ymax: corner_coordinates["upperRight"][1].as_f64().unwrap(),
        })
    }

    fn parse_vector_bbox(info: &Value) -> Result<BoundingBox, Box<dyn std::error::Error>> {
        let parse_extent = |extent: &Value| -> Option<BoundingBox> {
            let arr = extent.as_array()?;
            if arr.len() == 4 {
                Some(BoundingBox {
                    xmin: arr[0].as_f64().unwrap_or(0.0),
                    ymin: arr[1].as_f64().unwrap_or(0.0),
                    xmax: arr[2].as_f64().unwrap_or(0.0),
                    ymax: arr[3].as_f64().unwrap_or(0.0),
                })
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

    // Raster-specific methods

    pub fn raster_size(&self) -> Result<(usize, usize), Box<dyn std::error::Error>> {
        match &self.dataset_type {
            DatasetType::Raster(raster_info) => Ok((raster_info.width, raster_info.height)),
            DatasetType::Vector(_) => Err("Not a raster dataset".into()),
        }
    }

    pub fn geo_transform(&self) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        match &self.dataset_type {
            DatasetType::Raster(raster_info) => Ok(raster_info.geo_transform.to_vec()),
            DatasetType::Vector(_) => Err("Not a raster dataset".into()),
        }
    }

    pub fn projection(&self) -> String {
        match &self.dataset_type {
            DatasetType::Raster(raster_info) => raster_info.projection.clone(),
            DatasetType::Vector(layers) => layers
                .first()
                .map(|layer| layer.projection.clone())
                .unwrap_or_default(),
        }
    }

    pub fn rasterband(&self, band_index: usize) -> Result<RasterBand, Box<dyn std::error::Error>> {
        match &self.dataset_type {
            DatasetType::Raster(raster_info) => {
                if band_index == 0 || band_index > raster_info.bands.len() {
                    return Err("Band index out of range".into());
                }
                Ok(raster_info.bands[band_index - 1].clone())
            }
            DatasetType::Vector(_) => Err("Not a raster dataset".into()),
        }
    }

    pub fn raster_count(&self) -> usize {
        match &self.dataset_type {
            DatasetType::Raster(raster_info) => raster_info.bands.len(),
            DatasetType::Vector(_) => 0,
        }
    }

    // Vector-specific methods

    pub fn layer_count(&self) -> usize {
        match &self.dataset_type {
            DatasetType::Vector(layers) => layers.len(),
            DatasetType::Raster(_) => 0,
        }
    }

    pub fn layer(&self, layer_index: usize) -> Result<&LayerInfo, Box<dyn std::error::Error>> {
        match &self.dataset_type {
            DatasetType::Vector(layers) => layers
                .get(layer_index)
                .ok_or("Layer index out of range".into()),
            DatasetType::Raster(_) => Err("Not a vector dataset".into()),
        }
    }

    pub fn layer_by_name(&self, name: &str) -> Result<&LayerInfo, Box<dyn std::error::Error>> {
        match &self.dataset_type {
            DatasetType::Vector(layers) => layers
                .iter()
                .find(|layer| layer.name == name)
                .ok_or("Layer not found".into()),
            DatasetType::Raster(_) => Err("Not a vector dataset".into()),
        }
    }

    pub fn layer_name(&self, layer_index: usize) -> Result<String, Box<dyn std::error::Error>> {
        match &self.dataset_type {
            DatasetType::Vector(_layers) => self.layer(layer_index).map(|layer| layer.name.clone()),
            DatasetType::Raster(_) => Err("Not a vector dataset".into()),
        }
    }

    pub fn geometry_type(&self, layer_index: usize) -> Result<String, Box<dyn std::error::Error>> {
        match &self.dataset_type {
            DatasetType::Vector(_layers) => self
                .layer(layer_index)
                .map(|layer| layer.geometry_type.clone()),
            DatasetType::Raster(_) => Err("Not a vector dataset".into()),
        }
    }

    pub fn feature_count(
        &self,
        layer_index: usize,
    ) -> Result<Option<usize>, Box<dyn std::error::Error>> {
        match &self.dataset_type {
            DatasetType::Vector(_layers) => {
                self.layer(layer_index).map(|layer| layer.feature_count)
            }
            DatasetType::Raster(_) => Err("Not a vector dataset".into()),
        }
    }

    // Common methods
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
    pub use super::{Dataset, DatasetType, FieldInfo, GeoTransform, LayerInfo, RasterInfo};
}
