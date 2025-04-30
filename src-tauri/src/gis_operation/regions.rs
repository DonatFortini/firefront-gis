use std::{
    collections::HashMap,
    env::current_dir,
    error::Error,
    fs::{self, File},
    io::Write,
    path::Path,
};

use gdal::vector::Geometry;
use geojson::GeoJson;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use crate::utils::BoundingBox;

struct GeometryDef {
    wkt: String,
}

impl From<&Geometry> for GeometryDef {
    fn from(geom: &Geometry) -> Self {
        GeometryDef {
            wkt: geom.wkt().unwrap_or_default(),
        }
    }
}

fn serialize_geometry<S>(geom: &Geometry, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let geom_def = GeometryDef::from(geom);
    serializer.serialize_str(&geom_def.wkt)
}

struct GeometryVisitor;

impl Visitor<'_> for GeometryVisitor {
    type Value = Geometry;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string containing WKT geometry")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Geometry::from_wkt(value).map_err(|e| de::Error::custom(format!("Invalid WKT: {}", e)))
    }
}

fn deserialize_geometry<'de, D>(deserializer: D) -> Result<Geometry, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(GeometryVisitor)
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

/// Construit un graphe de dépendances entre les régions à partir d'un fichier GeoJSON.
/// Le graphe est sauvegardé dans un fichier JSON pour une utilisation ultérieure.
/// Si le fichier de sortie existe déjà, il est chargé à partir de ce fichier.
///
/// # Arguments
///
/// * `output_file` - Le chemin vers le fichier de sortie où le graphe sera sauvegardé.
///
/// # Returns
///
/// * `Result<bool, Box<dyn Error>>` - Retourne `true` si le graphe a été construit ou chargé avec succès.
pub fn build_regions_graph(output_file: Option<&str>) -> Result<bool, Box<dyn Error>> {
    if let Some(path) = &output_file {
        if Path::new(path).exists() {
            println!("Loading regions graph from cache file: {}", path);
            let json_str = fs::read_to_string(path)?;
            let _: HashMap<String, Region> = serde_json::from_str(&json_str)?;
            return Ok(true);
        }
    }

    let binding = current_dir()?.join("resources/regions.geojson");
    let regional_geojson_path = binding.to_str().unwrap();
    if !Path::new(regional_geojson_path).exists() {
        return Err(format!("Input file not found: {}", regional_geojson_path).into());
    }

    let geojson_str = fs::read_to_string(regional_geojson_path)?;
    let geojson: GeoJson = geojson_str.parse()?;

    let feature_collection = match geojson {
        GeoJson::FeatureCollection(fc) => fc,
        _ => return Err("GeoJSON is not a FeatureCollection".into()),
    };

    let mut regions_info: HashMap<String, Region> = HashMap::new();

    for feature in feature_collection.features.iter().filter_map(Some) {
        let code = match feature.property("code").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => continue,
        };

        let name = match feature.property("nom").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => code.clone(),
        };

        let geometry = match &feature.geometry {
            Some(g) => g,
            None => continue,
        };

        let geojson_value = serde_json::to_value(geometry).unwrap();
        let geojson_str = serde_json::to_string(&geojson_value).unwrap();

        let gdal_geom = match Geometry::from_geojson(&geojson_str) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("Failed to convert geometry for region {}: {}", code, e);
                continue;
            }
        };

        let region = Region::new(code.clone(), name, gdal_geom);
        regions_info.insert(code, region);
    }

    let region_codes: Vec<String> = regions_info.keys().cloned().collect();
    let mut adjacency_updates: Vec<(String, String)> = Vec::new();

    for i in 0..region_codes.len() {
        let code_i = &region_codes[i];
        let geom_i = regions_info[code_i].get_extent().clone();

        for code_j in &region_codes[(i + 1)..] {
            let geom_j = regions_info[code_j].get_extent().clone();

            let intersects = geom_i.intersects(&geom_j);
            if intersects {
                adjacency_updates.push((code_i.clone(), code_j.clone()));
            } else {
                let touches = geom_i.touches(&geom_j);
                if touches {
                    adjacency_updates.push((code_i.clone(), code_j.clone()));
                }
            }
        }
    }

    for (code_i, code_j) in adjacency_updates {
        if let Some(region_i) = regions_info.get_mut(&code_i) {
            region_i.add_neighbor(code_j.clone());
        }
        if let Some(region_j) = regions_info.get_mut(&code_j) {
            region_j.add_neighbor(code_i.clone());
        }
    }

    if let Some(path) = output_file {
        let json_str = serde_json::to_string_pretty(&regions_info)?;
        let mut file = File::create(path)?;
        file.write_all(json_str.as_bytes())?;
        println!("Regions graph saved to: {}", path);
    }

    Ok(true)
}

fn load_regions_graph() -> Result<HashMap<String, Region>, Box<dyn Error>> {
    let graph_path = "resources/regions_graph.json";

    if !Path::new(graph_path).exists() {
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
    let graph = load_regions_graph()?;

    if let Some(region_info) = graph.get(region_id) {
        let neighbors: Vec<Region> = region_info
            .neighbors
            .iter()
            .filter_map(|neighbor_code| graph.get(neighbor_code).cloned())
            .collect();
        Ok(neighbors)
    } else {
        Err(format!("Region code '{}' not found in the graph", region_id).into())
    }
}

pub fn get_region(region_id: &str) -> Result<Region, Box<dyn Error>> {
    let graph = load_regions_graph()?;

    graph
        .get(region_id)
        .cloned()
        .ok_or_else(|| format!("Region code '{}' not found in the graph", region_id).into())
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
    let graph = load_regions_graph()?;

    let mut intersecting_regions: Vec<Region> = Vec::new();

    for region in graph.values() {
        if region.intersects(bounding_box) {
            intersecting_regions.push(region.clone());
        }
    }

    Ok(intersecting_regions)
}

/// Crée un fichier GeoJSON pour une région donnée
///
/// # Arguments
///
/// * `region_id` - code départemental de la région
/// * `output_path` - chemin du fichier GeoJSON de sortie
///
/// # Returns
///
/// * `Result<(), Box<dyn Error>>` - un résultat indiquant si la création du fichier a réussi ou échoué
pub fn create_region_geojson(region_id: &str, output_path: &str) -> Result<(), Box<dyn Error>> {
    let region = get_region(region_id)?;
    let gdal_geom = region.get_extent();
    let geojson_string = gdal_geom.json()?;
    let geometry: geojson::Geometry = serde_json::from_str(&geojson_string)?;
    let mut properties = serde_json::Map::new();
    properties.insert(
        "code".to_string(),
        serde_json::Value::String(region.get_code().clone()),
    );
    properties.insert(
        "name".to_string(),
        serde_json::Value::String(region.get_name().clone()),
    );

    let neighbors_value: Vec<serde_json::Value> = region
        .get_neighbors()
        .iter()
        .map(|code| serde_json::Value::String(code.clone()))
        .collect();
    properties.insert(
        "neighbors".to_string(),
        serde_json::Value::Array(neighbors_value),
    );

    let feature = geojson::Feature {
        bbox: None,
        geometry: Some(geometry),
        id: None,
        properties: Some(properties),
        foreign_members: None,
    };

    let mut foreign_members = serde_json::Map::new();
    let mut crs_properties = serde_json::Map::new();
    crs_properties.insert(
        "name".to_string(),
        serde_json::Value::String("EPSG:2154".to_string()),
    );

    let mut crs = serde_json::Map::new();
    crs.insert(
        "type".to_string(),
        serde_json::Value::String("name".to_string()),
    );
    crs.insert(
        "properties".to_string(),
        serde_json::Value::Object(crs_properties),
    );

    foreign_members.insert("crs".to_string(), serde_json::Value::Object(crs));

    let feature_collection = geojson::FeatureCollection {
        bbox: None,
        features: vec![feature],
        foreign_members: Some(foreign_members),
    };

    let geojson = geojson::GeoJson::FeatureCollection(feature_collection);
    let geojson_string = geojson.to_string();

    let mut file = File::create(output_path)?;
    file.write_all(geojson_string.as_bytes())?;

    Ok(())
}
