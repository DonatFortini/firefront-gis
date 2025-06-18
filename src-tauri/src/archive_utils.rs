use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// External crate imports
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use sevenz_rust2::SevenZReader;
use tar::Archive;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::FileOptions};

/// Custom error type for archive operations
#[derive(Debug)]
pub enum ArchiveError {
    IoError(io::Error),
    UnsupportedFormat(String),
    ExtractionError(String),
    CompressionError(String),
    CommandError(String),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::IoError(e) => write!(f, "IO error: {}", e),
            ArchiveError::UnsupportedFormat(format) => write!(f, "Unsupported format: {}", format),
            ArchiveError::ExtractionError(msg) => write!(f, "Extraction error: {}", msg),
            ArchiveError::CompressionError(msg) => write!(f, "Compression error: {}", msg),
            ArchiveError::CommandError(msg) => write!(f, "Command error: {}", msg),
        }
    }
}

impl Error for ArchiveError {}

impl From<io::Error> for ArchiveError {
    fn from(error: io::Error) -> Self {
        ArchiveError::IoError(error)
    }
}

/// Supported archive formats
#[derive(Debug, PartialEq, Clone)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    TarGz,
    TarBz2,
    Tar,
}

impl ArchiveFormat {
    /// Detect format from file extension
    pub fn from_path(path: &Path) -> Result<Self, ArchiveError> {
        let path_str = path.to_string_lossy().to_lowercase();

        if path_str.ends_with(".zip") {
            Ok(ArchiveFormat::Zip)
        } else if path_str.ends_with(".7z") {
            Ok(ArchiveFormat::SevenZ)
        } else if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
            Ok(ArchiveFormat::TarGz)
        } else if path_str.ends_with(".tar.bz2") || path_str.ends_with(".tbz2") {
            Ok(ArchiveFormat::TarBz2)
        } else if path_str.ends_with(".tar") {
            Ok(ArchiveFormat::Tar)
        } else {
            Err(ArchiveError::UnsupportedFormat(path_str.clone()))
        }
    }
}

/// Configuration for performance optimization
pub struct ArchiveConfig {
    pub use_native_7z: bool,
    pub buffer_size: usize,
    pub compression_level: i32,
    pub multithread: bool,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            use_native_7z: true,    // Prefer native 7z for performance
            buffer_size: 64 * 1024, // 64KB buffer
            compression_level: 6,   // Balanced compression
            multithread: true,
        }
    }
}

/// Main archive helper struct
#[derive(Default)]
pub struct ArchiveHelper {
    config: ArchiveConfig,
}

impl ArchiveHelper {
    pub fn new(config: ArchiveConfig) -> Self {
        Self { config }
    }

    /// Check if native 7z is available
    fn is_7z_available() -> bool {
        Command::new("7z").arg("--help").output().is_ok()
    }

    /// Compress a folder into a ZIP archive with optimizations
    pub fn compress_folder(
        &self,
        source_folder_path: &str,
        output_zip_name: &str,
        destination_directory: &str,
    ) -> Result<(), ArchiveError> {
        let output_path = format!("{}/{}.zip", destination_directory, output_zip_name);
        self.create_zip_from_directory_optimized(source_folder_path, &output_path)
    }

    /// Extract files by basename - use native 7z when possible
    pub fn extract_files_by_name(
        &self,
        archive_path: &str,
        target_filename: &str,
        output_dir: &str,
    ) -> Result<(), ArchiveError> {
        let archive_path_obj = Path::new(archive_path);
        let format = ArchiveFormat::from_path(archive_path_obj)?;

        match format {
            ArchiveFormat::SevenZ if self.config.use_native_7z && Self::is_7z_available() => {
                self.extract_7z_native(archive_path, output_dir, Some(target_filename))
            }
            ArchiveFormat::Zip => {
                self.extract_files_from_zip_by_name(archive_path, target_filename, output_dir)
            }
            _ => {
                // For other formats, still need conversion but optimize it
                self.extract_with_conversion(archive_path, target_filename, output_dir)
            }
        }
    }

    /// Extract entire archive - use native tools when possible
    pub fn extract_archive(
        &self,
        archive_path: &str,
        output_dir: &str,
    ) -> Result<(), ArchiveError> {
        let archive_path_obj = Path::new(archive_path);
        let format = ArchiveFormat::from_path(archive_path_obj)?;

        match format {
            ArchiveFormat::SevenZ if self.config.use_native_7z && Self::is_7z_available() => {
                self.extract_7z_native(archive_path, output_dir, None)
            }
            _ => self.extract_archive_rust(archive_path, output_dir),
        }
    }

    // Optimized implementations

    fn extract_7z_native(
        &self,
        archive_path: &str,
        output_dir: &str,
        specific_file: Option<&str>,
    ) -> Result<(), ArchiveError> {
        fs::create_dir_all(output_dir)?;

        let mut cmd = Command::new("7z");
        cmd.arg("x") // Extract with full paths
            .arg(archive_path)
            .arg(format!("-o{}", output_dir));

        if self.config.multithread {
            cmd.arg("-mmt=on");
        }

        if let Some(file_pattern) = specific_file {
            cmd.arg(format!("*{}*", file_pattern));
        }

        let output = cmd
            .output()
            .map_err(|e| ArchiveError::CommandError(format!("Failed to run 7z: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ArchiveError::ExtractionError(format!(
                "7z extraction failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn create_zip_from_directory_optimized(
        &self,
        source_dir: &str,
        output_path: &str,
    ) -> Result<(), ArchiveError> {
        if let Some(parent) = Path::new(output_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let file = File::create(output_path)?;
        let buf_writer = BufWriter::new(file);
        let mut zip = ZipWriter::new(buf_writer);

        let options = FileOptions::<()>::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(self.config.compression_level.into()))
            .unix_permissions(0o755);

        let source_path = Path::new(source_dir);
        let files = self.collect_files_recursive(source_path)?;

        for file_path in files {
            let relative_path = file_path
                .strip_prefix(source_path)
                .map_err(|_| ArchiveError::CompressionError("Invalid file path".to_string()))?;
            let name = relative_path.to_string_lossy();

            zip.start_file(name, options).map_err(|e| {
                ArchiveError::CompressionError(format!("Failed to start file: {}", e))
            })?;

            // OPTIMIZED: Stream file instead of loading into memory
            let file = File::open(&file_path)?;
            let mut buf_reader = BufReader::new(file);
            let mut buffer = vec![0; self.config.buffer_size];

            loop {
                let bytes_read = buf_reader.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                zip.write_all(&buffer[..bytes_read]).map_err(|e| {
                    ArchiveError::CompressionError(format!("Failed to write file chunk: {}", e))
                })?;
            }
        }

        zip.finish()
            .map_err(|e| ArchiveError::CompressionError(format!("Failed to finish ZIP: {}", e)))?;
        Ok(())
    }

    fn extract_archive_rust(
        &self,
        archive_path: &str,
        output_dir: &str,
    ) -> Result<(), ArchiveError> {
        let archive_path_obj = Path::new(archive_path);
        let format = ArchiveFormat::from_path(archive_path_obj)?;

        fs::create_dir_all(output_dir)?;

        match format {
            ArchiveFormat::Zip => self.extract_zip_optimized(archive_path, output_dir),
            ArchiveFormat::SevenZ => self.extract_7z_rust(archive_path, output_dir),
            ArchiveFormat::TarGz => self.extract_tar_gz(archive_path, output_dir),
            ArchiveFormat::TarBz2 => self.extract_tar_bz2(archive_path, output_dir),
            ArchiveFormat::Tar => self.extract_tar(archive_path, output_dir),
        }
    }

    fn extract_zip_optimized(
        &self,
        archive_path: &str,
        output_dir: &str,
    ) -> Result<(), ArchiveError> {
        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| ArchiveError::ExtractionError(format!("Failed to open ZIP: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                ArchiveError::ExtractionError(format!("Failed to read entry {}: {}", i, e))
            })?;

            let outpath = Path::new(output_dir).join(file.name());

            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }

                // OPTIMIZED: Stream extraction instead of loading into memory
                let outfile = File::create(&outpath)?;
                let mut buf_writer = BufWriter::new(outfile);
                let mut buffer = vec![0; self.config.buffer_size];

                loop {
                    let bytes_read = file.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    buf_writer.write_all(&buffer[..bytes_read])?;
                }
                buf_writer.flush()?;
            }
        }
        Ok(())
    }

    fn extract_with_conversion(
        &self,
        archive_path: &str,
        target_filename: &str,
        output_dir: &str,
    ) -> Result<(), ArchiveError> {
        // Use streaming approach for conversion when possible
        let temp_zip = self.create_temp_zip_path();

        // Try to use native 7z for conversion if available
        if Self::is_7z_available() {
            let mut cmd = Command::new("7z");
            cmd.arg("a").arg("-tzip").arg(&temp_zip).arg(archive_path);

            let output = cmd
                .output()
                .map_err(|e| ArchiveError::CommandError(format!("Failed to run 7z: {}", e)))?;

            if output.status.success() {
                let result =
                    self.extract_files_from_zip_by_name(&temp_zip, target_filename, output_dir);
                let _ = fs::remove_file(&temp_zip);
                return result;
            }
        }

        // Fallback to Rust implementation
        self.convert_to_zip_rust(archive_path, &temp_zip)?;
        let result = self.extract_files_from_zip_by_name(&temp_zip, target_filename, output_dir);
        let _ = fs::remove_file(&temp_zip);
        result
    }

    // Keep existing methods but add optimizations...
    fn extract_7z_rust(&self, archive_path: &str, output_dir: &str) -> Result<(), ArchiveError> {
        let file = File::open(archive_path)?;
        let mut reader = SevenZReader::new(file, Default::default())
            .map_err(|e| ArchiveError::ExtractionError(format!("Failed to open 7Z: {}", e)))?;

        reader
            .for_each_entries(|entry, reader| {
                let name = entry.name();
                let output_path = Path::new(output_dir).join(name);

                if entry.is_directory() {
                    fs::create_dir_all(&output_path)?;
                } else {
                    if let Some(parent) = output_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    // OPTIMIZED: Use buffered writer
                    let output_file = File::create(&output_path)?;
                    let mut buf_writer = BufWriter::new(output_file);
                    io::copy(reader, &mut buf_writer)?;
                    buf_writer.flush()?;
                }
                Ok(true)
            })
            .map_err(|e| ArchiveError::ExtractionError(format!("7Z extraction failed: {}", e)))?;

        Ok(())
    }

    // ... (include other existing methods with similar optimizations)

    fn collect_files_recursive(&self, dir: &Path) -> Result<Vec<PathBuf>, ArchiveError> {
        let mut files = Vec::new();

        fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), ArchiveError> {
            if dir.is_dir() {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.is_dir() {
                        walk_dir(&path, files)?;
                    } else if path.is_file() {
                        files.push(path);
                    }
                }
            }
            Ok(())
        }

        walk_dir(dir, &mut files)?;
        Ok(files)
    }

    fn extract_files_from_zip_by_name(
        &self,
        archive_path: &str,
        target_filename: &str,
        output_dir: &str,
    ) -> Result<(), ArchiveError> {
        fs::create_dir_all(output_dir)?;

        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| ArchiveError::ExtractionError(format!("Failed to open ZIP: {}", e)))?;

        let destination = Path::new(output_dir).join(target_filename);
        fs::create_dir_all(&destination)?;

        let mut found_files = 0;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                ArchiveError::ExtractionError(format!("Failed to read entry {}: {}", i, e))
            })?;
            let file_path = file.name();

            if let Some(filename) = Path::new(file_path).file_name() {
                if let Some(file_stem) = Path::new(filename).file_stem() {
                    if file_stem.to_string_lossy() == target_filename {
                        let dest_path = destination.join(filename);

                        if file.is_file() {
                            let dest_file = File::create(dest_path)?;
                            let mut buf_writer = BufWriter::new(dest_file);
                            io::copy(&mut file, &mut buf_writer)?;
                            buf_writer.flush()?;
                            found_files += 1;
                        }
                    }
                }
            }
        }

        if found_files == 0 {
            return Err(ArchiveError::ExtractionError(format!(
                "No files matching '{}' found in archive",
                target_filename
            )));
        }

        Ok(())
    }

    fn convert_to_zip_rust(&self, input_path: &str, output_path: &str) -> Result<(), ArchiveError> {
        let temp_dir = self.create_temp_dir()?;
        self.extract_archive_rust(input_path, &temp_dir)?;
        self.create_zip_from_directory_optimized(&temp_dir, output_path)?;
        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    fn extract_tar_gz(&self, archive_path: &str, output_dir: &str) -> Result<(), ArchiveError> {
        let file = File::open(archive_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive.unpack(output_dir).map_err(|e| {
            ArchiveError::ExtractionError(format!("TAR.GZ extraction failed: {}", e))
        })?;
        Ok(())
    }

    fn extract_tar_bz2(&self, archive_path: &str, output_dir: &str) -> Result<(), ArchiveError> {
        let file = File::open(archive_path)?;
        let decoder = BzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive.unpack(output_dir).map_err(|e| {
            ArchiveError::ExtractionError(format!("TAR.BZ2 extraction failed: {}", e))
        })?;
        Ok(())
    }

    fn extract_tar(&self, archive_path: &str, output_dir: &str) -> Result<(), ArchiveError> {
        let file = File::open(archive_path)?;
        let mut archive = Archive::new(file);
        archive
            .unpack(output_dir)
            .map_err(|e| ArchiveError::ExtractionError(format!("TAR extraction failed: {}", e)))?;
        Ok(())
    }

    fn create_temp_dir(&self) -> Result<String, ArchiveError> {
        let temp_dir = std::env::temp_dir().join(format!("archive_temp_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        Ok(temp_dir.to_string_lossy().to_string())
    }

    fn create_temp_zip_path(&self) -> String {
        std::env::temp_dir()
            .join(format!("temp_archive_{}.zip", std::process::id()))
            .to_string_lossy()
            .to_string()
    }
}

// Public convenience functions with default optimized config

/// Create an optimized archive helper
pub fn create_optimized_helper() -> ArchiveHelper {
    ArchiveHelper::new(ArchiveConfig::default())
}

/// Compress a folder to ZIP format (optimized)
pub fn compress_folder(
    source_folder_path: &str,
    output_zip_name: &str,
    destination_directory: &str,
) -> Result<(), Box<dyn Error>> {
    let helper = create_optimized_helper();
    helper
        .compress_folder(source_folder_path, output_zip_name, destination_directory)
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

/// Extract files by basename (optimized)
pub fn extract_files_by_name(
    archive_path: &str,
    target_filename: &str,
    output_dir: &str,
) -> Result<(), Box<dyn Error>> {
    let helper = create_optimized_helper();
    helper
        .extract_files_by_name(archive_path, target_filename, output_dir)
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

/// Extract entire archive (optimized)
pub fn extract_archive(archive_path: &str, output_dir: &str) -> Result<(), Box<dyn Error>> {
    let helper = create_optimized_helper();
    helper
        .extract_archive(archive_path, output_dir)
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}
