use wasm_bindgen::prelude::*;
use yew::prelude::*;

use crate::types::{AppView, ProjectData, ViewMode};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "dialog"])]
    async fn save(args: JsValue) -> JsValue;
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

    let image_path = match view_mode {
        ViewMode::Vegetation => format!("projects/{}/{}_VEGET.jpeg", project_name, project_name),
        ViewMode::Satellite => format!("projects/{}/{}_ORTHO.jpeg", project_name, project_name),
    };

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

    html! {
        <div class="project-view">
            <div class="project-sidebar">
                <h3>{&project_data.name}</h3>

                <button onclick={on_toggle_view.clone()} class="view-toggle-btn">
                    { match project_data.view_mode {
                        ViewMode::Vegetation => "Switch to Satellite",
                        ViewMode::Satellite => "Switch to Vegetation",
                    }}
                </button>

                <button  class="export-btn">
                    {"Export"}
                </button>

                <button onclick={on_return.clone()} class="return-btn">
                    {"Return to Home"}
                </button>
            </div>

            <div class="project-content">
                <div class="map-container">
                    <img src={image_path.clone()} alt={format!("{} map view", project_data.name)} />
                </div>
            </div>
        </div>
    }
}
