use crate::types::{Route, ViewMode};
use std::rc::Rc;
use stylist::yew::styled_component;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

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
}

impl Default for ProgressState {
    fn default() -> Self {
        Self {
            message: Rc::new("Initialisation du projet...".to_string()),
            percentage: 0,
            error: None,
            subtask: None,
            subtask_count: None,
        }
    }
}

#[styled_component(Loading)]
pub fn loading(props: &LoadingProps) -> Html {
    let navigator = use_navigator().unwrap();
    let progress_state = use_state(ProgressState::default);
    let project_name = use_memo(props.project_name.clone(), |name| Rc::new(name.clone()));

    {
        let project_name = project_name.clone();
        let navigator = navigator.clone();
        let progress_state = progress_state.clone();

        use_effect_with((), move |_| {
            let cleanup =
                setup_progress_tracking((**project_name).clone(), navigator, progress_state);
            move || cleanup()
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
                <ProjectNameDisplay name={(**project_name).clone()} />
                <ProgressBar percentage={progress_state.percentage} />
                <StatusMessage state={(*progress_state).clone()} />

                {if let Some(error) = &progress_state.error {
                    html! { <ErrorDisplay error={(**error).clone()} /> }
                } else {
                    html! {}
                }}
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
            font-style: italic;
            margin: 8px 0;
        }
        
        .subtask-count {
            color: #ff4141;
            font-size: 0.9rem;
            font-weight: 600;
            margin-top: 8px;
            padding: 8px 16px;
            background-color: rgba(255, 65, 65, 0.1);
            border-radius: 4px;
            display: inline-block;
        }
        "#
    );

    html! {
        <div class={style}>
            <p class="main-message">
                <span class="spinner"></span>
                {&*props.state.message}
            </p>

            {if let Some(subtask) = &props.state.subtask {
                html! { <p class="subtask">{&**subtask}</p> }
            } else {
                html! {}
            }}

            {if let Some((current, total)) = props.state.subtask_count {
                html! {
                    <div class="subtask-count">
                        {format!("Étape {} sur {}", current, total)}
                    </div>
                }
            } else {
                html! {}
            }}
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
            <span>{&props.error}</span>
        </div>
    }
}

//TODO: modif
fn get_progress_percentage(message: &str) -> u8 {
    match message {
        "Recherche des fichiers" => 10,
        "Téléchargement des données" => 25,
        "Initialisation du projet" => 35,
        "Préparation des Couches" => 50,
        "Fusion des données" => 60,
        "Ajout des Couches" => 70,
        "Finalisation" => 85,
        "Nettoyage" => 95,
        "Projet créé avec succès" => 100,
        _ => 0,
    }
}

fn parse_progress_message(payload: &str) -> (String, Option<String>, Option<(usize, usize)>) {
    let parts: Vec<&str> = payload.split('|').collect();
    let main_message = parts.first().map_or("", |s| *s).to_string();

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
    navigator: Navigator,
    progress_state: UseStateHandle<ProgressState>,
) -> Box<dyn FnOnce()> {
    let progress_state_clone = progress_state.clone();
    let project_name_clone = project_name.clone();
    let navigator_clone = navigator.clone();

    let closure = Closure::<dyn FnMut(String)>::new(move |payload: String| {
        let (main_message, subtask, count) = parse_progress_message(&payload);
        let percentage = get_progress_percentage(&main_message);

        web_sys::console::log_1(&format!("Progress update: {}", payload).into());

        progress_state_clone.set(ProgressState {
            message: Rc::new(main_message.clone()),
            percentage,
            error: None,
            subtask: subtask.map(Rc::new),
            subtask_count: count,
        });

        if main_message == "Projet créé avec succès" {
            handle_project_success(project_name_clone.clone(), navigator_clone.clone());
        }
    });

    match setup_tauri_listener(&closure) {
        Ok(cleanup) => {
            closure.forget();
            cleanup
        }
        Err(error) => {
            progress_state.set(ProgressState {
                error: Some(Rc::new(error)),
                ..(*progress_state).clone()
            });
            Box::new(|| {})
        }
    }
}

fn handle_project_success(project_name: String, navigator: Navigator) {
    spawn_local(async move {
        wait_timeout(1000).await;
        navigator.push(&Route::Project {
            project_name,
            view_mode: ViewMode::Vegetation,
        });
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
