use chrono::NaiveDate;
use futures_util::StreamExt;
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};
use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::error::{DataError, DataResult};
use crate::types::BoundingBox;
use crate::utils::{cache_dir, create_directory_if_not_exists, get_rpg_for_dep_code, resolution};

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

pub struct FetchService;

impl FetchService {
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

    pub async fn fetch_orthophoto(output_path: &str, project_bb: &BoundingBox) -> DataResult<()> {
        let wms_cache = cache_dir().join("wms_cache");
        create_directory_if_not_exists(&wms_cache.to_string_lossy())
            .map_err(|e| DataError::ExtractionFailed(e.to_string()))?;

        let res = resolution();
        let width = ((project_bb.xmax - project_bb.xmin) / res).ceil() as usize;
        let height = ((project_bb.ymax - project_bb.ymin) / res).ceil() as usize;

        println!("Dimensions: width={}, height={} pixels", width, height);

        let cache_key = format!(
            "{:.6}_{:.6}_{:.6}_{:.6}_{}x{}",
            project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
        );
        let cache_file = wms_cache.join(format!("satellite_{}.jpg", cache_key));

        if cache_file.exists() {
            if let Ok(metadata) = fs::metadata(&cache_file)
                && metadata.len() > 0
            {
                fs::copy(&cache_file, output_path)?;
                println!("Retrieved from cache: {} bytes", metadata.len());
                return Ok(());
            }
            let _ = fs::remove_file(&cache_file);
        }

        let wms_url = format!(
            "https://data.geopf.fr/wms-r/wms?\
            SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&\
            LAYERS=ORTHOIMAGERY.ORTHOPHOTOS&STYLES=&CRS=EPSG:2154&\
            BBOX={},{},{},{}&WIDTH={}&HEIGHT={}&FORMAT=image/jpeg",
            project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent("Rust WMS Client")
            .build()
            .map_err(DataError::Http)?;

        let mut image_data = Vec::new();
        let max_attempts = 3;

        for attempt in 1..=max_attempts {
            println!("Download attempt {}/{}", attempt, max_attempts);

            match Self::download_wms_image(&client, &wms_url).await {
                Ok(data) => {
                    image_data = data;
                    break;
                }
                Err(e) if attempt < max_attempts => {
                    println!("Attempt {} failed: {}", attempt, e);
                    sleep(Duration::from_secs(5));
                }
                Err(e) => return Err(e),
            }
        }

        let temp_cache = format!("{}.tmp", cache_file.to_string_lossy());
        fs::write(&temp_cache, &image_data)?;
        fs::rename(&temp_cache, &cache_file)?;
        fs::copy(&cache_file, output_path)?;

        println!("Orthophoto downloaded: {} bytes", image_data.len());
        Ok(())
    }

    async fn download_wms_image(client: &reqwest::Client, url: &str) -> DataResult<Vec<u8>> {
        let response = client.get(url).send().await.map_err(DataError::Http)?;

        if !response.status().is_success() {
            return Err(DataError::Http(response.error_for_status().unwrap_err()));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("");

        if !content_type.starts_with("image/") {
            let error_text = response.text().await.map_err(DataError::Http)?;
            return Err(DataError::Scraping(format!(
                "Server error: {}",
                &error_text[..error_text.len().min(200)]
            )));
        }

        let image_data = response.bytes().await.map_err(DataError::Http)?.to_vec();

        if image_data.len() < 10 || image_data[0] != 0xFF || image_data[1] != 0xD8 {
            return Err(DataError::Scraping("Invalid JPEG data".to_string()));
        }

        Ok(image_data)
    }
}
