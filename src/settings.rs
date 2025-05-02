use gloo_utils::format::JsValueSerdeExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{console, window};
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_without_args(cmd: &str) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_with_args(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "dialog"])]
    async fn open(args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize)]
struct DialogOptions {
    directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_path: Option<String>,
    title: String,
}

#[function_component(SettingsComponent)]
pub fn settings_component() -> Html {
    let os = use_state(|| String::from("Inconnu"));
    let output_location = use_state(String::new);
    let gdal_path = use_state(String::new);
    let app_settings_loaded = use_state(|| false);
    let status_message = use_state(|| Option::<(String, bool)>::None);

    {
        let os = os.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Some(os_value) = invoke_without_args("get_os").await.as_string() {
                    os.set(os_value);
                }
            });
            || ()
        });
    }

    {
        let output_location = output_location.clone();
        let gdal_path = gdal_path.clone();
        let settings_loaded = app_settings_loaded.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                if !*settings_loaded {
                    let result = invoke_without_args("get_settings").await;

                    console::log_1(&format!("Settings response: {:?}", result.as_string()).into());

                    match result.into_serde::<serde_json::Value>() {
                        Ok(settings) => {
                            console::log_1(&format!("Parsed settings: {:?}", settings).into());

                            if let Some(output) =
                                settings.get("output_location").and_then(|v| v.as_str())
                            {
                                output_location.set(output.to_string());
                            }

                            if let Some(gdal) = settings.get("gdal_path") {
                                if !gdal.is_null() {
                                    if let Some(path) = gdal.as_str() {
                                        gdal_path.set(path.to_string());
                                    }
                                }
                            }

                            settings_loaded.set(true);
                        }
                        Err(e) => web_sys::console::error_1(
                            &format!("Failed to parse settings: {:?}", e).into(),
                        ),
                    }
                }
            });
            || ()
        });
    }

    let on_browse_output = {
        let output_location = output_location.clone();

        Callback::from(move |_| {
            let output_location = output_location.clone();
            let default_path = if output_location.is_empty() {
                Some("/Downloads".to_string())
            } else {
                Some((*output_location).clone())
            };

            spawn_local(async move {
                let options = DialogOptions {
                    directory: true,
                    default_path,
                    title: String::from("Sélectionner un dossier de sortie"),
                };

                if let Ok(args) = serde_wasm_bindgen::to_value(&options) {
                    if let Some(selected_path) = open(args).await.as_string() {
                        output_location.set(selected_path);
                    }
                }
            });
        })
    };

    let on_browse_gdal = {
        let gdal_path = gdal_path.clone();
        Callback::from(move |_| {
            let gdal_path = gdal_path.clone();
            spawn_local(async move {
                let options = DialogOptions {
                    directory: false,
                    default_path: if gdal_path.is_empty() {
                        None
                    } else {
                        Some((*gdal_path).clone())
                    },
                    title: String::from("Sélectionner l'exécutable GDAL"),
                };

                if let Ok(args) = serde_wasm_bindgen::to_value(&options) {
                    if let Some(selected_path) = open(args).await.as_string() {
                        gdal_path.set(selected_path);
                    }
                }
            });
        })
    };

    let on_clear_cache = {
        let status_message = status_message.clone();

        Callback::from(move |_| {
            let status_message = status_message.clone();

            spawn_local(async move {
                let _ = invoke_without_args("clear_cache").await;

                status_message.set(Some(("Cache vidé avec succès".to_string(), true)));

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
        let gdal_path = gdal_path.clone();
        let status_message = status_message.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let output_location = output_location.clone();
            let gdal_path = gdal_path.clone();
            let status_message = status_message.clone();

            spawn_local(async move {
                let mut map = HashMap::new();
                map.insert("output_location", Some((*output_location).clone()));
                map.insert(
                    "gdal_path",
                    if gdal_path.is_empty() {
                        None
                    } else {
                        Some((*gdal_path).clone())
                    },
                );

                let args = serde_wasm_bindgen::to_value(&map).unwrap();

                let _ = invoke_with_args("save_settings", args).await;

                status_message.set(Some((
                    "Paramètres sauvegardés avec succès".to_string(),
                    true,
                )));

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

    html! {
        <div class="settings-view">
            <h2>{"Paramètres"}</h2>
            <div class="settings-info">
                <p>{format!("Système d'exploitation détecté : {}", *os)}</p>

                {
                    if let Some((msg, is_success)) = &*status_message {
                        html! {
                            <div class={if *is_success { "alert alert-success" } else { "alert alert-error" }}>
                                { msg }
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
            <form onsubmit={on_submit}>
                <div class="form-group">
                    <label for="output-location">{"Emplacement de sortie"}</label>
                    <div class="input-with-button">
                        <input
                            type="text"
                            id="output-location"
                            value={(*output_location).clone()}
                            readonly=true
                        />
                        <button type="button" onclick={on_browse_output}>{"Parcourir"}</button>
                    </div>
                </div>
                <div class="form-group">
                    <label for="gdal-path">{"Chemin d'installation de GDAL"}</label>
                    <div class="input-with-button">
                        <input
                            type="text"
                            id="gdal-path"
                            placeholder="Détecté automatiquement"
                            value={(*gdal_path).clone()}
                            readonly=true
                        />
                        <button type="button" onclick={on_browse_gdal}>{"Parcourir"}</button>
                    </div>
                </div>

                <div class="button-group">
                    <div class="primary-action">
                        <button type="submit" class="save-btn">{"Sauvegarder les paramètres"}</button>
                    </div>
                    <div class="secondary-action">
                        <button type="button" onclick={on_clear_cache} class="clear-cache-btn">
                            {"Vider le cache"}
                        </button>
                    </div>
                </div>
            </form>
        </div>
    }
}
