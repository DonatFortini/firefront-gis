use std::{
    collections::HashMap,
    error::Error,
    fs::{self},
    path::Path,
};

use geo::{Contains, Intersects};
use geo_types::Geometry;
use serde::de::{self};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
    let geom = wkt::TryFromWkt::try_from_wkt_str(&wkt_string);
    match geom {
        Ok(g) => Ok(g),
        Err(e) => Err(de::Error::custom(format!(
            "Failed to deserialize Geometry from WKT: {}",
            e
        ))),
    }
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
        Region {
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

    pub fn get_neighbors(&self) -> &Vec<String> {
        &self.neighbors
    }

    pub fn get_code(&self) -> &String {
        &self.code
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_extent(&self) -> &Geometry {
        &self.extent
    }

    pub fn contains(&self, bounding_box: &BoundingBox) -> bool {
        match bounding_box.to_geometry() {
            Ok(bbox_geom) => self.extent.contains(&bbox_geom),
            Err(_) => false,
        }
    }

    pub fn intersects(&self, bounding_box: &BoundingBox) -> bool {
        match bounding_box.to_geometry() {
            Ok(bbox_geom) => self.extent.intersects(&bbox_geom),
            Err(_) => false,
        }
    }
}

fn get_regions_graph() -> Result<HashMap<String, Region>, Box<dyn Error>> {
    let graph_path = get_config(|config| config.regions_graph_path());
    if !Path::new(&graph_path).exists() {
        return Err("Regions graph file not found".into());
    }
    let json_str = fs::read_to_string(graph_path)?;
    let graph: HashMap<String, Region> = serde_json::from_str(&json_str)?;

    Ok(graph)
}

/// Renvoie la liste des régions voisines pour une région donnée
/// en utilisant le fichier JSON du graphe des régions.
///
/// # Arguments
///
/// * `region_id` - Le code de la région pour laquelle obtenir les voisins.
///
/// # Returns
///
/// * `Result<Vec<Region>, Box<dyn Error>>` - Une liste de `Region` représentant les voisins de la région.
pub fn get_neighbors(region_id: &str) -> Result<Vec<Region>, Box<dyn Error>> {
    let graph = get_regions_graph()?;

    if let Some(region_info) = graph.get(region_id) {
        let neighbors: Vec<Region> = region_info
            .neighbors
            .iter()
            .filter_map(|neighbor_code| graph.get(neighbor_code).cloned())
            .collect();
        Ok(neighbors)
    } else {
        Err(format!("Region code '{region_id}' not found in the graph").into())
    }
}

pub fn get_region(region_id: &str) -> Result<Region, Box<dyn Error>> {
    let graph = get_regions_graph()?;

    graph
        .get(region_id)
        .cloned()
        .ok_or_else(|| format!("Region code '{region_id}' not found in the graph").into())
}

/// Détermine quelles régions intersectent avec une boîte englobante donnée
///
/// # Arguments
///
/// * `bounding_box` - La boîte englobante à vérifier
///
/// # Returns
///
/// * `Result<Vec<Region>, Box<dyn Error>>` - Résultat contenant les informations d'intersection
pub fn find_intersecting_regions(
    bounding_box: &BoundingBox,
) -> Result<Vec<Region>, Box<dyn Error>> {
    let graph = get_regions_graph()?;

    let mut intersecting_regions: Vec<Region> = Vec::new();

    for region in graph.values() {
        if region.intersects(bounding_box) {
            intersecting_regions.push(region.clone());
        }
    }

    Ok(intersecting_regions)
}
