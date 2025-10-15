use crate::error::{GisError, GisResult};
use crate::services::VectorService;
use crate::types::regions::get_region;
use crate::types::{BoundingBox, Region};
use crate::utils::{resource_dir, temp_dir};
use geo::{Geometry, Intersects};
use lazy_static::lazy_static;
use rusqlite::{Connection, params};
use std::fs;
use std::str::FromStr;
use std::sync::Mutex;

pub struct RegionService;

lazy_static! {
    static ref DB_POOL: Mutex<Vec<Connection>> = Mutex::new(Vec::new());
}

const PRAGMA_INIT: &str = "PRAGMA journal_mode = DELETE;
     PRAGMA synchronous = NORMAL;
     PRAGMA cache_size = 10000;
     PRAGMA temp_store = MEMORY;
     PRAGMA mmap_size = 30000000000;";

impl RegionService {
    fn get_connection() -> GisResult<Connection> {
        if let Ok(mut pool) = DB_POOL.lock()
            && let Some(conn) = pool.pop()
        {
            return Ok(conn);
        }
        Self::create_connection()
    }

    fn return_connection(conn: Connection) {
        if let Ok(mut pool) = DB_POOL.lock()
            && pool.len() < 10
        {
            pool.push(conn);
        }
    }

    fn create_connection() -> GisResult<Connection> {
        let db_path = resource_dir().join("regions.db");

        if !db_path.exists() {
            return Err(GisError::Dataset(
                "Regions database not found. Please rebuild it.".to_string(),
            ));
        }
        let conn = Connection::open(&db_path)
            .map_err(|e| GisError::Dataset(format!("Failed to open database: {}", e)))?;
        conn.execute_batch(PRAGMA_INIT)?;
        Ok(conn)
    }

    #[inline]
    fn parse_wkt(wkt_str: &str) -> GisResult<Geometry> {
        let wkt = wkt::Wkt::from_str(wkt_str)
            .map_err(|e| GisError::InvalidGeometry(format!("WKT parse error: {:?}", e)))?;
        wkt.try_into()
            .map_err(|e| GisError::InvalidGeometry(format!("Geometry conversion error: {:?}", e)))
    }

    pub fn find_intersecting_regions(bbox: &BoundingBox) -> GisResult<Vec<(String, String)>> {
        let conn = Self::get_connection()?;
        let results = {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT code, name, geometry FROM regions 
                 WHERE bbox_xmax >= ?1 AND bbox_xmin <= ?2 
                 AND bbox_ymax >= ?3 AND bbox_ymin <= ?4",
                )
                .map_err(|e| GisError::Dataset(e.to_string()))?;
            let bbox_geom = bbox.to_geometry();
            stmt.query_map(params![bbox.xmin, bbox.xmax, bbox.ymin, bbox.ymax], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .filter_map(|result| {
                result.ok().and_then(|(code, name, wkt_str)| {
                    Self::parse_wkt(&wkt_str)
                        .ok()
                        .filter(|geom| geom.intersects(&bbox_geom))
                        .map(|_| (code, name))
                })
            })
            .collect()
        };
        Self::return_connection(conn);
        Ok(results)
    }

    pub fn get_neighbors(region_code: &str) -> GisResult<Vec<String>> {
        let conn = Self::get_connection()?;
        let neighbors = {
            let mut stmt = conn
                .prepare_cached("SELECT neighbor_code FROM region_neighbors WHERE region_code = ?")
                .map_err(|e| GisError::Dataset(e.to_string()))?;
            stmt.query_map(params![region_code], |row| row.get(0))
                .map_err(|e| GisError::Dataset(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| GisError::Dataset(e.to_string()))?
        };
        Self::return_connection(conn);
        Ok(neighbors)
    }

    pub fn get_region(region_code: &str) -> GisResult<Option<(String, String, Option<Geometry>)>> {
        let conn = Self::get_connection()?;
        let result = conn.query_row(
            "SELECT code, name, geometry FROM regions WHERE code = ?",
            params![region_code],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, String>(2)?)),
        );
        let output = match result {
            Ok((code, name, wkt_str)) => {
                let geom = Self::parse_wkt(&wkt_str).ok();
                Some((code, name, geom))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                Self::return_connection(conn);
                return Err(GisError::Dataset(e.to_string()));
            }
        };
        Self::return_connection(conn);
        Ok(output)
    }

    pub fn check_database() -> GisResult<bool> {
        let conn = Self::get_connection()?;
        let version_ok = conn
            .query_row(
                "SELECT value FROM metadata WHERE key = 'version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .is_ok();
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM regions", [], |row| row.get(0))
            .map_err(|e| GisError::Dataset(e.to_string()))?;
        Self::return_connection(conn);
        Ok(version_ok && count > 0)
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
            properties: properties.as_object().cloned(),
            foreign_members: None,
        };
        let feature_collection = geojson::FeatureCollection {
            bbox: None,
            features: vec![feature],
            foreign_members: serde_json::json!({
                "crs": {
                    "type": "name",
                    "properties": {"name": "EPSG:2154"}
                }
            })
            .as_object()
            .cloned(),
        };
        let geojson_string = geojson::GeoJson::FeatureCollection(feature_collection).to_string();
        fs::write(output_path, geojson_string)?;
        Ok(())
    }

    pub async fn create_region_file(region: &Region) -> GisResult<()> {
        let geojson_path = temp_dir().join(format!("{}.geojson", region.code()));
        let gpkg_path = geojson_path.with_extension("gpkg");

        Self::create_region_geojson(region.code(), geojson_path.to_str().unwrap())?;
        VectorService::convert_to_gpkg(geojson_path.to_str().unwrap(), gpkg_path.to_str().unwrap())
            .await?;

        Ok(())
    }
}
