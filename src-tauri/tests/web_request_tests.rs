mod common;

use firefront_gis_lib::web_request;

#[tokio::test]
async fn test_fetch_forest_shp_url_valid() {
    let url = web_request::get_departement_shp_file_url(
        "2A",
        "https://geoservices.ign.fr/bdforet#telechargementv2",
    )
    .await
    .unwrap();
    assert_eq!(
        url,
        "https://data.geopf.fr/telechargement/download/BDFORET/BDFORET_2-0__SHP_LAMB93_D02A_2017-05-10/BDFORET_2-0__SHP_LAMB93_D02A_2017-05-10.7z"
    );
}

#[tokio::test]
async fn test_fetch_forest_shp_url_invalid() {
    let error = web_request::get_departement_shp_file_url(
        "99",
        "https://geoservices.ign.fr/bdforet#telechargementv2",
    )
    .await
    .unwrap_err();
    assert_eq!(error.to_string(), "No file found");
}

#[tokio::test]
async fn test_fetch_topo_shp_url_valid() {
    let url = web_request::get_departement_shp_file_url(
        "2A",
        "https://geoservices.ign.fr/bdtopo#telechargementgpkgreg",
    )
    .await
    .unwrap();
    assert_eq!(
        url,
        "https://data.geopf.fr/telechargement/download/BDTOPO/BDTOPO_3-4_TOUSTHEMES_SHP_LAMB93_D02A_2025-03-15/BDTOPO_3-4_TOUSTHEMES_SHP_LAMB93_D02A_2025-03-15.7z"
    );
}

#[tokio::test]
async fn test_fetch_topo_shp_url_invalid() {
    let error = web_request::get_departement_shp_file_url(
        "99",
        "https://geoservices.ign.fr/bdtopo#telechargementgpkgreg",
    )
    .await
    .unwrap_err();
    assert_eq!(error.to_string(), "No file found");
}

#[tokio::test]
async fn test_download_forest_shp() {
    let url = "https://data.geopf.fr/telechargement/download/BDFORET/BDFORET_2-0__SHP_LAMB93_D02A_2017-05-10/BDFORET_2-0__SHP_LAMB93_D02A_2017-05-10.7z";
    web_request::download_shp_file(url, "2A").await.unwrap();
    assert!(std::path::Path::new("projects/cache/BDFORET_2A.7z").exists());
}

#[tokio::test]
async fn test_download_topo_shp() {
    let url = "https://data.geopf.fr/telechargement/download/BDTOPO/BDTOPO_3-4_TOUSTHEMES_SHP_LAMB93_D02A_2024-06-15/BDTOPO_3-4_TOUSTHEMES_SHP_LAMB93_D02A_2024-06-15.7z";
    web_request::download_shp_file(url, "2A").await.unwrap();
    assert!(std::path::Path::new("projects/cache/BDTOPO_2A.7z").exists());
}

#[tokio::test]
async fn test_download_rpg_shp() {
    let url = "https://data.geopf.fr/telechargement/download/RPG/RPG_2-2__SHP_LAMB93_R94_2023-01-01/RPG_2-2__SHP_LAMB93_R94_2023-01-01.7z";
    web_request::download_shp_file(url, "2A").await.unwrap();
    assert!(std::path::Path::new("projects/cache/RPG_2A.7z").exists());
}
