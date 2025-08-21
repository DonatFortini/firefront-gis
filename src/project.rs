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
    let image_src = use_state(String::new);
    let image_loading = use_state(|| true);
    let image_error = use_state(|| false);

    let load_image = {
        let project_data = project_data.clone();
        let image_src = image_src.clone();
        let image_loading = image_loading.clone();
        let image_error = image_error.clone();

        Callback::from(move |_| {
            let project_data = (*project_data).clone();
            let image_src = image_src.clone();
            let image_loading = image_loading.clone();
            let image_error = image_error.clone();

            spawn_local(async move {
                image_loading.set(true);
                image_error.set(false);

                let file_path = match project_data.view_mode {
                    ViewMode::Vegetation => format!("{}_VEGET.jpeg", project_data.name),
                    ViewMode::Satellite => format!("{}_ORTHO.jpeg", project_data.name),
                };

                #[derive(Serialize)]
                struct GetProjectDataArgs {
                    name: String,
                    data: String,
                }

                let args = GetProjectDataArgs {
                    name: project_data.name.clone(),
                    data: file_path,
                };

                match invoke(
                    "get_project_data",
                    serde_wasm_bindgen::to_value(&args).unwrap(),
                )
                .await
                {
                    result if result.is_string() => {
                        if let Some(relative_path) = result.as_string() {
                            let converted_path = convertFileSrc(&relative_path, None);
                            image_src.set(converted_path);
                            image_loading.set(false);
                        } else {
                            image_error.set(true);
                            image_loading.set(false);
                        }
                    }
                    _ => {
                        image_error.set(true);
                        image_loading.set(false);
                    }
                }
            });
        })
    };

    {
        let load_image = load_image.clone();
        let project_data = project_data.clone();
        use_effect_with(project_data.view_mode.clone(), move |_| {
            load_image.emit(());
            || ()
        });
    }

    {
        let load_image = load_image.clone();
        use_effect_with((), move |_| {
            load_image.emit(());
            || ()
        });
    }

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
                if let Ok(serialized_args) = serde_wasm_bindgen::to_value(&args)
                    && let Some(result) = invoke("export", serialized_args).await.as_string()
                {
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
                    {
                        if *image_loading {
                            html! {
                                <div class="image-loading">
                                    <p>{"Chargement de l'image..."}</p>
                                    <div class="loading-spinner"></div>
                                </div>
                            }
                        } else if *image_error {
                            html! {
                                <div class="image-error">
                                    <p>{"Erreur de chargement de l'image"}</p>
                                </div>
                            }
                        } else if !(*image_src).is_empty() {
                            html! {
                                <img
                                    src={(*image_src).clone()}
                                    alt={format!("Vue cartographique de {}", project_data.name)}
                                />
                            }
                        } else {
                            html! {
                                <div class="image-loading">
                                    <p>{"Chargement de l'image..."}</p>
                                    <div class="loading-spinner"></div>
                                </div>
                            }
                        }
                    }
                </div>
            </div>
        </div>
    }
}
