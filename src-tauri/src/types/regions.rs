use std::{collections::HashMap, error::Error, fs, path::Path};

use geo::{Contains, Intersects};
use geo_types::Geometry;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use wkt::ToWkt;

use crate::{config::get_config, types::BoundingBox};

fn serialize_geometry<S>(geom: &Geometry, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let wkt_string = geom.to_wkt().to_string();
    serializer.serialize_str(&wkt_string)
}

fn deserialize_geometry<'de, D>(deserializer: D) -> Result<Geometry, D::Error>
where
    D: Deserializer<'de>,
{
    let wkt_string = String::deserialize(deserializer)?;
    wkt::TryFromWkt::try_from_wkt_str(&wkt_string)
        .map_err(|e| de::Error::custom(format!("Failed to deserialize Geometry from WKT: {}", e)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub code: String,
    pub name: String,
    #[serde(
        serialize_with = "serialize_geometry",
        deserialize_with = "deserialize_geometry"
    )]
    pub extent: Geometry,
    pub neighbors: Vec<String>,
}

impl Region {
    pub fn new(code: String, name: String, extent: Geometry) -> Self {
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

    pub fn extent(&self) -> &Geometry {
        &self.extent
    }

    pub fn contains(&self, bounding_box: &BoundingBox) -> bool {
        self.extent.contains(&bounding_box.to_geometry())
    }

    pub fn intersects(&self, bounding_box: &BoundingBox) -> bool {
        self.extent.intersects(&bounding_box.to_geometry())
    }
}

fn load_regions_graph() -> Result<HashMap<String, Region>, Box<dyn Error>> {
    let graph_path = get_config(|config| config.regions_graph_path());

    if !Path::new(&graph_path).exists() {
        return Err(format!("Regions graph file not found: {}", graph_path.display()).into());
    }

    let json_str = fs::read_to_string(graph_path)?;
    let graph: HashMap<String, Region> = serde_json::from_str(&json_str)?;

    Ok(graph)
}

pub fn get_region(region_id: &str) -> Result<Region, Box<dyn Error>> {
    let graph = load_regions_graph()?;

    graph
        .get(region_id)
        .cloned()
        .ok_or_else(|| format!("Region code '{}' not found in the graph", region_id).into())
}

pub fn get_neighbors(region_id: &str) -> Result<Vec<Region>, Box<dyn Error>> {
    let graph = load_regions_graph()?;

    let region = graph
        .get(region_id)
        .ok_or_else(|| format!("Region code '{}' not found in the graph", region_id))?;

    let neighbors: Vec<Region> = region
        .neighbors
        .iter()
        .filter_map(|neighbor_code| graph.get(neighbor_code).cloned())
        .collect();

    Ok(neighbors)
}

pub fn find_intersecting_regions(
    bounding_box: &BoundingBox,
) -> Result<Vec<Region>, Box<dyn Error>> {
    let graph = load_regions_graph()?;

    let intersecting_regions: Vec<Region> = graph
        .values()
        .filter(|region| region.intersects(bounding_box))
        .cloned()
        .collect();

    Ok(intersecting_regions)
}

pub fn find_containing_regions(bounding_box: &BoundingBox) -> Result<Vec<Region>, Box<dyn Error>> {
    let graph = load_regions_graph()?;

    let containing_regions: Vec<Region> = graph
        .values()
        .filter(|region| region.contains(bounding_box))
        .cloned()
        .collect();

    Ok(containing_regions)
}

pub fn get_all_regions() -> Result<Vec<Region>, Box<dyn Error>> {
    let graph = load_regions_graph()?;
    Ok(graph.into_values().collect())
}

pub fn get_all_region_codes() -> Result<Vec<String>, Box<dyn Error>> {
    let graph = load_regions_graph()?;
    Ok(graph.keys().cloned().collect())
}

pub mod prelude {
    pub use super::{
        Region, find_containing_regions, find_intersecting_regions, get_all_region_codes,
        get_all_regions, get_neighbors, get_region,
    };
}
