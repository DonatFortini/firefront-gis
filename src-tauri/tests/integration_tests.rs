mod common;

use common::*;
use firefront_gis_lib::gis_processing::{
    add_regional_layer, add_rpg_layer, add_topo_layer, add_vegetation_layer, clip_to_bb,
    convert_to_gpkg, create_project, get_regional_extent,
};
use firefront_gis_lib::utils::{create_directory_if_not_exists, extract_files_by_name};
use gdal::Dataset;
use std::fs;
use std::path::Path;

#[test]
fn test_end_to_end_workflow() {
    create_directory_if_not_exists("tmp").unwrap();
    let project_bb = get_test_bounding_box();
    let project_file_path = "tests/res/test1.tiff";

    let files_to_extract = vec![
        (
            "tests/res/BDFORET_2A.7z",
            "FORMATION_VEGETALE",
            "tmp/FORMATION_VEGETALE/FORMATION_VEGETALE.shp",
        ),
        (
            "tests/res/RPG_2A.7z",
            "PARCELLES_GRAPHIQUES",
            "tmp/PARCELLES_GRAPHIQUES/PARCELLES_GRAPHIQUES.shp",
        ),
    ];

    for (archive, folder, expected_file) in files_to_extract {
        let result = extract_files_by_name(archive, folder, "tmp");
        assert_result_ok(&result, &format!("Extraction of {} failed", folder));
        assert_file_exists(expected_file, &format!("{} was not created", folder));
    }

    let topo_subfolders = vec![
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

    for subfolder in &topo_subfolders {
        let result = extract_files_by_name("tests/res/BDTOPO_2A.7z", subfolder, "tmp");
        assert_result_ok(&result, &format!("Extraction of {} failed", subfolder));
        assert_file_exists(
            &format!("tmp/{}/{}.shp", subfolder, subfolder),
            &format!("{} shapefile was not created", subfolder),
        );
    }
    let result = get_regional_extent("2A");
    assert_result_ok(&result, "Getting regional extent failed");
    let result = create_project(project_file_path, &project_bb);
    assert_result_ok(&result, "Project creation failed");

    let geojson_to_gpkg = vec![
        ("tmp/2A.geojson", "tests/res/test_regional.gpkg"),
        (
            "tmp/FORMATION_VEGETALE/FORMATION_VEGETALE.shp",
            "tests/res/test_vegetation.gpkg",
        ),
        (
            "tmp/PARCELLES_GRAPHIQUES/PARCELLES_GRAPHIQUES.shp",
            "tests/res/test_rpg.gpkg",
        ),
    ];

    for (input, output) in geojson_to_gpkg {
        let result = convert_to_gpkg(input, output);
        assert_result_ok(
            &result,
            &format!("Conversion of {} to GeoPackage failed", input),
        );
    }

    for subfolder in &topo_subfolders {
        let shapefile_path = format!("tmp/{}/{}.shp", subfolder, subfolder);
        let gpkg_path = format!("tests/res/test_{}.gpkg", subfolder);
        let result = convert_to_gpkg(&shapefile_path, &gpkg_path);
        assert_result_ok(
            &result,
            &format!("Conversion of {} to GeoPackage failed", subfolder),
        );
    }

    let gpkg_to_clip = vec![
        (
            "tests/res/test_vegetation.gpkg",
            "tests/res/test_vegetation_clipped.gpkg",
        ),
        ("tests/res/test_rpg.gpkg", "tests/res/test_rpg_clipped.gpkg"),
        (
            "tests/res/test_regional.gpkg",
            "tests/res/test_regional_clipped.gpkg",
        ),
    ];

    for (input, output) in gpkg_to_clip {
        let result = clip_to_bb(input, output, &project_bb);
        assert_result_ok(&result, &format!("Clipping of {} failed", input));
    }

    for subfolder in &topo_subfolders {
        let gpkg_path = format!("tests/res/test_{}.gpkg", subfolder);
        let clipped_gpkg_path = format!("tests/res/test_{}_clipped.gpkg", subfolder);
        let result = clip_to_bb(&gpkg_path, &clipped_gpkg_path, &project_bb);
        assert_result_ok(&result, &format!("Clipping of {} failed", subfolder));
    }

    type LayerAdder = fn(&str, &str) -> Result<(), Box<dyn std::error::Error>>;
    let layers_to_add: Vec<(&str, LayerAdder)> = vec![
        ("tests/res/test_regional_clipped.gpkg", add_regional_layer),
        (
            "tests/res/test_vegetation_clipped.gpkg",
            add_vegetation_layer,
        ),
        ("tests/res/test_rpg_clipped.gpkg", add_rpg_layer),
    ];

    for (layer, add_layer_fn) in layers_to_add {
        let result = add_layer_fn(project_file_path, layer);
        assert_result_ok(&result, &format!("Adding layer {} failed", layer));
    }

    for subfolder in &topo_subfolders {
        let clipped_gpkg_path = format!("tests/res/test_{}_clipped.gpkg", subfolder);
        let result = add_topo_layer(project_file_path, &clipped_gpkg_path);
        assert_result_ok(
            &result,
            &format!("Adding topography layer {} failed", subfolder),
        );
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

    // Clean up the test resources (excluding 7z archives)
    let test_dir = Path::new("tests/res");
    for entry in fs::read_dir(test_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension != "7z" {
                    fs::remove_file(path).unwrap();
                }
            }
        }
    }
    fs::remove_dir_all("tmp").unwrap();
}
