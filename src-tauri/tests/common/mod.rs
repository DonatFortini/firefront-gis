use firefront_gis_lib::utils::BoundingBox;
use gdal::Dataset;
use std::fs;
use std::path::Path;

#[allow(unused)]
pub fn remove_file_if_exists(file_path: &str) {
    if Path::new(file_path).exists() {
        fs::remove_file(file_path).unwrap();
    }
}

#[allow(unused)]
pub fn assert_file_exists(file_path: &str, message: &str) {
    assert!(Path::new(file_path).exists(), "{}", message);
}

#[allow(unused)]
pub fn assert_result_ok<T, E: std::fmt::Debug>(result: &Result<T, E>, message: &str) {
    assert!(result.is_ok(), "{}: {:?}", message, result.as_ref().err());
}

#[allow(unused)]
pub fn check_jpeg_properties(file_path: &str, expected_resolution: f64, label: &str) {
    let dataset = Dataset::open(file_path).unwrap();
    let (width, height) = dataset.raster_size();
    assert_eq!(
        width, height,
        "{} raster is not square: width = {}, height = {}",
        label, width, height
    );

    let geotransform = dataset.geo_transform().unwrap();
    let pixel_size_x = geotransform[1];
    let pixel_size_y = -geotransform[5];
    assert!(
        (pixel_size_x - expected_resolution).abs() < 0.001
            && (pixel_size_y - expected_resolution).abs() < 0.001,
        "{} resolution is not {} meters per pixel: pixel_size_x = {}, pixel_size_y = {}",
        label,
        expected_resolution,
        pixel_size_x,
        pixel_size_y
    );

    dataset.close().unwrap();
}

#[allow(unused)]
pub fn assert_jpegs_match(file1: &str, file2: &str) {
    let ds1 = Dataset::open(file1).unwrap();
    let ds2 = Dataset::open(file2).unwrap();

    assert_eq!(
        ds1.raster_size(),
        ds2.raster_size(),
        "JPEG dimensions do not match: file1 = {}, file2 = {}",
        file1,
        file2
    );

    let gt1 = ds1.geo_transform().unwrap();
    let gt2 = ds2.geo_transform().unwrap();
    assert!(
        gt1.iter()
            .zip(gt2.iter())
            .all(|(a, b)| (a - b).abs() < 0.001),
        "Geotransform values do not match: file1 = {:?}, file2 = {:?}",
        gt1,
        gt2
    );

    ds1.close().unwrap();
    ds2.close().unwrap();
}

#[allow(unused)]
/// Returns a test bounding box with Porto-Vecchio coordinates
pub fn get_test_bounding_box() -> BoundingBox {
    BoundingBox {
        xmin: 1210000.0,
        ymin: 6070000.0,
        xmax: 1235000.0,
        ymax: 6095000.0,
    }
}
