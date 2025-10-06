use std::path::Path;

use crate::error::{DataError, DataResult};
use crate::utils::execute_sidecar;

pub struct ArchiveService;

impl ArchiveService {
    pub async fn compress_folder(
        source_folder: &str,
        output_name: &str,
        destination: &str,
    ) -> DataResult<()> {
        let output_path = format!("{destination}/{output_name}.zip");

        execute_sidecar("_7z", &["a", &output_path, &format!("{}/*", source_folder)])
            .await
            .map_err(|e| DataError::ExtractionFailed(e.to_string()))?;

        println!("Successfully compressed '{source_folder}' to '{output_path}'");
        Ok(())
    }

    /// Extrait plusieurs fichiers d'une archive en une seule opération
    ///
    /// # Arguments
    /// * `archive_path` - Chemin vers l'archive
    /// * `target_filenames` - Liste des noms de fichiers à extraire (sans extension)
    /// * `output_dir` - Répertoire de destination
    ///
    /// # Returns
    /// HashMap associant chaque nom de fichier à son chemin d'extraction
    pub async fn extract_multiple_files(
        archive_path: &str,
        target_filenames: &[&str],
        output_dir: &str,
    ) -> DataResult<std::collections::HashMap<String, String>> {
        let output_path = Path::new(output_dir);
        let temp_extract_dir = output_path.join("temp_extract");

        std::fs::create_dir_all(output_path)?;
        std::fs::create_dir_all(&temp_extract_dir)?;

        execute_sidecar(
            "_7z",
            &[
                "x",
                archive_path,
                &format!("-o{}", temp_extract_dir.display()),
                "-y",
            ],
        )
        .await
        .map_err(|e| DataError::ExtractionFailed(e.to_string()))?;

        let mut extracted_files = std::collections::HashMap::new();

        for &target_filename in target_filenames {
            let mut found_files = Vec::new();
            Self::find_files_recursive(&temp_extract_dir, target_filename, &mut found_files)?;

            if found_files.is_empty() {
                println!("Warning: No files found for '{}'", target_filename);
                continue;
            }

            let destination = output_path.join(target_filename);
            std::fs::create_dir_all(&destination)?;

            for file_path in found_files {
                if let Some(file_name) = file_path.file_name() {
                    let dest_file = destination.join(file_name);
                    std::fs::copy(&file_path, &dest_file)?;
                }
            }

            extracted_files.insert(
                target_filename.to_string(),
                destination.to_string_lossy().to_string(),
            );
        }

        std::fs::remove_dir_all(temp_extract_dir)?;

        if extracted_files.is_empty() {
            return Err(DataError::NoMatchingFiles {
                pattern: target_filenames.join(", "),
            });
        }

        Ok(extracted_files)
    }

    pub async fn extract_files_by_name(
        archive_path: &str,
        target_filename: &str,
        output_dir: &str,
    ) -> DataResult<()> {
        let output_path = Path::new(output_dir);
        let temp_extract_dir = output_path.join("temp_extract");

        std::fs::create_dir_all(output_path)?;
        std::fs::create_dir_all(&temp_extract_dir)?;

        execute_sidecar(
            "_7z",
            &[
                "x",
                archive_path,
                &format!("-o{}", temp_extract_dir.display()),
            ],
        )
        .await
        .map_err(|e| DataError::ExtractionFailed(e.to_string()))?;

        let mut found_files = Vec::new();
        Self::find_files_recursive(&temp_extract_dir, target_filename, &mut found_files)?;

        if found_files.is_empty() {
            std::fs::remove_dir_all(&temp_extract_dir)?;
            return Err(DataError::NoMatchingFiles {
                pattern: target_filename.to_string(),
            });
        }

        let destination = output_path.join(target_filename);
        std::fs::create_dir_all(&destination)?;

        for file_path in found_files {
            if let Some(file_name) = file_path.file_name() {
                std::fs::copy(&file_path, destination.join(file_name))?;
            }
        }

        std::fs::remove_dir_all(temp_extract_dir)?;
        Ok(())
    }

    fn find_files_recursive(
        dir: &Path,
        target_basename: &str,
        result: &mut Vec<std::path::PathBuf>,
    ) -> DataResult<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();

            if path.is_file() {
                if let Some(file_stem) = path.file_stem()
                    && file_stem.to_string_lossy() == target_basename
                {
                    result.push(path);
                }
            } else if path.is_dir() {
                Self::find_files_recursive(&path, target_basename, result)?;
            }
        }

        Ok(())
    }

    pub async fn extract_all(archive_path: &str, output_dir: &str) -> DataResult<()> {
        let output_path = Path::new(output_dir);
        std::fs::create_dir_all(output_path)?;

        execute_sidecar(
            "_7z",
            &[
                "x",
                archive_path,
                &format!("-o{}", output_path.display()),
                "-y",
            ],
        )
        .await
        .map_err(|e| DataError::ExtractionFailed(e.to_string()))?;

        Ok(())
    }
}
