#[cfg(test)]
mod tests {
    use firefront_gis_lib::gis_processing::{
        add_regional_layer, add_rpg_layer, add_topo_layer, add_vegetation_layer, clip_to_extent,
        convert_to_gpkg, create_project, download_satellite_jpeg, export_to_jpg,
        get_regional_extent,
    };
    use firefront_gis_lib::utils::extract_files_by_name;
    use gdal::vector::LayerAccess;
    use gdal::Dataset;
    use std::fs;
    use std::path::Path;

    fn remove_file_if_exists(file_path: &str) {
        if Path::new(file_path).exists() {
            fs::remove_file(file_path).unwrap();
        }
    }

    fn assert_file_exists(file_path: &str, message: &str) {
        assert!(Path::new(file_path).exists(), "{}", message);
    }

    fn assert_result_ok<T, E: std::fmt::Debug>(result: &Result<T, E>, message: &str) {
        assert!(result.is_ok(), "{}: {:?}", message, result.as_ref().err());
    }

    #[test]
    fn test_create_project() {
        let project_file_path = "tests/test1.tiff";
        let (xmin, ymin, xmax, ymax) = (1210000.0, 6070000.0, 1235000.0, 6095000.0);

        remove_file_if_exists(project_file_path);

        let result = create_project(project_file_path, xmin, ymin, xmax, ymax);
        assert_result_ok(&result, "Project creation failed");

        assert_file_exists(project_file_path, "Project file was not created");

        let dataset = Dataset::open(project_file_path).unwrap();
        assert_eq!(dataset.raster_count(), 4, "Project should have 4 bands");

        let geotransform = dataset.geo_transform().unwrap();
        assert!(
            (geotransform[0] - xmin).abs() < 0.001,
            "Incorrect xmin in geotransform"
        );
        assert!(
            (geotransform[3] - ymax).abs() < 0.001,
            "Incorrect ymax in geotransform"
        );

        dataset.close().unwrap();
        remove_file_if_exists(project_file_path);
    }

    #[test]
    fn test_convert_to_gpkg() {
        let input_shapefile =
            "projects/porto-vecchio/Vegetation/FORMATION_VEGETALE/FORMATION_VEGETALE.shp";
        let output_gpkg = "tests/test_vegetation.gpkg";

        remove_file_if_exists(output_gpkg);

        let result = convert_to_gpkg(input_shapefile, output_gpkg);
        assert_result_ok(&result, "Conversion to GeoPackage failed");

        assert_file_exists(output_gpkg, "GeoPackage file was not created");

        let dataset = Dataset::open(output_gpkg).unwrap();
        assert!(dataset.layer_count() > 0, "GeoPackage has no layers");

        let layer = dataset.layer(0).unwrap();
        assert!(layer.feature_count() > 0, "Layer has no features");

        dataset.close().unwrap();
        remove_file_if_exists(output_gpkg);
    }

    #[test]
    fn test_clip_to_extent() {
        let input_shapefile =
            "projects/porto-vecchio/Vegetation/FORMATION_VEGETALE/FORMATION_VEGETALE.shp";
        let intermediate_gpkg = "tests/test_vegetation_intermediate.gpkg";
        let output_gpkg = "tests/test_vegetation_clipped.gpkg";
        let (xmin, ymin, xmax, ymax) = (1210000.0, 6070000.0, 1235000.0, 6095000.0);

        for file in [intermediate_gpkg, output_gpkg] {
            remove_file_if_exists(file);
        }

        let result = convert_to_gpkg(input_shapefile, intermediate_gpkg);
        assert_result_ok(&result, "Conversion to GeoPackage failed");

        let result = clip_to_extent(intermediate_gpkg, output_gpkg, xmin, ymin, xmax, ymax);
        assert_result_ok(&result, "Clipping GeoPackage failed");

        assert_file_exists(output_gpkg, "Clipped GeoPackage file was not created");

        let dataset = Dataset::open(output_gpkg).unwrap();
        assert!(
            dataset.layer_count() > 0,
            "Clipped GeoPackage has no layers"
        );

        dataset.close().unwrap();
        remove_file_if_exists(intermediate_gpkg);
        remove_file_if_exists(output_gpkg);
    }

    #[test]
    fn test_get_regional_extent() {
        let res = get_regional_extent("2A");
        assert_result_ok(&res, "Getting regional extent failed");
    }

    #[test]
    fn test_end_to_end_workflow() {
        let project_file_path = "tests/test1.tiff";
        let (xmin, ymin, xmax, ymax) = (1210000.0, 6070000.0, 1235000.0, 6095000.0);

        let regional_geojson = "projects/cache/2A.geojson";
        let regional_gpkg = "tests/test_regional.gpkg";
        let clipped_regional_gpkg = "tests/test_regional_clipped.gpkg";

        let result =
            extract_files_by_name("projects/cache/BDFORET_2A.7z", "FORMATION_VEGETALE", "tmp");
        assert_result_ok(&result, "Extraction of vegetation shapefile failed");
        assert_file_exists(
            "tmp/FORMATION_VEGETALE/FORMATION_VEGETALE.shp",
            "Vegetation shapefile was not created",
        );

        let vegetation_shapefile = "tmp/FORMATION_VEGETALE/FORMATION_VEGETALE.shp";
        let vegetation_gpkg = "tests/test_vegetation.gpkg";
        let clipped_veg_gpkg = "tests/test_vegetation_clipped.gpkg";

        let result =
            extract_files_by_name("projects/cache/RPG_2A.7z", "PARCELLES_GRAPHIQUES", "tmp");

        assert_result_ok(&result, "Extraction of RPG shapefile failed");
        assert_file_exists(
            "tmp/PARCELLES_GRAPHIQUES/PARCELLES_GRAPHIQUES.shp",
            "RPG shapefile was not created",
        );

        let rpg_shapefile = "tmp/PARCELLES_GRAPHIQUES/PARCELLES_GRAPHIQUES.shp";
        let rpg_gpkg = "tests/test_rpg.gpkg";
        let clipped_rpg_gpkg = "tests/test_rpg_clipped.gpkg";

        let topo_subfolder = vec![
            "AERODROME",
            "CONSTRUCTION_SURFACIQUE",
            "EQUIPEMENT_DE_TRANSPORT",
            "RESERVOIR",
            "TERRAIN_DE_SPORT",
            "TRONCON_DE_VOIE_FERREE",
            "ZONE_D_ESTRAN",
            "BATIMENT",
            "COURS_D_EAU",
            "PLAN_D_EAU",
            "SURFACE_HYDROGRAPHIQUE",
            "TRONCON_DE_ROUTE",
            "VOIE_NOMMEE",
        ];

        for subfolder in &topo_subfolder {
            let result = extract_files_by_name("projects/cache/BDTOPO_2A.7z", subfolder, "tmp");
            assert_result_ok(&result, "Extraction of topography shapefile failed");
            assert_file_exists(
                &format!("tmp/{}/{}.shp", subfolder, subfolder),
                &format!("Topography shapefile {} was not created", subfolder),
            );
        }

        let result = create_project(project_file_path, xmin, ymin, xmax, ymax);
        assert_result_ok(&result, "Project creation failed");

        let result = convert_to_gpkg(vegetation_shapefile, vegetation_gpkg);
        assert_result_ok(&result, "Conversion to GeoPackage failed");

        let result = convert_to_gpkg(rpg_shapefile, rpg_gpkg);
        assert_result_ok(&result, "Conversion to GeoPackage failed");

        let result = convert_to_gpkg(regional_geojson, regional_gpkg);
        assert_result_ok(&result, "Conversion to GeoPackage failed");

        for subfolder in &topo_subfolder {
            let shapefile_path = format!("tmp/{}/{}.shp", subfolder, subfolder);
            let gpkg_path = format!("tests/test_{}.gpkg", subfolder);
            let result = convert_to_gpkg(&shapefile_path, &gpkg_path);
            assert_result_ok(&result, "Conversion to GeoPackage failed");
        }

        let result = clip_to_extent(vegetation_gpkg, clipped_veg_gpkg, xmin, ymin, xmax, ymax);
        assert_result_ok(&result, "Clipping GeoPackage failed");

        let result = clip_to_extent(rpg_gpkg, clipped_rpg_gpkg, xmin, ymin, xmax, ymax);
        assert_result_ok(&result, "Clipping GeoPackage failed");

        let result = clip_to_extent(regional_gpkg, clipped_regional_gpkg, xmin, ymin, xmax, ymax);
        assert_result_ok(&result, "Clipping GeoPackage failed");

        for subfolder in &topo_subfolder {
            let gpkg_path = format!("tests/test_{}.gpkg", subfolder);
            let clipped_gpkg_path = format!("tests/test_{}_clipped.gpkg", subfolder);
            let result = clip_to_extent(&gpkg_path, &clipped_gpkg_path, xmin, ymin, xmax, ymax);
            assert_result_ok(&result, "Clipping GeoPackage failed");
        }

        let result = add_regional_layer(project_file_path, clipped_regional_gpkg);
        assert_result_ok(&result, "Adding regional layer failed");

        let result = add_vegetation_layer(project_file_path, clipped_veg_gpkg);
        assert_result_ok(&result, "Adding vegetation layer failed");

        let result = add_rpg_layer(project_file_path, clipped_rpg_gpkg);
        assert_result_ok(&result, "Adding RPG layer failed");

        for subfolder in &topo_subfolder {
            let clipped_gpkg_path = format!("tests/test_{}_clipped.gpkg", subfolder);
            let result = add_topo_layer(project_file_path, &clipped_gpkg_path);
            assert_result_ok(&result, "Adding topography layer failed");
        }

        assert_file_exists(project_file_path, "Final project file does not exist");

        let dataset = Dataset::open(project_file_path).unwrap();
        assert_eq!(dataset.raster_count(), 4, "Project should have 4 bands");

        let raster_size = dataset.raster_size();
        assert_eq!(
            raster_size.0, raster_size.1,
            "Final project raster is not square: width = {}, height = {}",
            raster_size.0, raster_size.1
        );

        let geotransform = dataset.geo_transform().unwrap();
        let pixel_size_x = geotransform[1];
        let pixel_size_y = -geotransform[5];
        assert!(
            (pixel_size_x - 10.0).abs() < 0.001 && (pixel_size_y - 10.0).abs() < 0.001,
            "Resolution is not 10 meters per pixel: pixel_size_x = {}, pixel_size_y = {}",
            pixel_size_x,
            pixel_size_y
        );

        let test_dir = Path::new("tests");
        for entry in fs::read_dir(test_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension != "rs" && extension != "tiff" {
                        fs::remove_file(path).unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn test_export_jpeg() {
        let project_file_path = "tests/test1.tiff";
        let output_jpeg_path = "tests/test1.jpg";

        let result = export_to_jpg(project_file_path, output_jpeg_path);
        assert_result_ok(&result, "Export to JPEG failed");
        assert_file_exists(output_jpeg_path, "JPEG file was not created");

        let dataset = Dataset::open(output_jpeg_path).unwrap();

        let raster_size = dataset.raster_size();
        assert_eq!(
            raster_size.0, raster_size.1,
            "Final project raster is not square: width = {}, height = {}",
            raster_size.0, raster_size.1
        );

        let geotransform = dataset.geo_transform().unwrap();
        let pixel_size_x = geotransform[1];
        let pixel_size_y = -geotransform[5];
        assert!(
            (pixel_size_x - 10.0).abs() < 0.001 && (pixel_size_y - 10.0).abs() < 0.001,
            "Resolution is not 10 meters per pixel: pixel_size_x = {}, pixel_size_y = {}",
            pixel_size_x,
            pixel_size_y
        );

        dataset.close().unwrap();

        //remove_file_if_exists(output_jpeg_path);
    }

    #[test]
    fn test_satellite() {
        let output_jpg_path = "tests/satellite.jpg";
        let veg_project_file_path = "tests/test1.tiff";
        let veg_project_jpeg_path = "tests/test1_vegetation.jpg";
        let (xmin, ymin, xmax, ymax) = (1210000.0, 6070000.0, 1235000.0, 6095000.0);

        let result = download_satellite_jpeg(output_jpg_path, xmin, ymin, xmax, ymax);
        assert_result_ok(&result, "Downloading satellite JPEG failed");
        assert_file_exists(output_jpg_path, "Satellite JPEG file was not created");

        validate_jpeg(output_jpg_path, 10.0, 10.0, "Satellite JPEG");
        let result = export_to_jpg(veg_project_file_path, veg_project_jpeg_path);
        assert_result_ok(&result, "Export to JPEG failed");

        validate_jpeg(veg_project_jpeg_path, 10.0, 10.0, "Vegetation JPEG");
        compare_jpegs(output_jpg_path, veg_project_jpeg_path);

        // // Cleanup
        // remove_file_if_exists(output_jpg_path);
        // remove_file_if_exists(veg_project_jpeg_path);
    }

    fn validate_jpeg(
        file_path: &str,
        expected_pixel_size_x: f64,
        expected_pixel_size_y: f64,
        label: &str,
    ) {
        let dataset = Dataset::open(file_path).unwrap();
        let raster_size = dataset.raster_size();
        assert_eq!(
            raster_size.0, raster_size.1,
            "{} raster is not square: width = {}, height = {}",
            label, raster_size.0, raster_size.1
        );

        let geotransform = dataset.geo_transform().unwrap();
        let pixel_size_x = geotransform[1];
        let pixel_size_y = -geotransform[5];
        assert!(
            (pixel_size_x - expected_pixel_size_x).abs() < 0.001
                && (pixel_size_y - expected_pixel_size_y).abs() < 0.001,
            "{} resolution is not {} meters per pixel: pixel_size_x = {}, pixel_size_y = {}",
            label,
            expected_pixel_size_x,
            pixel_size_x,
            pixel_size_y
        );

        dataset.close().unwrap();
    }

    fn compare_jpegs(file_path1: &str, file_path2: &str) {
        let dataset1 = Dataset::open(file_path1).unwrap();
        let dataset2 = Dataset::open(file_path2).unwrap();

        let raster_size1 = dataset1.raster_size();
        let raster_size2 = dataset2.raster_size();
        assert_eq!(
            raster_size1.0, raster_size2.0,
            "JPEG widths do not match: file1 = {}, file2 = {}",
            raster_size1.0, raster_size2.0
        );
        assert_eq!(
            raster_size1.1, raster_size2.1,
            "JPEG heights do not match: file1 = {}, file2 = {}",
            raster_size1.1, raster_size2.1
        );

        let geotransform1 = dataset1.geo_transform().unwrap();
        let geotransform2 = dataset2.geo_transform().unwrap();
        for i in 0..6 {
            assert!(
                (geotransform1[i] - geotransform2[i]).abs() < 0.001,
                "Geotransform values do not match at index {}: file1 = {}, file2 = {}",
                i,
                geotransform1[i],
                geotransform2[i]
            );
        }

        dataset1.close().unwrap();
        dataset2.close().unwrap();
    }
}
