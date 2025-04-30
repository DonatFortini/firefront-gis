use crate::app_setup::Config;
use std::process::Command;
use std::str;

#[derive(Debug)]
pub enum DependencyError {
    GDALNotInstalled,
    PythonNotInstalled,
    SevenZipNotInstalled,
}

/// Vérifie si une commande existe en l'exécutant avec un argument spécifique.
///
/// # Arguments
/// - `command`: La commande à vérifier.
/// - `arg`: L'argument à passer à la commande.
/// - `error`: L'erreur à retourner si la commande n'est pas trouvée.
///
/// # Retourne
/// - Result<(), DependencyError>
fn check_command(command: &str, arg: &str, error: DependencyError) -> Result<(), DependencyError> {
    if Command::new(command).arg(arg).output().is_err() {
        Err(error)
    } else {
        println!("{} is found", command);
        Ok(())
    }
}

/// Vérifie si toutes les dépendances sont installées.
///
/// # Retourne
/// - Result<(), DependencyError>
pub fn check_dependencies(config: &mut Config) -> Result<(), DependencyError> {
    let (gdal_command, python_command, path_command, seven_zip_command) =
        if cfg!(target_os = "windows") {
            ("gdalinfo.exe", "python", "where", "7z.exe")
        } else {
            ("gdalinfo", "python3", "which", "7z")
        };

    for (command, arg, error, path_field) in [
        (
            gdal_command,
            "--version",
            DependencyError::GDALNotInstalled,
            &mut config.gdal_path,
        ),
        (
            python_command,
            "--version",
            DependencyError::PythonNotInstalled,
            &mut config.python_path,
        ),
    ] {
        check_command(command, arg, error)?;
        if let Ok(path_output) = Command::new(path_command).arg(command).output() {
            let path = str::from_utf8(&path_output.stdout)
                .unwrap_or_default()
                .trim();
            *path_field = Some(path.into());
            println!("{} path set to: {}", command, path);
        }
    }

    check_command(
        seven_zip_command,
        "--help",
        DependencyError::SevenZipNotInstalled,
    )?;

    Ok(())
}
