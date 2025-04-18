use futures_util::StreamExt;
use reqwest;
use scraper::{Html, Selector};
use std::{error::Error, fs, path::Path};
use tokio::{fs::File, io::AsyncWriteExt};

/// Obtient l'URL d'un fichier SHP depuis la base de données IGN.
/// Cherche l'url le plus récent pour le département spécifié.
/// # Paramètres
/// - `code`: Une tranche de chaîne qui contient le code du département.
/// - `url`: Une tranche de chaîne qui contient l'URL d'une page spécifique de la base de données IGN.
///
/// # Retourne
/// - Une tranche de chaîne représentant l'URL de l'archive du fichier SHP correspondant au département.
pub async fn get_departement_shp_file_url(
    code: &str,
    url: &str,
    code_type: Option<&str>,
) -> Result<String, Box<dyn Error>> {
    let body = reqwest::get(url).await?.text().await?;
    let document = Html::parse_document(&body);
    let selector = Selector::parse("a")?;

    let code_prefix = code_type.unwrap_or("D0");

    let mut shp_files: Vec<_> = document
        .select(&selector)
        .filter_map(|element| element.value().attr("href"))
        .filter(|href| href.contains(&format!("{}{}", code_prefix, code)) && href.contains("SHP"))
        .collect();

    if shp_files.is_empty() {
        return Err("No file found".into());
    }

    shp_files.sort_by_key(|href| {
        href.split('_')
            .next_back()
            .and_then(|s| s.split('.').next())
            .unwrap_or("")
            .to_string()
    });

    Ok(shp_files.pop().unwrap().to_string())
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
