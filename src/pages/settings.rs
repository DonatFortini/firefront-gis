use crate::api::{invoke, invoke_without_args, open};
use gloo_utils::format::JsValueSerdeExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::rc::Rc;
use stylist::yew::styled_component;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::prelude::*;

#[derive(Serialize, Deserialize)]
struct DialogOptions {
    directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_path: Option<String>,
    title: String,
}

#[derive(Clone, PartialEq)]
enum StatusMessage {
    Success(Rc<String>),
    Error(Rc<String>),
    Info(Rc<String>),
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
struct CacheFileInfo {
    name: String,
    size: u64,
    path: String,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
struct CacheInfo {
    total_size: u64,
    file_count: usize,
    files: Vec<CacheFileInfo>,
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[styled_component(Settings)]
pub fn settings() -> Html {
    let output_location = use_state(String::new);
    let settings_loaded = use_state(|| false);
    let status_message = use_state(|| Option::<StatusMessage>::None);
    let is_checking_updates = use_state(|| false);
    let cache_info = use_state(|| Option::<CacheInfo>::None);
    let is_loading_cache = use_state(|| true);

    let load_cache_info = {
        let cache_info = cache_info.clone();
        let is_loading_cache = is_loading_cache.clone();

        Callback::from(move |_| {
            let cache_info = cache_info.clone();
            let is_loading_cache = is_loading_cache.clone();

            is_loading_cache.set(true);

            spawn_local(async move {
                match invoke_without_args("get_cache_info")
                    .await
                    .into_serde::<CacheInfo>()
                {
                    Ok(info) => {
                        cache_info.set(Some(info));
                    }
                    Err(e) => {
                        web_sys::console::error_1(
                            &format!("Failed to load cache info: {:?}", e).into(),
                        );
                    }
                }
                is_loading_cache.set(false);
            });
        })
    };

    {
        let load_cache_info = load_cache_info.clone();
        use_effect_with((), move |_| {
            load_cache_info.emit(());
            || ()
        });
    }

    {
        let output_location = output_location.clone();
        let settings_loaded = settings_loaded.clone();
        let status_message = status_message.clone();

        use_effect_with((), move |_| {
            let status_message = status_message.clone();
            spawn_local(async move {
                if !*settings_loaded {
                    let result = invoke_without_args("get_settings").await;
                    match result.into_serde::<serde_json::Value>() {
                        Ok(settings) => {
                            if let Some(output) =
                                settings.get("output_location").and_then(|v| v.as_str())
                            {
                                output_location.set(output.to_string());
                            }
                            settings_loaded.set(true);
                        }
                        Err(e) => {
                            web_sys::console::error_1(
                                &format!("Failed to parse settings: {e:?}").into(),
                            );
                            status_message.set(Some(StatusMessage::Error(Rc::new(format!(
                                "Erreur lors du chargement des paramètres ❌: {e:?}"
                            )))));
                        }
                    }
                }
            });
            || ()
        });
    }

    let on_browse_output = {
        let output_location = output_location.clone();
        Callback::from(move |_event: MouseEvent| {
            let output_location = output_location.clone();
            let default_path = if output_location.is_empty() {
                Some("/Downloads".to_string())
            } else {
                Some(output_location.to_string())
            };

            spawn_local(async move {
                let options = DialogOptions {
                    directory: true,
                    default_path,
                    title: String::from("Sélectionner un dossier de sortie"),
                };

                if let Ok(args) = serde_wasm_bindgen::to_value(&options)
                    && let Some(selected_path) = open(args).await.as_string()
                {
                    output_location.set(selected_path);
                }
            });
        })
    };

    let on_clear_cache = {
        let status_message = status_message.clone();
        let load_cache_info = load_cache_info.clone();

        Callback::from(move |_event: MouseEvent| {
            let status_message = status_message.clone();
            let load_cache_info = load_cache_info.clone();

            spawn_local(async move {
                let _ = invoke_without_args("clear_cache").await;
                status_message.set(Some(StatusMessage::Success(Rc::new(
                    "Cache vidé avec succès ✅".to_string(),
                ))));

                load_cache_info.emit(());

                if let Some(window) = window() {
                    let status_clone = status_message.clone();
                    let closure = Closure::once(move || {
                        status_clone.set(None);
                    });
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        closure.as_ref().unchecked_ref(),
                        3000,
                    );
                    closure.forget();
                }
            });
        })
    };

    let on_check_updates = {
        let status_message = status_message.clone();
        let is_checking_updates = is_checking_updates.clone();

        Callback::from(move |_event: MouseEvent| {
            if *is_checking_updates {
                return;
            }

            let status_message = status_message.clone();
            let is_checking_updates = is_checking_updates.clone();

            is_checking_updates.set(true);
            status_message.set(Some(StatusMessage::Info(Rc::new(
                "Recherche de mises à jour en cours... ⏳".to_string(),
            ))));

            spawn_local(async move {
                let result = invoke_without_args("check_for_updates_manual").await;

                match result.as_string() {
                    Some(msg) => {
                        if msg.contains("Aucune mise à jour") {
                            status_message.set(Some(StatusMessage::Info(Rc::new(msg))));
                        } else {
                            status_message.set(Some(StatusMessage::Success(Rc::new(msg))));
                        }
                    }
                    None => {
                        status_message.set(Some(StatusMessage::Error(Rc::new(
                            "Erreur lors de la vérification des mises à jour ❌".to_string(),
                        ))));
                    }
                }

                is_checking_updates.set(false);

                if let Some(window) = window() {
                    let status_clone = status_message.clone();
                    let closure = Closure::once(move || {
                        status_clone.set(None);
                    });
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        closure.as_ref().unchecked_ref(),
                        5000,
                    );
                    closure.forget();
                }
            });
        })
    };

    let on_submit = {
        let output_location = output_location.clone();
        let status_message = status_message.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let output_location = output_location.clone();
            let status_message = status_message.clone();

            spawn_local(async move {
                let mut map = HashMap::new();
                map.insert("output_location", Some((*output_location).clone()));

                let args = serde_wasm_bindgen::to_value(&map).unwrap();
                let _ = invoke("save_settings", args).await;

                status_message.set(Some(StatusMessage::Success(Rc::new(
                    "Paramètres sauvegardés avec succès ✅".to_string(),
                ))));

                if let Some(window) = window() {
                    let status_clone = status_message.clone();
                    let closure = Closure::once(move || {
                        status_clone.set(None);
                    });
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        closure.as_ref().unchecked_ref(),
                        3000,
                    );
                    closure.forget();
                }
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
        
        h2 {
            color: #ffffff;
            font-weight: 600;
            font-size: 1.25rem;
            margin: 0;
            display: flex;
            align-items: center;
            gap: 12px;
        }
        
        @media (max-width: 768px) {
            left: 70px;
        }
        "#
    );

    let container_style = css!(
        r#"
        min-height: calc(100vh - 58px);
        padding: 24px;
        
        .content {
            max-width: 1000px;
            margin: 0 auto;
        }
        "#
    );

    html! {
        <>
            <div class={header_style}>
                <h2>
                    <span>{"⚙️"}</span>
                    {"Paramètres"}
                </h2>
            </div>

            <div class={container_style}>
                <div class="content">

                    {if let Some(msg) = &*status_message {
                        html! { <StatusAlert message={msg.clone()} /> }
                    } else {
                        html! {}
                    }}

                    <SettingsForm
                        output_location={(*output_location).clone()}
                        on_browse={on_browse_output}
                        on_check_updates={on_check_updates}
                        on_submit={on_submit}
                        is_checking_updates={*is_checking_updates}
                    />

                    <CacheInfoCard
                        cache_info={(*cache_info).clone()}
                        is_loading={*is_loading_cache}
                        on_clear_cache={on_clear_cache}
                        on_refresh={load_cache_info}
                    />
                </div>
            </div>
        </>
    }
}

#[derive(Properties, PartialEq)]
struct StatusAlertProps {
    message: StatusMessage,
}

#[styled_component(StatusAlert)]
fn status_alert(props: &StatusAlertProps) -> Html {
    let (bg_color, border_color, text, icon) = match &props.message {
        StatusMessage::Success(msg) => ("rgba(46, 204, 113, 0.1)", "#2ecc71", msg, "✅"),
        StatusMessage::Error(msg) => ("rgba(231, 76, 60, 0.1)", "#e74c3c", msg, "❌"),
        StatusMessage::Info(msg) => ("rgba(52, 152, 219, 0.1)", "#3498db", msg, "ℹ️"),
    };

    let style = format!(
        r#"
        margin-bottom: 24px;
        padding: 16px 20px;
        background-color: {};
        border: 1px solid {};
        border-radius: 8px;
        display: flex;
        align-items: center;
        gap: 12px;
        animation: slideIn 0.3s ease-out;
        
        @keyframes slideIn {{
            from {{
                opacity: 0;
                transform: translateY(-10px);
            }}
            to {{
                opacity: 1;
                transform: translateY(0);
            }}
        }}
        
        .icon {{
            font-size: 1.5rem;
        }}
        
        .message {{
            color: #ffffff;
            font-size: 1rem;
            font-weight: 500;
        }}
        "#,
        bg_color, border_color
    );

    html! {
        <div style={style}>
            <span class="icon">{icon}</span>
            <span class="message">{&**text}</span>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct SettingsFormProps {
    output_location: String,
    on_browse: Callback<MouseEvent>,
    on_check_updates: Callback<MouseEvent>,
    on_submit: Callback<SubmitEvent>,
    is_checking_updates: bool,
}

#[styled_component(SettingsForm)]
fn settings_form(props: &SettingsFormProps) -> Html {
    let style = css!(
        r#"
        background: linear-gradient(135deg, #242424 0%, #1c1c1c 100%);
        padding: 32px;
        border-radius: 12px;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.1);
        margin-bottom: 24px;
        
        h3 {
            color: #ffffff;
            font-size: 1.2rem;
            font-weight: 600;
            margin: 0 0 24px 0;
            display: flex;
            align-items: center;
            gap: 10px;
        }
        
        .form-group {
            margin-bottom: 28px;
        }
        
        label {
            display: block;
            margin-bottom: 12px;
            font-weight: 600;
            color: #ffffff;
            font-size: 1rem;
            display: flex;
            align-items: center;
            gap: 8px;
        }
        
        .input-group {
            display: flex;
            gap: 12px;
        }
        
        input {
            flex: 1;
            padding: 14px 16px;
            background-color: #1c1c1c;
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 8px;
            color: #ffffff;
            font-size: 0.95rem;
            font-family: 'Fira Code', monospace;
            transition: all 0.2s;
        }
        
        input:focus {
            outline: none;
            border-color: #ff4141;
            box-shadow: 0 0 0 3px rgba(255, 65, 65, 0.1);
        }
        
        button {
            padding: 14px 24px;
            border: none;
            border-radius: 8px;
            font-size: 1rem;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 8px;
        }
        
        button:hover:not(:disabled) {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }
        
        button:disabled {
            opacity: 0.6;
            cursor: not-allowed;
            transform: none;
        }
        
        .browse-btn {
            background: linear-gradient(135deg, #3498db 0%, #2980b9 100%);
            color: white;
            white-space: nowrap;
        }
        
        .browse-btn:hover {
            background: linear-gradient(135deg, #5dade2 0%, #3498db 100%);
        }
        
        .actions {
            display: flex;
            gap: 16px;
            margin-top: 32px;
        }
        
        .primary-action {
            flex: 1;
        }
        
        .save-btn {
            background: linear-gradient(135deg, #ff4141 0%, #ff6b6b 100%);
            color: white;
            width: 100%;
        }
        
        .save-btn:hover {
            background: linear-gradient(135deg, #ff5757 0%, #ff7f7f 100%);
        }
        
        .check-updates-btn {
            background: linear-gradient(135deg, #9b59b6 0%, #8e44ad 100%);
            color: white;
        }
        
        .check-updates-btn:hover:not(:disabled) {
            background: linear-gradient(135deg, #af7ac5 0%, #9b59b6 100%);
        }
        
        .spinner {
            width: 16px;
            height: 16px;
            border: 3px solid rgba(255, 255, 255, 0.3);
            border-top-color: white;
            border-radius: 50%;
            animation: spin 1s linear infinite;
        }
        
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        
        @media (max-width: 768px) {
            .input-group {
                flex-direction: column;
            }
            
            .actions {
                flex-direction: column;
            }
            
            .browse-btn {
                width: 100%;
            }
        }
        "#
    );

    html! {
        <form class={style} onsubmit={props.on_submit.clone()}>
            <h3>
                <span>{"📁"}</span>
                {"Configuration générale"}
            </h3>

            <div class="form-group">
                <label>
                    <span>{"📂"}</span>
                    {"Emplacement de sortie"}
                </label>
                <div class="input-group">
                    <input
                        type="text"
                        value={props.output_location.clone()}
                        readonly=true
                        placeholder="Aucun dossier sélectionné"
                    />
                    <button type="button" class="browse-btn" onclick={props.on_browse.clone()}>
                        <span>{"📂"}</span>
                        {"Parcourir"}
                    </button>
                </div>
            </div>

            <div class="actions">
                <div class="primary-action">
                    <button type="submit" class="save-btn">
                        <span>{"💾"}</span>
                        {"Sauvegarder les paramètres"}
                    </button>
                </div>

                <button
                    type="button"
                    class="check-updates-btn"
                    onclick={props.on_check_updates.clone()}
                    disabled={props.is_checking_updates}
                >
                    {if props.is_checking_updates {
                        html! { <div class="spinner"></div> }
                    } else {
                        html! { <span>{"🔄"}</span> }
                    }}
                    {"Vérifier les mises à jour"}
                </button>
            </div>
        </form>
    }
}

#[derive(Properties, PartialEq)]
struct CacheInfoCardProps {
    cache_info: Option<CacheInfo>,
    is_loading: bool,
    on_clear_cache: Callback<MouseEvent>,
    on_refresh: Callback<()>,
}

#[styled_component(CacheInfoCard)]
fn cache_info_card(props: &CacheInfoCardProps) -> Html {
    let style = css!(
        r#"
        background: linear-gradient(135deg, #242424 0%, #1c1c1c 100%);
        padding: 32px;
        border-radius: 12px;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.1);
        
        h3 {
            color: #ffffff;
            font-size: 1.2rem;
            font-weight: 600;
            margin: 0 0 24px 0;
            display: flex;
            align-items: center;
            gap: 10px;
        }
        
        .cache-summary {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 20px;
            background: rgba(255, 255, 255, 0.03);
            border-radius: 8px;
            margin-bottom: 24px;
            border: 1px solid rgba(255, 255, 255, 0.05);
        }
        
        .summary-item {
            display: flex;
            flex-direction: column;
            gap: 8px;
        }
        
        .summary-label {
            color: #999999;
            font-size: 0.85rem;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }
        
        .summary-value {
            color: #ffffff;
            font-size: 1.5rem;
            font-weight: 700;
            font-family: 'Fira Code', monospace;
        }
        
        .summary-value.size {
            color: #ff4141;
        }
        
        .summary-value.count {
            color: #3498db;
        }
        
        .files-list {
            max-height: 400px;
            overflow-y: auto;
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 8px;
            background: rgba(0, 0, 0, 0.2);
            margin-bottom: 20px;
        }
        
        .file-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 12px 16px;
            border-bottom: 1px solid rgba(255, 255, 255, 0.05);
            transition: background 0.15s;
        }
        
        .file-item:last-child {
            border-bottom: none;
        }
        
        .file-item:hover {
            background: rgba(255, 255, 255, 0.03);
        }
        
        .file-info {
            flex: 1;
            display: flex;
            flex-direction: column;
            gap: 4px;
            overflow: hidden;
        }
        
        .file-name {
            color: #ffffff;
            font-weight: 500;
            font-size: 0.95rem;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }
        
        .file-path {
            color: #666666;
            font-size: 0.8rem;
            font-family: 'Fira Code', monospace;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }
        
        .file-size {
            color: #3498db;
            font-weight: 600;
            font-family: 'Fira Code', monospace;
            font-size: 0.9rem;
            white-space: nowrap;
            margin-left: 16px;
        }
        
        .actions {
            display: flex;
            gap: 12px;
        }
        
        button {
            padding: 14px 24px;
            border: none;
            border-radius: 8px;
            font-size: 1rem;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 8px;
            flex: 1;
        }
        
        button:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }
        
        .clear-cache-btn {
            background: linear-gradient(135deg, #f39c12 0%, #e67e22 100%);
            color: white;
        }
        
        .clear-cache-btn:hover {
            background: linear-gradient(135deg, #f5ab35 0%, #f39c12 100%);
        }
        
        .refresh-btn {
            background-color: #2a2a2a;
            color: #cccccc;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }
        
        .refresh-btn:hover {
            background: linear-gradient(135deg, #3498db 0%, #2980b9 100%);
            color: white;
            border-color: transparent;
        }
        
        .loading-state {
            text-align: center;
            padding: 40px 20px;
            color: #999999;
        }
        
        .loading-spinner {
            width: 40px;
            height: 40px;
            border: 4px solid rgba(255, 255, 255, 0.1);
            border-top-color: #ff4141;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin: 0 auto 16px;
        }
        
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        
        .empty-state {
            text-align: center;
            padding: 40px 20px;
            color: #999999;
        }
        
        .empty-icon {
            font-size: 3rem;
            margin-bottom: 16px;
        }
        
        @media (max-width: 768px) {
            .cache-summary {
                flex-direction: column;
                gap: 16px;
                align-items: stretch;
            }
            
            .summary-item {
                text-align: center;
            }
            
            .actions {
                flex-direction: column;
            }
            
            .file-item {
                flex-direction: column;
                align-items: flex-start;
                gap: 8px;
            }
            
            .file-size {
                margin-left: 0;
            }
        }
        "#
    );

    html! {
        <div class={style}>
            <h3>
                <span>{"💾"}</span>
                {"Gestion du cache"}
            </h3>

            {if props.is_loading {
                html! {
                    <div class="loading-state">
                        <div class="loading-spinner"></div>
                        <p>{"Chargement des informations du cache..."}</p>
                    </div>
                }
            } else if let Some(cache_info) = &props.cache_info {
                html! {
                    <>
                        <div class="cache-summary">
                            <div class="summary-item">
                                <span class="summary-label">{"Taille totale"}</span>
                                <span class="summary-value size">{format_size(cache_info.total_size)}</span>
                            </div>
                            <div class="summary-item">
                                <span class="summary-label">{"Nombre de fichiers"}</span>
                                <span class="summary-value count">{cache_info.file_count}</span>
                            </div>
                        </div>

                        {if cache_info.files.is_empty() {
                            html! {
                                <div class="empty-state">
                                    <div class="empty-icon">{"🗑️"}</div>
                                    <p>{"Le cache est vide"}</p>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="files-list">
                                    {cache_info.files.iter().map(|file| {
                                        html! {
                                            <div class="file-item">
                                                <div class="file-info">
                                                    <div class="file-name" title={file.name.clone()}>
                                                        {"📄 "}{&file.name}
                                                    </div>
                                                    <div class="file-path" title={file.path.clone()}>
                                                        {&file.path}
                                                    </div>
                                                </div>
                                                <div class="file-size">
                                                    {format_size(file.size)}
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Html>()}
                                </div>
                            }
                        }}

                        <div class="actions">
                            <button class="refresh-btn" onclick={props.on_refresh.reform(|_| ())}>
                                <span>{"🔄"}</span>
                                {"Actualiser"}
                            </button>
                            <button class="clear-cache-btn" onclick={props.on_clear_cache.clone()}>
                                <span>{"🗑️"}</span>
                                {"Vider le cache"}
                            </button>
                        </div>
                    </>
                }
            } else {
                html! {
                    <div class="empty-state">
                        <div class="empty-icon">{"⚠️"}</div>
                        <p>{"Impossible de charger les informations du cache"}</p>
                    </div>
                }
            }}
        </div>
    }
}
