mod common;

use common::*;
use firefront_gis_lib::utils::{build_regions_graph, get_neighbors};

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
