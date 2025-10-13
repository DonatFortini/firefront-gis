use crate::{
    api::{convertFileSrc, invoke},
    types::{Route, ViewMode},
};
use serde::Serialize;
use std::rc::Rc;
use stylist::yew::styled_component;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ProjectProps {
    pub project_name: String,
    pub view_mode: ViewMode,
}

#[derive(Clone, PartialEq)]
enum LoadingState {
    Loading,
    Loaded(Rc<String>),
    Error,
}

#[styled_component(Project)]
pub fn project(props: &ProjectProps) -> Html {
    let navigator = use_navigator().unwrap();

    let project_name = use_memo(props.project_name.clone(), |name| Rc::new(name.clone()));
    let view_mode = use_state(|| props.view_mode.clone());
    let loading_state = use_state(|| LoadingState::Loading);

    {
        let project_name = project_name.clone();
        let view_mode = view_mode.clone();
        let loading_state = loading_state.clone();

        use_effect_with((*view_mode).clone(), move |_| {
            loading_state.set(LoadingState::Loading);

            let project_name = (*project_name).clone();
            let view_mode = (*view_mode).clone();

            spawn_local(async move {
                match load_project_image(&project_name, &view_mode).await {
                    Ok(src) => loading_state.set(LoadingState::Loaded(Rc::new(src))),
                    Err(_) => loading_state.set(LoadingState::Error),
                }
            });

            || ()
        });
    }

    let on_toggle_view = {
        let view_mode = view_mode.clone();
        Callback::from(move |new_mode: ViewMode| {
            view_mode.set(new_mode);
        })
    };

    let on_export = {
        let project_name = (*project_name).clone();
        Callback::from(move |_event: MouseEvent| {
            let project_name = project_name.clone();
            spawn_local(async move {
                export_project(&project_name).await;
            });
        })
    };

    let on_return = {
        let navigator = navigator.clone();
        Callback::from(move |_event: MouseEvent| {
            navigator.push(&Route::Home);
        })
    };

    let container_style = css!(
        r#"
        display: flex;
        flex-direction: row;
        height: 100vh;
        width: 100%;
        background-color: #0e0e0e;
        overflow: hidden;
        
        @media (max-width: 768px) {
            flex-direction: column;
        }
        "#
    );

    html! {
        <div class={container_style}>
            <ProjectSidebar
                project_name={(**project_name).clone()}
                view_mode={(*view_mode).clone()}
                on_toggle_view={on_toggle_view}
                on_export={on_export}
                on_return={on_return}
            />

            <ProjectContent
                loading_state={(*loading_state).clone()}
                project_name={(**project_name).clone()}
            />
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ProjectSidebarProps {
    project_name: String,
    view_mode: ViewMode,
    on_toggle_view: Callback<ViewMode>,
    on_export: Callback<MouseEvent>,
    on_return: Callback<MouseEvent>,
}

async fn load_project_image(project_name: &str, view_mode: &ViewMode) -> Result<String, String> {
    let file_path = match view_mode {
        ViewMode::Vegetation => format!("{}_VEGET.jpeg", project_name),
        ViewMode::Satellite => format!("{}_ORTHO.jpeg", project_name),
        ViewMode::Altitude => format!("{}_ALTITUDE.jpeg", project_name),
    };

    #[derive(Serialize)]
    struct GetProjectDataArgs {
        name: String,
        data: String,
    }

    let args = GetProjectDataArgs {
        name: project_name.to_string(),
        data: file_path,
    };

    match invoke(
        "get_project_data",
        serde_wasm_bindgen::to_value(&args).unwrap(),
    )
    .await
    {
        result if result.is_string() => result
            .as_string()
            .map(|path| convertFileSrc(&path, None))
            .ok_or_else(|| "Invalid path".to_string()),
        _ => Err("Failed to load image".to_string()),
    }
}

#[styled_component(ProjectSidebar)]
fn project_sidebar(props: &ProjectSidebarProps) -> Html {
    let style = css!(
        r#"
        width: 20%;
        height: 100vh;
        flex-shrink: 0;
        background: linear-gradient(180deg, #242424 0%, #1c1c1c 100%);
        border-right: 1px solid rgba(255, 255, 255, 0.1);
        padding: 32px 24px;
        display: flex;
        flex-direction: column;
        gap: 16px;
        box-shadow: 2px 0 20px rgba(0, 0, 0, 0.3);
        
        h3 {
            font-size: 1.5rem;
            padding-bottom: 20px;
            margin-bottom: 20px;
            border-bottom: 2px solid rgba(255, 65, 65, 0.3);
            color: #ffffff;
            font-weight: 600;
            display: flex;
            align-items: center;
            gap: 12px;
        }
        
        .button-group {
            display: flex;
            flex-direction: column;
            gap: 12px;
            flex: 1;
        }
        
        .view-buttons {
            display: flex;
            flex-direction: column;
            gap: 8px;
            padding-bottom: 16px;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }
        
        button {
            width: 100%;
            padding: 16px 20px;
            border: none;
            border-radius: 8px;
            font-size: 1rem;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
            text-transform: none;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 10px;
            box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
        }
        
        button:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }
        
        button:active {
            transform: translateY(0);
        }
        
        .view-btn {
            background-color: #2a2a2a;
            color: #cccccc;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }
        
        .view-btn.active {
            background: linear-gradient(135deg, #ff4141 0%, #ff6b6b 100%);
            color: white;
            border-color: transparent;
        }
        
        .view-btn:hover:not(.active) {
            background-color: #333333;
            color: #ffffff;
            border-color: rgba(255, 255, 255, 0.2);
        }
        
        .export-btn {
            background: linear-gradient(135deg, #2ecc71 0%, #27ae60 100%);
            color: white;
        }
        
        .export-btn:hover {
            background: linear-gradient(135deg, #3de682 0%, #2ecc71 100%);
        }
        
        .return-btn {
            background-color: #2a2a2a;
            color: #cccccc;
            border: 1px solid rgba(255, 255, 255, 0.1);
            margin-top: auto;
        }
        
        .return-btn:hover {
            background-color: #333333;
            color: #ffffff;
            border-color: rgba(255, 255, 255, 0.2);
        }
        
        @media (max-width: 768px) {
            width: 100%;
            height: auto;
            flex-direction: row;
            align-items: center;
            justify-content: space-between;
            padding: 16px 20px;
            border-right: none;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
            
            h3 {
                margin: 0;
                padding: 0;
                border: none;
                font-size: 1.2rem;
            }
            
            .button-group {
                flex-direction: row;
                gap: 8px;
            }
            
            .view-buttons {
                flex-direction: row;
                border-bottom: none;
                padding-bottom: 0;
            }
            
            button {
                padding: 10px 16px;
                font-size: 0.9rem;
            }
        }
        "#
    );

    let on_view_change = {
        let on_toggle_view = props.on_toggle_view.clone();
        Callback::from(move |mode: ViewMode| on_toggle_view.emit(mode))
    };

    html! {
        <div class={style}>
            <h3>
                {&props.project_name}
            </h3>

            <div class="button-group">
                <div class="view-buttons">
                    <button
                        class={classes!("view-btn", (props.view_mode == ViewMode::Vegetation).then_some("active"))}
                        onclick={on_view_change.reform(|_| ViewMode::Vegetation)}
                    >
                        <span>{"🌿"}</span>
                        {"Végétation"}
                    </button>

                    <button
                        class={classes!("view-btn", (props.view_mode == ViewMode::Satellite).then_some("active"))}
                        onclick={on_view_change.reform(|_| ViewMode::Satellite)}
                    >
                        <span>{"🛰️"}</span>
                        {"Satellite"}
                    </button>

                    <button
                        class={classes!("view-btn", (props.view_mode == ViewMode::Altitude).then_some("active"))}
                        onclick={on_view_change.reform(|_| ViewMode::Altitude)}
                    >
                        <span>{"⛰️"}</span>
                        {"Altitude"}
                    </button>
                </div>

                <button class="export-btn" onclick={props.on_export.clone()}>
                    <span>{"📦"}</span>
                    {"Exporter le projet"}
                </button>

                <button class="return-btn" onclick={props.on_return.clone()}>
                    <span>{"←"}</span>
                    {"Retour à l'accueil"}
                </button>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ProjectContentProps {
    loading_state: LoadingState,
    project_name: String,
}

#[styled_component(ProjectContent)]
fn project_content(props: &ProjectContentProps) -> Html {
    let style = css!(
        r#"
        width: 80%;
        height: 100vh;
        padding: 32px;
        display: flex;
        flex-direction: column;
        background-color: #151515;
        overflow: hidden;
        
        @media (max-width: 768px) {
            width: 100%;
            height: auto;
            flex: 1;
            padding: 16px;
        }
        "#
    );

    html! {
        <div class={style}>
            <MapContainer loading_state={props.loading_state.clone()} project_name={props.project_name.clone()} />
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct MapContainerProps {
    loading_state: LoadingState,
    project_name: String,
}

#[derive(Clone)]
struct ViewState {
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    initial_zoom: f64,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            initial_zoom: 1.0,
        }
    }
}

#[styled_component(MapContainer)]
fn map_container(props: &MapContainerProps) -> Html {
    let view_state = use_state(ViewState::default);
    let is_dragging = use_state(|| false);
    let drag_start = use_state(|| (0.0, 0.0));
    let container_ref = use_node_ref();
    let image_ref = use_node_ref();

    {
        let view_state = view_state.clone();
        let container_ref = container_ref.clone();
        let image_ref = image_ref.clone();
        let loading_state = props.loading_state.clone();

        use_effect_with((loading_state.clone(),), move |_| {
            if matches!(loading_state, LoadingState::Loaded(_)) {
                gloo_timers::callback::Timeout::new(100, move || {
                    if let (Some(container), Some(img)) = (
                        container_ref.cast::<web_sys::HtmlElement>(),
                        image_ref.cast::<web_sys::HtmlImageElement>(),
                    ) {
                        let container_width = container.client_width() as f64;
                        let container_height = container.client_height() as f64;
                        let img_width = img.natural_width() as f64;
                        let img_height = img.natural_height() as f64;

                        if img_width > 0.0 && img_height > 0.0 {
                            let scale_x = container_width / img_width;
                            let scale_y = container_height / img_height;
                            let initial_zoom = scale_x.min(scale_y) * 0.95;

                            let mut new_state = (*view_state).clone();
                            new_state.zoom = initial_zoom;
                            new_state.initial_zoom = initial_zoom;
                            new_state.pan_x = 0.0;
                            new_state.pan_y = 0.0;
                            view_state.set(new_state);
                        }
                    }
                })
                .forget();
            }
            || ()
        });
    }

    let on_wheel = {
        let view_state = view_state.clone();
        Callback::from(move |e: WheelEvent| {
            e.prevent_default();

            let delta = if e.delta_y() > 0.0 { -0.1 } else { 0.1 };
            let mut new_state = (*view_state).clone();
            new_state.zoom = (new_state.zoom + delta).clamp(0.1, 5.0);
            view_state.set(new_state);
        })
    };

    let on_mouse_down = {
        let is_dragging = is_dragging.clone();
        let drag_start = drag_start.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_dragging.set(true);
            drag_start.set((e.client_x() as f64, e.client_y() as f64));
        })
    };

    let on_mouse_move = {
        let is_dragging = is_dragging.clone();
        let drag_start = drag_start.clone();
        let view_state = view_state.clone();
        Callback::from(move |e: MouseEvent| {
            if *is_dragging {
                e.prevent_default();
                let (start_x, start_y) = *drag_start;
                let dx = e.client_x() as f64 - start_x;
                let dy = e.client_y() as f64 - start_y;

                let mut new_state = (*view_state).clone();
                new_state.pan_x += dx;
                new_state.pan_y += dy;
                view_state.set(new_state);

                drag_start.set((e.client_x() as f64, e.client_y() as f64));
            }
        })
    };

    let on_mouse_up = {
        let is_dragging = is_dragging.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_dragging.set(false);
        })
    };

    let on_mouse_leave = {
        let is_dragging = is_dragging.clone();
        Callback::from(move |_: MouseEvent| {
            is_dragging.set(false);
        })
    };

    let on_double_click = {
        let view_state = view_state.clone();
        Callback::from(move |_: MouseEvent| {
            let mut new_state = (*view_state).clone();
            new_state.zoom = new_state.initial_zoom;
            new_state.pan_x = 0.0;
            new_state.pan_y = 0.0;
            view_state.set(new_state);
        })
    };

    let on_zoom_in = {
        let view_state = view_state.clone();
        Callback::from(move |_: MouseEvent| {
            let mut new_state = (*view_state).clone();
            new_state.zoom = (new_state.zoom + 0.2).min(5.0);
            view_state.set(new_state);
        })
    };

    let on_zoom_out = {
        let view_state = view_state.clone();
        Callback::from(move |_: MouseEvent| {
            let mut new_state = (*view_state).clone();
            new_state.zoom = (new_state.zoom - 0.2).max(0.1);
            view_state.set(new_state);
        })
    };

    let container_style = css!(
        r#"
        flex: 1;
        background: linear-gradient(135deg, #242424 0%, #1c1c1c 100%);
        border-radius: 12px;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.1);
        overflow: hidden;
        position: relative;
        cursor: grab;
        
        &:active {
            cursor: grabbing;
        }
        
        .image-wrapper {
            width: 100%;
            height: 100%;
            display: flex;
            align-items: center;
            justify-content: center;
            overflow: hidden;
            position: relative;
        }
        
        img {
            max-width: none;
            max-height: none;
            display: block;
            user-select: none;
            -webkit-user-drag: none;
            pointer-events: none;
        }
        
        .controls {
            position: absolute;
            top: 16px;
            right: 16px;
            display: flex;
            flex-direction: column;
            gap: 8px;
            z-index: 10;
        }
        
        .control-btn {
            width: 40px;
            height: 40px;
            background: rgba(36, 36, 36, 0.9);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 8px;
            color: #ffffff;
            font-size: 1.2rem;
            font-weight: bold;
            cursor: pointer;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.2s;
            backdrop-filter: blur(10px);
        }
        
        .control-btn:hover {
            background: rgba(255, 65, 65, 0.9);
            border-color: #ff4141;
            transform: scale(1.1);
        }
        
        .zoom-indicator {
            position: absolute;
            bottom: 16px;
            right: 16px;
            padding: 8px 12px;
            background: rgba(36, 36, 36, 0.9);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 6px;
            color: #ffffff;
            font-size: 0.9rem;
            font-weight: 600;
            font-family: 'Fira Code', monospace;
            backdrop-filter: blur(10px);
            z-index: 10;
        }
        
        .hint {
            position: absolute;
            bottom: 16px;
            left: 16px;
            padding: 8px 12px;
            background: rgba(36, 36, 36, 0.9);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 6px;
            color: #cccccc;
            font-size: 0.85rem;
            backdrop-filter: blur(10px);
            z-index: 10;
        }
        "#
    );

    let transform_style = format!(
        "transform: scale({}) translate({}px, {}px);",
        view_state.zoom,
        view_state.pan_x / view_state.zoom,
        view_state.pan_y / view_state.zoom
    );

    let zoom_percentage = if view_state.initial_zoom > 0.0 {
        ((view_state.zoom / view_state.initial_zoom) * 100.0) as i32
    } else {
        100
    };

    html! {
        <div class={container_style} ref={container_ref.clone()}>
            {match &props.loading_state {
                LoadingState::Loading => html! { <LoadingIndicator /> },
                LoadingState::Loaded(src) => html! {
                    <>
                        <div class="controls">
                            <button
                                class="control-btn"
                                onclick={on_zoom_in}
                                title="Zoom avant"
                            >
                                {"➕"}
                            </button>
                            <button
                                class="control-btn"
                                onclick={on_zoom_out}
                                title="Zoom arrière"
                            >
                                {"➖"}
                            </button>
                        </div>

                        <div class="zoom-indicator">
                            {format!("{}%", zoom_percentage)}
                        </div>

                        <div class="hint">
                            {"🖱️ Molette: Zoom | Glisser: Déplacer | Double-clic: Réinitialiser"}
                        </div>

                        <div
                            class="image-wrapper"
                            onwheel={on_wheel}
                            onmousedown={on_mouse_down}
                            onmousemove={on_mouse_move}
                            onmouseup={on_mouse_up}
                            onmouseleave={on_mouse_leave}
                            ondblclick={on_double_click}
                        >
                            <img
                                ref={image_ref}
                                src={(**src).clone()}
                                alt={format!("Carte de {}", props.project_name)}
                                style={transform_style}
                            />
                        </div>
                    </>
                },
                LoadingState::Error => html! { <ErrorIndicator /> },
            }}
        </div>
    }
}

#[styled_component(LoadingIndicator)]
fn loading_indicator() -> Html {
    let style = css!(
        r#"
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 20px;
        color: #cccccc;
        
        .spinner {
            width: 60px;
            height: 60px;
            border: 4px solid rgba(255, 255, 255, 0.1);
            border-top-color: #ff4141;
            border-radius: 50%;
            animation: spin 1s linear infinite;
        }
        
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        
        p {
            font-size: 1.1rem;
            font-weight: 500;
        }
        "#
    );

    html! {
        <div class={style}>
            <div class="spinner"></div>
            <p>{"Chargement de l'image..."}</p>
        </div>
    }
}

#[styled_component(ErrorIndicator)]
fn error_indicator() -> Html {
    let style = css!(
        r#"
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 16px;
        color: #e74c3c;
        padding: 32px;
        text-align: center;
        
        .error-icon {
            font-size: 4rem;
        }
        
        h4 {
            font-size: 1.3rem;
            font-weight: 600;
            margin: 0;
        }
        
        p {
            color: #999999;
            font-size: 0.95rem;
        }
        "#
    );

    html! {
        <div class={style}>
            <div class="error-icon">{"⚠️"}</div>
            <h4>{"Erreur de chargement"}</h4>
            <p>{"Impossible de charger l'image du projet"}</p>
        </div>
    }
}

async fn export_project(project_name: &str) {
    #[derive(Serialize)]
    struct ExportArgs {
        project_name: String,
    }

    let args = ExportArgs {
        project_name: project_name.to_string(),
    };

    if let Some(result) = invoke("export", serde_wasm_bindgen::to_value(&args).unwrap())
        .await
        .as_string()
    {
        let message = match result.as_str() {
            "success" => "✅ Exportation réussie !",
            _ => "❌ Erreur lors de l'exportation",
        };

        if let Some(window) = web_sys::window() {
            let _ = window.alert_with_message(message);
        }
    }
}
