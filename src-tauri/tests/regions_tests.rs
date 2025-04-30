mod common;

use common::*;
use firefront_gis_lib::{
    gis_operation::regions::{
        build_regions_graph, find_intersecting_regions, get_neighbors, get_region,
    },
    utils::BoundingBox,
};

#[test]
fn test_build_regions_graph() {
    let result = build_regions_graph(Some("resources/regions_graph.json"));
    assert_result_ok(&result, "Building regions graph failed");
}

#[test]
fn test_get_neighbors() {
    let neighbors = get_neighbors("2A").unwrap();
    assert!(!neighbors.is_empty(), "No neighbors found for region 2A");
    println!("Neighbors of 2A: {:?}", neighbors);
}

#[test]
fn test_region_intersects() {
    let bb = get_test_bounding_box();
    let region_2a = get_region("2A").unwrap();
    assert!(
        region_2a.intersects(&bb),
        "Bounding box should intersect with region 2A"
    );
}

#[test]
fn test_find_multiple_intersecting_regions() {
    // Cozzano
    let bb = BoundingBox::new(1199000.0, 6104000.0, 1219000.0, 6120000.0);
    let result = find_intersecting_regions(&bb).unwrap();

    assert!(
        result.len() >= 2,
        "Should intersect with at least two regions"
    );
    println!("Number of intersecting regions: {}", result.len());
    for region in &result {
        println!("Intersecting region: {}", region.get_name());
    }
}

#[test]
fn test_no_intersecting_regions() {
    let bb = BoundingBox::new(0.0, 0.0, 1.0, 1.0);
    let result = find_intersecting_regions(&bb).unwrap();

    assert_eq!(result.len(), 0, "Should have no intersecting regions");
}
