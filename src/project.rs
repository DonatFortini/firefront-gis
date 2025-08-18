use gloo_timers::callback::Timeout;
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
    let image_loaded = use_state(|| false);
    let image_error = use_state(|| false);
    let retry_count = use_state(|| 0);

    let image_path = {
        let project_data = project_data.clone();
        use_memo(
            (project_data.name.clone(), project_data.view_mode.clone()),
            |(project_name, view_mode)| {
                let file_path = match view_mode {
                    ViewMode::Vegetation => {
                        format!("projects/{project_name}/{project_name}_VEGET.jpeg")
                    }
                    ViewMode::Satellite => {
                        format!("projects/{project_name}/{project_name}_ORTHO.jpeg")
                    }
                };
                convertFileSrc(&file_path, None)
            },
        )
    };

    {
        let image_loaded = image_loaded.clone();
        let image_error = image_error.clone();
        let retry_count = retry_count.clone();
        use_effect_with(image_path.clone(), move |_| {
            image_loaded.set(false);
            image_error.set(false);
            retry_count.set(0);
            || ()
        });
    }

    let check_image_with_retry = {
        let image_path = image_path.clone();
        let image_loaded = image_loaded.clone();
        let image_error = image_error.clone();
        let retry_count = retry_count.clone();

        Callback::from(move |_| {
            let image_path = (*image_path).clone();
            let image_loaded = image_loaded.clone();
            let image_error = image_error.clone();
            let retry_count = retry_count.clone();

            spawn_local(async move {
                let img = web_sys::HtmlImageElement::new().unwrap();

                let onload = {
                    let image_loaded = image_loaded.clone();
                    Closure::wrap(Box::new(move |_: web_sys::Event| {
                        image_loaded.set(true);
                    }) as Box<dyn FnMut(_)>)
                };

                let onerror = {
                    let image_error = image_error.clone();
                    let retry_count = retry_count.clone();
                    let image_path = image_path.clone();
                    Closure::wrap(Box::new(move |_: web_sys::Event| {
                        let current_retry = *retry_count;
                        if current_retry < 5 {
                            retry_count.set(current_retry + 1);
                            let image_path = image_path.clone();
                            Timeout::new(1000, move || {
                                let timestamp = js_sys::Date::now() as u64;
                                let img_with_cache_bust = web_sys::HtmlImageElement::new().unwrap();
                                img_with_cache_bust.set_src(&format!("{image_path}?t={timestamp}"));
                            })
                            .forget();
                        } else {
                            image_error.set(true);
                        }
                    }) as Box<dyn FnMut(_)>)
                };

                img.set_onload(Some(onload.as_ref().unchecked_ref()));
                img.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                img.set_src(&image_path);

                onload.forget();
                onerror.forget();
            });
        })
    };

    {
        let check_image_with_retry = check_image_with_retry.clone();
        use_effect_with(image_path.clone(), move |_| {
            check_image_with_retry.emit(());
            || ()
        });
    }

    let on_image_load = {
        let image_loaded = image_loaded.clone();
        Callback::from(move |_: Event| {
            image_loaded.set(true);
        })
    };

    let on_image_error = {
        let check_image_with_retry = check_image_with_retry.clone();
        Callback::from(move |_: Event| {
            check_image_with_retry.emit(());
        })
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
                        if *image_loaded {
                            html! {
                                <img
                                    src={(*image_path).clone()}
                                    alt={format!("Vue cartographique de {}", project_data.name)}
                                    onload={on_image_load}
                                    onerror={on_image_error}
                                />
                            }
                        } else if *image_error {
                            html! {
                                <div class="image-error">
                                    <p>{"Erreur de chargement de l'image"}</p>
                                </div>
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
