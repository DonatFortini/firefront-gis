use chrono::NaiveDate;
use futures_util::StreamExt;
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};
use std::path::Path;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::error::{DataError, DataResult};
use crate::utils::{cache_dir, execute_sidecar, get_rpg_for_dep_code};

#[derive(Debug, Clone, Copy)]
pub enum DatabaseType {
    Foret,
    Topo,
    Rpg,
}

impl DatabaseType {
    fn from_url(url: &str) -> DataResult<Self> {
        match url {
            url if url.contains("bdforet#") => Ok(Self::Foret),
            url if url.contains("bdtopo#") => Ok(Self::Topo),
            url if url.contains("rpg#") => Ok(Self::Rpg),
            _ => Err(DataError::UnsupportedDbType),
        }
    }

    fn code_prefix(&self) -> &'static str {
        match self {
            Self::Rpg => "R",
            _ => "D0",
        }
    }

    fn archive_name(&self) -> &'static str {
        match self {
            Self::Foret => "BDFORET",
            Self::Topo => "BDTOPO",
            Self::Rpg => "RPG",
        }
    }
}

pub struct DataService;

impl DataService {
    pub async fn get_shp_file_urls(codes: &[String]) -> DataResult<Vec<String>> {
        let url_topo = "https://geoservices.ign.fr/bdtopo#";
        let url_foret = "https://geoservices.ign.fr/bdforet#";
        let url_rpg = "https://geoservices.ign.fr/rpg#";

        let mut urls = Vec::new();

        for code in codes {
            urls.push(Self::get_departement_shp_url(code, url_topo).await?);
            urls.push(Self::get_departement_shp_url(code, url_foret).await?);

            let rpg_code = get_rpg_for_dep_code(code)
                .ok_or_else(|| DataError::Scraping(format!("No RPG code for {}", code)))?;
            urls.push(Self::get_departement_shp_url(rpg_code, url_rpg).await?);
        }

        Ok(urls)
    }

    async fn get_departement_shp_url(code: &str, url: &str) -> DataResult<String> {
        let body = reqwest::get(url).await?.text().await?;
        let document = Html::parse_document(&body);
        let selector = Selector::parse("a")
            .map_err(|e| DataError::Scraping(format!("Selector error: {}", e)))?;

        let db_type = DatabaseType::from_url(url)?;
        let code_prefix = db_type.code_prefix();

        let mut shp_files: Vec<String> = document
            .select(&selector)
            .filter_map(|element| element.value().attr("href"))
            .filter(|href| href.contains(&format!("{code_prefix}{code}")) && href.contains("SHP"))
            .map(|s| s.to_string())
            .collect();

        if shp_files.is_empty() {
            return Err(DataError::Scraping("No file found".to_string()));
        }

        if matches!(db_type, DatabaseType::Foret) {
            shp_files.retain(|file| file.contains("BDFORET_2-0"));
            if shp_files.is_empty() {
                return Err(DataError::Scraping("No BDFORET V2 file found".to_string()));
            }
        }

        Self::sort_by_date(&mut shp_files[..]);

        shp_files
            .first()
            .cloned()
            .ok_or_else(|| DataError::Scraping("No valid file URL found".to_string()))
    }

    fn sort_by_date(files: &mut [String]) {
        let date_regex = Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap();

        files.sort_by(|a, b| {
            let date_a = Self::extract_date(&date_regex, a);
            let date_b = Self::extract_date(&date_regex, b);
            date_b.cmp(&date_a)
        });
    }

    fn extract_date(regex: &Regex, text: &str) -> NaiveDate {
        regex
            .captures(text)
            .and_then(|cap| cap.get(1))
            .and_then(|m| NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok())
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
    }

    pub async fn download_file(url: &str, path: &str) -> DataResult<()> {
        let mut file = File::create(path).await?;
        let mut stream = reqwest::get(url).await?.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;
        }

        file.flush().await?;
        Ok(())
    }

    pub async fn download_shp_file(url: &str, code: &str) -> DataResult<()> {
        let db_type = DatabaseType::from_url(url)?;
        let archive_name = db_type.archive_name();
        let archive_path = format!(
            "{}/{}_{}.7z",
            cache_dir().to_string_lossy(),
            archive_name,
            code
        );

        if Path::new(&archive_path).exists() {
            std::fs::remove_file(&archive_path)?;
        }

        Self::download_file(url, &archive_path).await
    }

    pub fn is_in_cache(name: &str) -> bool {
        cache_dir().join(name).exists()
    }
}

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
}
