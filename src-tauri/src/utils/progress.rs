use crate::utils::get_handle;
use tauri::Emitter;

pub struct Progress;

impl Progress {
    pub fn status(message: impl AsRef<str>) {
        Self::emit(message.as_ref());
    }

    pub fn stage(stage: impl AsRef<str>, current: usize, total: usize) {
        let message = format!("{}|{}/{}", stage.as_ref(), current, total);
        Self::emit(&message);
    }

    pub fn substage(stage: impl AsRef<str>, substage: impl AsRef<str>) {
        let message = format!("{}|{}", stage.as_ref(), substage.as_ref());
        Self::emit(&message);
    }

    pub fn full(stage: impl AsRef<str>, substage: impl AsRef<str>, current: usize, total: usize) {
        let message = format!(
            "{}|{}|{}/{}",
            stage.as_ref(),
            substage.as_ref(),
            current,
            total
        );
        Self::emit(&message);
    }

    pub fn full_with_eta(
        stage: impl AsRef<str>,
        substage: impl AsRef<str>,
        current: usize,
        total: usize,
        eta_secs: u64,
    ) {
        let message = format!(
            "{}|{}|{}/{}|{}",
            stage.as_ref(),
            substage.as_ref(),
            current,
            total,
            eta_secs
        );
        Self::emit(&message);
    }

    fn emit(message: &str) {
        if let Some(handle) = get_handle() {
            let _ = handle.emit("progress-update", message);
        }
    }
}

pub struct ProgressTracker {
    stage: String,
    total_steps: usize,
    current_step: usize,
}

impl ProgressTracker {
    pub fn new(stage: impl Into<String>, total_steps: usize) -> Self {
        let tracker = Self {
            stage: stage.into(),
            total_steps,
            current_step: 0,
        };
        tracker.emit_current();
        tracker
    }

    pub fn next(&mut self, description: impl AsRef<str>) {
        self.current_step += 1;
        self.current_step = self.current_step.min(self.total_steps);
        self.emit_with_description(description.as_ref());
    }

    pub fn update(&self, description: impl AsRef<str>) {
        self.emit_with_description(description.as_ref());
    }

    pub fn set_step(&mut self, step: usize, description: impl AsRef<str>) {
        self.current_step = step.min(self.total_steps);
        self.emit_with_description(description.as_ref());
    }

    pub fn complete(mut self, message: impl AsRef<str>) {
        self.current_step = self.total_steps;
        Progress::status(message.as_ref());
    }

    fn emit_current(&self) {
        Progress::stage(&self.stage, self.current_step, self.total_steps);
    }

    fn emit_with_description(&self, description: &str) {
        Progress::full(
            &self.stage,
            description,
            self.current_step,
            self.total_steps,
        );
    }
}

pub struct DownloadProgress {
    stage: String,
    current_file: usize,
    total_files: usize,
}

impl DownloadProgress {
    pub fn new(total_files: usize) -> Self {
        Self {
            stage: "Téléchargement des données".to_string(),
            current_file: 0,
            total_files,
        }
    }

    pub fn start_file(&mut self, file_type: &str) {
        self.current_file += 1;
        Progress::full(
            &self.stage,
            format!("Téléchargement: {}", file_type),
            self.current_file,
            self.total_files,
        );
    }

    pub fn file_progress(
        &self,
        file_type: &str,
        downloaded_mb: f64,
        total_mb: f64,
        speed_mbps: f64,
        eta_secs: u64,
    ) {
        let detail = format!(
            "{} - {:.2}/{:.2} MB ({:.2} MB/s)",
            file_type, downloaded_mb, total_mb, speed_mbps
        );
        Progress::full_with_eta(
            &self.stage,
            &detail,
            self.current_file,
            self.total_files,
            eta_secs,
        );
    }

    pub fn status(&self, message: &str) {
        Progress::substage(&self.stage, message);
    }
}

pub struct LayerProgress {
    stage: String,
    total_layers: usize,
    current_layer: usize,
}

impl LayerProgress {
    pub fn new(stage: impl Into<String>, total_layers: usize) -> Self {
        Self {
            stage: stage.into(),
            total_layers,
            current_layer: 0,
        }
    }

    pub fn next_layer(&mut self, layer_name: &str) {
        self.current_layer += 1;
        Progress::full(
            &self.stage,
            format!("Ajout de {}", layer_name),
            self.current_layer,
            self.total_layers,
        );
    }

    pub fn layer_operation(&self, layer_name: &str, operation: &str, current: usize, total: usize) {
        let substage = format!("{} - {}", operation, layer_name);
        Progress::full(&self.stage, &substage, current, total);
    }
}

pub mod prelude {
    pub use super::{DownloadProgress, LayerProgress, Progress, ProgressTracker};
}
