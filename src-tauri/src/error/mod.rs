use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("Data service error: {0}")]
    Data(#[from] DataError),
    #[error("Project service error: {0}")]
    Project(#[from] ProjectError),
    #[error("GIS service error: {0}")]
    Gis(#[from] GisError),
    #[error("Command execution error: {0}")]
    Command(#[from] CommandError),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Configuration directory not found")]
    ConfigDirNotFound,
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Failed to resolve resource path '{path}': {source}")]
    ResourcePathResolution {
        path: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("App handle not available")]
    NoAppHandle,
}

#[derive(Error, Debug)]
pub enum DataError {
    #[error("Download failed for '{url}': {message}")]
    DownloadFailed { url: String, message: String },
    #[error("File not found in cache: {0}")]
    CacheNotFound(PathBuf),
    #[error("No files matching '{pattern}' found in archive")]
    NoMatchingFiles { pattern: String },
    #[error("Archive extraction failed: {0}")]
    ExtractionFailed(String),
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Scraping error: {0}")]
    Scraping(String),
    #[error("Unsupported database type")]
    UnsupportedDbType,
}

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("Project '{name}' not found")]
    NotFound { name: String },
    #[error("Invalid project structure: {0}")]
    InvalidStructure(String),
    #[error("Export failed for project '{project}': {message}")]
    ExportFailed { project: String, message: String },
    #[error("Project creation failed: {0}")]
    CreationFailed(String),
    #[error("Invalid bounding box: {0}")]
    InvalidBoundingBox(String),
    #[error("No intersecting regions found")]
    NoIntersectingRegions,
}

#[derive(Error, Debug)]
pub enum GisError {
    #[error("GDAL operation failed: {operation} - {message}")]
    GdalOperation { operation: String, message: String },
    #[error("Invalid geometry: {0}")]
    InvalidGeometry(String),
    #[error("Layer '{layer}' not found")]
    LayerNotFound { layer: String },
    #[error("Rasterization failed: {0}")]
    RasterizationFailed(String),
    #[error("Merge operation failed: {0}")]
    MergeFailed(String),
    #[error("Slice operation failed: {0}")]
    SliceFailed(String),
    #[error("WMS fetch failed: {0}")]
    WmsFetchFailed(String),
    #[error("Dataset error: {0}")]
    Dataset(String),
    #[error("JSON parsing failed: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("Extent not found in output")]
    ExtentNotFound,
    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseFloatError),
    #[error("Image processing error: {0}")]
    ImageProcessing(String),
}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Command '{command}' failed with status {status}\nStdout: {stdout}\nStderr: {stderr}")]
    ExecutionFailed {
        command: String,
        status: String,
        stdout: String,
        stderr: String,
    },
    #[error("Sidecar error: {0}")]
    Sidecar(String),
    #[error("Shell plugin error: {0}")]
    ShellPlugin(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
pub type DataResult<T> = std::result::Result<T, DataError>;
pub type ProjectResult<T> = std::result::Result<T, ProjectError>;
pub type GisResult<T> = std::result::Result<T, GisError>;
pub type CommandResult<T> = std::result::Result<T, CommandError>;

impl From<Box<dyn std::error::Error>> for DataError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        DataError::ExtractionFailed(err.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for GisError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        GisError::Dataset(err.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for ProjectError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        ProjectError::CreationFailed(err.to_string())
    }
}

impl From<std::io::Error> for ProjectError {
    fn from(err: std::io::Error) -> Self {
        ProjectError::CreationFailed(err.to_string())
    }
}

impl From<std::io::Error> for DataError {
    fn from(err: std::io::Error) -> Self {
        DataError::ExtractionFailed(err.to_string())
    }
}

impl From<std::io::Error> for GisError {
    fn from(err: std::io::Error) -> Self {
        GisError::Dataset(err.to_string())
    }
}
