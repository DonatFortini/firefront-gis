use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::error::{AppError, Result};
use crate::utils::temp_dir;

pub fn create_directory_if_not_exists(path: &str) -> Result<()> {
    if !Path::new(path).exists() {
        fs::create_dir_all(path).map_err(AppError::Io)?;
    }
    Ok(())
}

pub fn clean_tmp(ignore_extension: Option<&str>) -> Result<()> {
    let tmp_dir = temp_dir();

    if !tmp_dir.exists() {
        return Ok(());
    }

    match ignore_extension {
        Some(ext) => {
            for entry in fs::read_dir(&tmp_dir).map_err(AppError::Io)? {
                let entry = entry.map_err(AppError::Io)?;
                let path = entry.path();

                if path.is_dir() {
                    fs::remove_dir_all(&path).map_err(AppError::Io)?;
                    continue;
                }

                let should_remove = path
                    .extension()
                    .map(|extension| {
                        let extension_str = extension.to_string_lossy();
                        let target_ext = ext.trim_start_matches('.');
                        extension_str != target_ext
                    })
                    .unwrap_or(true);

                if should_remove {
                    fs::remove_file(&path).map_err(AppError::Io)?;
                }
            }
        }
        None => {
            fs::remove_dir_all(&tmp_dir).map_err(AppError::Io)?;
            fs::create_dir(&tmp_dir).map_err(AppError::Io)?;
        }
    }

    Ok(())
}

#[derive(Serialize)]
pub struct CacheFileInfo {
    pub name: String,
    pub size: u64,
    pub path: String,
}

#[derive(Serialize)]
pub struct CacheInfo {
    pub total_size: u64,
    pub file_count: usize,
    pub files: Vec<CacheFileInfo>,
}

pub fn get_dir_size_and_files(dir: &Path) -> Result<(u64, Vec<CacheFileInfo>)> {
    let mut total_size = 0u64;
    let mut files = Vec::new();

    if !dir.exists() {
        return Ok((0, files));
    }

    fn visit_dirs(
        dir: &Path,
        total_size: &mut u64,
        files: &mut Vec<CacheFileInfo>,
        base_path: &Path,
    ) -> Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir).map_err(AppError::Io)? {
                let entry = entry.map_err(AppError::Io)?;
                let path = entry.path();

                if path.is_dir() {
                    visit_dirs(&path, total_size, files, base_path)?;
                } else if path.is_file() {
                    let metadata = fs::metadata(&path).map_err(AppError::Io)?;
                    let size = metadata.len();
                    *total_size += size;

                    let relative_path = path
                        .strip_prefix(base_path)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    files.push(CacheFileInfo {
                        name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        size,
                        path: relative_path,
                    });
                }
            }
        }
        Ok(())
    }

    visit_dirs(dir, &mut total_size, &mut files, dir)?;
    Ok((total_size, files))
}
