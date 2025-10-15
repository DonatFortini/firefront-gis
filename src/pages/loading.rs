use crate::types::{Route, ViewMode};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use stylist::yew::styled_component;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::api::listen;

#[derive(Properties, PartialEq)]
pub struct LoadingProps {
    pub project_name: String,
}

#[derive(Clone, PartialEq)]
struct ProgressState {
    message: Rc<String>,
    percentage: u8,
    error: Option<Rc<String>>,
    subtask: Option<Rc<String>>,
    subtask_count: Option<(usize, usize)>,
    eta: Option<u64>,
}

impl Default for ProgressState {
    fn default() -> Self {
        Self {
            message: Rc::new("Initialisation du projet...".to_string()),
            percentage: 0,
            error: None,
            subtask: None,
            subtask_count: None,
            eta: None,
        }
    }
}

#[styled_component(Loading)]
pub fn loading(props: &LoadingProps) -> Html {
    let navigator = use_navigator().unwrap();
    let progress_state = use_state(ProgressState::default);
    let project_name = Rc::new(props.project_name.clone());

    {
        let project_name = project_name.clone();
        let navigator = navigator.clone();
        let progress_state = progress_state.clone();

        use_effect_with((), move |_| {
            let cleanup_handle = Rc::new(RefCell::new(None));

            {
                let cleanup_handle2 = cleanup_handle.clone();
                spawn_local(async move {
                    let cleanup = setup_progress_tracking(
                        (*project_name).clone(),
                        navigator.clone(),
                        progress_state.clone(),
                    )
                    .await;
                    *cleanup_handle2.borrow_mut() = Some(cleanup);
                });
            }

            move || {
                if let Some(cleanup_fn) = cleanup_handle.borrow_mut().take() {
                    cleanup_fn();
                }
            }
        });
    }

    let container_style = css!(
        r#"
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100vh;
        padding: 24px;
        background: linear-gradient(135deg, #0e0e0e 0%, #1a1a1a 100%);
        "#
    );

    let header_style = css!(
        r#"
        text-align: center;
        margin-bottom: 48px;
        
        h2 {
            color: #ffffff;
            font-size: 2rem;
            font-weight: 600;
            margin-bottom: 8px;
            background: linear-gradient(135deg, #ff4141 0%, #ff6b6b 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }
        
        .subtitle {
            color: #999999;
            font-size: 0.95rem;
        }
        "#
    );

    let card_style = css!(
        r#"
        background-color: #242424;
        border-radius: 16px;
        padding: 48px;
        width: 100%;
        max-width: 700px;
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        backdrop-filter: blur(10px);
        
        @media (max-width: 768px) {
            padding: 32px 24px;
            max-width: 100%;
        }
        "#
    );

    html! {
        <div class={container_style}>
            <div class={header_style}>
                <h2>{"Création du projet"}</h2>
                <p class="subtitle">{"Veuillez patienter pendant la configuration..."}</p>
            </div>

            <div class={card_style}>
                <ProjectNameDisplay name={(**project_name).to_string()} />
                <ProgressBar percentage={progress_state.percentage} />
                <StatusMessage state={(*progress_state).clone()} />

                {
                    if let Some(err) = &progress_state.error {
                        html! { <ErrorDisplay error={(**err).clone()} /> }
                    } else {
                        html! {}
                    }
                }
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ProjectNameProps {
    name: String,
}

#[styled_component(ProjectNameDisplay)]
fn project_name_display(props: &ProjectNameProps) -> Html {
    let style = css!(
        r#"
        text-align: center;
        margin-bottom: 32px;
        
        h3 {
            font-size: 1.5rem;
            font-weight: 600;
            color: #ffffff;
            margin: 0;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 12px;
        }
        
        .icon {
            font-size: 1.8rem;
        }
        "#
    );

    html! {
        <div class={style}>
            <h3>
                <span class="icon">{"📁"}</span>
                {&props.name}
            </h3>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ProgressBarProps {
    percentage: u8,
}

#[styled_component(ProgressBar)]
fn progress_bar(props: &ProgressBarProps) -> Html {
    let style = css!(
        r#"
        margin: 32px 0;
        
        .progress-container {
            width: 100%;
            height: 12px;
            background-color: #1c1c1c;
            border-radius: 6px;
            overflow: hidden;
            position: relative;
            box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.3);
        }
        
        .progress-bar {
            height: 100%;
            background: linear-gradient(90deg, #ff4141 0%, #ff6b6b 100%);
            border-radius: 6px;
            transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            position: relative;
            box-shadow: 0 0 10px rgba(255, 65, 65, 0.5);
        }
        
        .progress-bar::after {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: linear-gradient(
                90deg,
                transparent 0%,
                rgba(255, 255, 255, 0.3) 50%,
                transparent 100%
            );
            animation: shimmer 2s infinite;
        }
        
        @keyframes shimmer {
            0% { transform: translateX(-100%); }
            100% { transform: translateX(100%); }
        }
        
        .percentage-text {
            text-align: center;
            font-weight: 700;
            color: #ff4141;
            font-size: 2.5rem;
            margin-top: 24px;
            font-family: 'Fira Code', monospace;
            text-shadow: 0 0 20px rgba(255, 65, 65, 0.3);
        }
        "#
    );

    html! {
        <div class={style}>
            <div class="progress-container">
                <div
                    class="progress-bar"
                    style={format!("width: {}%;", props.percentage)}
                />
            </div>
            <div class="percentage-text">
                {format!("{}%", props.percentage)}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct StatusMessageProps {
    state: ProgressState,
}

#[styled_component(StatusMessage)]
fn status_message(props: &StatusMessageProps) -> Html {
    let style = css!(
        r#"
        text-align: center;
        
        .main-message {
            color: #ffffff;
            font-size: 1.1rem;
            font-weight: 500;
            margin-bottom: 16px;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 12px;
        }
        
        .spinner {
            width: 20px;
            height: 20px;
            border: 3px solid rgba(255, 255, 255, 0.1);
            border-top-color: #ff4141;
            border-radius: 50%;
            animation: spin 1s linear infinite;
        }
        
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        
        .subtask {
            color: #999999;
            font-size: 0.95rem;
            margin: 8px 0;
        }

        .eta {
            color: #66b3ff;
            font-size: 0.9rem;
            font-weight: 500;
            margin: 12px auto 8px auto;
            padding: 6px 12px;
            background-color: rgba(102, 179, 255, 0.1);
            border-radius: 4px;
            display: block;
            width: fit-content;
        }
        
        .subtask-count {
            color: #ff4141;
            font-size: 0.9rem;
            font-weight: 600;
            margin: 0 auto;
            padding: 8px 16px;
            background-color: rgba(255, 65, 65, 0.1);
            border-radius: 4px;
            display: block;
            width: fit-content;
        }
        "#
    );

    html! {
        <div class={style}>
            <p class="main-message">
                <span class="spinner"></span>
                { &*props.state.message }
            </p>

            {
                if let Some(sub) = &props.state.subtask {
                    html! { <p class="subtask">{ &**sub }</p> }
                } else {
                    html! {}
                }
            }

            {
                if let Some(eta) = props.state.eta {
                    html! {
                        <div class="eta">
                            { format_eta(eta) }
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            {
                if let Some((curr, tot)) = props.state.subtask_count {
                    html! {
                        <div class="subtask-count">
                            { format!("Étape {} sur {}", curr, tot) }
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ErrorDisplayProps {
    error: String,
}

#[styled_component(ErrorDisplay)]
fn error_display(props: &ErrorDisplayProps) -> Html {
    let style = css!(
        r#"
        margin-top: 24px;
        padding: 16px 20px;
        background-color: rgba(231, 76, 60, 0.1);
        border: 1px solid #e74c3c;
        border-radius: 8px;
        color: #e74c3c;
        text-align: center;
        font-size: 0.95rem;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 12px;
        
        .icon {
            font-size: 1.5rem;
        }
        "#
    );

    html! {
        <div class={style}>
            <span class="icon">{"⚠️"}</span>
            <span>{ &props.error }</span>
        </div>
    }
}

fn format_eta(seconds: u64) -> String {
    if seconds < 60 {
        format!("⏱ Temps restant: {}s", seconds)
    } else if seconds < 3600 {
        let mins = seconds / 60;
        let sec = seconds % 60;
        format!("⏱ Temps restant: {}m {}s", mins, sec)
    } else {
        let hours = seconds / 3600;
        let mins = (seconds % 3600) / 60;
        format!("⏱ Temps restant: {}h {}m", hours, mins)
    }
}

type ProgressMessage = (String, Option<String>, Option<(usize, usize)>, Option<u64>);

fn parse_progress_message(payload: &str) -> ProgressMessage {
    let parts: Vec<&str> = payload.split('|').collect();
    let main = parts.first().map_or("", |s| *s).to_string();

    let subtask = parts.get(1).and_then(|s| {
        if !s.is_empty() {
            Some(s.to_string())
        } else {
            None
        }
    });

    let count = parts.get(2).and_then(|s| {
        if let Some((a, b)) = s.split_once('/')
            && let (Ok(c), Ok(t)) = (a.parse(), b.parse())
        {
            return Some((c, t));
        }
        None
    });

    let eta = parts.get(3).and_then(|s| s.parse::<u64>().ok());

    (main, subtask, count, eta)
}

fn calculate_progress_percentage(
    message: &str,
    current: Option<usize>,
    total: Option<usize>,
) -> u8 {
    let base = match message {
        "Recherche des régions" => 0,
        "Téléchargement des données" => 10,
        "Initialisation du projet" => 25,
        "Préparation des Couches" => 35,
        "Traitement de l'élévation" => 50,
        "Fusion des tuiles d'élévation" => 55,
        "Fusion des données" => 60,
        "Ajout des Couches" => 70,
        "Finalisation" => 85,
        "Nettoyage" => 95,
        "Projet créé avec succès" => 100,
        _ => 0,
    };

    let range = match message {
        "Recherche des régions" => 10,
        "Téléchargement des données" => 15,
        "Initialisation du projet" => 10,
        "Préparation des Couches" => 15,
        "Traitement de l'élévation" => 5,
        "Fusion des tuiles d'élévation" => 5,
        "Fusion des données" => 10,
        "Ajout des Couches" => 15,
        "Finalisation" => 10,
        "Nettoyage" => 5,
        _ => 0,
    };

    if let (Some(c), Some(t)) = (current, total)
        && t > 0
    {
        let step = (range as f64 * (c as f64) / (t as f64)) as u8;
        return (base + step).min(100);
    }
    base
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

async fn setup_progress_tracking(
    project_name: String,
    navigator: Navigator,
    progress_state: UseStateHandle<ProgressState>,
) -> Box<dyn FnOnce()> {
    let progress_state_clone = progress_state.clone();
    let project_name_clone = project_name.clone();
    let navigator_clone = navigator.clone();

    let max_percentage = Rc::new(Cell::new(0u8));
    let max_percentage_clone = max_percentage.clone();

    let is_active = Rc::new(Cell::new(true));
    let is_active_in_closure = is_active.clone();
    let navigated = Rc::new(Cell::new(false));
    let navigated_in_closure = navigated.clone();

    let unlisten_fn: Rc<RefCell<Option<js_sys::Function>>> = Rc::new(RefCell::new(None));
    let unlisten_in_closure = unlisten_fn.clone();

    let closure = Closure::<dyn FnMut(JsValue)>::new(move |event_val: JsValue| {
        if !is_active_in_closure.get() {
            return;
        }

        let payload_opt = if let Some(s) = event_val.as_string() {
            Some(s)
        } else if let Ok(field) = js_sys::Reflect::get(&event_val, &JsValue::from_str("payload")) {
            field.as_string()
        } else {
            None
        };

        let payload = match payload_opt {
            Some(s) => s,
            None => {
                web_sys::console::warn_1(
                    &format!(
                        "Impossible de récupérer le payload de l’événement: {:?}",
                        event_val
                    )
                    .into(),
                );
                return;
            }
        };

        web_sys::console::log_1(&format!("Progress update: {}", payload).into());

        let lower = payload.to_lowercase();
        let is_cancel = payload == "Création du projet annulée"
            || payload == "Project creation cancelled"
            || lower.contains("cancel");

        if is_cancel {
            is_active_in_closure.set(false);
            if let Some(f) = unlisten_in_closure.borrow_mut().take() {
                let _ = f.call0(&JsValue::NULL);
            }

            progress_state_clone.set(ProgressState {
                message: Rc::new("Projet annulé".to_string()),
                percentage: 0,
                error: Some(Rc::new(
                    "Création du projet annulée par l’utilisateur".to_string(),
                )),
                subtask: None,
                subtask_count: None,
                eta: None,
            });

            if !navigated_in_closure.get() {
                navigated_in_closure.set(true);
                spawn_local({
                    let nav = navigator_clone.clone();
                    async move {
                        wait_timeout(100).await;
                        nav.push(&Route::Home);
                    }
                });
            }
            return;
        }

        let (msg, sub, count, eta) = parse_progress_message(&payload);

        let mut percentage =
            calculate_progress_percentage(&msg, count.map(|(c, _)| c), count.map(|(_, t)| t));

        let curr_max = max_percentage_clone.get();
        if percentage < curr_max {
            percentage = curr_max;
        } else {
            max_percentage_clone.set(percentage);
        }

        if !is_active_in_closure.get() {
            return;
        }

        progress_state_clone.set(ProgressState {
            message: Rc::new(msg.clone()),
            percentage,
            error: None,
            subtask: sub.map(Rc::new),
            subtask_count: count,
            eta,
        });

        if msg == "Projet créé avec succès" {
            is_active_in_closure.set(false);
            if let Some(f) = unlisten_in_closure.borrow_mut().take() {
                let _ = f.call0(&JsValue::NULL);
            }

            if !navigated_in_closure.get() {
                navigated_in_closure.set(true);
                spawn_local({
                    let proj = project_name_clone.clone();
                    let nav = navigator_clone.clone();
                    async move {
                        wait_timeout(1000).await;
                        nav.push(&Route::Project {
                            project_name: proj,
                            view_mode: ViewMode::Vegetation,
                        });
                    }
                });
            }
        }
    });

    match setup_tauri_listener(&closure).await {
        Ok(unlisten) => {
            *unlisten_fn.borrow_mut() = Some(unlisten.clone());
            closure.forget();

            Box::new(move || {
                is_active.set(false);
                if let Some(f) = unlisten_fn.borrow_mut().take() {
                    let _ = f.call0(&JsValue::NULL);
                }
            })
        }
        Err(err_msg) => {
            progress_state.set(ProgressState {
                error: Some(Rc::new(err_msg.clone())),
                ..(*progress_state).clone()
            });
            Box::new(|| {})
        }
    }
}

async fn setup_tauri_listener(
    closure: &Closure<dyn FnMut(JsValue)>,
) -> Result<js_sys::Function, String> {
    let unlisten_js = listen("progress-update", closure.as_ref().unchecked_ref())
        .await
        .map_err(|e| format!("Erreur lors de listen : {:?}", e))?;

    unlisten_js
        .dyn_ref::<js_sys::Function>()
        .cloned()
        .ok_or_else(|| "Le retour de listen n’est pas une fonction".to_string())
}
