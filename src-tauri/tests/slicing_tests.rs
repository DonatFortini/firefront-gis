mod common;

use firefront_gis_lib::{gis_operation::slicing::slice_images, utils::get_project_bounding_box};

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
