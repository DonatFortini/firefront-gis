use serde::{Deserialize, Serialize};
use std::rc::Rc;
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq, Debug)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/new-project")]
    NewProject,
    #[at("/settings")]
    Settings,
    #[at("/documentation")]
    Documentation,
    #[at("/loading/:project_name")]
    Loading { project_name: String },
    #[at("/project/:project_name/:view_mode")]
    Project {
        project_name: String,
        view_mode: ViewMode,
    },
    #[not_found]
    #[at("/404")]
    NotFound,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum ViewMode {
    #[serde(rename = "vegetation")]
    Vegetation,
    #[serde(rename = "satellite")]
    Satellite,
}

impl std::fmt::Display for ViewMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewMode::Vegetation => write!(f, "vegetation"),
            ViewMode::Satellite => write!(f, "satellite"),
        }
    }
}

impl std::str::FromStr for ViewMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "vegetation" => Ok(ViewMode::Vegetation),
            "satellite" => Ok(ViewMode::Satellite),
            _ => Err(format!("Invalid view mode: {}", s)),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Project {
    pub name: Rc<String>,
    pub preview_path: Rc<String>,
    pub file_path: Rc<String>,
}

impl Project {
    pub fn new(name: String, preview_path: String, file_path: String) -> Self {
        Self {
            name: Rc::new(name),
            preview_path: Rc::new(preview_path),
            file_path: Rc::new(file_path),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Copy)]
pub struct ProjectBoundingBox {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

#[derive(Serialize, Deserialize)]
pub struct NewProjectArgs {
    pub name: String,
    pub project_bb: ProjectBoundingBox,
}
