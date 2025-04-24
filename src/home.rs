use crate::types::{AppView, Project, ProjectData, ViewMode};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_without_args(cmd: &str) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    fn convertFileSrc(filePath: &str, protocol: Option<&str>) -> String;
}

#[derive(Properties, PartialEq)]
pub struct HomeProps {
    pub on_view_change: Callback<AppView>,
}

#[function_component(Home)]
pub fn home(props: &HomeProps) -> Html {
    let projects = use_state(Vec::<Project>::new);
    let delete_in_progress = use_state(|| false);

    {
        let projects = projects.clone();
        use_effect_with((), move |_| {
            load_projects(projects);
            || ()
        });
    }

    let on_open_project = {
        let on_view_change = props.on_view_change.clone();
        Callback::from(move |project: Project| {
            let on_view_change = on_view_change.clone();
            on_view_change.emit(AppView::Project(ProjectData {
                name: project.name.clone(),
                file_path: project.file_path.clone(),
                view_mode: ViewMode::Vegetation,
            }));
        })
    };

    let on_delete_project = {
        let projects = projects.clone();
        let delete_in_progress = delete_in_progress.clone();
        Callback::from(move |project_name: String| {
            let projects = projects.clone();
            let delete_in_progress = delete_in_progress.clone();

            if *delete_in_progress {
                return;
            }

            delete_in_progress.set(true);

            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                    "project_name": project_name
                }))
                .unwrap();

                match invoke("delete_project", args).await {
                    response => {
                        if let Ok(result) = serde_wasm_bindgen::from_value::<String>(response) {
                            if result == "success" {
                                load_projects(projects.clone());
                            } else {
                                web_sys::console::error_1(
                                    &format!("Erreur lors de la suppression: {}", result).into(),
                                );
                            }
                        }
                    }
                }

                delete_in_progress.set(false);
            });
        })
    };

    html! {
        <div class="home-view">
            <h2>{"Projets précédents"}</h2>
            <div class="project-grid">
                {
                    (*projects).iter().map(|project| {
                        let project_clone = project.clone();
                        let converted_preview_path = convertFileSrc(&project.preview_path, None);
                        let on_click = {
                            let on_open = on_open_project.clone();
                            let project = project_clone.clone();
                            Callback::from(move |_| {
                                on_open.emit(project.clone());
                            })
                        };
                        let on_delete = {
                            let on_delete_project = on_delete_project.clone();
                            let project_name = project.name.clone();
                            Callback::from(move |_: MouseEvent| {
                                on_delete_project.emit(project_name.clone());
                            })
                        };
                        html! {
                            <div class="project-card">
                                <img src={converted_preview_path} alt={format!("Aperçu de {}", project.name)} />
                                <h3>{&project.name}</h3>
                                <div class="project-card-actions">
                                    <button class="open-btn" onclick={on_click}>{"Ouvrir"}</button>
                                    <button class="delete-btn" onclick={on_delete}>{"Supprimer"}</button>
                                </div>
                            </div>
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}

fn load_projects(projects: UseStateHandle<Vec<Project>>) {
    spawn_local(async move {
        let result = invoke_without_args("get_projects").await;
        if let Ok(projects_map) =
            serde_wasm_bindgen::from_value::<HashMap<String, Vec<String>>>(result)
        {
            let loaded_projects = projects_map
                .into_iter()
                .filter_map(|(name, paths)| {
                    if paths.len() >= 2 {
                        Some(Project {
                            name,
                            preview_path: paths[0].clone(),
                            file_path: paths[1].clone(),
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<Project>>();

            if !loaded_projects.is_empty() {
                projects.set(loaded_projects);
            }
        } else {
            web_sys::console::error_1(&"Échec de l'analyse des projets".into());
        }
    });
}
