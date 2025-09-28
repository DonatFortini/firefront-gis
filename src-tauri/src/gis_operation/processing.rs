use crate::types::Dataset;
use crate::utils::{VulcainColors, executor};

/// Rasterise une couche vectorielle pour correspondre à l'étendue et à la résolution d'un raster de projet.
/// # Arguments
/// - `project_path`: Chemin vers le fichier raster du projet.
/// - `vector_gpkg`: Chemin vers le fichier vectoriel GPKG.
/// - `layer_name`: Nom de la couche dans le GPKG à rasteriser.
/// - `output_raster`: Chemin pour enregistrer le fichier raster de sortie.
/// - `burn_values`: Tableau de trois chaînes représentant les valeurs de brûlage pour R, G, B.
/// - `where_clause`: Clause SQL WHERE optionnelle pour filtrer les entités.
/// - `additional_args`: Arguments supplémentaires optionnels pour gdal_rasterize.
/// # Retour
/// - Result<(), Box<dyn std::error::Error>>: Ok si réussi, Err sinon.
pub async fn rasterize_layer(
    project_path: &str,
    vector_gpkg: &str,
    layer_name: &str,
    output_raster: &str,
    burn_values: [&str; 3],
    where_clause: Option<&str>,
    additional_args: Option<Vec<&str>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_info = Dataset::open(project_path).await?;

    let bbox = project_info.bbox();
    let xmin = bbox.xmin.to_string();
    let ymin = bbox.ymin.to_string();
    let xmax = bbox.xmax.to_string();
    let ymax = bbox.ymax.to_string();

    let width_str = project_info.raster_size().unwrap().0.to_string();
    let height_str = project_info.raster_size().unwrap().1.to_string();

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
        width_str.as_str(),
        height_str.as_str(),
        "-te",
        &xmin,
        &ymin,
        &xmax,
        &ymax,
    ];

    if let Some(clause) = where_clause {
        args.push("-where");
        args.push(clause);
    }

    if let Some(extra_args) = additional_args {
        args.extend(extra_args);
    }

    args.push(vector_gpkg);
    args.push(output_raster);

    executor("gdal_rasterize", &args).await?;
    Ok(())
}

pub async fn integrity_check(project_file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (stdout, _stderr) = executor(
        "magick",
        &[project_file_path, "-format", "%c", "histogram:info:"],
    )
    .await?;

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
        println!("Corrupted layers: some colors are not in VulcainColors");
    } else {
        println!("All layers colors are valid");
    }
    Ok(())
}

pub mod prelude {
    pub use super::{integrity_check, rasterize_layer};
}
