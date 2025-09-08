use std::{fs, path::Path, thread::sleep, time::Duration};

use crate::{
    types::BoundingBox,
    utils::{cache_dir, create_directory_if_not_exists, resolution},
};

/// Télécharge une image satellite JPEG pour une étendue donnée avec une résolution de 10m/pixel
/// Cette fonction utilise le service WMS de geoportail pour télécharger une image satellite
/// et utilise ImageMagick pour traiter l'image.
///
/// # Arguments
///
/// * `output_jpg_path` - chemin de sortie pour l'image JPEG
/// * `project_bb` - BoundingBox de l'étendue du projet
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si le téléchargement a réussi ou échoué
pub async fn fetch_orthophoto_wms(
    output_jpg_path: &str,
    project_bb: &BoundingBox,
) -> Result<(), Box<dyn std::error::Error>> {
    let wms_cache_dir = format!("{}/wms_cache", cache_dir().to_string_lossy());
    create_directory_if_not_exists(&wms_cache_dir)?;

    let resolution = resolution();
    let width = ((project_bb.xmax - project_bb.xmin) / resolution).ceil() as usize;
    let height = ((project_bb.ymax - project_bb.ymin) / resolution).ceil() as usize;

    println!("Dimensions calculées : largeur={width}, hauteur={height} pixels");

    let cache_key = format!(
        "{:.6}_{:.6}_{:.6}_{:.6}_{}x{}",
        project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
    );
    let cache_file = format!("{wms_cache_dir}/satellite_{cache_key}.jpg");

    if Path::new(&cache_file).exists() {
        if let Ok(metadata) = fs::metadata(&cache_file)
            && metadata.len() > 0
        {
            fs::copy(&cache_file, output_jpg_path)?;
            println!(
                "Image satellite récupérée depuis le cache: {} bytes",
                metadata.len()
            );
            return Ok(());
        }
        let _ = fs::remove_file(&cache_file);
    }

    let wms_url = format!(
        "https://data.geopf.fr/wms-r/wms?\
        SERVICE=WMS&\
        VERSION=1.3.0&\
        REQUEST=GetMap&\
        LAYERS=ORTHOIMAGERY.ORTHOPHOTOS&\
        STYLES=&\
        CRS=EPSG:2154&\
        BBOX={},{},{},{}&\
        WIDTH={}&\
        HEIGHT={}&\
        FORMAT=image/jpeg",
        project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("Rust WMS Client")
        .build()?;

    let mut success = false;
    let mut attempts = 0;
    let max_attempts = 3;
    let mut image_data = Vec::new();

    while !success && attempts < max_attempts {
        attempts += 1;
        println!("Tentative de téléchargement {attempts}/{max_attempts}");

        match download_attempt(&client, &wms_url).await {
            Ok(data) => {
                if data.is_empty() {
                    return Err("Le fichier téléchargé est vide".into());
                }

                image_data = data;
                success = true;
            }
            Err(e) => {
                println!("Tentative {} échouée: {}", attempts, e);
                if attempts < max_attempts {
                    println!("Nouvelle tentative dans 5 secondes...");
                    sleep(Duration::from_secs(5));
                }
            }
        }
    }

    if !success {
        return Err(
            "Échec du téléchargement de l'image satellite après plusieurs tentatives".into(),
        );
    }

    let temp_cache_file = format!("{}.tmp", cache_file);
    fs::write(&temp_cache_file, &image_data)?;
    fs::rename(&temp_cache_file, &cache_file)?;

    fs::copy(&cache_file, output_jpg_path)?;

    if !Path::new(&output_jpg_path).exists() {
        return Err("Échec de l'écriture du fichier final".into());
    }

    if let Ok(metadata) = fs::metadata(output_jpg_path)
        && metadata.len() == 0
    {
        return Err("Le fichier final est vide".into());
    }

    println!(
        "Image satellite téléchargée avec succès: {} bytes",
        image_data.len()
    );
    Ok(())
}

async fn download_attempt(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()).into());
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("image/") {
        let error_text = response.text().await?;
        return Err(format!(
            "Server returned error response: {}",
            error_text.chars().take(200).collect::<String>()
        )
        .into());
    }

    let image_data = response.bytes().await?;

    if image_data.len() < 10 || image_data[0] != 0xFF || image_data[1] != 0xD8 {
        return Err("Downloaded data is not a valid JPEG image".into());
    }

    Ok(image_data.to_vec())
}

pub mod prelude {
    pub use super::fetch_orthophoto_wms;
}
