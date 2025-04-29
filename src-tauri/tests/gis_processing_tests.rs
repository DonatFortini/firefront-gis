mod common;

use common::*;
use firefront_gis_lib::gis_processing::{
    clip_to_bb, convert_to_gpkg, create_project, download_satellite_jpeg, fusion_datasets,
    get_regional_extent,
};
use firefront_gis_lib::utils::{
    create_directory_if_not_exists, export_to_jpg, extract_files_by_name,
};
use gdal::Dataset;
use std::fs;

#[test]
fn test_project_creation() {
    let project_path = "tests/res/test_project.tiff";
    remove_file_if_exists(project_path);

    let bbox = get_test_bounding_box();
    let result = create_project(project_path, &bbox);
    assert_result_ok(&result, "Failed to create project");
    assert_file_exists(project_path, "Project file not created");

    let dataset = Dataset::open(project_path).unwrap();
    assert_eq!(dataset.raster_count(), 4, "Expected 4 raster bands");

    let geotransform = dataset.geo_transform().unwrap();
    assert!(
        (geotransform[0] - bbox.xmin).abs() < 0.001,
        "Incorrect xmin"
    );
    assert!(
        (geotransform[3] - bbox.ymax).abs() < 0.001,
        "Incorrect ymax"
    );

    dataset.close().unwrap();
    remove_file_if_exists(project_path);
}

#[test]
fn test_shapefile_to_gpkg_conversion() {
    let input_shapefile = "tmp/FORMATION_VEGETALE/FORMATION_VEGETALE.shp";
    let output_gpkg = "tests/res/vegetation.gpkg";

    extract_files_by_name("tests/res/BDFORET_2a.7z", "FORMATION_VEGETALE", "tmp").unwrap();
    remove_file_if_exists(output_gpkg);

    let result = convert_to_gpkg(input_shapefile, output_gpkg);
    assert_result_ok(&result, "Failed to convert shapefile to GeoPackage");
    assert_file_exists(output_gpkg, "GeoPackage file was not created");

    let dataset = Dataset::open(output_gpkg).unwrap();
    assert!(dataset.layer_count() > 0, "GeoPackage has no layers");
    dataset.close().unwrap();
}

#[test]
fn test_clip_shapefile() {
    let input_shapefile = "tmp/FORMATION_VEGETALE/FORMATION_VEGETALE.shp";
    let output_gpkg = "tests/res/clipped_vegetation.gpkg";
    let project_bb = get_test_bounding_box();

    remove_file_if_exists(output_gpkg);

    extract_files_by_name("tests/res/BDFORET_2a.7z", "FORMATION_VEGETALE", "tmp").unwrap();
    let result = clip_to_bb(input_shapefile, output_gpkg, &project_bb);
    assert_result_ok(&result, "Clipping shapefile failed");

    assert_file_exists(output_gpkg, "Clipped GeoPackage file was not created");

    let dataset = Dataset::open(output_gpkg).unwrap();
    assert!(
        dataset.layer_count() > 0,
        "Clipped GeoPackage has no layers"
    );
    dataset.close().unwrap();

    remove_file_if_exists(output_gpkg);
}

#[test]
fn test_get_regional_extent() {
    create_directory_if_not_exists("tmp").unwrap();
    let res = get_regional_extent("2A");
    assert_result_ok(&res, "Getting regional extent failed");
}

#[test]
fn test_export_to_jpeg() {
    let input_tiff = "tests/res/test1.tiff";
    let output_jpeg = "tests/res/test1.jpg";

    export_to_jpg(input_tiff, output_jpeg).expect("Export to JPEG failed");
    assert_file_exists(output_jpeg, "JPEG file was not created");

    let dataset = Dataset::open(output_jpeg).unwrap();
    let (width, height) = dataset.raster_size();
    assert_eq!(
        width, height,
        "JPEG raster is not square: width = {}, height = {}",
        width, height
    );

    let geotransform = dataset.geo_transform().unwrap();
    let (pixel_size_x, pixel_size_y) = (geotransform[1], -geotransform[5]);
    assert!(
        (pixel_size_x - 10.0).abs() < 0.001 && (pixel_size_y - 10.0).abs() < 0.001,
        "Resolution is not 10 meters per pixel: pixel_size_x = {}, pixel_size_y = {}",
        pixel_size_x,
        pixel_size_y
    );

    dataset.close().unwrap();
}

#[test]
fn test_satellite_download_and_compare() {
    let satellite_jpg = "tests/res/satellite.jpg";
    let vegetation_tiff = "tests/res/test1.tiff";
    let vegetation_jpg = "tests/res/test1_vegetation.jpg";
    let bounding_box = get_test_bounding_box();

    let result = download_satellite_jpeg(satellite_jpg, &bounding_box);
    assert_result_ok(&result, "Failed to download satellite JPEG");
    assert_file_exists(satellite_jpg, "Satellite JPEG not created");
    check_jpeg_properties(satellite_jpg, 10.0, "Satellite JPEG");

    let result = export_to_jpg(vegetation_tiff, vegetation_jpg);
    assert_result_ok(&result, "Failed to export vegetation to JPEG");
    check_jpeg_properties(vegetation_jpg, 10.0, "Vegetation JPEG");

    assert_jpegs_match(satellite_jpg, vegetation_jpg);

    // Cleanup
    remove_file_if_exists(satellite_jpg);
    remove_file_if_exists(vegetation_jpg);
}

#[test]
fn test_fusion() {
    let veget_path_2a = "tests/res/BDFORET_2A.7z";
    let veget_path_2b = "tests/res/BDFORET_2B.7z";
    create_directory_if_not_exists("tmp").unwrap();

    extract_files_by_name(veget_path_2a, "FORMATION_VEGETALE", "tmp").unwrap();
    fs::rename("tmp/FORMATION_VEGETALE", "tmp/FORMATION_VEGETALE_2A").unwrap();
    extract_files_by_name(veget_path_2b, "FORMATION_VEGETALE", "tmp").unwrap();
    fs::rename("tmp/FORMATION_VEGETALE", "tmp/FORMATION_VEGETALE_2B").unwrap();

    let dataset = vec![
        "tmp/FORMATION_VEGETALE_2A/FORMATION_VEGETALE.shp",
        "tmp/FORMATION_VEGETALE_2B/FORMATION_VEGETALE.shp",
    ];

    let res = fusion_datasets(&dataset);
    assert_result_ok(&res, "Fusion of datasets failed");
}
