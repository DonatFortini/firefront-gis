mod common;

use firefront_gis_lib::slicing::{get_project_bounding_box, slice_images};

#[test]
fn test_project_bounding_box() {
    let project_name = "porto-vecchio";

    let bounding_box = get_project_bounding_box(project_name).expect("Failed to get bounding box");

    assert_eq!(bounding_box.xmin, 1210000.0, "Xmin mismatch");
    assert_eq!(bounding_box.ymax, 6095000.0, "Ymax mismatch");
    assert_eq!(bounding_box.xmax, 1235000.0, "Xmax mismatch");
    assert_eq!(bounding_box.ymin, 6070000.0, "Ymin mismatch");
}

#[test]
fn test_slice_images() {
    let project_name = "porto-vecchio";
    slice_images(project_name, 500).unwrap();
    assert!(std::path::Path::new(&format!("projects/{}/slices", project_name)).exists());
}

// The following tests are commented out but preserved for reference
// Uncomment and update when the corresponding functionalities are implemented

/*
#[test]
fn test_extent_fits_with_neighboring_region() {
    let project_bb = BoundingBox {
        xmin: 1196000.0,
        ymin: 6132000.0,
        xmax: 1246000.0,
        ymax: 6162000.0,
    };

    get_regional_extent("2B").unwrap();
    let temp_extent_file_path = "tmp/2B.geojson";
    let is_fitting = is_extent_fitting(temp_extent_file_path, &project_bb, true).unwrap();
    assert!(
        is_fitting.0,
        "Extent is not fitting. Neighboring region: {:?}",
        is_fitting.1
    );
    assert_eq!(
        is_fitting.1,
        Some(vec!["2A".to_string()]),
        "Neighboring region mismatch"
    );
}

#[test]
fn test_extent_fits_with_coastal_buffer() {
    let project_bb = ProjectBoundingBox {
        xmin: 1210000.0,
        ymin: 6070000.0,
        xmax: 1235000.0,
        ymax: 6095000.0,
    };

    get_regional_extent("2A").unwrap();
    let temp_extent_file_path = "tmp/2A.geojson";
    let is_fitting = is_extent_fitting(temp_extent_file_path, &project_bb, true).unwrap();
    assert!(is_fitting.0, "Extent is not fitting but should be.");
}

#[test]
fn test_extent_fits_with_inland_neighbor() {
    let project_bb = ProjectBoundingBox {
        xmin: 1199000.0,
        ymin: 6104000.0,
        xmax: 1219000.0,
        ymax: 6120000.0,
    };

    get_regional_extent("2A").unwrap();
    let temp_extent_file_path = "tmp/2A.geojson";
    let is_fitting = is_extent_fitting(temp_extent_file_path, &project_bb, true).unwrap();
    assert!(
        is_fitting.0,
        "Extent is not fitting. Neighboring region: {:?}",
        is_fitting.1
    );
    assert_eq!(
        is_fitting.1,
        Some(vec!["2B".to_string()]),
        "Neighboring region mismatch"
    );
}

#[test]
fn test_extent_does_not_fit_wrong_region() {
    let project_bb = ProjectBoundingBox {
        xmin: 1210000.0,
        ymin: 6070000.0,
        xmax: 1235000.0,
        ymax: 6095000.0,
    };

    get_regional_extent("2B").unwrap();
    let temp_extent_file_path = "tmp/2B.geojson";
    let is_fitting = is_extent_fitting(temp_extent_file_path, &project_bb, true).unwrap();
    assert!(
        !is_fitting.0,
        "Extent is fitting but should not be. Neighboring region: {:?}",
        is_fitting.1
    );
}
*/
