use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Debug)]
pub enum AppView {
    Home,
    Settings,
    Documentation,
    NewProject,
    Loading(String),
    Project(ProjectData),
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub preview_path: String,
    pub file_path: String,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ProjectData {
    pub name: String,
    pub file_path: String,
    pub view_mode: ViewMode,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum ViewMode {
    Vegetation,
    Satellite,
}
