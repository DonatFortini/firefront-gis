use crate::error::ConfigError;
use crate::utils::{OUTPUT_DIR, resolve_resource_dir};
use rusqlite::{Connection, Result as SqliteResult, params};
use serde::{Deserialize, Serialize};
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use tauri::{AppHandle, Manager};

const CACHE_DIR: &str = "cache";
const WMS_CACHE_DIR: &str = "wms";
const PROJECTS_DIR: &str = "projects";
const TEMP_DIR: &str = "tmp";
const RESOURCES_DIR: &str = "resources";
const REGIONS_GRAPH_FILE: &str = "regions_graph.json";
const DEFAULT_RESOLUTION: f64 = 10.0;
const DEFAULT_SLICE_FACTOR: u32 = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    pub url: String,
    pub storage_name: String,
    pub prefix: Option<String>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataSources {
    sources: Vec<DataSource>,
}

impl DataSources {
    pub fn new() -> Self {
        Self {
            sources: Vec::from([
                DataSource {
                    url: "https://geoservices.ign.fr/bdtopo".to_string(),
                    storage_name: "BDTOPO".to_string(),
                    prefix: Some("D0".to_string()),
                    keyword: None,
                },
                DataSource {
                    url: "https://geoservices.ign.fr/bdforet".to_string(),
                    storage_name: "BDFORET".to_string(),
                    prefix: Some("D0".to_string()),
                    keyword: Some("BDFORET_2-0".to_string()),
                },
                DataSource {
                    url: "https://geoservices.ign.fr/rpg".to_string(),
                    storage_name: "RPG".to_string(),
                    prefix: Some("R".to_string()),
                    keyword: None,
                },
                DataSource {
                    url: "https://geoservices.ign.fr/rgealti".to_string(),
                    storage_name: "RGEALTI".to_string(),
                    prefix: Some("D0".to_string()),
                    keyword: Some("RGEALTI_2-0_5M".to_string()),
                },
                DataSource {
                    url: "https://data.geopf.fr/wms-r/wms?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=ORTHOIMAGERY.ORTHOPHOTOS&STYLES=&CRS=EPSG:2154&".to_string(),
                    storage_name: "ORTHOIMAGERY".to_string(),
                    prefix: None,
                    keyword: None,
                },
            ]),
        }
    }

    pub fn add_source(
        &mut self,
        url: String,
        storage_name: String,
        prefix: Option<String>,
        keyword: Option<String>,
    ) {
        self.sources.push(DataSource {
            url,
            storage_name,
            prefix,
            keyword,
        });
    }

    pub fn get_sources(&self) -> &Vec<DataSource> {
        &self.sources
    }

    pub fn get_source_by_name(&self, name: &str) -> Option<&DataSource> {
        self.sources.iter().find(|s| s.storage_name == name)
    }
}

type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub cache_dir: PathBuf,
    pub wms_cache_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub resource_dir: PathBuf,
    pub resolution: f64,
    pub slice_factor: u32,
    pub output_location: PathBuf,
    pub data_sources: DataSources,
    #[serde(skip)]
    pub handle: Option<AppHandle>,
    #[serde(skip)]
    db_path: PathBuf,
    #[serde(skip)]
    app_data_dir: PathBuf,
}

static CONFIG_INSTANCE: OnceLock<Arc<RwLock<AppConfig>>> = OnceLock::new();

impl AppConfig {
    pub fn init(app_handle: AppHandle) -> Result<()> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|_| ConfigError::ConfigDirNotFound)?;

        let db_path = app_data_dir.join("config.db");
        let resource_dir = resolve_resource_dir(&app_handle, RESOURCES_DIR)
            .unwrap_or_else(|_| app_data_dir.join(RESOURCES_DIR));

        let config = Self::new(resource_dir, db_path, app_data_dir, Some(app_handle))?;
        CONFIG_INSTANCE
            .set(Arc::new(RwLock::new(config)))
            .map_err(|_| {
                ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "Config already initialized",
                ))
            })?;
        Ok(())
    }

    fn new(
        resource_dir: PathBuf,
        db_path: PathBuf,
        app_data_dir: PathBuf,
        handle: Option<AppHandle>,
    ) -> Result<Self> {
        create_dir_all(db_path.parent().unwrap_or(&PathBuf::from(".")))?;

        let mut config = Self {
            cache_dir: app_data_dir.join(CACHE_DIR),
            wms_cache_dir: app_data_dir.join(CACHE_DIR).join(WMS_CACHE_DIR),
            projects_dir: app_data_dir.join(PROJECTS_DIR),
            temp_dir: app_data_dir.join(TEMP_DIR),
            resource_dir,
            resolution: DEFAULT_RESOLUTION,
            slice_factor: DEFAULT_SLICE_FACTOR,
            output_location: OUTPUT_DIR.lock().unwrap().clone(),
            data_sources: DataSources::new(),
            handle,
            db_path,
            app_data_dir,
        };

        config.initialize_and_load()?;
        Ok(config)
    }

    fn initialize_and_load(&mut self) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        self.initialize_database(&conn)?;
        self.load_from_database(&conn)?;
        Ok(())
    }

    fn initialize_database(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        let path_fields = [
            ("cache_dir", &self.cache_dir),
            ("wms_cache_dir", &self.wms_cache_dir),
            ("projects_dir", &self.projects_dir),
            ("temp_dir", &self.temp_dir),
            ("resource_dir", &self.resource_dir),
            ("output_location", &self.output_location),
        ];

        for (key, path) in path_fields {
            if !Self::config_key_exists(conn, key)? {
                let absolute_path = path.to_string_lossy().to_string();
                Self::set_config_value(conn, key, &absolute_path)?;
            }
        }

        let special_fields = [
            ("resolution", self.resolution.to_string()),
            ("slice_factor", self.slice_factor.to_string()),
        ];

        for (key, value) in special_fields {
            if !Self::config_key_exists(conn, key)? {
                Self::set_config_value(conn, key, &value)?;
            }
        }
        Ok(())
    }

    fn load_from_database(&mut self, conn: &Connection) -> Result<()> {
        if let Ok(value) = Self::get_config_value(conn, "cache_dir") {
            self.cache_dir = PathBuf::from(value);
        }
        if let Ok(value) = Self::get_config_value(conn, "wms_cache_dir") {
            self.wms_cache_dir = PathBuf::from(value);
        }
        if let Ok(value) = Self::get_config_value(conn, "projects_dir") {
            self.projects_dir = PathBuf::from(value);
        }
        if let Ok(value) = Self::get_config_value(conn, "temp_dir") {
            self.temp_dir = PathBuf::from(value);
        }
        if let Ok(value) = Self::get_config_value(conn, "resource_dir") {
            self.resource_dir = PathBuf::from(value);
        }
        if let Ok(value) = Self::get_config_value(conn, "output_location") {
            self.output_location = PathBuf::from(value);
        }
        if let Ok(value) = Self::get_config_value(conn, "resolution")
            && let Ok(resolution) = value.parse::<f64>()
        {
            self.resolution = resolution;
        }
        if let Ok(value) = Self::get_config_value(conn, "slice_factor")
            && let Ok(slice_factor) = value.parse::<u32>()
        {
            self.slice_factor = slice_factor;
        }
        Ok(())
    }

    fn save_to_database(&self) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;

        let path_fields = [
            ("cache_dir", &self.cache_dir),
            ("wms_cache_dir", &self.wms_cache_dir),
            ("projects_dir", &self.projects_dir),
            ("temp_dir", &self.temp_dir),
            ("resource_dir", &self.resource_dir),
            ("output_location", &self.output_location),
        ];

        for (key, path) in path_fields {
            let value = path.to_string_lossy().to_string();
            Self::set_config_value(&conn, key, &value)?;
        }

        Self::set_config_value(&conn, "resolution", &self.resolution.to_string())?;
        Self::set_config_value(&conn, "slice_factor", &self.slice_factor.to_string())?;

        Ok(())
    }

    fn config_key_exists(conn: &Connection, key: &str) -> SqliteResult<bool> {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM config WHERE key = ?1)",
            params![key],
            |row| row.get(0),
        )
    }

    fn get_config_value(conn: &Connection, key: &str) -> SqliteResult<String> {
        conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
    }

    fn set_config_value(conn: &Connection, key: &str, value: &str) -> SqliteResult<()> {
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn with_read<F, R>(f: F) -> R
    where
        F: FnOnce(&AppConfig) -> R,
    {
        let instance = CONFIG_INSTANCE
            .get()
            .expect("Config not initialized. Call AppConfig::init() first.");
        let config = instance.read().unwrap();
        f(&config)
    }

    pub fn with_write<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&mut AppConfig) -> Result<R>,
    {
        let instance = CONFIG_INSTANCE
            .get()
            .expect("Config not initialized. Call AppConfig::init() first.");
        let mut config = instance.write().unwrap();
        let result = f(&mut config)?;
        config.save_to_database()?;
        Ok(result)
    }

    pub fn update(&mut self, output_location: Option<String>) -> Result<()> {
        if let Some(output) = output_location {
            let path = PathBuf::from(output);
            self.output_location = if path.is_absolute() {
                path
            } else {
                self.app_data_dir.join(path)
            };
        }
        Ok(())
    }

    pub fn regions_graph_path(&self) -> PathBuf {
        self.resource_dir.join(REGIONS_GRAPH_FILE)
    }
}

pub fn get_config<F, R>(f: F) -> R
where
    F: FnOnce(&AppConfig) -> R,
{
    AppConfig::with_read(f)
}

pub fn update_config<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut AppConfig) -> Result<R>,
{
    AppConfig::with_write(f)
}
