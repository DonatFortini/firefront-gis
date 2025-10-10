use crate::error::{GisError, GisResult};
use crate::types::BoundingBox;
use crate::utils::resource_dir;
use geo::Intersects;
use rusqlite::{Connection, params};
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

    pub fn get_region(region_code: &str) -> GisResult<Option<(String, String)>> {
        let conn = Self::get_connection()?;

        let result = conn.query_row(
            "SELECT code, name FROM regions WHERE code = ?",
            params![region_code],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        match result {
            Ok(data) => Ok(Some(data)),
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
}
