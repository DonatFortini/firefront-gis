use gdal::vector::{LayerAccess, OGRwkbGeometryType};
use gdal::{Dataset, DriverManager};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::Emitter;

use super::processing::{apply_overlay, rasterize_layer};
use super::regions::create_region_geojson;
use super::{clip_to_bb, convert_to_gpkg};

use crate::utils::{
    BoundingBox, cache_dir, create_directory_if_not_exists, extract_files_by_name, resolution,
    temp_dir,
};

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
    app_handle: &tauri::AppHandle,
    project_bb: &BoundingBox,
    code: &str,
) -> Result<(String, String, String, HashMap<String, Vec<String>>), String> {
    let cache_folder_path = cache_dir().to_string_lossy().to_string();
    let temp_dir = temp_dir().to_string_lossy().to_string();

    let _ = app_handle.emit(
        "progress-update",
        "Préparation des Couches|Préparation de l'étendue régionale|1/4",
    );

    let regional_geojson_path = format!("{}/{}.geojson", temp_dir, code);
    create_region_geojson(code, &regional_geojson_path).unwrap();

    let temp_regional_gpkg = format!("{}/{}.gpkg", temp_dir, code);
    let regional_gpkg = format!("{}/{}_region.gpkg", temp_dir, code);

    let _ = convert_to_gpkg(&regional_geojson_path, &temp_regional_gpkg);
    let _ = clip_to_bb(&temp_regional_gpkg, &regional_gpkg, project_bb);

    let mut layers: HashMap<String, Vec<&str>> = HashMap::new();
    layers.insert(format!("BDFORET_{}.7z", code), vec!["FORMATION_VEGETALE"]);
    layers.insert(format!("RPG_{}.7z", code), vec!["PARCELLES_GRAPHIQUES"]);
    layers.insert(
        format!("BDTOPO_{}.7z", code),
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

        let _ = app_handle.emit(
            "progress-update",
            format!(
                "Préparation des Couches|Préparation des couches {}|{}/{}",
                layer_type,
                layer_index,
                total_archives + 1
            ),
        );

        let archive_path = format!("{}/{}", cache_folder_path, archive);

        let total_files = files.len();
        for (file_index, file) in files.iter().enumerate() {
            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Préparation des Couches|Extraction de {}|{}/{}",
                    file,
                    file_index + 1,
                    total_files
                ),
            );

            extract_files_by_name(&archive_path, file, &temp_dir).map_err(|e| {
                format!(
                    "Erreur lors de l'extraction du fichier {} depuis l'archive {}: {:?}",
                    file, archive, e
                )
            })?;

            let temp_file = format!("{}/{}/{}.shp", temp_dir, file, file);
            let temp_gpkg = format!("{}/{}.gpkg", temp_dir, file);
            let output_gpkg = format!("{}/{}_{}.gpkg", temp_dir, code, file);

            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Préparation des Couches|Conversion de {}|{}/{}",
                    file,
                    file_index + 1,
                    total_files
                ),
            );

            if let Err(e) = convert_to_gpkg(&temp_file, &temp_gpkg) {
                return Err(format!(
                    "Erreur lors de la conversion du fichier {} en GPKG: {:?}",
                    temp_file, e
                ));
            }

            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Préparation des Couches|Découpage de {}|{}/{}",
                    file,
                    file_index + 1,
                    total_files
                ),
            );

            if let Err(e) = clip_to_bb(&temp_gpkg, &output_gpkg, project_bb) {
                return Err(format!(
                    "Erreur lors du découpage du fichier {}: {:?}",
                    temp_gpkg, e
                ));
            }

            // Stocker les chemins des fichiers GPKG selon leur type
            if file == &"FORMATION_VEGETALE" {
                vegetation_gpkg = output_gpkg.clone();
            } else if file == &"PARCELLES_GRAPHIQUES" {
                rpg_gpkg = output_gpkg.clone();
            } else {
                // Pour les couches topo, on les stocke par nom de fichier
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
        ["0", "0", "0"],
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
    // FIXME : add the cross-platform support
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
                        if mask_value { 0 } else { base_value }
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
pub fn add_layers(
    app_handle: &tauri::AppHandle,
    project_folder: &str,
    project_file_path: &str,
    project_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = app_handle.emit(
        "progress-update",
        "Ajout des Couches|Ajout de la couche régionale|1/4",
    );

    if let Err(e) = add_regional_layer(
        project_file_path,
        &format!("{}/resources/{}.gpkg", project_folder, project_name),
    ) {
        println!("Failed to add regional layer: {:?}", e);
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

        let _ = app_handle.emit(
            "progress-update",
            format!(
                "Ajout des Couches|Ajout des couches {}|{}/{}",
                layer_type, layer_index, total_layer_types
            ),
        );

        let total_files = value.len();
        for (file_index, file) in value.iter().enumerate() {
            let _ = app_handle.emit(
                "progress-update",
                format!(
                    "Ajout des Couches|Ajout de {}|{}/{}",
                    file,
                    file_index + 1,
                    total_files
                ),
            );

            let layer_path = format!("{}/resources/{}.gpkg", project_folder, file);
            match key {
                1 => add_vegetation_layer(project_file_path, &layer_path),
                2 => add_rpg_layer(project_file_path, &layer_path),
                3 => add_topo_layer(project_file_path, &layer_path),
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

//FIXME: orthophoto sur les format paysage

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
pub fn download_satellite_jpeg(
    output_jpg_path: &str,
    project_bb: &BoundingBox,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = temp_dir().to_string_lossy().to_string();
    create_directory_if_not_exists(&temp_dir)?;

    let wms_cache_dir = format!("{}/wms_cache", temp_dir);
    create_directory_if_not_exists(&wms_cache_dir)?;

    let resolution = resolution();
    let width = ((project_bb.xmax - project_bb.xmin) / resolution).ceil() as usize;
    let height = ((project_bb.ymax - project_bb.ymin) / resolution).ceil() as usize;

    println!(
        "Dimensions calculées : largeur={}, hauteur={} pixels",
        width, height
    );

    let temp_satellite = format!("{}/satellite_temp.tif", temp_dir);
    let wms_file = format!("{}/wms_config.xml", temp_dir);
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
      <BlockSizeX>2048</BlockSizeX>
      <BlockSizeY>2048</BlockSizeY>
      <OverviewCount>0</OverviewCount>
      <ZeroBlockHttpCodes>204,400,404,502,503,504</ZeroBlockHttpCodes>
      <MaxConnections>10</MaxConnections>
      <Timeout>120</Timeout>
      <Cache>
        <Type>Disk</Type>
        <Path>{}/wms_cache</Path>
        <MaxSize>500000000</MaxSize>
      </Cache>
      <UserAgent>GDAL WMS driver (https://gdal.org/drivers/raster/wms.html)</UserAgent>
      <UnsafeSSL>true</UnsafeSSL>
      <Retry>
        <Count>5</Count>
        <Delay>1</Delay>
      </Retry>
    </GDAL_WMS>"#,
        project_bb.xmin, project_bb.ymax, project_bb.xmax, project_bb.ymin, width, height, temp_dir
    );

    std::fs::write(wms_file.clone(), wms_xml)?;

    let mut success = false;
    let mut attempts = 0;
    let max_attempts = 3;

    while !success && attempts < max_attempts {
        attempts += 1;
        println!("Tentative de téléchargement {}/{}", attempts, max_attempts);
        // FIXME : add the cross-platform support
        let status = Command::new("gdal_translate")
            .args([
                "-of",
                "GTiff",
                "-co",
                "COMPRESS=JPEG",
                "-co",
                "JPEG_QUALITY=95",
                "-co",
                "PHOTOMETRIC=RGB",
                "-co",
                "BIGTIFF=YES",
                &wms_file,
                &temp_satellite,
            ])
            .status()?;

        if status.success() {
            success = true;
        } else if attempts < max_attempts {
            println!("Échec, nouvelle tentative dans 5 secondes...");
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    }

    if !success {
        return Err(
            "Échec du téléchargement de l'image satellite après plusieurs tentatives".into(),
        );
    }

    let metadata = fs::metadata(&temp_satellite)?;
    if metadata.len() == 0 {
        return Err("Le fichier téléchargé est vide".into());
    }

    let temp_jpg = format!("{}/satellite_temp.jpg", temp_dir);
    // TODO : check if ImageMagick is installed, need to be installed on the system and is cross-platform
    let magick_status = Command::new("magick")
        .args([
            &temp_satellite,
            "-resize",
            &format!("{}x{}", width, height),
            "-colorspace",
            "sRGB",
            "-type",
            "TrueColor",
            &temp_jpg,
        ])
        .status()?;

    if !magick_status.success() {
        return Err("Échec de la conversion en JPEG avec ImageMagick".into());
    }

    if Path::new(&temp_jpg).exists() {
        std::fs::rename(temp_jpg, output_jpg_path)?;
    } else {
        return Err("Le fichier JPEG temporaire n'a pas été créé".into());
    }

    std::fs::remove_file(temp_satellite)?;
    std::fs::remove_file(wms_file)?;

    Ok(())
}
