use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::types::{AppView, ProjectData, ViewMode};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    fn convertFileSrc(filePath: &str, protocol: Option<&str>) -> String;
}

#[derive(Properties, PartialEq)]
pub struct ProjectProps {
    pub project_data: ProjectData,
    pub on_view_change: Callback<AppView>,
}

#[function_component(Project)]
pub fn project(props: &ProjectProps) -> Html {
    let project_data = use_state(|| props.project_data.clone());
    let view_mode = project_data.view_mode.clone();
    let project_name = project_data.name.clone();

    let file_path = match view_mode {
        ViewMode::Vegetation => format!("projects/{}/{}_VEGET.jpeg", project_name, project_name),
        ViewMode::Satellite => format!("projects/{}/{}_ORTHO.jpeg", project_name, project_name),
    };

    let image_path = convertFileSrc(&file_path, None);

    let on_toggle_view = {
        let project_data = project_data.clone();
        Callback::from(move |_| {
            let mut updated_data = (*project_data).clone();
            updated_data.view_mode = match updated_data.view_mode {
                ViewMode::Vegetation => ViewMode::Satellite,
                ViewMode::Satellite => ViewMode::Vegetation,
            };
            project_data.set(updated_data);
        })
    };

    let on_return = {
        let on_view_change = props.on_view_change.clone();
        Callback::from(move |_| {
            on_view_change.emit(AppView::Home);
        })
    };

    #[derive(Serialize)]
    struct ExportArgs {
        project_name: String,
    }

    let on_export = {
        let project_name = project_data.name.clone();
        Callback::from(move |_: MouseEvent| {
            let project_name = project_name.clone();
            spawn_local(async move {
                let args = ExportArgs {
                    project_name: project_name.clone(),
                };
                if let Ok(serialized_args) = serde_wasm_bindgen::to_value(&args) {
                    if let Some(result) = invoke("export", serialized_args).await.as_string() {
                        match result.as_str() {
                            "success" => {
                                web_sys::window()
                                    .unwrap()
                                    .alert_with_message("Exportation réussie")
                                    .unwrap();
                            }
                            "error" => {
                                web_sys::window()
                                    .unwrap()
                                    .alert_with_message("Erreur lors de l'exportation")
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                }
            });
        })
    };

    html! {
        <div class="project-view">
            <div class="project-sidebar">
                <h3>{&project_data.name}</h3>

                <button onclick={on_toggle_view.clone()} class="view-toggle-btn">
                    { match project_data.view_mode {
                        ViewMode::Vegetation => "Passer à la vue satellite",
                        ViewMode::Satellite => "Passer à la vue végétation",
                    }}
                </button>

                <button onclick={on_export.clone()} class="export-btn">
                    {"Exporter"}
                </button>

                <button onclick={on_return.clone()} class="return-btn">
                    {"Retour à l'accueil"}
                </button>
            </div>

            <div class="project-content">
                <div class="map-container">
                    <img src={image_path.clone()} alt={format!("Vue cartographique de {}", project_data.name)} />
                </div>
            </div>
        </div>
    }
}
