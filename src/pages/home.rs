use crate::{
    api::{convertFileSrc, invoke, invoke_without_args},
    types::{Project, Route, ViewMode},
};
use std::collections::HashMap;
use std::rc::Rc;
use stylist::yew::styled_component;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

#[styled_component(Home)]
pub fn home() -> Html {
    let projects = use_state(Vec::<Project>::new);
    let delete_in_progress = use_state(|| false);
    let app_version = use_state(|| Rc::new(String::from("...")));
    let navigator = use_navigator().unwrap();

    {
        let projects = projects.clone();
        use_effect_with((), move |_| {
            load_projects(projects);
            || ()
        });
    }

    {
        let app_version = app_version.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Some(version) = invoke_without_args("get_app_version").await.as_string() {
                    app_version.set(Rc::new(version));
                }
            });
            || ()
        });
    }

    let on_open_project = {
        let navigator = navigator.clone();
        Callback::from(move |project: Project| {
            navigator.push(&Route::Project {
                project_name: (*project.name).clone(),
                view_mode: ViewMode::Vegetation,
            });
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

                let response = invoke("delete_project", args).await;
                if let Ok(result) = serde_wasm_bindgen::from_value::<String>(response) {
                    if result == "success" {
                        load_projects(projects.clone());
                    } else {
                        web_sys::console::error_1(
                            &format!("Erreur lors de la suppression: {result}").into(),
                        );
                    }
                };

                delete_in_progress.set(false);
            });
        })
    };

    let header_style = css!(
        r#"
        position: fixed;
        top: 0;
        left: 260px;
        right: 0;
        padding: 16px 20px;
        background-color: #0e0e0e;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        z-index: 100;
        height: 58px;
        display: flex;
        align-items: center;
        justify-content: space-between;
        
        h2 {
            color: #ffffff;
            font-weight: 600;
            font-size: 1.25rem;
            margin: 0;
        }
        "#
    );

    let version_style = css!(
        r#"
        font-size: 0.85rem;
        color: #999999;
        font-weight: 500;
        padding: 6px 12px;
        background-color: #242424;
        border-radius: 4px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        font-family: 'Fira Code', monospace;
        "#
    );

    let grid_style = css!(
        r#"
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
        gap: 16px;
        margin-top: 8px;
        padding: 12px;
        "#
    );

    html! {
        <>
            <div class={header_style}>
                <h2>{"Projets précédents"}</h2>
                <span class={version_style}>{format!("v{}", **app_version)}</span>
            </div>
            <div class={grid_style}>
                {
                    (*projects).iter().map(|project| {
                        let project_clone = project.clone();
                        html! {
                            <ProjectCard
                                project={project_clone}
                                on_open={on_open_project.clone()}
                                on_delete={on_delete_project.clone()}
                            />
                        }
                    }).collect::<Html>()
                }
            </div>
        </>
    }
}

#[derive(Properties, PartialEq)]
struct ProjectCardProps {
    pub project: Project,
    pub on_open: Callback<Project>,
    pub on_delete: Callback<String>,
}

#[styled_component(ProjectCard)]
fn project_card(props: &ProjectCardProps) -> Html {
    let converted_preview_path = convertFileSrc(&props.project.preview_path, None);

    let on_click = {
        let on_open = props.on_open.clone();
        let project = props.project.clone();
        Callback::from(move |_| {
            on_open.emit(project.clone());
        })
    };

    let on_delete = {
        let on_delete = props.on_delete.clone();
        let project_name = (*props.project.name).clone();
        Callback::from(move |_: MouseEvent| {
            on_delete.emit(project_name.clone());
        })
    };

    let card_style = css!(
        r#"
        background-color: #242424;
        border-radius: 8px;
        overflow: hidden;
        box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
        transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
        border: 1px solid rgba(255, 255, 255, 0.1);
        
        &:hover {
            transform: translateY(-4px);
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
            border-color: #ff4141;
        }
        
        img {
            width: 100%;
            height: 180px;
            object-fit: cover;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }
        
        h3 {
            padding: 14px 16px;
            font-size: 1rem;
            font-weight: 600;
            color: #ffffff;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
            margin: 0;
        }
        
        .actions {
            display: flex;
            gap: 8px;
            padding: 12px;
        }
        
        button {
            flex: 1;
            padding: 10px 8px;
            font-size: 0.85rem;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            font-weight: 500;
            transition: all 0.15s;
        }
        
        .open-btn {
            background-color: #ff4141;
            color: white;
        }
        
        .open-btn:hover {
            background-color: #ff5757;
        }
        
        .delete-btn {
            background-color: #e74c3c;
            color: white;
        }
        
        .delete-btn:hover {
            background-color: #c0392b;
        }
        "#
    );

    html! {
        <div class={card_style}>
            <img src={converted_preview_path} alt={format!("Aperçu de {}", props.project.name)} />
            <h3>{&*props.project.name}</h3>
            <div class="actions">
                <button class="open-btn" onclick={on_click}>{"Ouvrir"}</button>
                <button class="delete-btn" onclick={on_delete}>{"Supprimer"}</button>
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
                        Some(Project::new(name, paths[0].clone(), paths[1].clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<Project>>();

            projects.set(loaded_projects);
        }
    });
}
