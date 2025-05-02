use std::process::Command;

use gdal::{Dataset, DriverManager};

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
pub fn rasterize_layer(
    project: &Dataset,
    vector_gpkg: &str,
    layer_name: &str,
    output_raster: &str,
    burn_values: [&str; 3],
    where_clause: Option<&str>,
    additional_args: Option<Vec<&str>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let geo_transform = project.geo_transform()?;
    let (width, height) = project.raster_size();

    let xmin = geo_transform[0].to_string();
    let ymin = (geo_transform[3] + geo_transform[5] * height as f64).to_string();
    let xmax = (geo_transform[0] + geo_transform[1] * width as f64).to_string();
    let ymax = geo_transform[3].to_string();

    let (arg_width, arg_height) = (&width.to_string(), &height.to_string());
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
        arg_width,
        arg_height,
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
    // FIXME : add the cross-platform support
    let status = Command::new("gdal_rasterize").args(args).status()?;

    if !status.success() {
        return Err("gdal_rasterize failed".into());
    }

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
pub fn apply_overlay<F>(
    project_file_path: &str,
    overlay_raster_path: &str,
    mask_condition: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(&u8) -> bool,
{
    let project = Dataset::open(project_file_path)?;
    let overlay_raster = Dataset::open(overlay_raster_path)?;

    let output_file = "tmp/output.tif";
    let driver_manager = DriverManager::get_driver_by_name("GTiff")?;

    let mut output_dataset = driver_manager.create(
        output_file,
        project.raster_size().0,
        project.raster_size().1,
        4,
    )?;

    output_dataset.set_geo_transform(&project.geo_transform()?)?;
    output_dataset.set_projection(&project.projection())?;

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

    let (width, height) = project.raster_size();
    let size = width * height;
    let mut mask = vec![false; size];

    for band in &overlay_bands {
        let band_data: Vec<u8> = band
            .read_as::<u8>((0, 0), (width, height), (width, height), None)?
            .data()
            .to_vec();

        for (i, value) in band_data.iter().enumerate() {
            if mask_condition(value) {
                mask[i] = true;
            }
        }
    }

    for (i, base_band) in base_data.iter().enumerate() {
        let mut out_band = output_dataset.rasterband(i + 1)?;
        let base_band_data: Vec<u8> = base_band
            .read_as::<u8>((0, 0), (width, height), (width, height), None)?
            .data()
            .to_vec();

        let data = if i < overlay_bands.len() {
            let overlay_band_data: Vec<u8> = overlay_bands[i]
                .read_as::<u8>((0, 0), (width, height), (width, height), None)?
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

        out_band.write(
            (0, 0),
            (width, height),
            &mut gdal::raster::Buffer::new((width, height), data),
        )?;
    }

    output_dataset.close().unwrap();
    overlay_raster.close().unwrap();
    project.close().unwrap();

    std::fs::rename(output_file, project_file_path)?;

    Ok(())
}
