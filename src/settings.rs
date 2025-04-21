use directories::UserDirs;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use yew::prelude::*;
// FIXME :: fix the path cross platform issue and implement functionanlity
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_without_args(cmd: &str) -> JsValue;

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

#[function_component(Settings)]
pub fn settings() -> Html {
    let os = use_state(|| String::from("Inconnu"));

    let download_dir = match UserDirs::new() {
        Some(dirs) => dirs.download_dir().map(|p| p.to_path_buf()),
        None => xdg_user::UserDirs::new()
            .unwrap()
            .downloads()
            .map(|p| p.to_path_buf())
            .or_else(|| Some(std::path::PathBuf::from("caca"))),
    };

    let output_location = use_state(move || download_dir.clone());
    let gdal_path = use_state(|| String::from(""));
    let python_path = use_state(|| String::from(""));

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

    let on_output_location_change = {
        let output_location = output_location.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            output_location.set(Some(std::path::PathBuf::from(input.value())));
        })
    };

    let on_gdal_path_change = {
        let gdal_path = gdal_path.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            gdal_path.set(input.value());
        })
    };

    let on_python_path_change = {
        let python_path = python_path.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            python_path.set(input.value());
        })
    };

    let on_browse_output = {
        let output_location = output_location.clone();

        Callback::from(move |_| {
            let output_location = output_location.clone();
            let default_path =
                UserDirs::new().map(|dirs| dirs.home_dir().to_string_lossy().to_string());

            spawn_local(async move {
                let options = DialogOptions {
                    directory: true,
                    default_path,
                    title: String::from("Sélectionner un dossier de sortie"),
                };

                if let Ok(args) = serde_wasm_bindgen::to_value(&options) {
                    if let Some(selected_path) = open(args).await.as_string() {
                        output_location.set(Some(std::path::PathBuf::from(selected_path)));
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

    let on_browse_python = {
        let python_path = python_path.clone();
        Callback::from(move |_| {
            let python_path = python_path.clone();
            spawn_local(async move {
                let options = DialogOptions {
                    directory: false,
                    default_path: if python_path.is_empty() {
                        None
                    } else {
                        Some((*python_path).clone())
                    },
                    title: String::from("Sélectionner l'exécutable Python"),
                };

                if let Ok(args) = serde_wasm_bindgen::to_value(&options) {
                    if let Some(selected_path) = open(args).await.as_string() {
                        python_path.set(selected_path);
                    }
                }
            });
        })
    };

    let on_clear_cache = Callback::from(|_| {
        spawn_local(async {
            invoke_without_args("clear_cache").await;
        });
    });

    let on_submit = Callback::from(|e: SubmitEvent| {
        e.prevent_default();
    });

    html! {
        <div class="settings-view">
            <h2>{"Paramètres"}</h2>
            <div class="settings-info">
                <p>{format!("Système d'exploitation détecté : {}", *os)}</p>
            </div>
            <form onsubmit={on_submit}>
                <div class="form-group">
                    <label for="output-location">{"Emplacement de sortie"}</label>
                    <div class="input-with-button">
                        <input
                            type="text"
                            id="output-location"
                            value={output_location.as_ref().and_then(|path| path.to_str()).unwrap_or("").to_string()}
                            onchange={on_output_location_change}
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
                            onchange={on_gdal_path_change}
                        />
                        <button type="button" onclick={on_browse_gdal}>{"Parcourir"}</button>
                    </div>
                </div>
                <div class="form-group">
                    <label for="python-path">{"Chemin d'installation de Python"}</label>
                    <div class="input-with-button">
                        <input
                            type="text"
                            id="python-path"
                            placeholder="Détecté automatiquement"
                            value={(*python_path).clone()}
                            onchange={on_python_path_change}
                        />
                        <button type="button" onclick={on_browse_python}>{"Parcourir"}</button>
                    </div>
                </div>
                <div class="button-group">
                    <button type="submit">{"Sauvegarder les paramètres"}</button>
                    <button type="button" onclick={on_clear_cache}>{"Vider le cache"}</button>
                </div>
            </form>
        </div>
    }
}
