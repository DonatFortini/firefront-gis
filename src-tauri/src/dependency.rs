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
pub fn check_dependencies() -> Result<(), DependencyError> {
    let gdal_command = if cfg!(target_os = "windows") {
        "gdalinfo.exe"
    } else {
        "gdalinfo"
    };
    check_command(gdal_command, "--version", DependencyError::GDALNotInstalled)?;

    let python_command = if cfg!(target_os = "windows") {
        "python"
    } else {
        "python3"
    };
    check_command(
        python_command,
        "--version",
        DependencyError::PythonNotInstalled,
    )?;
    let seven_zip_command = if cfg!(target_os = "windows") {
        "7z.exe"
    } else {
        "7z"
    };
    check_command(
        seven_zip_command,
        "--help",
        DependencyError::SevenZipNotInstalled,
    )?;

    Ok(())
}
