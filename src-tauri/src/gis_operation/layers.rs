use gdal::vector::{LayerAccess, OGRwkbGeometryType};
use gdal::{Dataset, DriverManager};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;

use super::create_region_geojson;
use super::processing::{apply_overlay, rasterize_layer};
use super::{clip_to_bb, convert_to_gpkg};

use crate::utils::{
    cache_dir, create_directory_if_not_exists, emit_progress, executor, extract_files_by_name,
    resolution, temp_dir,
};

use crate::types::BoundingBox;

/// Prépare les couches pour le projet, en les convertissant au format GPKG et en les découpant à l'extent régional.
/// Retourne les chemins vers les fichiers GPKG pour chaque type de couche
///
/// # Arguments
///
/// * `app_handle` - Handle de l'application Tauri
/// * `project_bb` - BoundingBox du projet
/// * `code` - Code départemental de la région traitée
///
/// # Returns
///
/// * `Result<(String, String, String, HashMap<String, Vec<String>>), String>` - Un tuple contenant les chemins vers les fichiers GPKG pour la région, la végétation, le RPG et les couches topographiques
pub async fn prepare_layers(
    project_bb: &BoundingBox,
    code: &str,
) -> Result<(String, String, String, HashMap<String, Vec<String>>), String> {
    let cache_folder_path = cache_dir().to_string_lossy().to_string();
    let temp_dir = temp_dir().to_string_lossy().to_string();

    emit_progress("Préparation des Couches|Préparation de l'étendue régionale|1/4");

    let regional_geojson_path = format!("{temp_dir}/{code}.geojson");
    create_region_geojson(code, &regional_geojson_path).unwrap();

    let temp_regional_gpkg = format!("{temp_dir}/{code}.gpkg");
    let regional_gpkg = format!("{temp_dir}/{code}_region.gpkg");

    convert_to_gpkg(&regional_geojson_path, &temp_regional_gpkg)
        .await
        .unwrap();
    clip_to_bb(&temp_regional_gpkg, &regional_gpkg, project_bb)
        .await
        .unwrap();

    let mut layers: HashMap<String, Vec<&str>> = HashMap::new();
    layers.insert(format!("BDFORET_{code}.7z"), vec!["FORMATION_VEGETALE"]);
    layers.insert(format!("RPG_{code}.7z"), vec!["PARCELLES_GRAPHIQUES"]);
    layers.insert(
        format!("BDTOPO_{code}.7z"),
        vec![
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
    );

    let mut vegetation_gpkg = String::new();
    let mut rpg_gpkg = String::new();
    let mut topo_gpkgs: HashMap<String, Vec<String>> = HashMap::new();

    let mut layer_index = 2;
    let total_archives = layers.len();

    for (archive, files) in layers {
        let layer_type = if archive.contains("BDFORET") {
            "Végétation"
        } else if archive.contains("RPG") {
            "Parcelles agricoles"
        } else if archive.contains("BDTOPO") {
            "Topographie"
        } else {
            "Inconnu"
        };

        emit_progress(&format!(
            "Préparation des Couches|Préparation des couches {layer_type}|{layer_index}/{}",
            total_archives + 1
        ));

        let archive_path = format!("{cache_folder_path}/{archive}");

        let total_files = files.len();
        for (file_index, file) in files.iter().enumerate() {
            emit_progress(&format!(
                "Préparation des Couches|Extraction de {file}|{}/{total_files}",
                file_index + 1
            ));

            extract_files_by_name(&archive_path, file, &temp_dir).await.map_err(|e| {
                format!(
                    "Erreur lors de l'extraction du fichier {file} depuis l'archive {archive}: {e:?}"
                )
            })?;

            let temp_file = format!("{temp_dir}/{file}/{file}.shp");
            let temp_gpkg = format!("{temp_dir}/{file}.gpkg");
            let output_gpkg = format!("{temp_dir}/{code}_{file}.gpkg");

            emit_progress(&format!(
                "Préparation des Couches|Conversion de {file}|{}/{total_files}",
                file_index + 1
            ));

            if let Err(e) = convert_to_gpkg(&temp_file, &temp_gpkg).await {
                return Err(format!(
                    "Erreur lors de la conversion du fichier {temp_file} en GPKG: {e:?}"
                ));
            }

            emit_progress(&format!(
                "Préparation des Couches|Découpage de {file}|{}/{total_files}",
                file_index + 1
            ));

            if let Err(e) = clip_to_bb(&temp_gpkg, &output_gpkg, project_bb).await {
                return Err(format!(
                    "Erreur lors du découpage du fichier {temp_gpkg}: {e:?}"
                ));
            }

            if file == &"FORMATION_VEGETALE" {
                vegetation_gpkg = output_gpkg.clone();
            } else if file == &"PARCELLES_GRAPHIQUES" {
                rpg_gpkg = output_gpkg.clone();
            } else {
                topo_gpkgs
                    .entry(file.to_string())
                    .or_default()
                    .push(output_gpkg.clone());
            }
        }

        layer_index += 1;
    }

    Ok((regional_gpkg, vegetation_gpkg, rpg_gpkg, topo_gpkgs))
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
pub async fn add_regional_layer(
    project_file_path: &str,
    regional_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let regional_layer_name = {
        let regional_dataset = Dataset::open(regional_gpkg)?;
        regional_dataset.layer(0)?.name()
    };

    let temp_layer = format!("{}/temp_layer.tif", temp_dir().to_string_lossy());

    rasterize_layer(
        project_file_path,
        regional_gpkg,
        &regional_layer_name,
        &temp_layer,
        ["0", "0", "0"],
        None,
        None,
    )
    .await?;

    apply_overlay(project_file_path, &temp_layer, |&value| value > 0)?;

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
pub async fn add_rpg_layer(
    project_file_path: &str,
    rpg_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let rpg_layer_name = {
        let rpg_dataset = Dataset::open(rpg_gpkg)?;
        rpg_dataset.layer(0)?.name()
    };

    let temp_rpg_layer = format!("{}/temp_rpg_layer.tif", temp_dir().to_string_lossy());

    rasterize_layer(
        project_file_path,
        rpg_gpkg,
        &rpg_layer_name,
        &temp_rpg_layer,
        ["25", "50", "60"],
        None,
        None,
    )
    .await?;

    apply_overlay(project_file_path, &temp_rpg_layer, |&value| value > 0)?;

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
pub async fn add_vegetation_layer(
    project_file_path: &str,
    vegetation_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let vegetation_layer_name = {
        let vegetation_dataset = Dataset::open(vegetation_gpkg)?;
        vegetation_dataset.layer(0)?.name()
    };

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
        .map(|t| format!("'{t}'"))
        .collect::<Vec<String>>()
        .join(", ");
    let other_where = format!("ESSENCE NOT IN ({all_types})");

    let temp_path = temp_dir().to_string_lossy().to_string();

    let temp_vegetation = format!("{}/temp_vegetation.tif", temp_path);
    let temp_feuillus = format!("{}/temp_feuillus.tif", temp_path);
    let temp_undefined = format!("{}/temp_undefined.tif", temp_path);
    let temp_other = format!("{}/temp_other.tif", temp_path);

    rasterize_layer(
        project_file_path,
        vegetation_gpkg,
        &vegetation_layer_name,
        &temp_feuillus,
        ["80", "200", "120"],
        Some(&feuillus_where),
        None,
    )
    .await?;

    rasterize_layer(
        project_file_path,
        vegetation_gpkg,
        &vegetation_layer_name,
        &temp_undefined,
        ["25", "50", "60"],
        Some(&undefined_where),
        None,
    )
    .await?;

    rasterize_layer(
        project_file_path,
        vegetation_gpkg,
        &vegetation_layer_name,
        &temp_other,
        ["50", "200", "80"],
        Some(&other_where),
        None,
    )
    .await?;

    let project = Dataset::open(project_file_path)?;

    let driver_manager = DriverManager::get_driver_by_name("GTiff")?;
    let (width, height) = project.raster_size();

    let mut vegetation_raster = driver_manager.create(&temp_vegetation, width, height, 3)?;

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
    let feuillus_dataset = Dataset::open(&temp_feuillus)?;
    let undefined_dataset = Dataset::open(&temp_undefined)?;
    let other_dataset = Dataset::open(&temp_other)?;

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
            .map(|((&f, &u), &o)| match (f, u, o) {
                (v, _, _) if v > 0 => v,
                (_, v, _) if v > 0 => v,
                (_, _, v) if v > 0 => v,
                _ => 0,
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
    apply_overlay(project_file_path, &temp_vegetation, |&value| value > 0)?;

    // TODO : Clean_tmp ?
    std::fs::remove_file(&temp_vegetation)?;
    std::fs::remove_file(&temp_feuillus)?;
    std::fs::remove_file(&temp_undefined)?;
    std::fs::remove_file(&temp_other)?;

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
pub async fn add_topo_layer(
    project_file_path: &str,
    topo_gpkg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (project_raster_size, project_geo_transform, project_projection, geom_type, layer_name) = {
        let project = Dataset::open(project_file_path)?;
        let project_raster_size = project.raster_size();
        let project_geo_transform = project.geo_transform()?;
        let project_projection = project.projection();

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

        let layer_name = topo_layer.name();
        topo_dataset.close().unwrap();
        project.close().unwrap();

        (
            project_raster_size,
            project_geo_transform,
            project_projection,
            geom_type,
            layer_name,
        )
    };

    let temp_topo_layer = format!("{}/temp_topo_layer.tif", temp_dir().to_string_lossy());

    {
        let driver_manager = DriverManager::get_driver_by_name("GTiff")?;
        let mut dummy_raster = driver_manager.create(
            &temp_topo_layer,
            project_raster_size.0,
            project_raster_size.1,
            3,
        )?;

        dummy_raster.set_geo_transform(&project_geo_transform)?;
        dummy_raster.set_projection(&project_projection)?;

        for i in 1..=3 {
            let mut band = dummy_raster.rasterband(i)?;
            let dummy_data = vec![255u8; project_raster_size.0 * project_raster_size.1];
            band.write(
                (0, 0),
                (project_raster_size.0, project_raster_size.1),
                &mut gdal::raster::Buffer::new(
                    (project_raster_size.0, project_raster_size.1),
                    dummy_data,
                ),
            )?;
        }

        dummy_raster.close().unwrap();
    }

    let mut args = vec!["-burn", "0", "-burn", "0", "-burn", "0", "-l", &layer_name];

    if geom_type == OGRwkbGeometryType::wkbLineString
        || geom_type == OGRwkbGeometryType::wkbMultiLineString
    {
        args.push("-at");
    }

    args.extend_from_slice(&[topo_gpkg, &temp_topo_layer]);

    let status = executor("gdal_rasterize", &args).await?.1;

    if !status.success() {
        return Err("gdal_rasterize failed".into());
    }

    let output_file = format!("{}/output.tif", temp_dir().to_string_lossy());

    {
        let driver_manager = DriverManager::get_driver_by_name("GTiff")?;
        let mut output_dataset = driver_manager.create(
            &output_file,
            project_raster_size.0,
            project_raster_size.1,
            4,
        )?;

        output_dataset.set_geo_transform(&project_geo_transform)?;
        output_dataset.set_projection(&project_projection)?;

        let project = Dataset::open(project_file_path)?;
        let topo_raster = Dataset::open(&temp_topo_layer)?;

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

        let mut mask = vec![false; project_raster_size.0 * project_raster_size.1];
        for band in &overlay_data {
            let band_data: Vec<u8> = band
                .read_as::<u8>(
                    (0, 0),
                    (project_raster_size.0, project_raster_size.1),
                    (project_raster_size.0, project_raster_size.1),
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
                    (project_raster_size.0, project_raster_size.1),
                    (project_raster_size.0, project_raster_size.1),
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
                            if mask_value { 0 } else { base_value }
                        },
                    )
                    .collect::<Vec<u8>>()
            } else {
                base_band_data
            };

            out_band.write(
                (0, 0),
                (project_raster_size.0, project_raster_size.1),
                &mut gdal::raster::Buffer::new(
                    (project_raster_size.0, project_raster_size.1),
                    data,
                ),
            )?;
        }

        output_dataset.close().unwrap();
        topo_raster.close().unwrap();
        project.close().unwrap();
    }

    std::fs::rename(output_file, project_file_path)?;
    std::fs::remove_file(&temp_topo_layer)?;

    Ok(())
}

/// Ajoute les couches au projet.
/// Cette fonction est responsable de l'ajout des couches régionales, de végétation, de RPG et topographiques
/// au projet en utilisant les chemins fournis.
/// Elle émet également des événements de mise à jour de progression pour informer l'utilisateur
/// de l'état d'avancement de l'ajout des couches.
///
/// # Arguments
///
/// * `app_handle` - Handle de l'application Tauri
/// * `project_folder` - chemin du dossier du projet
/// * `project_file_path` - chemin du fichier projet
/// * `project_name` - nom du projet
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si l'ajout a réussi ou échoué
pub async fn add_layers(project_file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    emit_progress("Ajout des Couches|Ajout de la couche régionale|1/4");

    let project_file_path_obj = Path::new(project_file_path);
    let project_folder = project_file_path_obj
        .parent()
        .ok_or("Invalid project_file_path: no parent directory")?
        .to_string_lossy()
        .to_string();
    let project_name = project_file_path_obj
        .file_stem()
        .ok_or("Invalid project_file_path: no file stem")?
        .to_string_lossy()
        .to_string();

    if let Err(e) = add_regional_layer(
        project_file_path,
        &format!("{project_folder}/resources/{project_name}.gpkg"),
    )
    .await
    {
        println!("Failed to add regional layer: {e:?}");
        return Err(e);
    }

    let mut layers: BTreeMap<i8, Vec<&str>> = BTreeMap::new();
    layers.insert(1, vec!["FORMATION_VEGETALE"]);
    layers.insert(2, vec!["PARCELLES_GRAPHIQUES"]);
    layers.insert(
        3,
        vec![
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
    );

    let mut layer_index = 2;
    let total_layer_types = layers.len() + 1;

    for (key, value) in layers {
        let layer_type = match key {
            1 => "Végétation",
            2 => "Parcelles agricoles",
            3 => "Topographie",
            _ => "Inconnu",
        };

        emit_progress(&format!(
            "Ajout des Couches|Ajout des couches {layer_type}|{layer_index}/{total_layer_types}"
        ));

        let total_files = value.len();
        for (file_index, file) in value.iter().enumerate() {
            emit_progress(&format!(
                "Ajout des Couches|Ajout de {file}|{}/{total_files}",
                file_index + 1
            ));

            let layer_path = format!("{project_folder}/resources/{file}.gpkg");
            match key {
                1 => add_vegetation_layer(project_file_path, &layer_path).await,
                2 => add_rpg_layer(project_file_path, &layer_path).await,
                3 => add_topo_layer(project_file_path, &layer_path).await,
                _ => {
                    println!("Unknown layer type");
                    return Err(Box::new(std::io::Error::other("Unknown layer type")));
                }
            }?
        }

        layer_index += 1;
    }

    Ok(())
}

/// Télécharge une image satellite JPEG pour une étendue donnée avec une résolution de 10m/pixel
/// Cette fonction utilise le service WMS de geoportail pour télécharger une image satellite
/// et utilise ImageMagick pour traiter l'image.
///
/// # Arguments
///
/// * `output_jpg_path` - chemin de sortie pour l'image JPEG
/// * `project_bb` - BoundingBox de l'étendue du projet
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - un résultat indiquant si le téléchargement a réussi ou échoué
pub async fn download_satellite_jpeg(
    output_jpg_path: &str,
    project_bb: &BoundingBox,
) -> Result<(), Box<dyn std::error::Error>> {
    let wms_cache_dir = format!("{}/wms_cache", cache_dir().to_string_lossy());
    create_directory_if_not_exists(&wms_cache_dir)?;

    let resolution = resolution();
    let width = ((project_bb.xmax - project_bb.xmin) / resolution).ceil() as usize;
    let height = ((project_bb.ymax - project_bb.ymin) / resolution).ceil() as usize;

    println!("Dimensions calculées : largeur={width}, hauteur={height} pixels");

    let cache_key = format!(
        "{:.6}_{:.6}_{:.6}_{:.6}_{}x{}",
        project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
    );
    let cache_file = format!("{wms_cache_dir}/satellite_{cache_key}.jpg");

    if Path::new(&cache_file).exists() {
        if let Ok(metadata) = fs::metadata(&cache_file)
            && metadata.len() > 0
        {
            fs::copy(&cache_file, output_jpg_path)?;
            println!(
                "Image satellite récupérée depuis le cache: {} bytes",
                metadata.len()
            );
            return Ok(());
        }
        let _ = fs::remove_file(&cache_file);
    }

    let wms_url = format!(
        "https://data.geopf.fr/wms-r/wms?\
        SERVICE=WMS&\
        VERSION=1.3.0&\
        REQUEST=GetMap&\
        LAYERS=ORTHOIMAGERY.ORTHOPHOTOS&\
        STYLES=&\
        CRS=EPSG:2154&\
        BBOX={},{},{},{}&\
        WIDTH={}&\
        HEIGHT={}&\
        FORMAT=image/jpeg",
        project_bb.xmin, project_bb.ymin, project_bb.xmax, project_bb.ymax, width, height
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("Rust WMS Client")
        .build()?;

    let mut success = false;
    let mut attempts = 0;
    let max_attempts = 3;
    let mut image_data = Vec::new();

    while !success && attempts < max_attempts {
        attempts += 1;
        println!("Tentative de téléchargement {attempts}/{max_attempts}");

        match download_attempt(&client, &wms_url).await {
            Ok(data) => {
                if data.is_empty() {
                    return Err("Le fichier téléchargé est vide".into());
                }

                image_data = data;
                success = true;
            }
            Err(e) => {
                println!("Tentative {} échouée: {}", attempts, e);
                if attempts < max_attempts {
                    println!("Nouvelle tentative dans 5 secondes...");
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    if !success {
        return Err(
            "Échec du téléchargement de l'image satellite après plusieurs tentatives".into(),
        );
    }

    let temp_cache_file = format!("{}.tmp", cache_file);
    fs::write(&temp_cache_file, &image_data)?;
    fs::rename(&temp_cache_file, &cache_file)?;

    fs::copy(&cache_file, output_jpg_path)?;

    if !Path::new(&output_jpg_path).exists() {
        return Err("Échec de l'écriture du fichier final".into());
    }

    if let Ok(metadata) = fs::metadata(output_jpg_path)
        && metadata.len() == 0
    {
        return Err("Le fichier final est vide".into());
    }

    println!(
        "Image satellite téléchargée avec succès: {} bytes",
        image_data.len()
    );
    Ok(())
}

async fn download_attempt(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()).into());
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("image/") {
        let error_text = response.text().await?;
        return Err(format!(
            "Server returned error response: {}",
            error_text.chars().take(200).collect::<String>()
        )
        .into());
    }

    let image_data = response.bytes().await?;

    if image_data.len() < 10 || image_data[0] != 0xFF || image_data[1] != 0xD8 {
        return Err("Downloaded data is not a valid JPEG image".into());
    }

    Ok(image_data.to_vec())
}
