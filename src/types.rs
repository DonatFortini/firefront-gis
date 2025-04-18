use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Debug)]
pub enum AppView {
    Home,
    Settings,
    Documentation,
    NewProject,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub preview_path: String,
    pub file_path: String,
}
