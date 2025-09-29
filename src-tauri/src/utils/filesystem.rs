use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::utils::temp_dir;

pub fn create_directory_if_not_exists(path: &str) -> Result<()> {
    if !Path::new(path).exists() {
        fs::create_dir_all(path)?;
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
            for entry in fs::read_dir(&tmp_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    fs::remove_dir_all(&path)?;
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
                    fs::remove_file(&path)?;
                }
            }
        }
        None => {
            fs::remove_dir_all(&tmp_dir)?;
            fs::create_dir(&tmp_dir)?;
        }
    }

    Ok(())
}

pub fn find_files_by_basename(
    dir: &Path,
    target_basename: &str,
    result: &mut Vec<PathBuf>,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_file() {
            if let Some(file_stem) = path.file_stem()
                && file_stem.to_string_lossy() == target_basename
            {
                result.push(path);
            }
        } else if path.is_dir() {
            find_files_by_basename(&path, target_basename, result)?;
        }
    }

    Ok(())
}

pub fn file_exists_in_dir(dir: &Path, filename: &str) -> bool {
    dir.join(filename).exists()
}

pub fn list_files_with_extension(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let target_ext = extension.trim_start_matches('.');

    if !dir.exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext.to_string_lossy() == target_ext
        {
            files.push(path);
        }
    }

    Ok(files)
}

pub fn copy_file_with_dirs(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination)?;
    Ok(())
}

pub fn move_file_with_dirs(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    if fs::rename(source, destination).is_err() {
        fs::copy(source, destination)?;
        fs::remove_file(source)?;
    }

    Ok(())
}

pub fn remove_file_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

pub fn get_file_size(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path)?;
    Ok(metadata.len())
}

pub fn is_dir_empty(path: &Path) -> Result<bool> {
    if !path.is_dir() {
        return Ok(false);
    }

    let mut entries = fs::read_dir(path)?;
    Ok(entries.next().is_none())
}

pub fn ensure_extension(path: &Path, extension: &str) -> PathBuf {
    let ext = extension.trim_start_matches('.');

    if let Some(current_ext) = path.extension()
        && current_ext == ext
    {
        return path.to_path_buf();
    }

    path.with_extension(ext)
}

pub fn get_filename_without_extension(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

pub fn get_directory_size(path: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                total_size += get_directory_size(&entry_path)?;
            } else {
                total_size += fs::metadata(&entry_path)?.len();
            }
        }
    }

    Ok(total_size)
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}
