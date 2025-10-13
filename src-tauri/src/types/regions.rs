use geo::{Contains, Intersects};
use geo_types::Geometry;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use wkt::ToWkt;

use crate::{error::GisError, services::gis::RegionService, types::BoundingBox};

fn serialize_geometry<S>(geom: &Option<Geometry>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match geom {
        Some(g) => {
            let wkt_string = g.to_wkt().to_string();
            serializer.serialize_some(&wkt_string)
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_geometry<'de, D>(deserializer: D) -> Result<Option<Geometry>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(wkt_string) => {
            let geom = wkt::TryFromWkt::try_from_wkt_str(&wkt_string).map_err(|e| {
                de::Error::custom(format!("Failed to deserialize Geometry from WKT: {}", e))
            })?;
            Ok(Some(geom))
        }
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub code: String,
    pub name: String,
    #[serde(
        serialize_with = "serialize_geometry",
        deserialize_with = "deserialize_geometry",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub extent: Option<Geometry>,
    #[serde(default)]
    pub neighbors: Vec<String>,
}

impl Region {
    pub fn new(code: String, name: String, extent: Geometry) -> Self {
        Self {
            code,
            name,
            extent: Some(extent),
            neighbors: Vec::new(),
        }
    }

    pub fn from_db(code: String, name: String, extent: Option<Geometry>) -> Self {
        Self {
            code,
            name,
            extent,
            neighbors: Vec::new(),
        }
    }

    pub fn add_neighbor(&mut self, neighbor_code: String) {
        if !self.neighbors.contains(&neighbor_code) {
            self.neighbors.push(neighbor_code);
        }
    }

    pub fn neighbors(&self) -> &[String] {
        &self.neighbors
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn extent(&self) -> Option<&Geometry> {
        self.extent.as_ref()
    }

    pub fn contains(&self, bounding_box: &BoundingBox) -> bool {
        self.extent
            .as_ref()
            .is_some_and(|e| e.contains(&bounding_box.to_geometry()))
    }

    pub fn intersects(&self, bounding_box: &BoundingBox) -> bool {
        self.extent
            .as_ref()
            .is_some_and(|e| e.intersects(&bounding_box.to_geometry()))
    }
}

pub fn get_region(region_id: &str) -> Result<Region, GisError> {
    let (code, name, geom) = RegionService::get_region(region_id)?
        .ok_or_else(|| GisError::NotFound(format!("Region code '{}' not found", region_id)))?;

    Ok(Region::from_db(code, name, geom))
}

pub fn get_neighbors(region_id: &str) -> Result<Vec<String>, GisError> {
    RegionService::get_neighbors(region_id)
}

pub fn find_intersecting_regions(bounding_box: &BoundingBox) -> Result<Vec<Region>, GisError> {
    RegionService::find_intersecting_regions(bounding_box)?
        .into_iter()
        .map(|(code, _)| get_region(&code))
        .collect()
}

pub mod prelude {
    pub use super::{Region, find_intersecting_regions, get_neighbors, get_region};
}
