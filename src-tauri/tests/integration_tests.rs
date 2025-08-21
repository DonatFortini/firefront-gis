mod common;

use common::*;

use firefront_gis_lib::gis_operation::layers::{
    add_regional_layer, add_rpg_layer, add_topo_layer, add_vegetation_layer,
};
use firefront_gis_lib::gis_operation::regions::create_region_geojson;
use firefront_gis_lib::gis_operation::{clip_to_bb, convert_to_gpkg, create_project};
use firefront_gis_lib::utils::{create_directory_if_not_exists, extract_files_by_name};
use gdal::Dataset;
use std::fs;
use std::path::Path;

#[tokio::test]
async fn test_end_to_end_workflow() {
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
        let result = extract_files_by_name(archive, folder, "tmp").await;
        assert_result_ok(&result, &format!("Extraction of {folder} failed"));
        assert_file_exists(expected_file, &format!("{folder} was not created"));
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
        let result = extract_files_by_name("tests/res/BDTOPO_2A.7z", subfolder, "tmp").await;
        assert_result_ok(&result, &format!("Extraction of {subfolder} failed"));
        assert_file_exists(
            &format!("tmp/{subfolder}/{subfolder}.shp"),
            &format!("{subfolder} shapefile was not created"),
        );
    }
    let result = create_region_geojson("2A", "tmp/2A.geojson");
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
        let result = convert_to_gpkg(input, output).await;
        assert_result_ok(
            &result,
            &format!("Conversion of {input} to GeoPackage failed"),
        );
    }

    for subfolder in &topo_subfolders {
        let shapefile_path = format!("tmp/{subfolder}/{subfolder}.shp");
        let gpkg_path = format!("tests/res/test_{subfolder}.gpkg");
        let result = convert_to_gpkg(&shapefile_path, &gpkg_path).await;
        assert_result_ok(
            &result,
            &format!("Conversion of {subfolder} to GeoPackage failed"),
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
        let result = clip_to_bb(input, output, &project_bb).await;
        assert_result_ok(&result, &format!("Clipping of {input} failed"));
    }

    for subfolder in &topo_subfolders {
        let gpkg_path = format!("tests/res/test_{subfolder}.gpkg");
        let clipped_gpkg_path = format!("tests/res/test_{subfolder}_clipped.gpkg");
        let result = clip_to_bb(&gpkg_path, &clipped_gpkg_path, &project_bb).await;
        assert_result_ok(&result, &format!("Clipping of {subfolder} failed"));
    }

    let layers_to_add = vec![
        ("tests/res/test_regional_clipped.gpkg", "regional"),
        ("tests/res/test_vegetation_clipped.gpkg", "vegetation"),
        ("tests/res/test_rpg_clipped.gpkg", "rpg"),
    ];

    for (layer, layer_type) in layers_to_add {
        let result = match layer_type {
            "regional" => add_regional_layer(project_file_path, layer).await,
            "vegetation" => add_vegetation_layer(project_file_path, layer).await,
            "rpg" => add_rpg_layer(project_file_path, layer).await,
            _ => unreachable!(),
        };
        assert_result_ok(&result, &format!("Adding layer {layer} failed"));
    }

    for subfolder in &topo_subfolders {
        let clipped_gpkg_path = format!("tests/res/test_{subfolder}_clipped.gpkg");
        let result = add_topo_layer(project_file_path, &clipped_gpkg_path).await;
        assert_result_ok(
            &result,
            &format!("Adding topography layer {subfolder} failed"),
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
        "Resolution is not 10 meters per pixel: pixel_size_x = {pixel_size_x}, pixel_size_y = {pixel_size_y}"
    );

    let test_dir = Path::new("tests/res");
    for entry in fs::read_dir(test_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file()
            && let Some(extension) = path.extension()
            && extension != "7z"
        {
            fs::remove_file(path).unwrap();
        }
    }
    fs::remove_dir_all("tmp").unwrap();
}
