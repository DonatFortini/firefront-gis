use crate::gis_operation::regions::build_regions_graph;
use crate::utils::{OUTPUT_DIR, create_directory_if_not_exists};
use rusqlite::{Connection, Result as SqliteResult, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use tauri::{AppHandle, Manager};
use thiserror::Error;

const CACHE_DIR: &str = "projects/cache";
const PROJECTS_DIR: &str = "projects";
const TEMP_DIR: &str = "tmp";
const RESOURCES_DIR: &str = "resources";
const REGIONS_GRAPH_FILE: &str = "regions_graph.json";
const DEFAULT_RESOLUTION: f64 = 10.0;
const DEFAULT_SLICE_FACTOR: u32 = 500;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Configuration directory not found")]
    ConfigDirNotFound,
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to resolve resource path '{path}': {source}")]
    ResourcePathResolution {
        path: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("GIS operation error: {0}")]
    GisOperation(String),
}

type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub cache_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub resource_dir: PathBuf,
    pub resolution: f64,
    pub slice_factor: u32,
    pub output_location: PathBuf,
    pub gdal_path: Option<PathBuf>,
    #[serde(skip)]
    pub handle: Option<AppHandle>,
    #[serde(skip)]
    db_path: PathBuf,
}

static CONFIG_INSTANCE: OnceLock<Arc<RwLock<AppConfig>>> = OnceLock::new();

impl Default for AppConfig {
    fn default() -> Self {
        Self::with_resource_dir(PathBuf::from(RESOURCES_DIR))
    }
}

impl AppConfig {
    pub fn init(app_handle: AppHandle) -> Result<()> {
        let config = Self::new(app_handle)?;
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

    fn new(app_handle: AppHandle) -> Result<Self> {
        let db_path = Self::get_database_path(&app_handle)?;
        let resource_dir = resolve_resource_dir(&app_handle, RESOURCES_DIR)
            .unwrap_or_else(|_| PathBuf::from(RESOURCES_DIR));

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut config = Self::with_resource_dir_and_db(resource_dir, db_path);
        config.handle = Some(app_handle);
        config.initialize_database()?;
        config.load_from_database()?;
        Ok(config)
    }

    fn get_database_path(app_handle: &AppHandle) -> Result<PathBuf> {
        Ok(app_handle
            .path()
            .app_data_dir()
            .map_err(|_| ConfigError::ConfigDirNotFound)?
            .join("config.db"))
    }

    fn with_resource_dir(resource_dir: PathBuf) -> Self {
        Self::with_resource_dir_and_db(resource_dir, PathBuf::from("config.db"))
    }

    fn with_resource_dir_and_db(resource_dir: PathBuf, db_path: PathBuf) -> Self {
        Self {
            cache_dir: PathBuf::from(CACHE_DIR),
            projects_dir: PathBuf::from(PROJECTS_DIR),
            temp_dir: PathBuf::from(TEMP_DIR),
            resource_dir,
            resolution: DEFAULT_RESOLUTION,
            slice_factor: DEFAULT_SLICE_FACTOR,
            output_location: OUTPUT_DIR.lock().unwrap().clone(),
            gdal_path: None,
            handle: None,
            db_path,
        }
    }

    fn get_connection(&self) -> SqliteResult<Connection> {
        Connection::open(&self.db_path)
    }

    fn initialize_database(&self) -> Result<()> {
        let conn = self.get_connection()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        self.initialize_default_values(&conn)?;
        Ok(())
    }

    fn initialize_default_values(&self, conn: &Connection) -> Result<()> {
        let settings = [
            ("cache_dir", self.cache_dir.to_string_lossy().to_string()),
            (
                "projects_dir",
                self.projects_dir.to_string_lossy().to_string(),
            ),
            ("temp_dir", self.temp_dir.to_string_lossy().to_string()),
            (
                "resource_dir",
                self.resource_dir.to_string_lossy().to_string(),
            ),
            ("resolution", self.resolution.to_string()),
            ("slice_factor", self.slice_factor.to_string()),
            (
                "output_location",
                self.output_location.to_string_lossy().to_string(),
            ),
        ];

        for (key, value) in settings.iter() {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM config WHERE key = ?1)",
                params![key],
                |row| row.get(0),
            )?;

            if !exists {
                conn.execute(
                    "INSERT INTO config (key, value) VALUES (?1, ?2)",
                    params![key, value],
                )?;
            }
        }

        Ok(())
    }

    fn load_from_database(&mut self) -> Result<()> {
        let conn = self.get_connection()?;

        if let Ok(value) = conn.query_row(
            "SELECT value FROM config WHERE key = 'cache_dir'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            self.cache_dir = PathBuf::from(value);
        }

        if let Ok(value) = conn.query_row(
            "SELECT value FROM config WHERE key = 'projects_dir'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            self.projects_dir = PathBuf::from(value);
        }

        if let Ok(value) = conn.query_row(
            "SELECT value FROM config WHERE key = 'temp_dir'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            self.temp_dir = PathBuf::from(value);
        }

        if let Ok(value) = conn.query_row(
            "SELECT value FROM config WHERE key = 'resource_dir'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            self.resource_dir = PathBuf::from(value);
        }

        if let Ok(value) = conn.query_row(
            "SELECT value FROM config WHERE key = 'resolution'",
            [],
            |row| row.get::<_, String>(0),
        ) && let Ok(resolution) = value.parse::<f64>()
        {
            self.resolution = resolution;
        }

        if let Ok(value) = conn.query_row(
            "SELECT value FROM config WHERE key = 'slice_factor'",
            [],
            |row| row.get::<_, String>(0),
        ) && let Ok(slice_factor) = value.parse::<u32>()
        {
            self.slice_factor = slice_factor;
        }

        if let Ok(value) = conn.query_row(
            "SELECT value FROM config WHERE key = 'output_location'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            self.output_location = PathBuf::from(value);
        }

        if let Ok(value) = conn.query_row(
            "SELECT value FROM config WHERE key = 'gdal_path'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            self.gdal_path = Some(PathBuf::from(value));
        }

        Ok(())
    }

    fn save_to_database(&self) -> Result<()> {
        let conn = self.get_connection()?;

        let settings = [
            ("cache_dir", self.cache_dir.to_string_lossy().to_string()),
            (
                "projects_dir",
                self.projects_dir.to_string_lossy().to_string(),
            ),
            ("temp_dir", self.temp_dir.to_string_lossy().to_string()),
            (
                "resource_dir",
                self.resource_dir.to_string_lossy().to_string(),
            ),
            ("resolution", self.resolution.to_string()),
            ("slice_factor", self.slice_factor.to_string()),
            (
                "output_location",
                self.output_location.to_string_lossy().to_string(),
            ),
        ];

        for (key, value) in settings.iter() {
            conn.execute(
                "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }

        if let Some(ref gdal_path) = self.gdal_path {
            conn.execute(
                "INSERT OR REPLACE INTO config (key, value) VALUES ('gdal_path', ?1)",
                params![gdal_path.to_string_lossy().to_string()],
            )?;
        } else {
            conn.execute("DELETE FROM config WHERE key = 'gdal_path'", [])?;
        }

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

    pub fn update(
        &mut self,
        output_location: Option<String>,
        gdal_path: Option<String>,
    ) -> Result<()> {
        if let Some(output) = output_location {
            self.output_location = PathBuf::from(output);
        }
        self.gdal_path = gdal_path.map(PathBuf::from);
        self.save_to_database()?;
        Ok(())
    }

    pub fn regions_graph_path(&self) -> PathBuf {
        self.resource_dir.join(REGIONS_GRAPH_FILE)
    }

    pub fn required_dirs(&self) -> Vec<&PathBuf> {
        vec![&self.cache_dir, &self.temp_dir]
    }
}

fn resolve_resource_dir(app_handle: &AppHandle, resource_path: &str) -> Result<PathBuf> {
    app_handle
        .path()
        .resolve(resource_path, tauri::path::BaseDirectory::Resource)
        .map_err(|e| ConfigError::ResourcePathResolution {
            path: resource_path.to_string(),
            source: Box::new(e),
        })
}

/// Initialise l'application en créant les répertoires nécessaires et en chargeant la configuration.
///
/// # Arguments
/// * `app_handle` - Un handle vers l'application Tauri.
///
/// # Returns
/// * `Result<(), String>` - Un résultat indiquant si l'initialisation a réussi ou non.
pub fn initialize_app(app_handle: &AppHandle) -> Result<()> {
    AppConfig::init(app_handle.clone())?;

    AppConfig::with_read(|config| {
        for dir in config.required_dirs() {
            create_directory_if_not_exists(&dir.to_string_lossy())
                .map_err(|e| ConfigError::Io(std::io::Error::other(e.to_string())))?;
        }

        let regions_graph_path = config.regions_graph_path();
        let regions_graph_str = regions_graph_path.to_str().ok_or_else(|| {
            ConfigError::InvalidPath("Invalid UTF-8 in regions graph path".to_string())
        })?;

        build_regions_graph(Some(regions_graph_str))
            .map_err(|e| ConfigError::GisOperation(e.to_string()))?;

        Ok(())
    })
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
