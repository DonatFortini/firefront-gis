use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Event};
use yew::prelude::*;

use crate::types::{AppView, ProjectData, ViewMode};

#[derive(Properties, PartialEq)]
pub struct LoadingProps {
    pub project_name: String,
    pub on_view_change: Callback<AppView>,
}

#[function_component(Loading)]
pub fn loading(props: &LoadingProps) -> Html {
    let project_name = props.project_name.clone();
    let progress_message = use_state(|| "Initializing project...".to_string());
    let progress_percentage = use_state(|| 0);

    {
        let progress_message = progress_message.clone();
        let progress_percentage = progress_percentage.clone();
        let project_name = project_name.clone();
        let on_view_change = props.on_view_change.clone();

        use_effect_with((), move |_| {
            let progress_message = progress_message.clone();
            let progress_percentage = progress_percentage.clone();
            let project_name = project_name.clone();
            let on_view_change = on_view_change.clone();

            let closure = Closure::wrap(Box::new(move |event: Event| {
                let message = Reflect::get(&event, &"detail".into())
                    .map(|detail| detail.as_string())
                    .unwrap_or_default();
                let message_str = message.clone().unwrap_or_default();
                progress_message.set(message_str.clone());

                let percentage = match message_str.as_str() {
                    "Recherche des fichiers" => 10,
                    "Téléchargement des données" => 25,
                    "Initialisation du projet" => 35,
                    "Preparation des Couches" => 50,
                    "Ajout des Couches" => 70,
                    "Finalisation" => 85,
                    "Nettoyage" => 95,
                    "Projet créé avec succès" => 100,
                    _ => *progress_percentage,
                };

                progress_percentage.set(percentage);

                if message == Some("Projet créé avec succès".to_string()) {
                    let on_view_change = on_view_change.clone();
                    let project_name = project_name.clone();

                    spawn_local(async move {
                        let promise = js_sys::Promise::new(&mut |resolve, _| {
                            let timeout_id = web_sys::window()
                                .unwrap()
                                .set_timeout_with_callback_and_timeout_and_arguments_0(
                                    &resolve, 1000,
                                )
                                .unwrap();
                            let _ = timeout_id;
                            resolve.call0(&resolve).unwrap();
                        });
                        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;

                        on_view_change.emit(AppView::Project(ProjectData {
                            name: project_name.clone(),
                            file_path: format!("projects/{}/{}.tiff", project_name, project_name),
                            view_mode: ViewMode::Vegetation,
                        }));
                    });
                }
            }) as Box<dyn FnMut(_)>);

            if let Some(win) = window() {
                win.add_event_listener_with_callback(
                    "tauri-progress-update",
                    closure.as_ref().unchecked_ref(),
                )
                .unwrap();
            }

            closure.forget(); // Prevent drop
            || ()
        });
    }

    html! {
        <div class="loading-view">
            <h2>{"Creating Project"}</h2>
            <div class="loading-card">
                <h3>{&props.project_name}</h3>
                <div class="progress-container">
                    <div class="progress-bar" style={format!("width: {}%;", *progress_percentage)}></div>
                </div>
                <p class="status-message">{&*progress_message}</p>
                <p class="percentage">{format!("{}%", *progress_percentage)}</p>
            </div>
        </div>
    }
}
