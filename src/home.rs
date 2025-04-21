use crate::types::{AppView, Project, ProjectData, ViewMode};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_without_args(cmd: &str) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
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

    {
        let projects = projects.clone();
        use_effect_with((), move |_| {
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
                        html! {
                            <div class="project-card">
                                <img src={converted_preview_path} alt={format!("Aperçu de {}", project.name)} />
                                <h3>{&project.name}</h3>
                                <button onclick={on_click}>{"Ouvrir"}</button>
                            </div>
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
