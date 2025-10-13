use std::fs;

use crate::error::{GisError, GisResult};
use crate::types::BoundingBox;
use crate::utils::execute_sidecar;

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
        where_clause: Option<&str>,
    ) -> GisResult<()> {
        let current_dir = std::env::current_dir()?;
        let input_path = current_dir.join(input_gpkg);
        let output_path = current_dir.join(output_gpkg);

        let xmin = project_bb.xmin.to_string();
        let ymin = project_bb.ymin.to_string();
        let xmax = project_bb.xmax.to_string();
        let ymax = project_bb.ymax.to_string();

        let mut args = vec![
            "-f",
            "GPKG",
            output_path.to_str().unwrap(),
            input_path.to_str().unwrap(),
            "-clipsrc",
            &xmin,
            &ymin,
            &xmax,
            &ymax,
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

        if let Some(clause) = where_clause {
            args.extend(&["-where", clause]);
        }

        execute_sidecar("ogr2ogr", &args)
            .await
            .map_err(|e| GisError::GdalOperation {
                operation: "clip_to_bb".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }
}
