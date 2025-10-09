use crate::error::{CommandError, CommandResult};
use crate::utils::get_handle;
use tauri_plugin_shell::ShellExt;

/// Executes a sidecar command and returns stdout and status
///
/// # Arguments
/// * `command` - The name of the sidecar command to execute
/// * `args` - The arguments to pass to the command
///
/// # Returns
/// * `CommandResult<(String, ExitStatus)>` - Tuple containing stdout and exit status
///
/// # Example
/// ```rust
/// use crate::utils::command::execute_sidecar;
/// let (output, status) = execute_sidecar("gdalinfo", &["file.tif", "-json"]).await?;
/// ```
pub async fn execute_sidecar(
    command: &str,
    args: &[&str],
) -> CommandResult<(String, tauri_plugin_shell::process::ExitStatus)> {
    let app_handle = get_handle()
        .ok_or_else(|| CommandError::Sidecar("App handle not available".to_string()))?;

    let output = app_handle
        .shell()
        .sidecar(command)
        .map_err(|e| CommandError::ShellPlugin(e.to_string()))?
        .args(args)
        .output()
        .await
        .map_err(|e| CommandError::ShellPlugin(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CommandError::ExecutionFailed {
            command: command.to_string(),
            status: format!("{:?}", output.status),
            stdout,
            stderr,
        });
    }

    Ok((stdout, output.status))
}
