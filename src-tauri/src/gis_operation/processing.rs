use crate::gis_operation::Overlay;
use crate::types::Dataset;
use crate::utils::executor;

/// Convertit une couche vectorielle en raster en utilisant gdal_rasterize
///
/// # Arguments
///
/// * `project` - dataset du projet
/// * `vector_gpkg` - chemin du fichier GeoPackage contenant la couche vectorielle
/// * `layer_name` - nom de la couche à rasteriser
/// * `output_raster` - chemin du fichier raster de sortie
/// * `burn_values` - valeurs à appliquer pour chaque bande (RGB)
/// * `where_clause` - clause WHERE SQL optionnelle pour filtrer les entités
/// * `additional_args` - arguments supplémentaires pour gdal_rasterize
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si la rastérisation a réussi ou échoué
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

/// Applique une superposition de couches raster sur un projet
/// Cette fonction est le cœur de la logique de combinaison des données:
/// - Lecture des données du projet de base et de la couche de superposition
/// - Création d'un masque pour déterminer où la superposition doit être appliquée
/// - Pour chaque pixel, si le masque est vrai, utilisation de la valeur de superposition,
///   sinon conservation de la valeur originale
/// - Écriture du résultat dans un nouveau fichier qui remplacera le projet original
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `overlay_raster_path` - chemin du fichier raster de superposition
/// * `mask_condition` - fonction pour déterminer si un pixel doit être inclus dans le masque
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si la superposition a réussi ou échoué
pub async fn apply_overlay<F>(
    project_file_path: &str,
    overlay_raster_path: &str,
    mask_condition: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(&u8) -> bool,
{
    let mut overlay_processor = Overlay::new();
    overlay_processor
        .apply_overlay(project_file_path, overlay_raster_path, mask_condition)
        .await
}

pub async fn apply_black_overlay<F>(
    project_file_path: &str,
    mask_raster_path: &str,
    mask_condition: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(&u8) -> bool,
{
    let mut overlay_processor = Overlay::new();
    overlay_processor
        .apply_overlay_with_fixed_color(
            project_file_path,
            mask_raster_path,
            mask_condition,
            [0, 0, 0],
        )
        .await
}

pub mod prelude {
    pub use super::{apply_black_overlay, apply_overlay, rasterize_layer};
}
