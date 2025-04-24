use chrono::NaiveDate;
use futures_util::StreamExt;
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};
use std::{error::Error, fs, path::Path};
use tokio::{fs::File, io::AsyncWriteExt};

pub enum DBType {
    FORET,
    TOPO,
    RPG,
}

/// Obtient l'URL d'un fichier SHP depuis la base de données IGN.
/// Cherche l'url le plus récent pour le département spécifié.
/// # Paramètres
/// - `code`: Une tranche de chaîne qui contient le code du département.
/// - `url`: Une tranche de chaîne qui contient l'URL d'une page spécifique de la base de données IGN.
///
/// # Retourne
/// - Une tranche de chaîne représentant l'URL de l'archive du fichier SHP correspondant au département.
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
/// # Paramètres
/// - `url`: Une tranche de chaîne qui contient l'URL à télécharger.
/// - `path`: Une tranche de chaîne qui contient le chemin où le fichier sera enregistré.
///
/// # Retourne
/// - Un résultat vide indiquant le succès ou une erreur.
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
/// # Paramètres
/// - `url`: Une tranche de chaîne qui contient l'URL à vérifier.
/// - `code`: Une tranche de chaîne qui contient le code du département.
///
/// # Retourne
/// - Un résultat vide indiquant le succès ou une erreur.
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
