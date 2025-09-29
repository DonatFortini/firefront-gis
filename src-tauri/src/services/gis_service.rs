use geo::{Geometry, Intersects, Relate};
use geojson::GeoJson;
use image::{DynamicImage, GenericImageView};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use crate::error::{GisError, GisResult};
use crate::services::ArchiveService;
use crate::types::{BoundingBox, Dataset, Driver, GTiff, Region, get_region};
use crate::utils::{
    LayerProgress, PathBuilder, VulcainColors, cache_dir, clean_tmp,
    create_directory_if_not_exists, execute_sidecar, projects_dir, resolution, resource_dir,
};

struct LayerConfig {
    archive_name: &'static str,
    layer_type: &'static str,
    files: Vec<&'static str>,
    order: i8,
}

impl LayerConfig {
    fn get_configs() -> Vec<Self> {
        vec![
            Self {
                archive_name: "BDFORET",
                layer_type: "Végétation",
                files: vec!["FORMATION_VEGETALE"],
                order: 1,
            },
            Self {
                archive_name: "RPG",
                layer_type: "Parcelles agricoles",
                files: vec!["PARCELLES_GRAPHIQUES"],
                order: 2,
            },
            Self {
                archive_name: "BDTOPO",
                layer_type: "Topographie",
                files: vec![
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
                ],
                order: 3,
            },
        ]
    }

    fn by_order() -> BTreeMap<i8, Vec<&'static str>> {
        let mut result = BTreeMap::new();
        for config in Self::get_configs() {
            result.insert(config.order, config.files);
        }
        result
    }
}

pub struct GisService;

impl GisService {
    pub async fn create_reference_raster(
        output_path: &str,
        project_bb: &BoundingBox,
    ) -> GisResult<()> {
        let resolution = resolution();
        let width = (project_bb.width() / resolution).ceil() as usize;
        let height = (project_bb.height() / resolution).ceil() as usize;

        if !width.is_multiple_of(500) || !height.is_multiple_of(500) {
            return Err(GisError::InvalidGeometry(
                "Width and height must be multiples of 500".to_string(),
            ));
        }

        let args = [
            "-ot",
            "Byte",
            "-outsize",
            &width.to_string(),
            &height.to_string(),
            "-bands",
            "4",
            "-burn",
            "0",
            "-burn",
            "0",
            "-burn",
            "0",
            "-burn",
            "255",
            "-a_srs",
            "EPSG:2154",
            "-a_ullr",
            &project_bb.xmin.to_string(),
            &project_bb.ymax.to_string(),
            &project_bb.xmax.to_string(),
            &project_bb.ymin.to_string(),
            "-co",
            "TILED=YES",
            "-co",
            "COMPRESS=LZW",
            "-co",
            "BIGTIFF=IF_SAFER",
            output_path,
        ];

        Driver::<GTiff>::new()
            .create(&args)
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "create_reference_raster".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    pub async fn convert_to_gpkg(input_file: &str, output_gpkg: &str) -> GisResult<()> {
        let current_dir = std::env::current_dir()?;
        let input_path = current_dir.join(input_file);
        let output_path = current_dir.join(output_gpkg);

        let args = [
            "-f",
            "GPKG",
            output_path.to_str().unwrap(),
            input_path.to_str().unwrap(),
            "-t_srs",
            "EPSG:2154",
            "-nlt",
            "PROMOTE_TO_MULTI",
            "--config",
            "OGR_GEOMETRY_ACCEPT_UNCLOSED_RING",
            "NO",
            "-dim",
            "XY",
            "--config",
            "OGR_ARC_STEPSIZE",
            "0.1",
            "--config",
            "OGR_GEOMETRY_CORRECT_UNCLOSED_RINGS",
            "YES",
        ];

        execute_sidecar("ogr2ogr", &args)
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "convert_to_gpkg".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    pub async fn merge_datasets(datasets: &[String], output_gpkg: &str) -> GisResult<()> {
        if datasets.is_empty() {
            return Err(GisError::Dataset("No datasets provided".to_string()));
        }

        if Path::new(output_gpkg).exists() {
            fs::remove_file(output_gpkg)?;
        }

        execute_sidecar("ogr2ogr", &["-f", "GPKG", output_gpkg, &datasets[0]])
            .await
            .map_err(|e| GisError::MergeFailed(e.to_string()))?;

        for dataset in datasets.iter().skip(1) {
            execute_sidecar(
                "ogr2ogr",
                &["-f", "GPKG", "-append", "-update", output_gpkg, dataset],
            )
            .await
            .map_err(|e| GisError::MergeFailed(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn clip_to_bb(
        input_gpkg: &str,
        output_gpkg: &str,
        project_bb: &BoundingBox,
    ) -> GisResult<()> {
        let current_dir = std::env::current_dir()?;
        let input_path = current_dir.join(input_gpkg);
        let output_path = current_dir.join(output_gpkg);

        let args = [
            "-f",
            "GPKG",
            output_path.to_str().unwrap(),
            input_path.to_str().unwrap(),
            "-clipsrc",
            &project_bb.xmin.to_string(),
            &project_bb.ymin.to_string(),
            &project_bb.xmax.to_string(),
            &project_bb.ymax.to_string(),
            "-nlt",
            "PROMOTE_TO_MULTI",
            "--config",
            "OGR_GEOMETRY_ACCEPT_UNCLOSED_RING",
            "NO",
            "-skipfailures",
            "--config",
            "OGR_ENABLE_PARTIAL_REPROJECTION",
            "YES",
            "--config",
            "OGR_GEOMETRY_CORRECT_UNCLOSED_RINGS",
            "YES",
        ];

        execute_sidecar("ogr2ogr", &args)
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "clip_to_bb".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    pub async fn rasterize_layer(
        project_path: &str,
        vector_gpkg: &str,
        layer_name: &str,
        output_raster: &str,
        burn_values: [&str; 3],
        where_clause: Option<&str>,
    ) -> GisResult<()> {
        let project = Dataset::open(project_path)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        let bbox = project.bbox();
        let (width, height) = project
            .raster_size()
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        let width_str = width.to_string();
        let height_str = height.to_string();
        let xmin_str = bbox.xmin.to_string();
        let ymin_str = bbox.ymin.to_string();
        let xmax_str = bbox.xmax.to_string();
        let ymax_str = bbox.ymax.to_string();

        let mut args = vec![
            "-burn",
            burn_values[0],
            "-burn",
            burn_values[1],
            "-burn",
            burn_values[2],
            "-l",
            layer_name,
            "-ts",
            &width_str,
            &height_str,
            "-te",
            &xmin_str,
            &ymin_str,
            &xmax_str,
            &ymax_str,
        ];

        if let Some(clause) = where_clause {
            args.extend_from_slice(&["-where", clause]);
        }

        args.extend_from_slice(&[vector_gpkg, output_raster]);

        execute_sidecar("gdal_rasterize", &args)
            .await
            .map_err(|e| GisError::RasterizationFailed(e.to_string()))?;

        Ok(())
    }
}

impl GisService {
    pub async fn get_project_bounding_box(project_name: &str) -> GisResult<BoundingBox> {
        let tiff_path = format!(
            "{}/{}/{}.tiff",
            projects_dir().to_string_lossy(),
            project_name,
            project_name
        );

        let (output, _) = execute_sidecar("gdalinfo", &[&tiff_path, "-json"])
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "gdalinfo".to_string(),
                message: e.to_string(),
            })?;

        let json: Value = serde_json::from_str(&output)?;

        let coords = json["cornerCoordinates"]
            .as_object()
            .ok_or_else(|| GisError::Dataset("Invalid gdalinfo output".to_string()))?;

        Ok(BoundingBox {
            xmin: coords["lowerLeft"][0].as_f64().unwrap(),
            ymin: coords["lowerLeft"][1].as_f64().unwrap(),
            xmax: coords["upperRight"][0].as_f64().unwrap(),
            ymax: coords["upperRight"][1].as_f64().unwrap(),
        })
    }

    pub async fn get_geojson_bbox(file_path: &str) -> GisResult<BoundingBox> {
        let (output, _) = execute_sidecar("ogrinfo", &["-so", "-al", file_path])
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "ogrinfo".to_string(),
                message: e.to_string(),
            })?;

        let pattern = r"Extent:\s*\(([\d.-]+),\s*([\d.-]+)\)\s*-\s*\(([\d.-]+),\s*([\d.-]+)\)";
        let caps = regex::Regex::new(pattern)?
            .captures(&output)
            .ok_or(GisError::ExtentNotFound)?;

        Ok(BoundingBox {
            xmin: caps[1].parse()?,
            ymin: caps[2].parse()?,
            xmax: caps[3].parse()?,
            ymax: caps[4].parse()?,
        })
    }
}

impl GisService {
    pub fn create_region_geojson(region_id: &str, output_path: &str) -> GisResult<()> {
        let region = get_region(region_id).map_err(|e| GisError::InvalidGeometry(e.to_string()))?;

        let geometry: geojson::Geometry = region.extent().into();

        let properties = serde_json::json!({
            "code": region.code(),
            "name": region.name(),
            "neighbors": region.neighbors()
        });

        let feature = geojson::Feature {
            bbox: None,
            geometry: Some(geometry),
            id: None,
            properties: Some(properties.as_object().unwrap().clone()),
            foreign_members: None,
        };

        let feature_collection = geojson::FeatureCollection {
            bbox: None,
            features: vec![feature],
            foreign_members: Some(
                serde_json::json!({
                    "crs": {
                        "type": "name",
                        "properties": {"name": "EPSG:2154"}
                    }
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        };

        let geojson_string = geojson::GeoJson::FeatureCollection(feature_collection).to_string();
        fs::write(output_path, geojson_string)?;

        Ok(())
    }

    pub async fn build_regions_graph(output_file: Option<&str>) -> GisResult<bool> {
        if let Some(path) = output_file
            && Path::new(path).exists()
        {
            println!("Loading regions graph from cache: {}", path);
            let json_str = fs::read_to_string(path)?;
            let _: HashMap<String, Region> = serde_json::from_str(&json_str)?;
            return Ok(true);
        }

        let geojson_path = resource_dir().join("regions.geojson");
        let geojson_str = fs::read_to_string(&geojson_path)?;
        let geojson: GeoJson = geojson_str
            .parse()
            .map_err(|e| GisError::InvalidGeometry(format!("Failed to parse GeoJSON: {:?}", e)))?;

        let feature_collection = match geojson {
            GeoJson::FeatureCollection(fc) => fc,
            _ => {
                return Err(GisError::InvalidGeometry(
                    "Not a FeatureCollection".to_string(),
                ));
            }
        };

        let mut regions_info: HashMap<String, Region> = HashMap::new();
        let total = feature_collection.features.len();

        println!("Parsing {} features...", total);

        for (idx, feature) in feature_collection.features.iter().enumerate() {
            if idx % 100 == 0 {
                print!(
                    "\rProgress: {}/{} ({:.1}%)",
                    idx,
                    total,
                    (idx as f64 / total as f64) * 100.0
                );
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            }

            let Some(code) = feature.property("code").and_then(|v| v.as_str()) else {
                continue;
            };
            let name = feature
                .property("nom")
                .and_then(|v| v.as_str())
                .unwrap_or(code);
            let Some(geometry) = &feature.geometry else {
                continue;
            };

            let geojson_value = serde_json::to_value(geometry)?;
            let gdal_geom: Geometry = serde_json::to_string(&geojson_value)?
                .parse::<geojson::Geometry>()
                .map_err(|e| GisError::InvalidGeometry(format!("Geometry parse error: {:?}", e)))?
                .try_into()
                .map_err(|e| {
                    GisError::InvalidGeometry(format!("Geometry conversion error: {:?}", e))
                })?;

            regions_info.insert(
                code.to_string(),
                Region::new(code.to_string(), name.to_string(), gdal_geom),
            );
        }

        let codes: Vec<String> = regions_info.keys().cloned().collect();
        let total_comparisons = (codes.len() * (codes.len() - 1)) / 2;
        let mut done = 0;

        for i in 0..codes.len() {
            let code_i = &codes[i];
            let geom_i = regions_info[code_i].extent().clone();

            for code_j in &codes[i + 1..] {
                let geom_j = regions_info[code_j].extent().clone();

                if geom_i.intersects(&geom_j) || geom_i.relate(&geom_j).is_touches() {
                    regions_info
                        .get_mut(code_i)
                        .unwrap()
                        .add_neighbor(code_j.clone());
                    regions_info
                        .get_mut(code_j)
                        .unwrap()
                        .add_neighbor(code_i.clone());
                }

                done += 1;
                if done % 1000 == 0 {
                    print!(
                        "\rComparisons: {}/{} ({:.1}%)",
                        done,
                        total_comparisons,
                        (done as f64 / total_comparisons as f64) * 100.0
                    );
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                    tokio::task::yield_now().await;
                }
            }
        }

        if let Some(path) = output_file {
            let json_str = serde_json::to_string_pretty(&regions_info)?;
            fs::write(path, json_str)?;
            println!("\nRegions graph saved to: {}", path);
        }

        Ok(true)
    }
}

impl GisService {
    pub async fn prepare_layers(
        project_bb: &BoundingBox,
        code: &str,
    ) -> GisResult<(String, String, String, HashMap<String, Vec<String>>)> {
        let path_builder = PathBuilder::new();
        let mut layer_progress = LayerProgress::new("Préparation des Couches", 4);

        layer_progress.next_layer("étendue régionale");
        let regional_gpkg = Self::prepare_regional_layer(&path_builder, code, project_bb).await?;

        let mut vegetation_gpkg = String::new();
        let mut rpg_gpkg = String::new();
        let mut topo_gpkgs: HashMap<String, Vec<String>> = HashMap::new();

        for config in LayerConfig::get_configs() {
            layer_progress.next_layer(&format!("couches {}", config.layer_type));

            let archive_path =
                path_builder.cache_file(&format!("{}_{}.7z", config.archive_name, code));

            for (file_idx, file) in config.files.iter().enumerate() {
                layer_progress.layer_operation(
                    file,
                    "Traitement",
                    file_idx + 1,
                    config.files.len(),
                );

                let output_gpkg =
                    Self::process_layer_file(&path_builder, &archive_path, file, code, project_bb)
                        .await?;

                match config.order {
                    1 => vegetation_gpkg = output_gpkg,
                    2 => rpg_gpkg = output_gpkg,
                    3 => topo_gpkgs
                        .entry(file.to_string())
                        .or_default()
                        .push(output_gpkg),
                    _ => {}
                }
            }
        }

        Ok((regional_gpkg, vegetation_gpkg, rpg_gpkg, topo_gpkgs))
    }

    async fn prepare_regional_layer(
        path_builder: &PathBuilder,
        code: &str,
        project_bb: &BoundingBox,
    ) -> GisResult<String> {
        let geojson_path = path_builder.temp_file(code, "geojson");
        let temp_gpkg = path_builder.temp_file(code, "gpkg");
        let output_gpkg = path_builder.temp_file(&format!("{}_region", code), "gpkg");

        Self::create_region_geojson(code, &geojson_path)?;
        Self::convert_to_gpkg(&geojson_path, &temp_gpkg).await?;
        Self::clip_to_bb(&temp_gpkg, &output_gpkg, project_bb).await?;

        Ok(output_gpkg)
    }

    async fn process_layer_file(
        path_builder: &PathBuilder,
        archive_path: &str,
        file: &str,
        code: &str,
        project_bb: &BoundingBox,
    ) -> GisResult<String> {
        ArchiveService::extract_files_by_name(archive_path, file, &path_builder.temp_dir)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        let shp_file = format!("{}/{}/{}.shp", path_builder.temp_dir, file, file);
        let temp_gpkg = path_builder.temp_file(file, "gpkg");
        Self::convert_to_gpkg(&shp_file, &temp_gpkg).await?;

        let output_gpkg = path_builder.temp_file(&format!("{}_{}", code, file), "gpkg");
        Self::clip_to_bb(&temp_gpkg, &output_gpkg, project_bb).await?;

        Ok(output_gpkg)
    }
}

impl GisService {
    pub async fn add_all_layers(project_file_path: &str) -> GisResult<()> {
        let project_folder = Path::new(project_file_path)
            .parent()
            .ok_or_else(|| GisError::InvalidGeometry("Invalid project path".to_string()))?
            .to_string_lossy()
            .to_string();

        let project_name = Path::new(project_file_path)
            .file_stem()
            .ok_or_else(|| GisError::InvalidGeometry("Invalid project name".to_string()))?
            .to_string_lossy()
            .to_string();

        let path_builder = PathBuilder::new();
        let mut processor = LayerProcessor::new();
        let mut layer_progress = LayerProgress::new("Ajout des Couches", 4);

        layer_progress.next_layer("couche régionale");
        let regional_path = path_builder.project_resource(&project_folder, &project_name);
        processor
            .apply_black_layer(project_file_path, &regional_path, "regional")
            .await?;

        for (order, files) in LayerConfig::by_order() {
            let layer_type = LayerConfig::get_configs()
                .iter()
                .find(|l| l.order == order)
                .map(|l| l.layer_type)
                .unwrap_or("Inconnu");

            layer_progress.next_layer(&format!("couches {}", layer_type));

            for (idx, file) in files.iter().enumerate() {
                layer_progress.layer_operation(
                    &format!("couches {}", layer_type),
                    &format!("Ajout de {}", file),
                    idx + 1,
                    files.len(),
                );

                let layer_path = path_builder.project_resource(&project_folder, file);

                match order {
                    1 => {
                        processor
                            .apply_vegetation_layer(project_file_path, &layer_path)
                            .await?
                    }
                    2 => {
                        processor
                            .apply_colored_layer(
                                project_file_path,
                                &layer_path,
                                VulcainColors["Brousaille"],
                                "rpg",
                            )
                            .await?
                    }
                    3 => {
                        if Path::new(&layer_path).exists() {
                            let dataset = Dataset::open(&layer_path)
                                .await
                                .map_err(|e| GisError::Dataset(e.to_string()))?;
                            if let Some(count) = dataset
                                .feature_count(0)
                                .map_err(|e| GisError::Dataset(e.to_string()))?
                                && count > 0
                            {
                                processor
                                    .apply_black_layer(project_file_path, &layer_path, "topo")
                                    .await?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Self::integrity_check(project_file_path).await?;
        Ok(())
    }

    async fn integrity_check(project_file_path: &str) -> GisResult<()> {
        let (stdout, _) = execute_sidecar(
            "magick",
            &[project_file_path, "-format", "%c", "histogram:info:"],
        )
        .await
        .map_err(|e| GisError::Dataset(e.to_string()))?;

        let mut corrupted = false;
        for line in stdout.lines() {
            if let Some(start) = line.find('(')
                && let Some(end) = line[start..].find(')')
            {
                let rgb_str = &line[start + 1..start + end];
                let rgb_parts: Vec<&str> = rgb_str.split(',').collect();
                if rgb_parts.len() >= 3 {
                    let rgb = [
                        rgb_parts[0].trim(),
                        rgb_parts[1].trim(),
                        rgb_parts[2].trim(),
                    ];
                    if !VulcainColors.values().any(|c| c == &rgb) {
                        corrupted = true;
                        break;
                    }
                }
            }
        }

        if corrupted {
            println!("Warning: some colors are not in VulcainColors");
        } else {
            println!("All layer colors are valid");
        }

        Ok(())
    }
}

impl GisService {
    pub async fn fetch_orthophoto(output_path: &str, project_bb: &BoundingBox) -> GisResult<()> {
        let wms_cache = cache_dir().join("wms_cache");
        create_directory_if_not_exists(&wms_cache.to_string_lossy())
            .map_err(|e| GisError::WmsFetchFailed(e.to_string()))?;

        let res = resolution();
        let width = ((project_bb.xmax - project_bb.xmin) / res).ceil() as usize;
        let height = ((project_bb.ymax - project_bb.ymin) / res).ceil() as usize;

        println!("Dimensions: width={}, height={} pixels", width, height);

        let cache_key = format!(
            "{:.6}_{:.6}_{:.6}_{:.6}_{}x{}",
            project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
        );
        let cache_file = wms_cache.join(format!("satellite_{}.jpg", cache_key));

        if cache_file.exists() {
            if let Ok(metadata) = fs::metadata(&cache_file)
                && metadata.len() > 0
            {
                fs::copy(&cache_file, output_path)?;
                println!("Retrieved from cache: {} bytes", metadata.len());
                return Ok(());
            }
            let _ = fs::remove_file(&cache_file);
        }

        let wms_url = format!(
            "https://data.geopf.fr/wms-r/wms?\
            SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&\
            LAYERS=ORTHOIMAGERY.ORTHOPHOTOS&STYLES=&CRS=EPSG:2154&\
            BBOX={},{},{},{}&WIDTH={}&HEIGHT={}&FORMAT=image/jpeg",
            project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent("Rust WMS Client")
            .build()
            .map_err(|e| GisError::WmsFetchFailed(e.to_string()))?;

        let mut image_data = Vec::new();
        let max_attempts = 3;

        for attempt in 1..=max_attempts {
            println!("Download attempt {}/{}", attempt, max_attempts);

            match Self::download_wms_image(&client, &wms_url).await {
                Ok(data) => {
                    image_data = data;
                    break;
                }
                Err(e) if attempt < max_attempts => {
                    println!("Attempt {} failed: {}", attempt, e);
                    sleep(Duration::from_secs(5));
                }
                Err(e) => return Err(e),
            }
        }

        let temp_cache = format!("{}.tmp", cache_file.to_string_lossy());
        fs::write(&temp_cache, &image_data)?;
        fs::rename(&temp_cache, &cache_file)?;
        fs::copy(&cache_file, output_path)?;

        println!("Orthophoto downloaded: {} bytes", image_data.len());
        Ok(())
    }

    async fn download_wms_image(client: &reqwest::Client, url: &str) -> GisResult<Vec<u8>> {
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| GisError::WmsFetchFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(GisError::WmsFetchFailed(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("");

        if !content_type.starts_with("image/") {
            let error_text = response
                .text()
                .await
                .map_err(|e| GisError::WmsFetchFailed(e.to_string()))?;
            return Err(GisError::WmsFetchFailed(format!(
                "Server error: {}",
                &error_text[..error_text.len().min(200)]
            )));
        }

        let image_data = response
            .bytes()
            .await
            .map_err(|e| GisError::WmsFetchFailed(e.to_string()))?
            .to_vec();

        if image_data.len() < 10 || image_data[0] != 0xFF || image_data[1] != 0xD8 {
            return Err(GisError::WmsFetchFailed("Invalid JPEG data".to_string()));
        }

        Ok(image_data)
    }
}

impl GisService {
    pub async fn slice_images(project_name: &str, slice_factor: u32) -> GisResult<String> {
        let project_dir = projects_dir().join(project_name);
        let slice_dir = project_dir.join("slices");

        create_directory_if_not_exists(&slice_dir.to_string_lossy())
            .map_err(|e| GisError::SliceFailed(e.to_string()))?;

        let veget_path = project_dir.join(format!("{}_VEGET.jpeg", project_name));
        let ortho_path = project_dir.join(format!("{}_ORTHO.jpeg", project_name));

        let veget_image = Self::load_image(&veget_path)?;
        let ortho_image = Self::load_image(&ortho_path)?;

        let project_bb = Self::get_project_bounding_box(project_name).await?;
        let (base_x, base_y) = (
            (project_bb.xmin / 1000.0) as u32,
            (project_bb.ymin / 1000.0) as u32,
        );

        Self::process_slices(
            &veget_image,
            &ortho_image,
            &slice_dir,
            slice_factor,
            base_x,
            base_y,
        )?;

        Ok("Slicing completed".to_string())
    }

    fn load_image(path: &Path) -> GisResult<DynamicImage> {
        image::ImageReader::open(path)
            .map_err(|e| GisError::ImageProcessing(format!("Failed to open: {}", e)))?
            .decode()
            .map_err(|e| GisError::ImageProcessing(format!("Failed to decode: {}", e)))
    }

    fn process_slices(
        veget: &DynamicImage,
        ortho: &DynamicImage,
        slice_dir: &Path,
        factor: u32,
        base_x: u32,
        base_y: u32,
    ) -> GisResult<()> {
        let (width, height) = veget.dimensions();

        for img_y in (0..height).step_by(factor as usize).rev() {
            for img_x in (0..width).step_by(factor as usize) {
                if img_x + factor > width || img_y + factor > height {
                    continue;
                }

                let cropped_veget = veget.crop_imm(img_x, img_y, factor, factor);
                let cropped_ortho = ortho.crop_imm(img_x, img_y, factor, factor);

                let coord_x = base_x + img_x / 100;
                let coord_y = base_y + (height - img_y - factor) / 100;

                let veget_file =
                    slice_dir.join(format!("{}_{}_{}_veget.jpg", coord_x, coord_y, factor));
                let ortho_file = slice_dir.join(format!(
                    "{}_{}_{}_{}.jpg",
                    coord_x, coord_y, factor, "ortho"
                ));

                cropped_veget
                    .save(&veget_file)
                    .map_err(|e| GisError::SliceFailed(e.to_string()))?;
                cropped_ortho
                    .save(&ortho_file)
                    .map_err(|e| GisError::SliceFailed(e.to_string()))?;
            }
        }

        Ok(())
    }
}

struct LayerProcessor {
    path_builder: PathBuilder,
}

impl LayerProcessor {
    fn new() -> Self {
        Self {
            path_builder: PathBuilder::new(),
        }
    }

    async fn apply_black_layer(
        &mut self,
        project_file: &str,
        gpkg_path: &str,
        prefix: &str,
    ) -> GisResult<()> {
        let temp_layer = self
            .path_builder
            .temp_file(&format!("temp_{}", prefix), "tif");
        let layer_name = Dataset::open(gpkg_path)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .layer_name(0)
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        GisService::rasterize_layer(
            project_file,
            gpkg_path,
            &layer_name,
            &temp_layer,
            ["255", "255", "255"],
            None,
        )
        .await?;

        use crate::gis_operation::Overlay;
        let mut overlay = Overlay::new();
        overlay
            .apply_overlay(project_file, &temp_layer, |&v| v > 0, Some([0, 0, 0]))
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        std::fs::remove_file(temp_layer)?;
        Ok(())
    }

    async fn apply_colored_layer(
        &mut self,
        project_file: &str,
        gpkg_path: &str,
        color: [&str; 3],
        prefix: &str,
    ) -> GisResult<()> {
        let temp_layer = self
            .path_builder
            .temp_file(&format!("temp_{}", prefix), "tif");
        let layer_name = Dataset::open(gpkg_path)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .layer_name(0)
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        GisService::rasterize_layer(
            project_file,
            gpkg_path,
            &layer_name,
            &temp_layer,
            color,
            None,
        )
        .await?;

        use crate::gis_operation::Overlay;
        let mut overlay = Overlay::new();
        overlay
            .apply_overlay(project_file, &temp_layer, |&v| v > 0, None)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        std::fs::remove_file(temp_layer)?;
        Ok(())
    }

    async fn apply_vegetation_layer(
        &mut self,
        project_file: &str,
        veg_gpkg: &str,
    ) -> GisResult<()> {
        let vegetation_types = vec![
            (
                vec![
                    "Feuillus",
                    "Châtaignier",
                    "Chênes sempervirents",
                    "Chênes décidus",
                    "Hêtre",
                ],
                VulcainColors["Chêne"],
                "feuillus",
            ),
            (vec!["NC", "NR"], VulcainColors["Brousaille"], "undefined"),
        ];

        for (types, color, prefix) in vegetation_types {
            let where_clause = format!(
                "ESSENCE IN ({})",
                types
                    .iter()
                    .map(|t| format!("'{}'", t))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            let temp_layer = self.path_builder.temp_file(prefix, "tif");
            let layer_name = Dataset::open(veg_gpkg)
                .await
                .map_err(|e| GisError::Dataset(e.to_string()))?
                .layer_name(0)
                .map_err(|e| GisError::Dataset(e.to_string()))?;

            GisService::rasterize_layer(
                project_file,
                veg_gpkg,
                &layer_name,
                &temp_layer,
                color,
                Some(&where_clause),
            )
            .await?;

            use crate::gis_operation::Overlay;
            let mut overlay = Overlay::new();
            overlay
                .apply_overlay(project_file, &temp_layer, |&v| v > 0, None)
                .await
                .map_err(|e| GisError::Dataset(e.to_string()))?;

            std::fs::remove_file(temp_layer)?;
        }

        let all_defined: Vec<String> = [
            "Feuillus",
            "Châtaignier",
            "Chênes sempervirents",
            "Chênes décidus",
            "Hêtre",
            "NC",
            "NR",
        ]
        .iter()
        .map(|t| format!("'{}'", t))
        .collect();

        let other_where = format!("ESSENCE NOT IN ({})", all_defined.join(", "));
        let temp_layer = self.path_builder.temp_file("other", "tif");
        let layer_name = Dataset::open(veg_gpkg)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?
            .layer_name(0)
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        GisService::rasterize_layer(
            project_file,
            veg_gpkg,
            &layer_name,
            &temp_layer,
            VulcainColors["Pin"],
            Some(&other_where),
        )
        .await?;

        use crate::gis_operation::Overlay;
        let mut overlay = Overlay::new();
        overlay
            .apply_overlay(project_file, &temp_layer, |&v| v > 0, None)
            .await
            .map_err(|e| GisError::Dataset(e.to_string()))?;

        std::fs::remove_file(temp_layer)?;
        clean_tmp(None).map_err(|e| GisError::Dataset(e.to_string()))?;

        Ok(())
    }
}
