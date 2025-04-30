use chrono::NaiveDate;
use futures_util::StreamExt;
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};
use std::{error::Error, fs, path::Path};
use tokio::{fs::File, io::AsyncWriteExt};

use crate::utils::get_rpg_for_dep_code;

pub enum DBType {
    FORET,
    TOPO,
    RPG,
}

/// Obtient l'URL d'un fichier SHP depuis la base de données IGN.
/// Cherche l'url le plus récent pour le département spécifié.
///
/// # Arguments
/// - `code`: Le code du département.
/// - `url`: L'URL de la base de données.
///
/// # Retourne
/// - Result<String, Box<dyn Error>> - L'URL du fichier SHP.
pub async fn get_departement_shp_file_url(code: &str, url: &str) -> Result<String, Box<dyn Error>> {
    let body = reqwest::get(url).await?.text().await?;
    let document = Html::parse_document(&body);
    let selector = Selector::parse("a")?;

    let dbtype = match true {
        _ if url.contains("bdforet#") => DBType::FORET,
        _ if url.contains("bdtopo#") => DBType::TOPO,
        _ if url.contains("rpg#") => DBType::RPG,
        _ => return Err("Unsupported database type".into()),
    };

    let code_prefix = match dbtype {
        DBType::RPG => "R",
        _ => "D0",
    };

    let mut shp_files: Vec<String> = document
        .select(&selector)
        .filter_map(|element| element.value().attr("href"))
        .filter(|href| href.contains(&format!("{}{}", code_prefix, code)) && href.contains("SHP"))
        .map(|s| s.to_string())
        .collect();

    if shp_files.is_empty() {
        return Err("No file found".into());
    }

    if matches!(dbtype, DBType::FORET) {
        shp_files.retain(|file| file.contains("BDFORET_2-0"));

        if shp_files.is_empty() {
            return Err("No BDFORET V2 file found".into());
        }
    }

    let date_regex = Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap();

    shp_files.sort_by(|a, b| {
        let date_a = date_regex
            .captures(a)
            .and_then(|cap| cap.get(1))
            .and_then(|m| NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok())
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());

        let date_b = date_regex
            .captures(b)
            .and_then(|cap| cap.get(1))
            .and_then(|m| NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok())
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        date_b.cmp(&date_a)
    });

    match shp_files.first() {
        Some(url) => Ok(url.clone()),
        None => Err("No valid file URL found after filtering".into()),
    }
}

/// Télécharge un fichier depuis une URL donnée et l'enregistre à l'emplacement spécifié.
///
/// # Arguments
/// - `url`: L'URL du fichier à télécharger.
/// - `path`: Le chemin où le fichier sera enregistré.
///
/// # Retourne
/// - Result<(), Box<dyn Error>> - Un résultat vide indiquant le succès ou une erreur.
pub async fn download_file(url: &str, path: &str) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(path).await?;
    let mut stream = reqwest::get(url).await?.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}

/// Télécharge un fichier SHP depuis une URL donnée de la base de données IGN.
///
/// - Si l'URL contient "BDTOPO", le nom sera "BDTOPO".
/// - Si l'URL contient "BDFORET", le nom sera "BDFORET".
/// - Si l'URL contient "RPG", le nom sera "RPG".
/// - Sinon, le nom sera "inconnu".
///
/// # Arguments
/// - `url`:  l'URL à télécharger.
/// - `code`: le code du département.
///     
/// # Retourne
/// - Result<(), Box<dyn Error>> - Un résultat vide indiquant le succès ou une erreur.
pub async fn download_shp_file(url: &str, code: &str) -> Result<(), Box<dyn Error>> {
    let name = match url {
        url if url.contains("BDTOPO") => "BDTOPO",
        url if url.contains("BDFORET") => "BDFORET",
        url if url.contains("RPG") => "RPG",
        _ => "unknown",
    };
    let cache_folder_path = "projects/cache";
    let archive_path = format!("{}/{}_{}.7z", cache_folder_path, name, code);

    if Path::new(&archive_path).exists() {
        fs::remove_file(&archive_path)?;
    }

    download_file(url, &archive_path).await
}

/// Obtients les URLs des fichiers SHP pour les départements spécifiés.
///
/// # Arguments
/// - `codes`: Une liste de chaînes contenant les codes des départements.
///
/// # Retourne
/// - Result<Vec<String>, Box<dyn Error>> - Une liste de chaînes contenant les URLs des fichiers SHP.
pub async fn get_shp_file_urls(codes: &[String]) -> Result<Vec<String>, Box<dyn Error>> {
    let url_dl_topo = "https://geoservices.ign.fr/bdtopo#";
    let url_dl_foret = "https://geoservices.ign.fr/bdforet#";
    let url_dl_rpg = "https://geoservices.ign.fr/rpg#";

    let mut urls = Vec::new();

    for code in codes {
        let url_topo = get_departement_shp_file_url(code, url_dl_topo).await?;
        urls.push(url_topo);

        let url_foret = get_departement_shp_file_url(code, url_dl_foret).await?;
        urls.push(url_foret);

        let rpg_code = get_rpg_for_dep_code(code).unwrap();
        let url_rpg = get_departement_shp_file_url(rpg_code, url_dl_rpg).await?;
        urls.push(url_rpg);
    }

    Ok(urls)
}
