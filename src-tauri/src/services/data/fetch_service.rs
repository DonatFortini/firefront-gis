use chrono::NaiveDate;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::error::{DataError, DataResult};
use crate::types::BoundingBox;
use crate::utils::{
    DownloadProgress, cache_dir, get_data_sources, get_rpg_for_dep_code, resolution, wms_cache_dir,
};

#[derive(Debug, Clone, Default)]
pub struct FetchService {
    client: Arc<reqwest::Client>,
}

lazy_static! {
    static ref FETCH_SERVICE: FetchService = FetchService::new();
    static ref DATE_REGEX: Regex = Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap();
}

impl FetchService {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(600))
            .tcp_nodelay(true)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client: Arc::new(client),
        }
    }

    async fn download_file(
        &self,
        url: &str,
        path: &PathBuf,
        file_name: &str,
        progress: &DownloadProgress,
    ) -> DataResult<()> {
        let response = self.client.get(url).send().await?;

        Self::validate_response(&response)?;

        let total_size = response.content_length();
        let mut file = tokio::io::BufWriter::with_capacity(1024 * 1024, File::create(path).await?);
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let start_time = Instant::now();
        let mut last_update = Instant::now();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if last_update.elapsed().as_millis() > 200 {
                Self::update_progress(
                    progress,
                    file_name,
                    downloaded,
                    total_size,
                    start_time.elapsed().as_secs_f64(),
                );
                last_update = Instant::now();
            }
        }

        file.flush().await?;
        Self::validate_download(path, total_size).await?;

        Ok(())
    }

    fn validate_response(response: &reqwest::Response) -> DataResult<()> {
        if !response.status().is_success() {
            return Err(DataError::Scraping(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        if let Some(content_type) = response.headers().get("content-type")
            && content_type.to_str().unwrap_or("").contains("text/html")
        {
            return Err(DataError::Scraping(
                "Server returned HTML instead of file".to_string(),
            ));
        }

        Ok(())
    }

    fn update_progress(
        progress: &DownloadProgress,
        file_name: &str,
        downloaded: u64,
        total_size: Option<u64>,
        elapsed_secs: f64,
    ) {
        let downloaded_mb = downloaded as f64 / 1_048_576.0;
        let speed_mbps = if elapsed_secs > 0.0 {
            downloaded_mb / elapsed_secs
        } else {
            0.0
        };

        if let Some(total) = total_size {
            let total_mb = total as f64 / 1_048_576.0;
            let remaining_bytes = total.saturating_sub(downloaded);
            let eta_secs = if speed_mbps > 0.0 {
                (remaining_bytes as f64 / 1_048_576.0 / speed_mbps) as u64
            } else {
                0
            };

            progress.file_progress(file_name, downloaded_mb, total_mb, speed_mbps, eta_secs);
        } else {
            progress.file_progress(file_name, downloaded_mb, 0.0, speed_mbps, 0);
        }
    }

    async fn validate_download(path: &PathBuf, expected_size: Option<u64>) -> DataResult<()> {
        let final_size = tokio::fs::metadata(path).await?.len();

        if final_size < 1024 {
            let _ = tokio::fs::remove_file(path).await;
            return Err(DataError::Scraping(format!(
                "File too small ({} bytes), likely an error page",
                final_size
            )));
        }

        if let Some(expected) = expected_size
            && final_size != expected
        {
            let _ = tokio::fs::remove_file(path).await;
            return Err(DataError::Scraping(format!(
                "Download incomplete: expected {} bytes, got {} bytes",
                expected, final_size
            )));
        }

        Ok(())
    }

    async fn with_retry<F, Fut, T>(f: F, retries: usize) -> DataResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = DataResult<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt < retries => {
                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap())
    }

    fn sort_by_date(files: &mut [String]) {
        files.sort_by(|a, b| {
            let date_a = Self::extract_date(a);
            let date_b = Self::extract_date(b);
            date_b.cmp(&date_a)
        });
    }

    fn extract_date(text: &str) -> NaiveDate {
        DATE_REGEX
            .captures(text)
            .and_then(|cap| cap.get(1))
            .and_then(|m| NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok())
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
    }

    async fn scrap_page_for_link(
        &self,
        url: &str,
        code: &str,
        keyword: Option<&str>,
    ) -> DataResult<String> {
        let body = self.client.get(url).send().await?.text().await?;
        let document = Html::parse_document(&body);
        let selector = Selector::parse("a")
            .map_err(|e| DataError::Scraping(format!("Selector error: {}", e)))?;

        let mut links: Vec<String> = document
            .select(&selector)
            .filter_map(|element| element.value().attr("href"))
            .filter(|href| href.contains(code))
            .filter(|href| keyword.is_none_or(|kw| href.contains(kw)))
            .map(String::from)
            .collect();

        if links.is_empty() {
            return Err(DataError::Scraping(format!(
                "No file found for code: {}",
                code
            )));
        }

        Self::sort_by_date(&mut links);

        links
            .first()
            .cloned()
            .ok_or_else(|| DataError::Scraping("No valid file URL found".to_string()))
    }

    fn build_cache_path(storage_name: &str, code: &str) -> PathBuf {
        cache_dir().join(format!("{}_{}.7z", storage_name, code))
    }

    fn is_file_cached(path: &std::path::Path) -> bool {
        path.exists() && path.is_file()
    }

    async fn fetch_with_retry(
        &self,
        url: &str,
        path: &PathBuf,
        file_name: &str,
        progress: &DownloadProgress,
    ) -> DataResult<()> {
        let result =
            Self::with_retry(|| self.download_file(url, path, file_name, progress), 3).await;

        if result.is_err() {
            let _ = tokio::fs::remove_file(path).await;
        }

        result
    }

    /// Fetch download URLs for a given dataset code from configured data sources.
    /// Returns a map of storage names to their corresponding download URLs.
    pub async fn get_download_urls(code: &str) -> DataResult<HashMap<String, String>> {
        let data_sources = get_data_sources().get_sources().clone();
        let mut dl_sources = HashMap::new();

        for source in data_sources {
            if source.storage_name == "ORTHOIMAGERY" {
                continue;
            }

            let lookup_code = match source.storage_name.as_str() {
                "RPG" => get_rpg_for_dep_code(code)
                    .ok_or_else(|| DataError::Scraping(format!("No RPG code for {}", code)))?
                    .to_string(),
                _ => code.to_string(),
            };

            let link = FETCH_SERVICE
                .scrap_page_for_link(&source.url, &lookup_code, source.keyword.as_deref())
                .await
                .map_err(|e| {
                    DataError::Scraping(format!(
                        "Failed to fetch from {}: {}",
                        source.storage_name, e
                    ))
                })?;

            dl_sources.insert(source.storage_name.clone(), link);
        }

        if dl_sources.is_empty() {
            return Err(DataError::Scraping("No data sources found".to_string()));
        }

        Ok(dl_sources)
    }

    /// Fetch all required data sources for the given dataset codes.
    /// Only downloads files that are not already cached.
    pub async fn fetch_data_sources(codes: &[&str]) -> DataResult<()> {
        let cache_status: Vec<(String, String, PathBuf, bool)> = get_data_sources()
            .get_sources()
            .iter()
            .filter(|s| s.storage_name != "ORTHOIMAGERY")
            .flat_map(|source| {
                codes.iter().map(move |&code| {
                    let lookup_code = match source.storage_name.as_str() {
                        "RPG" => get_rpg_for_dep_code(code).unwrap_or(code),
                        _ => code,
                    };
                    let cache_code = match source.storage_name.as_str() {
                        "RPG" => lookup_code,
                        _ => code,
                    };
                    let path = Self::build_cache_path(&source.storage_name, cache_code);
                    let is_cached = Self::is_file_cached(path.as_path());
                    (
                        source.storage_name.clone(),
                        lookup_code.to_string(),
                        path,
                        is_cached,
                    )
                })
            })
            .collect();

        let files_to_download: Vec<_> = cache_status
            .iter()
            .filter(|(_, _, _, is_cached)| !is_cached)
            .collect();

        if files_to_download.is_empty() {
            return Ok(());
        }

        let sources_needed: HashMap<String, Vec<String>> =
            files_to_download
                .iter()
                .fold(HashMap::new(), |mut acc, (storage, code, _, _)| {
                    acc.entry(storage.clone())
                        .or_insert_with(Vec::new)
                        .push(code.clone());
                    acc
                });

        let mut download_urls = HashMap::new();
        for (storage_name, codes) in sources_needed {
            let data_sources = get_data_sources();
            let source = data_sources
                .get_source_by_name(&storage_name)
                .ok_or_else(|| DataError::Scraping(format!("Source {} not found", storage_name)))?;

            for code in codes {
                let link = FETCH_SERVICE
                    .scrap_page_for_link(&source.url, &code, source.keyword.as_deref())
                    .await
                    .map_err(|e| {
                        DataError::Scraping(format!("Failed to fetch from {}: {}", storage_name, e))
                    })?;
                download_urls.insert((storage_name.clone(), code), link);
            }
        }

        let mut progress = DownloadProgress::new(files_to_download.len());

        for (storage_name, code, path, _) in &files_to_download {
            let file_name = format!("{}_{}.7z", storage_name, code);
            progress.start_file(&file_name);

            if let Some(url) = download_urls.get(&(storage_name.clone(), code.clone())) {
                FETCH_SERVICE
                    .fetch_with_retry(url, path, &file_name, &progress)
                    .await?;
            }
        }

        progress.status(&format!(
            "Téléchargement terminé: {} fichiers",
            files_to_download.len()
        ));

        Ok(())
    }

    /// Fetch orthophoto for a given bounding box and save it to the WMS cache directory.
    pub async fn fetch_orthophoto(project_bb: &BoundingBox) -> DataResult<String> {
        let wms_cache = wms_cache_dir();
        let res = resolution();
        let width = ((project_bb.xmax - project_bb.xmin) / res).ceil() as usize;
        let height = ((project_bb.ymax - project_bb.ymin) / res).ceil() as usize;

        let cache_key = format!(
            "{:.6}_{:.6}_{:.6}_{:.6}_{}x{}",
            project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
        );
        let output_path = wms_cache.join(format!("satellite_{}.jpg", cache_key));

        if Self::is_file_cached(output_path.as_path()) {
            return Ok(output_path.to_string_lossy().to_string());
        }

        let data_sources = get_data_sources();
        let source = data_sources
            .get_source_by_name("ORTHOIMAGERY")
            .ok_or_else(|| DataError::Scraping("No ORTHOIMAGERY data source found".to_string()))?;

        let wms_url = format!(
            "{}BBOX={},{},{},{}&WIDTH={}&HEIGHT={}&FORMAT=image/jpeg",
            source.url,
            project_bb.xmin,
            project_bb.ymin,
            project_bb.xmax,
            project_bb.ymax,
            width,
            height
        );

        let mut progress = DownloadProgress::new(1);
        progress.start_file("Orthophoto");

        FETCH_SERVICE
            .fetch_with_retry(&wms_url, &output_path, "Orthophoto", &progress)
            .await?;

        Ok(output_path.to_string_lossy().to_string())
    }
}
