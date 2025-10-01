use serde_json::Value;
use std::fs;

use crate::error::{GisError, GisResult};
use crate::types::BoundingBox;
use crate::utils::{execute_sidecar, projects_dir};

pub struct VectorService;

impl VectorService {
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

        if std::path::Path::new(output_gpkg).exists() {
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
