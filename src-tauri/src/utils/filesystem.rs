use std::fs;
use std::path::Path;

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
