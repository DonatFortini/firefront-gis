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
}

#[styled_component(Settings)]
pub fn settings() -> Html {
    let os = use_state(|| Rc::new(String::from("Détection...")));
    let output_location = use_state(String::new);
    let settings_loaded = use_state(|| false);
    let status_message = use_state(|| Option::<StatusMessage>::None);

    {
        let os = os.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Some(os_value) = invoke_without_args("get_os").await.as_string() {
                    os.set(Rc::new(os_value));
                }
            });
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
        Callback::from(move |_event: MouseEvent| {
            let status_message = status_message.clone();

            spawn_local(async move {
                let _ = invoke_without_args("clear_cache").await;
                status_message.set(Some(StatusMessage::Success(Rc::new(
                    "Cache vidé avec succès ✅".to_string(),
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
            max-width: 800px;
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
                    <SystemInfo os={(**os).clone()} />

                    {if let Some(msg) = &*status_message {
                        html! { <StatusAlert message={msg.clone()} /> }
                    } else {
                        html! {}
                    }}

                    <SettingsForm
                        output_location={(*output_location).clone()}
                        on_browse={on_browse_output}
                        on_clear_cache={on_clear_cache}
                        on_submit={on_submit}
                    />
                </div>
            </div>
        </>
    }
}

#[derive(Properties, PartialEq)]
struct SystemInfoProps {
    os: String,
}

#[styled_component(SystemInfo)]
fn system_info(props: &SystemInfoProps) -> Html {
    let style = css!(
        r#"
        margin-bottom: 24px;
        padding: 20px 24px;
        background: linear-gradient(135deg, #242424 0%, #1c1c1c 100%);
        border-radius: 12px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        
        .info-row {
            display: flex;
            align-items: center;
            gap: 12px;
            color: #cccccc;
            font-size: 1rem;
        }
        
        .label {
            font-weight: 500;
        }
        
        .value {
            color: #ff4141;
            font-weight: 600;
            font-family: 'Fira Code', monospace;
        }
        
        .icon {
            font-size: 1.5rem;
        }
        "#
    );

    html! {
        <div class={style}>
            <div class="info-row">
                <span class="icon">{"💻"}</span>
                <span class="label">{"Système d'exploitation détecté :"}</span>
                <span class="value">{&props.os}</span>
            </div>
        </div>
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
    on_clear_cache: Callback<MouseEvent>,
    on_submit: Callback<SubmitEvent>,
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
        
        button:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
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
        
        .clear-cache-btn {
            background-color: #2a2a2a;
            color: #cccccc;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }
        
        .clear-cache-btn:hover {
            background: linear-gradient(135deg, #f39c12 0%, #e67e22 100%);
            color: white;
            border-color: transparent;
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
            <div class="form-group">
                <label>
                    <span>{"📁"}</span>
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

                <button type="button" class="clear-cache-btn" onclick={props.on_clear_cache.clone()}>
                    <span>{"🗑️"}</span>
                    {"Vider le cache"}
                </button>
            </div>
        </form>
    }
}
