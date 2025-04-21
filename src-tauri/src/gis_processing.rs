use gdal::spatial_ref::SpatialRef;
use gdal::vector::LayerAccess;
use gdal::Dataset;
use gdal::DriverManager;
use gdal_sys::OGRwkbGeometryType;
use serde_json::json;
use serde_json::Value;
use std::env::current_dir;
use std::fs;
use std::process::Command;

use crate::utils::create_directory_if_not_exists;

/// Crée un projet de carte avec une résolution donnée (10m/pixel)
/// et calcule la taille de l'image en fonction des coordonnées
/// xmin, ymin, xmax, ymax
/// et une projection Lambert-93 (EPSG:2154)
/// avec 4 bandes (RGB + alpha)
///
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier de projet à créer
/// * `xmin` - coordonnée x minimale
/// * `ymin` - coordonnée y minimale
/// * `xmax` - coordonnée x maximale
/// * `ymax` - coordonnée y maximale
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si la création du projet a réussi ou échoué
///
/// # Example
///
/// ```rust
///
/// let project_file_path = "tests/test1.tiff";
///
/// let (xmin, ymin, xmax, ymax) = (1210000.0, 6070000.0, 1235000.0, 6095000.0);
///
/// let result = create_project(project_file_path, xmin, ymin, xmax, ymax);
/// match result {
///     Ok(_) => println!("Project created successfully"),
///     Err(e) => println!("Error creating project: {}", e),
///}
///
/// ```
///
pub fn create_project(
    project_file_path: &str,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolution = 10.0;
    let width = ((xmax - xmin) / resolution).ceil() as usize;
    let height = ((ymax - ymin) / resolution).ceil() as usize;

    if width != height {
        return Err("Width and height must be equal for square project".into());
    }

    let driver = DriverManager::get_driver_by_name("GTiff")?;
    let mut dataset = driver.create(project_file_path, height, width, 4)?;
    let geotransform = [xmin, resolution, 0.0, ymax, 0.0, -resolution];
    dataset.set_geo_transform(&geotransform)?;
    let srs = SpatialRef::from_epsg(2154)?;
    dataset.set_projection(&srs.to_wkt()?)?;

    for band_idx in 1..=3 {
        let mut band = dataset.rasterband(band_idx)?;
        band.fill(0.0, None)?;
    }
    let mut band = dataset.rasterband(4)?;
    band.fill(255.0, None)?;

    Ok(())
}

/// Convertit un fichier en format GeoPackage (GPKG) en utilisant ogr2ogr
///
/// # Arguments
///
/// * `input_file` - chemin du fichier d'entrée
/// * `output_gpkg` - chemin du fichier GeoPackage de sortie
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si la conversion a réussi ou échoué
///
pub fn convert_to_gpkg(
    input_file: &str,
    output_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    let input_file_path = current_dir.join(input_file);
    let output_gpkg_path = current_dir.join(output_gpkg);

    let status = Command::new("ogr2ogr")
        .args([
            "-f",
            "GPKG",
            output_gpkg_path.to_str().unwrap(),
            input_file_path.to_str().unwrap(),
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
        ])
        .status()?;

    if !status.success() {
        return Err("Failed to convert to GeoPackage".into());
    }

    Ok(())
}

/// Découpe un GeoPackage selon une étendue spatiale spécifiée
///
/// # Arguments
///
/// * `input_gpkg` - chemin du fichier GeoPackage d'entrée
/// * `output_gpkg` - chemin du fichier GeoPackage de sortie
/// * `xmin` - coordonnée x minimale
/// * `ymin` - coordonnée y minimale
/// * `xmax` - coordonnée x maximale
/// * `ymax` - coordonnée y maximale
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si le découpage a réussi ou échoué
///
pub fn clip_to_extent(
    input_gpkg: &str,
    output_gpkg: &str,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    let input_gpkg = current_dir.join(input_gpkg);
    let output_gpkg = current_dir.join(output_gpkg);

    let status = Command::new("ogr2ogr")
        .args([
            "-f",
            "GPKG",
            output_gpkg.to_str().unwrap(),
            input_gpkg.to_str().unwrap(),
            "-clipsrc",
            &xmin.to_string(),
            &ymin.to_string(),
            &xmax.to_string(),
            &ymax.to_string(),
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
        ])
        .status()?;

    if !status.success() {
        return Err("Failed to clip GeoPackage".into());
    }

    Ok(())
}

/// Extrait les données d'une région spécifique à partir d'un fichier GeoJSON
///
/// # Arguments
///
/// * `regional_id` - identifiant de la région à extraire
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'extraction a réussi ou échoué
///
pub fn get_regional_extent(regional_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let binding = current_dir()?.join("resources/regions.geojson");
    let regional_geojson_path = binding.to_str().unwrap();
    if !std::path::Path::new(regional_geojson_path).exists() {
        return Err(format!("Input file not found: {}", regional_geojson_path).into());
    }
    let geojson_str = std::fs::read_to_string(regional_geojson_path)?;
    let mut geojson: serde_json::Value = serde_json::from_str(&geojson_str)?;

    if let Some(features) = geojson["features"].as_array_mut() {
        let filtered_features: Vec<Value> = features
            .iter()
            .filter(|feature| {
                if let Some(code) = feature["properties"]["code"].as_str() {
                    code == regional_id
                } else {
                    false
                }
            })
            .cloned()
            .collect();
        let output_geojson = json!({
            "type": "FeatureCollection",
            "features": filtered_features
        });

        let out_path = format!("projects/cache/{}.geojson", regional_id);
        fs::write(
            current_dir()?.join(&out_path),
            serde_json::to_string(&output_geojson)?,
        )?;

        Ok(())
    } else {
        Err("No features found in GeoJSON".into())
    }
}

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
///
fn rasterize_layer(
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
///
fn apply_overlay<F>(
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

/// Ajoute une couche départementale à un projet
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `regional_gpkg` - chemin du fichier GeoPackage contenant les données départementales
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
///
pub fn add_regional_layer(
    project_file_path: &str,
    regional_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    create_directory_if_not_exists("tmp")?;

    let project = Dataset::open(project_file_path)?;
    let regional_dataset = Dataset::open(regional_gpkg)?;
    let regional_layer = regional_dataset.layer(0)?;
    let temp_layer = "tmp/temp_layer.tif";

    rasterize_layer(
        &project,
        regional_gpkg,
        &regional_layer.name(),
        temp_layer,
        ["4", "25", "30"],
        None,
        None,
    )?;

    apply_overlay(project_file_path, temp_layer, |&value| value > 0)?;

    std::fs::remove_file(temp_layer)?;

    Ok(())
}

/// Ajoute une couche RPG (Registre Parcellaire Graphique) à un projet
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `rpg_gpkg` - chemin du fichier GeoPackage contenant les données RPG
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
///
pub fn add_rpg_layer(
    project_file_path: &str,
    rpg_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    create_directory_if_not_exists("tmp")?;

    let project = Dataset::open(project_file_path)?;
    let rpg_dataset = Dataset::open(rpg_gpkg)?;
    let rpg_layer = rpg_dataset.layer(0)?;
    let temp_rpg_layer = "tmp/temp_rpg_layer.tif";

    rasterize_layer(
        &project,
        rpg_gpkg,
        &rpg_layer.name(),
        temp_rpg_layer,
        ["25", "50", "60"],
        None,
        None,
    )?;

    apply_overlay(project_file_path, temp_rpg_layer, |&value| value > 0)?;

    std::fs::remove_file(temp_rpg_layer)?;

    Ok(())
}

/// Ajoute une couche de végétation à un projet en distinguant différents types
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `vegetation_gpkg` - chemin du fichier GeoPackage contenant les données de végétation
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
///
pub fn add_vegetation_layer(
    project_file_path: &str,
    vegetation_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    create_directory_if_not_exists("tmp")?;
    let vegetation_dataset = Dataset::open(vegetation_gpkg)?;
    let vegetation_layer = vegetation_dataset.layer(0)?;
    let project = Dataset::open(project_file_path)?;

    let feuillus_types = [
        "Feuillus",
        "Châtaignier",
        "Chênes sempervirents",
        "Chênes décidus",
        "Hêtre",
    ];
    let undefined_types = ["NC", "NR"];

    let feuillus_where = format!(
        "ESSENCE IN ('{}', '{}', '{}', '{}', '{}')",
        feuillus_types[0],
        feuillus_types[1],
        feuillus_types[2],
        feuillus_types[3],
        feuillus_types[4]
    );

    let undefined_where = format!(
        "ESSENCE IN ('{}', '{}')",
        undefined_types[0], undefined_types[1]
    );

    let all_types = feuillus_types
        .iter()
        .chain(undefined_types.iter())
        .map(|t| format!("'{}'", t))
        .collect::<Vec<String>>()
        .join(", ");
    let other_where = format!("ESSENCE NOT IN ({})", all_types);
    let temp_vegetation = "tmp/temp_vegetation.tif";
    let temp_feuillus = "tmp/temp_feuillus.tif";
    let temp_undefined = "tmp/temp_undefined.tif";
    let temp_other = "tmp/temp_other.tif";

    rasterize_layer(
        &project,
        vegetation_gpkg,
        &vegetation_layer.name(),
        temp_feuillus,
        ["80", "200", "120"],
        Some(&feuillus_where),
        None,
    )?;

    rasterize_layer(
        &project,
        vegetation_gpkg,
        &vegetation_layer.name(),
        temp_undefined,
        ["25", "50", "60"],
        Some(&undefined_where),
        None,
    )?;

    rasterize_layer(
        &project,
        vegetation_gpkg,
        &vegetation_layer.name(),
        temp_other,
        ["50", "200", "80"],
        Some(&other_where),
        None,
    )?;
    let driver_manager = DriverManager::get_driver_by_name("GTiff")?;
    let (width, height) = project.raster_size();

    let mut vegetation_raster = driver_manager.create(temp_vegetation, width, height, 3)?;

    vegetation_raster.set_geo_transform(&project.geo_transform()?)?;
    vegetation_raster.set_projection(&project.projection())?;

    for i in 1..=3 {
        let mut band = vegetation_raster.rasterband(i)?;
        let zeros = vec![0u8; width * height];
        band.write(
            (0, 0),
            (width, height),
            &mut gdal::raster::Buffer::new((width, height), zeros),
        )?;
    }
    let feuillus_dataset = Dataset::open(temp_feuillus)?;
    let undefined_dataset = Dataset::open(temp_undefined)?;
    let other_dataset = Dataset::open(temp_other)?;

    for band_idx in 1..=3 {
        let mut veg_band = vegetation_raster.rasterband(band_idx)?;

        let feuillus_band = feuillus_dataset.rasterband(band_idx)?;
        let feuillus_data: Vec<u8> = feuillus_band
            .read_as::<u8>((0, 0), (width, height), (width, height), None)?
            .data()
            .to_vec();

        let undefined_band = undefined_dataset.rasterband(band_idx)?;
        let undefined_data: Vec<u8> = undefined_band
            .read_as::<u8>((0, 0), (width, height), (width, height), None)?
            .data()
            .to_vec();

        let other_band = other_dataset.rasterband(band_idx)?;
        let other_data: Vec<u8> = other_band
            .read_as::<u8>((0, 0), (width, height), (width, height), None)?
            .data()
            .to_vec();

        let combined_data: Vec<u8> = feuillus_data
            .iter()
            .zip(undefined_data.iter())
            .zip(other_data.iter())
            .map(|((&f, &u), &o)| {
                if f > 0 {
                    f
                } else if u > 0 {
                    u
                } else if o > 0 {
                    o
                } else {
                    0
                }
            })
            .collect();

        veg_band.write(
            (0, 0),
            (width, height),
            &mut gdal::raster::Buffer::new((width, height), combined_data),
        )?;
    }

    feuillus_dataset.close().unwrap();
    undefined_dataset.close().unwrap();
    other_dataset.close().unwrap();
    vegetation_raster.close().unwrap();
    apply_overlay(project_file_path, temp_vegetation, |&value| value > 0)?;

    std::fs::remove_file(temp_vegetation)?;
    std::fs::remove_file(temp_feuillus)?;
    std::fs::remove_file(temp_undefined)?;
    std::fs::remove_file(temp_other)?;

    Ok(())
}

/// Ajoute une couche topographique à un projet
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet
/// * `topo_gpkg` - chemin du fichier GeoPackage contenant les données topographiques
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
///
pub fn add_topo_layer(
    project_file_path: &str,
    topo_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    create_directory_if_not_exists("tmp")?;

    let project = Dataset::open(project_file_path)?;
    let topo_dataset = Dataset::open(topo_gpkg)?;
    let mut topo_layer = topo_dataset.layer(0)?;

    if topo_layer.features().next().is_none() {
        println!("Layer has no features");
        return Ok(());
    }

    let geom_type = topo_layer
        .features()
        .next()
        .ok_or("No features in layer")?
        .geometry()
        .ok_or("Feature has no geometry")?
        .geometry_type();

    let temp_topo_layer = "tmp/temp_topo_layer.tif";

    let driver_manager = DriverManager::get_driver_by_name("GTiff")?;
    let mut dummy_raster = driver_manager.create(
        temp_topo_layer,
        project.raster_size().0,
        project.raster_size().1,
        3,
    )?;

    dummy_raster.set_geo_transform(&project.geo_transform()?)?;
    dummy_raster.set_projection(&project.projection())?;

    for i in 1..=3 {
        let mut band = dummy_raster.rasterband(i)?;
        let dummy_data = vec![255u8; project.raster_size().0 * project.raster_size().1];
        band.write(
            (0, 0),
            (project.raster_size().0, project.raster_size().1),
            &mut gdal::raster::Buffer::new(
                (project.raster_size().0, project.raster_size().1),
                dummy_data,
            ),
        )?;
    }

    dummy_raster.close().unwrap();

    let layer_name = topo_layer.name();
    let args = if geom_type == OGRwkbGeometryType::wkbLineString
        || geom_type == OGRwkbGeometryType::wkbMultiLineString
    {
        vec![
            "-burn",
            "0",
            "-burn",
            "0",
            "-burn",
            "0",
            "-l",
            &layer_name,
            "-at",
            topo_gpkg,
            temp_topo_layer,
        ]
    } else {
        vec![
            "-burn",
            "0",
            "-burn",
            "0",
            "-burn",
            "0",
            "-l",
            &layer_name,
            topo_gpkg,
            temp_topo_layer,
        ]
    };

    let status = Command::new("gdal_rasterize").args(args).status()?;

    if !status.success() {
        return Err("gdal_rasterize failed".into());
    }

    let output_file = "tmp/output.tif";
    let mut output_dataset = driver_manager.create(
        output_file,
        project.raster_size().0,
        project.raster_size().1,
        4,
    )?;

    output_dataset.set_geo_transform(&project.geo_transform()?)?;
    output_dataset.set_projection(&project.projection())?;

    let topo_raster = Dataset::open(temp_topo_layer)?;

    let base_data = [
        project.rasterband(1)?,
        project.rasterband(2)?,
        project.rasterband(3)?,
        project.rasterband(4)?,
    ];

    let overlay_data = [
        topo_raster.rasterband(1)?,
        topo_raster.rasterband(2)?,
        topo_raster.rasterband(3)?,
    ];

    let mut mask = vec![false; project.raster_size().0 * project.raster_size().1];
    for band in &overlay_data {
        let band_data: Vec<u8> = band
            .read_as::<u8>(
                (0, 0),
                (project.raster_size().0, project.raster_size().1),
                (project.raster_size().0, project.raster_size().1),
                None,
            )?
            .data()
            .to_vec();
        for (i, &value) in band_data.iter().enumerate() {
            if value != 255 {
                mask[i] = true;
            }
        }
    }

    for (i, base_band) in base_data.iter().enumerate() {
        let mut out_band = output_dataset.rasterband(i + 1)?;
        let base_band_data: Vec<u8> = base_band
            .read_as::<u8>(
                (0, 0),
                (project.raster_size().0, project.raster_size().1),
                (project.raster_size().0, project.raster_size().1),
                None,
            )?
            .data()
            .to_vec();

        let data = if i < 3 {
            base_band_data
                .iter()
                .zip(mask.iter())
                .map(
                    |(&base_value, &mask_value)| {
                        if mask_value {
                            0
                        } else {
                            base_value
                        }
                    },
                )
                .collect::<Vec<u8>>()
        } else {
            base_band_data
        };

        out_band.write(
            (0, 0),
            (project.raster_size().0, project.raster_size().1),
            &mut gdal::raster::Buffer::new(
                (project.raster_size().0, project.raster_size().1),
                data,
            ),
        )?;
    }

    output_dataset.close().unwrap();
    topo_raster.close().unwrap();
    project.close().unwrap();

    std::fs::rename(output_file, project_file_path)?;
    std::fs::remove_file(temp_topo_layer)?;

    Ok(())
}

/// Exporte un projet en format JPEG
/// Cette fonction est utilisée pour créer une image JPEG à partir d'un projet GDAL.
///
/// # Arguments
///
/// * `project_file_path` - chemin du fichier projet à exporter
/// * `output_jpg_path` - chemin du fichier JPEG de sortie
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'exportation a réussi ou échoué
pub fn export_to_jpg(
    project_file_path: &str,
    output_jpg_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("gdal_translate")
        .args(["-of", "JPEG", project_file_path, output_jpg_path])
        .status()?;

    if !status.success() {
        return Err("Failed to export to JPEG".into());
    }

    Ok(())
}

/// Télécharge une image satellite JPEG pour une étendue donnée avec une résolution de 10m/pixel
/// Cette fonction utilise le service WMS de geoportail pour télécharger une image satellite
///
/// # Arguments
///
/// * `output_jpg_path` - chemin du fichier JPEG de sortie
/// * `xmin` - coordonnée x minimale (Lambert-93)
/// * `ymin` - coordonnée y minimale (Lambert-93)
/// * `xmax` - coordonnée x maximale (Lambert-93)
/// * `ymax` - coordonnée y maximale (Lambert-93)
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si le téléchargement a réussi ou échoué
///
pub fn download_satellite_jpeg(
    output_jpg_path: &str,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    create_directory_if_not_exists("tmp")?;

    let resolution = 10.0;
    let width = ((xmax - xmin) / resolution).ceil() as usize;
    let height = ((ymax - ymin) / resolution).ceil() as usize;

    println!(
        "Dimensions calculées : largeur={}, hauteur={} pixels",
        width, height
    );

    let temp_satellite = "tmp/satellite_temp.tif";
    let temp_satellite_expanded = "tmp/satellite_expanded.tif";
    let temp_satellite_color_fixed = "tmp/satellite_color_fixed.tif";
    let wms_file = "tmp/wms_config.xml";

    let wms_xml = format!(
        r#"<GDAL_WMS>
      <Service name="WMS">
        <Version>1.3.0</Version>
        <ServerUrl>https://data.geopf.fr/wms-r/wms</ServerUrl>
        <CRS>EPSG:2154</CRS>
        <ImageFormat>image/jpeg</ImageFormat>
        <Layers>ORTHOIMAGERY.ORTHOPHOTOS</Layers>
        <Styles></Styles>
      </Service>
      <DataWindow>
        <UpperLeftX>{}</UpperLeftX>
        <UpperLeftY>{}</UpperLeftY>
        <LowerRightX>{}</LowerRightX>
        <LowerRightY>{}</LowerRightY>
        <SizeX>{}</SizeX>
        <SizeY>{}</SizeY>
      </DataWindow>
      <BandsCount>3</BandsCount>
      <BlockSizeX>2500</BlockSizeX>
      <BlockSizeY>2500</BlockSizeY>
      <OverviewCount>0</OverviewCount>
      <ZeroBlockHttpCodes>204,400,404</ZeroBlockHttpCodes>
      <MaxConnections>5</MaxConnections>
      <Timeout>60</Timeout>
      <Cache>
        <Type>Disk</Type>
        <Path>tmp/wms_cache</Path>
        <MaxSize>200000000</MaxSize>
      </Cache>
    </GDAL_WMS>"#,
        xmin, ymax, xmax, ymin, width, height
    );

    std::fs::write(wms_file, wms_xml)?;

    let status = Command::new("gdal_translate")
        .args([
            "-of",
            "GTiff",
            "-co",
            "COMPRESS=JPEG",
            "-co",
            "JPEG_QUALITY=95",
            "-co",
            "BLOCKYSIZE=2500",
            "-co",
            "PHOTOMETRIC=RGB",
            wms_file,
            temp_satellite,
        ])
        .status()?;

    if !status.success() {
        println!("La première source a échoué, essai avec SCAN1000_PYR-JPEG_WLD_WM...");

        let wms_xml_scan = format!(
            r#"<GDAL_WMS>
          <Service name="WMS">
            <Version>1.3.0</Version>
            <ServerUrl>https://data.geopf.fr/wms-r/wms</ServerUrl>
            <CRS>EPSG:3857</CRS>
            <ImageFormat>image/jpeg</ImageFormat>
            <Layers>SCAN1000_PYR-JPEG_WLD_WM</Layers>
            <Styles></Styles>
          </Service>
          <DataWindow>
            <UpperLeftX>{}</UpperLeftX>
            <UpperLeftY>{}</UpperLeftY>
            <LowerRightX>{}</LowerRightX>
            <LowerRightY>{}</LowerRightY>
            <SizeX>{}</SizeX>
            <SizeY>{}</SizeY>
          </DataWindow>
          <BandsCount>3</BandsCount>
          <BlockSizeX>2500</BlockSizeX>
          <BlockSizeY>2500</BlockSizeY>
          <ZeroBlockHttpCodes>204,400,404</ZeroBlockHttpCodes>
          <Cache>
            <Type>Disk</Type>
            <Path>tmp/wms_cache</Path>
            <MaxSize>200000000</MaxSize>
          </Cache>
        </GDAL_WMS>"#,
            xmin, ymax, xmax, ymin, width, height
        );

        std::fs::write(wms_file, wms_xml_scan)?;

        let status = Command::new("gdal_translate")
            .args([
                "-of",
                "GTiff",
                "-co",
                "COMPRESS=JPEG",
                "-co",
                "JPEG_QUALITY=95",
                "-co",
                "BLOCKYSIZE=2500",
                "-co",
                "PHOTOMETRIC=RGB",
                wms_file,
                temp_satellite,
            ])
            .status()?;

        if !status.success() {
            return Err("Échec du téléchargement de l'image satellite".into());
        }
    }

    let info = Command::new("gdalinfo").args([temp_satellite]).output()?;

    if info.status.success() {
        let info_text = String::from_utf8_lossy(&info.stdout);
        println!("Information TIFF téléchargé:\n{}", info_text);
        if info_text.contains("Size is")
            && !info_text.contains(&format!("Size is {}, {}", width, height))
        {
            println!("L'image téléchargée n'a pas les bonnes dimensions. Extension de l'image...");

            let status = Command::new("gdal_translate")
                .args([
                    "-of",
                    "GTiff",
                    "-outsize",
                    &width.to_string(),
                    &height.to_string(),
                    "-co",
                    "COMPRESS=JPEG",
                    "-co",
                    "JPEG_QUALITY=95",
                    "-co",
                    "PHOTOMETRIC=RGB",
                    temp_satellite,
                    temp_satellite_expanded,
                ])
                .status()?;

            if status.success() {
                std::fs::rename(temp_satellite_expanded, temp_satellite)?;
            } else {
                let status = Command::new("gdal_create")
                    .args([
                        "-of",
                        "GTiff",
                        "-outsize",
                        &width.to_string(),
                        &height.to_string(),
                        "-bands",
                        "3",
                        "-burn",
                        "128",
                        "-burn",
                        "128",
                        "-burn",
                        "128",
                        "-a_srs",
                        "EPSG:2154",
                        "-a_ullr",
                        &xmin.to_string(),
                        &ymax.to_string(),
                        &xmax.to_string(),
                        &ymin.to_string(),
                        "-co",
                        "COMPRESS=JPEG",
                        "-co",
                        "JPEG_QUALITY=95",
                        "-co",
                        "PHOTOMETRIC=RGB",
                        temp_satellite_expanded,
                    ])
                    .status()?;

                if status.success() {
                    std::fs::rename(temp_satellite_expanded, temp_satellite)?;
                }
            }
        }
    }

    let status = Command::new("gdal_translate")
        .args([
            "-of",
            "GTiff",
            "-b",
            "1",
            "-b",
            "2",
            "-b",
            "3",
            "-scale",
            "-co",
            "COMPRESS=JPEG",
            "-co",
            "JPEG_QUALITY=95",
            "-co",
            "PHOTOMETRIC=RGB",
            temp_satellite,
            temp_satellite_color_fixed,
        ])
        .status()?;

    if status.success() {
        std::fs::rename(temp_satellite_color_fixed, temp_satellite)?;
    }

    let status = Command::new("gdal_translate")
        .args([
            "--config",
            "GDAL_PAM_ENABLED",
            "NO",
            "-of",
            "JPEG",
            "-outsize",
            &width.to_string(),
            &height.to_string(),
            "-co",
            "QUALITY=95",
            "-b",
            "1",
            "-b",
            "2",
            "-b",
            "3",
            temp_satellite,
            output_jpg_path,
        ])
        .status()?;

    if !status.success() {
        return Err("Échec de la conversion en JPEG".into());
    }

    let info = Command::new("gdalinfo").args([output_jpg_path]).output()?;

    if info.status.success() {
        let info_text = String::from_utf8_lossy(&info.stdout);
        println!("Information JPEG final:\n{}", info_text);
        if info_text.contains("Size is")
            && !info_text.contains(&format!("Size is {}, {}", width, height))
        {
            println!("Le JPEG final n'a pas les bonnes dimensions. Création d'un nouveau JPEG...");

            let temp_jpg = "tmp/output_correct_size.jpg";
            let status = Command::new("magick")
                .args([
                    temp_satellite,
                    "-resize",
                    &format!("{}x{}", width, height),
                    "-colorspace",
                    "sRGB",
                    "-type",
                    "TrueColor",
                    temp_jpg,
                ])
                .status();

            if status.is_ok() && status.unwrap().success() {
                std::fs::copy(temp_jpg, output_jpg_path)?;
                std::fs::remove_file(temp_jpg)?;
            }
        }
    }

    std::fs::remove_file(temp_satellite)?;
    std::fs::remove_file(wms_file)?;

    Ok(())
}
