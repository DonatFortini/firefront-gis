use crate::error::{GisError, GisResult};
use crate::services::VectorService;
use crate::types::regions::get_region;
use crate::types::{BoundingBox, Region};
use crate::utils::resource_dir;
use crate::utils::temp_dir;
use geo::{Geometry, Intersects};
use rusqlite::{Connection, params};
use std::fs;
use std::str::FromStr;

pub struct RegionService;

impl RegionService {
    fn get_connection() -> GisResult<Connection> {
        let db_path = resource_dir().join("regions.db");

        if !db_path.exists() {
            return Err(GisError::Dataset(
                "Regions database not found. Please rebuild it.".to_string(),
            ));
        }

        Connection::open(&db_path)
            .map_err(|e| GisError::Dataset(format!("Failed to open database: {}", e)))
    }

    pub fn find_intersecting_regions(bbox: &BoundingBox) -> GisResult<Vec<(String, String)>> {
        let conn = Self::get_connection()?;

        let mut stmt = conn
            .prepare(
                "SELECT code, name, geometry FROM regions 
             WHERE bbox_xmax >= ?1 AND bbox_xmin <= ?2 
             AND bbox_ymax >= ?3 AND bbox_ymin <= ?4",
            )
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        let candidates: Vec<(String, String, String)> = stmt
            .query_map(params![bbox.xmin, bbox.xmax, bbox.ymin, bbox.ymax], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        let bbox_geom = bbox.to_geometry();
        let mut results = Vec::new();

        for (code, name, wkt_str) in candidates {
            let wkt = wkt::Wkt::from_str(&wkt_str)
                .map_err(|e| GisError::InvalidGeometry(format!("{:?}", e)))?;

            let geom: geo::Geometry = wkt
                .try_into()
                .map_err(|e| GisError::InvalidGeometry(format!("{:?}", e)))?;

            if geom.intersects(&bbox_geom) {
                results.push((code, name));
            }
        }

        Ok(results)
    }

    pub fn get_neighbors(region_code: &str) -> GisResult<Vec<String>> {
        let conn = Self::get_connection()?;

        let mut stmt = conn
            .prepare("SELECT neighbor_code FROM region_neighbors WHERE region_code = ?")
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        let neighbors: Vec<String> = stmt
            .query_map(params![region_code], |row| row.get(0))
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        Ok(neighbors)
    }

    pub fn get_region(region_code: &str) -> GisResult<Option<(String, String, Option<Geometry>)>> {
        let conn = Self::get_connection()?;

        let result = conn.query_row(
            "SELECT code, name, geometry FROM regions WHERE code = ?",
            params![region_code],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, String>(2)?)),
        );

        match result {
            Ok((code, name, wkt_str)) => {
                let geom = wkt::Wkt::from_str(&wkt_str)
                    .ok()
                    .and_then(|wkt| wkt.try_into().ok());
                Ok(Some((code, name, geom)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(GisError::Dataset(e.to_string())),
        }
    }

    pub fn check_database() -> GisResult<bool> {
        let conn = Self::get_connection()?;

        let version: Result<String, _> = conn.query_row(
            "SELECT value FROM metadata WHERE key = 'version'",
            [],
            |row| row.get(0),
        );

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM regions", [], |row| row.get(0))
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        Ok(version.is_ok() && count > 0)
    }

    pub fn create_region_geojson(region_id: &str, output_path: &str) -> GisResult<()> {
        let region = get_region(region_id).map_err(|e| GisError::InvalidGeometry(e.to_string()))?;

        let geometry: geojson::Geometry = region.extent().ok_or(GisError::ExtentNotFound)?.into();

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

    pub async fn create_region_file(region: &Region) -> GisResult<()> {
        let geojson_path = temp_dir().join(format!("{}.geojson", region.code()));
        let gpkg_path = geojson_path.with_extension("gpkg");
        if geojson_path.exists() {
            fs::remove_file(&geojson_path).ok();
        }
        if gpkg_path.exists() {
            fs::remove_file(&gpkg_path).ok();
        }

        Self::create_region_geojson(region.code(), geojson_path.to_str().unwrap())?;

        VectorService::convert_to_gpkg(geojson_path.to_str().unwrap(), gpkg_path.to_str().unwrap())
            .await?;

        fs::remove_file(&geojson_path).ok();

        Ok(())
    }
}
