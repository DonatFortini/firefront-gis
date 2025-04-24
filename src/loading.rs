use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::types::{AppView, ProjectData, ViewMode};

#[derive(Properties, PartialEq)]
pub struct LoadingProps {
    pub project_name: String,
    pub on_view_change: Callback<AppView>,
}

#[derive(Debug)]
struct ProgressState {
    message: String,
    percentage: u8,
    error: Option<String>,
    subtask: Option<String>,
    subtask_count: Option<(usize, usize)>, // (current, total)
}

impl Default for ProgressState {
    fn default() -> Self {
        Self {
            message: "Initialisation du projet...".to_string(),
            percentage: 0,
            error: None,
            subtask: None,
            subtask_count: None,
        }
    }
}

#[function_component(Loading)]
pub fn loading(props: &LoadingProps) -> Html {
    let progress_state = use_state(ProgressState::default);

    {
        let project_name = props.project_name.clone();
        let on_view_change = props.on_view_change.clone();
        let progress_state = progress_state.clone();

        use_effect_with((), move |_| {
            let cleanup = setup_progress_tracking(project_name, on_view_change, progress_state);
            move || cleanup()
        });
    }

    html! {
        <div class="loading-view">
            <h2>{"Création du projet"}</h2>
            <div class="loading-card">
                <h3>{&props.project_name}</h3>
                <LoadingProgressBar percentage={progress_state.percentage} />
                <p class="status-message">{&progress_state.message}</p>
                {
                    if let Some(subtask) = &progress_state.subtask {
                        html! {
                            <p class="subtask-message">{subtask}</p>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some((current, total)) = progress_state.subtask_count {
                        html! {
                            <p class="subtask-count">{format!("({}/{})", current, total)}</p>
                        }
                    } else {
                        html! {}
                    }
                }
                <p class="percentage">{format!("{}%", progress_state.percentage)}</p>
                {progress_state.error.as_ref().map(|error| html! {
                    <p class="error-message">{error}</p>
                }).unwrap_or_default()}
            </div>
        </div>
    }
}

#[function_component(LoadingProgressBar)]
fn loading_progress_bar(props: &LoadingProgressBarProps) -> Html {
    html! {
        <div class="progress-container">
            <div
                class="progress-bar"
                style={format!("width: {}%;", props.percentage)}
            />
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct LoadingProgressBarProps {
    percentage: u8,
}

fn get_progress_percentage(message: &str) -> u8 {
    match message {
        "Recherche des fichiers" => 10,
        "Téléchargement des données" => 25,
        "Initialisation du projet" => 35,
        "Préparation des Couches" => 50,
        "Ajout des Couches" => 70,
        "Finalisation" => 85,
        "Nettoyage" => 95,
        "Projet créé avec succès" => 100,
        _ => 0,
    }
}

fn parse_progress_message(payload: &str) -> (String, Option<String>, Option<(usize, usize)>) {
    let parts: Vec<&str> = payload.split('|').collect();
    let main_message = parts.get(0).map_or("", |s| *s).to_string();
    let subtask = if parts.len() > 1 {
        Some(parts[1].to_string())
    } else {
        None
    };

    let count = if parts.len() > 2 {
        if let Some((current_str, total_str)) = parts[2].split_once('/') {
            if let (Ok(current), Ok(total)) =
                (current_str.parse::<usize>(), total_str.parse::<usize>())
            {
                Some((current, total))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    (main_message, subtask, count)
}

fn setup_progress_tracking(
    project_name: String,
    on_view_change: Callback<AppView>,
    progress_state: UseStateHandle<ProgressState>,
) -> Box<dyn FnOnce()> {
    let progress_state_clone = progress_state.clone();
    let project_name_clone = project_name.clone();
    let on_view_change_clone = on_view_change.clone();

    let closure = Closure::<dyn FnMut(String)>::new(move |payload: String| {
        let (main_message, subtask, count) = parse_progress_message(&payload);
        let percentage = get_progress_percentage(&main_message);

        progress_state_clone.set(ProgressState {
            message: main_message.clone(),
            percentage,
            error: None,
            subtask,
            subtask_count: count,
        });

        if main_message == "Projet créé avec succès" {
            handle_project_success(project_name_clone.clone(), on_view_change_clone.clone());
        }
    });

    match setup_tauri_listener(&closure) {
        Ok(cleanup) => {
            closure.forget();
            cleanup
        }
        Err(error) => {
            progress_state.set(ProgressState {
                error: Some(error),
                message: progress_state.message.clone(),
                percentage: progress_state.percentage,
                subtask: progress_state.subtask.clone(),
                subtask_count: progress_state.subtask_count,
            });
            Box::new(|| {})
        }
    }
}

fn handle_project_success(project_name: String, on_view_change: Callback<AppView>) {
    spawn_local(async move {
        wait_timeout(1000).await;
        on_view_change.emit(AppView::Project(ProjectData {
            name: project_name.clone(),
            file_path: format!("projects/{}/{}_VEGET.jpeg", project_name, project_name),
            view_mode: ViewMode::Vegetation,
        }));
    });
}

async fn wait_timeout(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
            .unwrap();
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

fn setup_tauri_listener(closure: &Closure<dyn FnMut(String)>) -> Result<Box<dyn FnOnce()>, String> {
    let window = web_sys::window().ok_or("Failed to get window object")?;
    js_sys::Reflect::set(
        &window,
        &"__tauri_progress_callback".into(),
        closure.as_ref().unchecked_ref(),
    )
    .map_err(|_| "Failed to set up callback")?;

    let js_code = r#"
        const callback = (event) => {
            console.log('Tauri event received:', event);
            if (event && event.payload) {
                window.__tauri_progress_callback(event.payload);
            }
        };
        window.__TAURI__.event.listen('progress-update', callback)
            .then(unlisten => {
                console.log('Tauri listener registered successfully');
                window.__tauri_unlisten = unlisten;
            })
            .catch(err => {
                console.error('Error registering Tauri listener:', err);
            });
    "#;

    js_sys::eval(js_code).map_err(|_| "Failed to set up event listener")?;

    Ok(Box::new(|| {
        if let Some(win) = web_sys::window() {
            let cleanup_js = "if (window.__tauri_unlisten) window.__tauri_unlisten();";
            let _ = js_sys::eval(cleanup_js);
            let _ = js_sys::Reflect::delete_property(&win, &"__tauri_progress_callback".into());
        }
    }))
}
