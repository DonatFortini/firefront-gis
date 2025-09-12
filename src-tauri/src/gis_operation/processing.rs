use crate::types::{Dataset, Driver, GTiff};
use crate::utils::{executor, temp_dir};

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
    let project = Dataset::open(project_file_path).await?;
    let overlay_raster = Dataset::open(overlay_raster_path).await?;
    let (width, height) = project.raster_size()?;

    let output_file = format!("{}/output.tif", temp_dir().to_string_lossy());

    let args = [
        "-ot",
        "Byte",
        "-outsize",
        &width.to_string(),
        &height.to_string(),
        "-bands",
        "4",
        "-a_srs",
        &project.projection().to_string(),
        "-a_ullr",
        &project.bbox().xmin.to_string(),
        &project.bbox().ymax.to_string(),
        &project.bbox().xmax.to_string(),
        &project.bbox().ymin.to_string(),
        "-co",
        "TILED=YES",
        "-co",
        "COMPRESS=LZW",
        "-co",
        "BIGTIFF=IF_SAFER",
        &output_file,
    ];

    Driver::<GTiff>::new().create(&args).await?;
    let output_dataset = Dataset::open(&output_file).await?;

    let base_data = [
        project.rasterband(1)?,
        project.rasterband(2)?,
        project.rasterband(3)?,
        project.rasterband(4)?,
    ];

    let overlay_bands = [
        overlay_raster.rasterband(1)?,
        overlay_raster.rasterband(2)?,
        overlay_raster.rasterband(3)?,
    ];

    let (width, height) = project.raster_size()?;
    let size = width * height;
    let mut mask = vec![false; size];

    for band in &overlay_bands {
        let band_data: Vec<u8> = band
            .read_as::<u8>((0, 0), (width, height), (width, height), None)
            .await?
            .data()
            .to_vec();

        for (i, value) in band_data.iter().enumerate() {
            if mask_condition(value) {
                mask[i] = true;
            }
        }
    }

    for (i, base_band) in base_data.iter().enumerate() {
        let mut out_band = output_dataset.rasterband(((i + 1) as isize).try_into().unwrap())?;
        let base_band_data: Vec<u8> = base_band
            .read_as::<u8>((0, 0), (width, height), (width, height), None)
            .await?
            .data()
            .to_vec();

        let data = if i < overlay_bands.len() {
            let overlay_band_data: Vec<u8> = overlay_bands[i]
                .read_as::<u8>((0, 0), (width, height), (width, height), None)
                .await?
                .data()
                .to_vec();

            base_band_data
                .iter()
                .zip(overlay_band_data.iter())
                .zip(mask.iter())
                .map(|((&base_value, &overlay_value), &mask_value)| {
                    if mask_value {
                        overlay_value
                    } else {
                        base_value
                    }
                })
                .collect::<Vec<u8>>()
        } else {
            base_band_data
        };

        out_band
            .write(
                (0, 0),
                (width, height),
                &mut gdal::raster::Buffer::new((width, height), data),
            )
            .await?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::fs::rename(output_file, project_file_path)?;
    }
    #[cfg(target_os = "windows")]
    {
        std::fs::copy(output_file, project_file_path)?;
        std::fs::remove_file(output_file)?;
    }

    Ok(())
}

pub mod prelude {
    pub use super::{apply_overlay, rasterize_layer};
}
