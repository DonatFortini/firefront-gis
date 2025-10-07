use chrono::NaiveDate;
use futures_util::StreamExt;
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;
use tokio::time::Instant;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::error::{DataError, DataResult};
use crate::types::BoundingBox;
use crate::utils::{
    DownloadProgress, cache_dir, get_data_sources, get_rpg_for_dep_code, path_exists_in,
    resolution, wms_cache_dir,
};

pub struct FetchService;

impl FetchService {
    // ====================
    // atomic download file
    // ====================

    async fn download_file(
        client: &reqwest::Client,
        url: &str,
        path: &str,
        progress: &DownloadProgress,
    ) -> DataResult<()> {
        let response = client.get(url).send().await?;
        let total_size = response.content_length();

        let mut file = File::create(path).await?;
        let mut stream = response.bytes_stream();

        let mut downloaded: u64 = 0;
        let start_time = Instant::now();
        let mut last_update = Instant::now();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;

            downloaded += chunk.len() as u64;

            if last_update.elapsed().as_millis() > 500 {
                let elapsed_secs = start_time.elapsed().as_secs_f64();
                let downloaded_mb = downloaded as f64 / 1_048_576.0;
                let speed_mbps = downloaded_mb / elapsed_secs;

                if let Some(total) = total_size {
                    let total_mb = total as f64 / 1_048_576.0;
                    let remaining_bytes = total - downloaded;
                    let eta_secs = if speed_mbps > 0.0 {
                        (remaining_bytes as f64 / 1_048_576.0 / speed_mbps) as u64
                    } else {
                        0
                    };
                    progress.download_detail(downloaded_mb, total_mb, speed_mbps, eta_secs);
                } else {
                    progress.download_detail(downloaded_mb, 0.0, speed_mbps, 0);
                }

                last_update = Instant::now();
            }
        }

        file.flush().await?;
        Ok(())
    }

    async fn with_retry<F, Fut, T>(mut f: F, retries: usize) -> DataResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = DataResult<T>>,
    {
        let mut attempts = 0;
        loop {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) if attempts < retries => {
                    attempts += 1;
                    println!("Attempt {} failed: {}", attempts, e);
                    sleep(Duration::from_secs(5));
                }
                Err(e) => return Err(e),
            }
        }
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

    async fn scrap_page_for_link(
        url: &str,
        code: &str,
        keyword: Option<&str>,
    ) -> DataResult<String> {
        let body = reqwest::get(url).await?.text().await?;
        let document = Html::parse_document(&body);
        let selector = Selector::parse("a")
            .map_err(|e| DataError::Scraping(format!("Selector error: {}", e)))?;

        let mut links: Vec<String> = document
            .select(&selector)
            .filter_map(|element| element.value().attr("href"))
            .filter(|href| href.contains(code))
            .map(|s| s.to_string())
            .collect();

        if let Some(kw) = keyword {
            links.retain(|link| link.contains(kw));
        }

        if links.is_empty() {
            return Err(DataError::Scraping("No file found".to_string()));
        }

        Self::sort_by_date(&mut links[..]);

        let link = links
            .first()
            .ok_or_else(|| DataError::Scraping("No valid file URL found".to_string()))?;
        Ok(link.to_string())
    }

    /// Fetch a data source from a given URL and save it to the specified output path.
    /// If the file already exists in the cache directory, it will not be downloaded again.
    ///
    /// This function uses a retry mechanism to handle transient errors.
    /// # Arguments
    /// * `url` - The URL of the data source to fetch.
    /// * `output_path` - The local file path where the data source should be saved.
    /// # Errors
    /// Returns a `DataError` if the download fails after retries.
    async fn fetch_data_source(url: &str, output_path: &str) -> DataResult<()> {
        if path_exists_in(cache_dir(), output_path) {
            return Ok(());
        }

        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3";
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .build()
            .map_err(DataError::Http)?;

        let progress = DownloadProgress::new();
        Self::with_retry(
            || Self::download_file(&client, url, output_path, &progress),
            3,
        )
        .await
    }

    // ====================
    // public methods
    // ====================

    /// Fetch download URLs for a given dataset code from configured data sources.
    /// Returns a map of storage names to their corresponding download URLs.
    ///
    /// # Arguments
    ///
    /// * `code` - The dataset code to search for (e.g., "D0", "R").
    ///
    /// # Errors
    ///
    /// Returns a `DataError` if no data sources are found or if scraping fails.
    pub async fn get_download_urls(code: &str) -> DataResult<HashMap<String, String>> {
        let data_sources = get_data_sources().get_sources().clone();
        let mut dl_sources = HashMap::new();
        for source in data_sources {
            if source.storage_name == "ORTHOIMAGERY" {
                continue;
            }
            let code = match source.storage_name.as_str() {
                "RPG" => get_rpg_for_dep_code(code)
                    .ok_or_else(|| DataError::Scraping(format!("No RPG code for {}", code)))?
                    .to_string(),
                _ => code.to_string(),
            };

            let link = Self::scrap_page_for_link(&source.url, &code, source.keyword.as_deref())
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
    ///
    /// # Arguments
    /// * `codes` - A slice of dataset id in IGN database (e.g., ["D0", "R"]).
    /// # Errors
    /// Returns a `DataError` if fetching any of the data sources fails.
    pub async fn fetch_data_sources(codes: &[&str]) -> DataResult<()> {
        let progress = DownloadProgress::new();
        let mut total_files = 0;
        let mut all_downloads = Vec::new();

        for &code in codes {
            let dl_sources = Self::get_download_urls(code).await?;
            total_files += dl_sources.len();
            all_downloads.push((code, dl_sources));
        }

        let mut current_file = 0;
        for (code, dl_sources) in all_downloads {
            for (storage_name, url) in dl_sources {
                current_file += 1;
                progress.status(&format!("Téléchargement: {}_{}.7z", storage_name, code));
                progress.file_progress(
                    &format!("{}_{}", storage_name, code),
                    current_file,
                    total_files,
                );
                Self::fetch_data_source(
                    &url,
                    &format!("{}/{}_{}.7z", cache_dir().display(), storage_name, code),
                )
                .await?;
            }
        }

        progress.status(&format!("Téléchargement terminé: {} fichiers", total_files));

        Ok(())
    }

    /// Fetch orthophoto for a given bounding box and save it to the WMS cache directory.
    ///
    /// # Arguments
    /// * `project_bb` - The bounding box for which to fetch the orthophoto.
    /// # Errors
    /// Returns a `DataError` if fetching the orthophoto fails.
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

        let sources = get_data_sources().clone();
        let source = sources
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

        Self::fetch_data_source(&wms_url, output_path.to_str().unwrap()).await?;

        Ok(output_path.to_str().unwrap().to_string())
    }
}
