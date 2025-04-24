use firefront_gis_lib::app_setup;
use firefront_gis_lib::dependency;
use firefront_gis_lib::web_request;

#[cfg(test)]
mod tests {

    use firefront_gis_lib::slicing;

    use super::*;

    #[test]
    fn test_setup_check_success() {
        let result = app_setup::setup_check();
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_dependencies_success() {
        let result = dependency::check_dependencies();
        assert!(result.is_ok());
    }

    // test IGN

    #[tokio::test]
    async fn test_get_departement_shp_forest_url_success() {
        let result = web_request::get_departement_shp_file_url(
            "2A",
            "https://geoservices.ign.fr/bdforet#telechargementv2",
        )
        .await;
        assert_eq!(
            result.unwrap(),
            "https://data.geopf.fr/telechargement/download/BDFORET/BDFORET_2-0__SHP_LAMB93_D02A_2017-05-10/BDFORET_2-0__SHP_LAMB93_D02A_2017-05-10.7z"
        );
    }

    #[tokio::test]
    async fn test_get_departement_shp_forest_no_file_found() {
        let result = web_request::get_departement_shp_file_url(
            "99",
            "https://geoservices.ign.fr/bdforet#telechargementv2",
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No file found");
    }

    #[tokio::test]
    async fn test_get_departement_shp_topo_url_success() {
        let result = web_request::get_departement_shp_file_url(
            "2A",
            "https://geoservices.ign.fr/bdtopo#telechargementgpkgreg",
        )
        .await;
        assert_eq!(
            result.unwrap(),
            "https://data.geopf.fr/telechargement/download/BDTOPO/BDTOPO_3-4_TOUSTHEMES_SHP_LAMB93_D02A_2025-03-15/BDTOPO_3-4_TOUSTHEMES_SHP_LAMB93_D02A_2025-03-15.7z"
        );
    }

    #[tokio::test]
    async fn test_get_departement_shp_topo_no_file_found() {
        let result = web_request::get_departement_shp_file_url(
            "99",
            "https://geoservices.ign.fr/bdtopo#telechargementgpkgreg",
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No file found");
    }

    #[tokio::test]
    async fn test_download_shp_file_foret_success() {
        let url = "https://data.geopf.fr/telechargement/download/BDFORET/BDFORET_2-0__SHP_LAMB93_D02A_2017-05-10/BDFORET_2-0__SHP_LAMB93_D02A_2017-05-10.7z";
        match web_request::download_shp_file(url, "2A").await {
            Ok(_) => {
                assert!(std::path::Path::new("projects/cache/BDFORET_2A.7z").exists());
            }
            Err(e) => {
                panic!("Download failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_download_shp_file_topo_success() {
        let url = "https://data.geopf.fr/telechargement/download/BDTOPO/BDTOPO_3-4_TOUSTHEMES_SHP_LAMB93_D02A_2024-06-15/BDTOPO_3-4_TOUSTHEMES_SHP_LAMB93_D02A_2024-06-15.7z";
        match web_request::download_shp_file(url, "2A").await {
            Ok(_) => {
                assert!(std::path::Path::new("projects/cache/BDTOPO_2A.7z").exists());
            }
            Err(e) => {
                panic!("Download failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_download_rpg_file_success() {
        let url = "https://data.geopf.fr/telechargement/download/RPG/RPG_2-2__SHP_LAMB93_R94_2023-01-01/RPG_2-2__SHP_LAMB93_R94_2023-01-01.7z";
        match web_request::download_shp_file(url, "2A").await {
            Ok(_) => {
                assert!(std::path::Path::new("projects/cache/RPG_2A.7z").exists());
            }
            Err(e) => {
                panic!("Download failed: {:?}", e);
            }
        }
    }

    #[test]
    fn test_slices() {
        let project_name = "porto-vecchio";
        match slicing::slice_images(project_name, 500) {
            Ok(_) => {
                assert!(std::path::Path::new(&format!("projects/{}/slices", project_name)).exists())
            }
            Err(e) => {
                panic!("Slicing failed: {:?}", e);
            }
        }
    }
}
