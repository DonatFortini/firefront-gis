use lazy_static::lazy_static;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

lazy_static! {
    pub static ref DEPARTEMENTS: HashMap<String, String> = [
        ("01", "Ain"),
        ("02", "Aisne"),
        ("03", "Allier"),
        ("04", "Alpes-de-Haute-Provence"),
        ("05", "Hautes-Alpes"),
        ("06", "Alpes-Maritimes"),
        ("07", "Ardèche"),
        ("08", "Ardennes"),
        ("09", "Ariège"),
        ("10", "Aube"),
        ("11", "Aude"),
        ("12", "Aveyron"),
        ("13", "Bouches-du-Rhône"),
        ("14", "Calvados"),
        ("15", "Cantal"),
        ("16", "Charente"),
        ("17", "Charente-Maritime"),
        ("18", "Cher"),
        ("19", "Corrèze"),
        ("2A", "Corse-du-Sud"),
        ("2B", "Haute-Corse"),
        ("21", "Côte-d'Or"),
        ("22", "Côtes-d'Armor"),
        ("23", "Creuse"),
        ("24", "Dordogne"),
        ("25", "Doubs"),
        ("26", "Drôme"),
        ("27", "Eure"),
        ("28", "Eure-et-Loir"),
        ("29", "Finistère"),
        ("30", "Gard"),
        ("31", "Haute-Garonne"),
        ("32", "Gers"),
        ("33", "Gironde"),
        ("34", "Hérault"),
        ("35", "Ille-et-Vilaine"),
        ("36", "Indre"),
        ("37", "Indre-et-Loire"),
        ("38", "Isère"),
        ("39", "Jura"),
        ("40", "Landes"),
        ("41", "Loir-et-Cher"),
        ("42", "Loire"),
        ("43", "Haute-Loire"),
        ("44", "Loire-Atlantique"),
        ("45", "Loiret"),
        ("46", "Lot"),
        ("47", "Lot-et-Garonne"),
        ("48", "Lozère"),
        ("49", "Maine-et-Loire"),
        ("50", "Manche"),
        ("51", "Marne"),
        ("52", "Haute-Marne"),
        ("53", "Mayenne"),
        ("54", "Meurthe-et-Moselle"),
        ("55", "Meuse"),
        ("56", "Morbihan"),
        ("57", "Moselle"),
        ("58", "Nièvre"),
        ("59", "Nord"),
        ("60", "Oise"),
        ("61", "Orne"),
        ("62", "Pas-de-Calais"),
        ("63", "Puy-de-Dôme"),
        ("64", "Pyrénées-Atlantiques"),
        ("65", "Hautes-Pyrénées"),
        ("66", "Pyrénées-Orientales"),
        ("67", "Bas-Rhin"),
        ("68", "Haut-Rhin"),
        ("69", "Rhône"),
        ("70", "Haute-Saône"),
        ("71", "Saône-et-Loire"),
        ("72", "Sarthe"),
        ("73", "Savoie"),
        ("74", "Haute-Savoie"),
        ("75", "Paris"),
        ("76", "Seine-Maritime"),
        ("77", "Seine-et-Marne"),
        ("78", "Yvelines"),
        ("79", "Deux-Sèvres"),
        ("80", "Somme"),
        ("81", "Tarn"),
        ("82", "Tarn-et-Garonne"),
        ("83", "Var"),
        ("84", "Vaucluse"),
        ("85", "Vendée"),
        ("86", "Vienne"),
        ("87", "Haute-Vienne"),
        ("88", "Vosges"),
        ("89", "Yonne"),
        ("90", "Territoire de Belfort"),
        ("91", "Essonne"),
        ("92", "Hauts-de-Seine"),
        ("93", "Seine-Saint-Denis"),
        ("94", "Val-de-Marne"),
        ("95", "Val-d'Oise"),
        ("971", "Guadeloupe"),
        ("972", "Martinique"),
        ("973", "Guyane"),
        ("974", "La Réunion"),
        ("976", "Mayotte"),
    ]
    .iter()
    .map(|&(code, name)| (code.to_string(), name.to_string()))
    .collect();
    pub static ref RPG_DEP: HashMap<&'static str, Vec<&'static str>> = HashMap::from([
        (
            "84",
            vec![
                "1", "3", "7", "15", "26", "38", "42", "43", "63", "69", "73", "74"
            ]
        ),
        ("27", vec!["21", "25", "39", "58", "70", "71", "89", "90"]),
        ("53", vec!["22", "29", "35", "56"]),
        ("24", vec!["18", "28", "36", "37", "41", "45"]),
        ("94", vec!["2A", "2B"]),
        (
            "44",
            vec!["8", "10", "51", "52", "54", "55", "57", "67", "68", "88"]
        ),
        ("32", vec!["2", "59", "60", "62", "80"]),
        ("11", vec!["75", "77", "78", "91", "92", "93", "94", "95"]),
        ("28", vec!["14", "27", "50", "61", "76"]),
        (
            "75",
            vec![
                "16", "17", "19", "23", "24", "33", "40", "47", "64", "79", "86", "87"
            ]
        ),
        (
            "76",
            vec![
                "9", "11", "12", "30", "31", "32", "34", "46", "48", "65", "66", "81", "82"
            ]
        ),
        ("52", vec!["44", "49", "53", "72", "85"]),
        ("93", vec!["4", "5", "6", "13", "83", "84"]),
        ("01", vec!["971"]),
        ("02", vec!["972"]),
        ("03", vec!["973"]),
        ("04", vec!["974"]),
        ("06", vec!["976"]),
    ]);
    pub static ref VULCAIN_COLORS: Vec<(u8, u8, u8)> = vec![
        (0, 0, 0),
        (4, 25, 30),
        (25, 50, 60),
        (50, 200, 80),
        (80, 200, 120)
    ];
}

pub fn get_departement_list() -> HashMap<String, String> {
    DEPARTEMENTS.clone()
}

pub fn get_departement_name(code: &str) -> Option<String> {
    DEPARTEMENTS.get(code).cloned()
}

pub fn get_departement_code(name: &str) -> Option<String> {
    DEPARTEMENTS.iter().find_map(
        |(code, n)| {
            if n == name { Some(code.clone()) } else { None }
        },
    )
}

pub fn get_departements_names() -> Vec<String> {
    DEPARTEMENTS.values().cloned().collect()
}

pub fn get_rpg_for_dep_code(code: &str) -> Option<&str> {
    RPG_DEP
        .iter()
        .find_map(|(rpg, deps)| {
            if deps.contains(&code) {
                Some(rpg)
            } else {
                None
            }
        })
        .map(|v| &**v)
}

pub fn create_directory_if_not_exists(path: &str) -> Result<(), Box<dyn Error>> {
    if !Path::new(path).exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

pub fn compress_folder(
    folder_directory_path: &str,
    folder_name: &str,
    destination_directory_path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("7z");
    command.arg("a");

    let archive_path = format!("{}.zip", folder_name);

    command.arg(&archive_path);
    command.current_dir(destination_directory_path.unwrap_or(folder_directory_path));
    command.arg(folder_name);
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!("Failed to execute command: {:?}", output).into());
    }

    Ok(())
}

pub fn extract_archive(
    archive_path: &str,
    destination_directory_path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("7z");
    command.arg("x");

    if let Some(destination_directory_path) = destination_directory_path {
        command.arg(format!("-o{}", destination_directory_path));
    } else {
        let parent_dir = Path::new(archive_path).parent().unwrap();
        command.arg(format!("-o{}", parent_dir.to_str().unwrap()));
    }

    command.arg(archive_path);
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!("Failed to execute command: {:?}", output).into());
    }

    Ok(())
}

pub fn find_filepath_in_archive(
    archive_path: &str,
    file_name: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    let output = Command::new("7z").args(["l", archive_path]).output()?;

    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains(file_name) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(path) = parts.last() {
                    let path_str = path.to_string();
                    if let Some(pos) = path_str.rfind(file_name) {
                        return Ok(Some(path_str[..pos].to_string()));
                    }
                }
            }
        }
        Ok(None)
    } else {
        Err(format!("Failed to execute command: {:?}", output).into())
    }
}

fn move_folder_contents(src_dir: &Path, dst_dir: &Path) -> Result<(), Box<dyn Error>> {
    if !src_dir.exists() {
        return Err(format!("Source directory does not exist: {}", src_dir.display()).into());
    }

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst_dir.join(entry.file_name());

        if path.is_dir() {
            fs::create_dir_all(&dest_path)?;
            move_folder_contents(&path, &dest_path)?;
            fs::remove_dir_all(&path)?;
        } else {
            fs::copy(&path, &dest_path)?;
            fs::remove_file(&path)?;
        }
    }

    Ok(())
}

pub fn extract_specific_folder(
    archive_path: &str,
    folder_name: &str,
    output_dir: &str,
    extracted_name: Option<&str>,
    filter: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    create_directory_if_not_exists(output_dir)?;
    let temp_extract_dir = Path::new(output_dir).join("temp_extract");
    create_directory_if_not_exists(temp_extract_dir.to_str().unwrap())?;
    Command::new("7z")
        .args(["x", archive_path])
        .arg(format!("-o{}", temp_extract_dir.to_str().unwrap()))
        .output()?;

    let extracted_folder_path = temp_extract_dir.join(folder_name);
    let destination = if let Some(extracted_name) = extracted_name {
        create_directory_if_not_exists(
            Path::new(output_dir).join(extracted_name).to_str().unwrap(),
        )?;
        Path::new(output_dir).join(extracted_name)
    } else {
        create_directory_if_not_exists(Path::new(output_dir).join("extracted").to_str().unwrap())?;
        Path::new(output_dir).join("extracted")
    };

    for entry in fs::read_dir(&extracted_folder_path)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(filter) = filter {
            let file_name = path.file_stem().unwrap().to_str().unwrap();
            if file_name == filter {
                let dest_path = destination.join(entry.file_name());
                if path.is_dir() {
                    fs::create_dir_all(&dest_path)?;
                    move_folder_contents(&path, &dest_path)?;
                    fs::remove_dir_all(&path)?;
                } else {
                    fs::copy(&path, &dest_path)?;
                    fs::remove_file(&path)?;
                }
            }
        } else {
            let dest_path = destination.join(entry.file_name());
            if path.is_dir() {
                fs::create_dir_all(&dest_path)?;
                move_folder_contents(&path, &dest_path)?;
                fs::remove_dir_all(&path)?;
            } else {
                fs::copy(&path, &dest_path)?;
                fs::remove_file(&path)?;
            }
        }
    }

    fs::remove_dir_all(temp_extract_dir)?;
    Ok(())
}

pub fn get_previous_projects() -> Result<HashMap<String, Vec<String>>, Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd")
        .args(&["/C", "dir", "projects\\", "/b", "/a:d"])
        .output()?;
    #[cfg(not(target_os = "windows"))]
    let output = Command::new("ls").args(["projects/"]).output()?;
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut projects = HashMap::new();
    for line in output_str.lines() {
        let project_name = line.trim();
        if project_name != "cache" {
            let project_path = format!("projects/{}/", project_name);
            let preview_image_path = format!("{}preview.png", project_path);
            let project_file_path = format!("{}{}.qgz", project_path, project_name);
            projects.insert(
                project_name.to_string(),
                vec![preview_image_path, project_file_path],
            );
        }
    }
    Ok(projects)
}

pub fn get_operating_system() -> &'static str {
    std::env::consts::OS
}
