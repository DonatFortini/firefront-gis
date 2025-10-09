use geo::{Geometry, Intersects, Relate};
use geojson::GeoJson;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::error::{GisError, GisResult};
use crate::types::{Region, get_region};
use crate::utils::resource_dir;

pub struct RegionService;

impl RegionService {
    pub fn create_region_geojson(region_id: &str, output_path: &str) -> GisResult<()> {
        let region = get_region(region_id).map_err(|e| GisError::InvalidGeometry(e.to_string()))?;

        let geometry: geojson::Geometry = region.extent().into();

        let properties = serde_json::json!({
            "code": region.code(),
            "name": region.name(),
            "neighbors": region.neighbors()
        });

        let feature = geojson::Feature {
            bbox: None,
            geometry: Some(geometry),
            id: None,
            properties: Some(properties.as_object().unwrap().clone()),
            foreign_members: None,
        };

        let feature_collection = geojson::FeatureCollection {
            bbox: None,
            features: vec![feature],
            foreign_members: Some(
                serde_json::json!({
                    "crs": {
                        "type": "name",
                        "properties": {"name": "EPSG:2154"}
                    }
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        };

        let geojson_string = geojson::GeoJson::FeatureCollection(feature_collection).to_string();
        fs::write(output_path, geojson_string)?;

        Ok(())
    }

    pub async fn build_regions_graph(output_file: Option<&str>) -> GisResult<bool> {
        if let Some(path) = output_file
            && Path::new(path).exists()
        {
            println!("Loading regions graph from cache: {}", path);
            let json_str = fs::read_to_string(path)?;
            let _: HashMap<String, Region> = serde_json::from_str(&json_str)?;
            return Ok(true);
        }

        let geojson_path = resource_dir().join("regions.geojson");
        let geojson_str = fs::read_to_string(&geojson_path)?;
        let geojson: GeoJson = geojson_str
            .parse()
            .map_err(|e| GisError::InvalidGeometry(format!("Failed to parse GeoJSON: {:?}", e)))?;

        let feature_collection = match geojson {
            GeoJson::FeatureCollection(fc) => fc,
            _ => {
                return Err(GisError::InvalidGeometry(
                    "Not a FeatureCollection".to_string(),
                ));
            }
        };

        let mut regions_info: HashMap<String, Region> = HashMap::new();
        let total = feature_collection.features.len();

        println!("Parsing {} features...", total);

        for (idx, feature) in feature_collection.features.iter().enumerate() {
            if idx % 100 == 0 {
                print!(
                    "\rProgress: {}/{} ({:.1}%)",
                    idx,
                    total,
                    (idx as f64 / total as f64) * 100.0
                );
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            }

            let Some(code) = feature.property("code").and_then(|v| v.as_str()) else {
                continue;
            };
            let name = feature
                .property("nom")
                .and_then(|v| v.as_str())
                .unwrap_or(code);
            let Some(geometry) = &feature.geometry else {
                continue;
            };

            let geojson_value = serde_json::to_value(geometry)?;
            let gdal_geom: Geometry = serde_json::to_string(&geojson_value)?
                .parse::<geojson::Geometry>()
                .map_err(|e| GisError::InvalidGeometry(format!("Geometry parse error: {:?}", e)))?
                .try_into()
                .map_err(|e| {
                    GisError::InvalidGeometry(format!("Geometry conversion error: {:?}", e))
                })?;

            regions_info.insert(
                code.to_string(),
                Region::new(code.to_string(), name.to_string(), gdal_geom),
            );
        }

        let codes: Vec<String> = regions_info.keys().cloned().collect();
        let total_comparisons = (codes.len() * (codes.len() - 1)) / 2;
        let mut done = 0;

        for i in 0..codes.len() {
            let code_i = &codes[i];
            let geom_i = regions_info[code_i].extent().clone();

            for code_j in &codes[i + 1..] {
                let geom_j = regions_info[code_j].extent().clone();

                if geom_i.intersects(&geom_j) || geom_i.relate(&geom_j).is_touches() {
                    regions_info
                        .get_mut(code_i)
                        .unwrap()
                        .add_neighbor(code_j.clone());
                    regions_info
                        .get_mut(code_j)
                        .unwrap()
                        .add_neighbor(code_i.clone());
                }

                done += 1;
                if done % 1000 == 0 {
                    print!(
                        "\rComparisons: {}/{} ({:.1}%)",
                        done,
                        total_comparisons,
                        (done as f64 / total_comparisons as f64) * 100.0
                    );
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                    tokio::task::yield_now().await;
                }
            }
        }

        if let Some(path) = output_file {
            let json_str = serde_json::to_string_pretty(&regions_info)?;
            fs::write(path, json_str)?;
            println!("\nRegions graph saved to: {}", path);
        }

        Ok(true)
    }
}
