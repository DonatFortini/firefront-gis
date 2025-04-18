use crate::types::Project;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[function_component(Home)]
pub fn home() -> Html {
    // State for projects
    let projects = use_state(|| {
        vec![
            Project {
                name: "Project 1".to_string(),
                preview_path: "public/tauri.svg".to_string(),
                file_path: "projects/project1/project1.qgz".to_string(),
            },
            Project {
                name: "Project 2".to_string(),
                preview_path: "public/tauri.svg".to_string(),
                file_path: "projects/project2/project2.qgz".to_string(),
            },
            Project {
                name: "Paris Map".to_string(),
                preview_path: "public/tauri.svg".to_string(),
                file_path: "projects/paris_map/paris_map.qgz".to_string(),
            },
        ]
    });

    // Load projects on component mount
    {
        let projects = projects.clone();
        use_effect(move || {
            spawn_local(async move {
                let result = invoke("get_projects", JsValue::NULL).await;
                if let Some(projects_json) = result.as_string() {
                    match serde_json::from_str::<HashMap<String, Vec<String>>>(&projects_json) {
                        Ok(projects_map) => {
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
                        }
                        Err(e) => {
                            // Fallback to trying to parse as a Vec<Project> directly
                            match serde_json::from_str::<Vec<Project>>(&projects_json) {
                                Ok(loaded_projects) => {
                                    if !loaded_projects.is_empty() {
                                        projects.set(loaded_projects);
                                    }
                                }
                                Err(_) => {
                                    web_sys::console::error_1(
                                        &format!("Error parsing projects: {:?}", e).into(),
                                    );
                                }
                            }
                        }
                    }
                } else {
                    web_sys::console::error_1(&"Failed to get projects".into());
                }
            });

            || () // Cleanup function
        });
    }

    let on_open_project = Callback::from(|_project: Project| {
        // TODO: Implement opening a project
        web_sys::console::log_1(&"Opening project".into());
    });

    html! {
        <div class="home-view">
            <h2>{"Previous Projects"}</h2>
            <div class="project-grid">
                {
                    (*projects).iter().map(|project| {
                        let project_clone = project.clone();
                        let on_click = {
                            let on_open = on_open_project.clone();
                            let project = project_clone.clone();
                            Callback::from(move |_| {
                                on_open.emit(project.clone());
                            })
                        };
                        html! {
                            <div class="project-card">
                                <img src={project.preview_path.clone()} alt={format!("{} preview", project.name)} />
                                <h3>{&project.name}</h3>
                                <button onclick={on_click}>{"Open"}</button>
                            </div>
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
