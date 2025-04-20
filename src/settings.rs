use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_without_args(cmd: &str) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "dialog"])]
    async fn open(args: JsValue) -> JsValue;
}

#[function_component(Settings)]
pub fn settings() -> Html {
    let os = use_state(|| String::from("Inconnu"));
    let cache_location = use_state(|| String::from("projects/cache"));
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

    let on_cache_location_change = {
        let cache_location = cache_location.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            cache_location.set(input.value());
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

    let on_browse_cache = {
        let cache_location = cache_location.clone();
        Callback::from(move |_| {
            let cache_location = cache_location.clone();
            spawn_local(async move {
                web_sys::console::log_1(&"Parcourir pour l'emplacement du cache".into());
                cache_location.set(String::from("projects/cache"));
            });
        })
    };

    let on_browse_gdal = {
        let gdal_path = gdal_path.clone();
        Callback::from(move |_| {
            let gdal_path = gdal_path.clone();
            spawn_local(async move {
                web_sys::console::log_1(&"Parcourir pour le chemin GDAL".into());
                gdal_path.set(String::from("/usr/local/bin/gdal"));
            });
        })
    };

    let on_browse_python = {
        let python_path = python_path.clone();
        Callback::from(move |_| {
            let python_path = python_path.clone();
            spawn_local(async move {
                web_sys::console::log_1(&"Parcourir pour le chemin Python".into());
                python_path.set(String::from("/usr/bin/python3"));
            });
        })
    };

    let on_submit = Callback::from(|e: SubmitEvent| {
        e.prevent_default();
        web_sys::console::log_1(&"Sauvegarde des paramètres".into());
    });

    html! {
        <div class="settings-view">
            <h2>{"Paramètres"}</h2>
            <div class="settings-info">
                <p>{format!("Système d'exploitation détecté : {}", *os)}</p>
            </div>
            <form onsubmit={on_submit}>
                <div class="form-group">
                    <label for="cache-location">{"Emplacement du cache"}</label>
                    <div class="input-with-button">
                        <input
                            type="text"
                            id="cache-location"
                            value={(*cache_location).clone()}
                            onchange={on_cache_location_change}
                        />
                        <button type="button" onclick={on_browse_cache}>{"Parcourir"}</button>
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
                <button type="submit">{"Sauvegarder les paramètres"}</button>
            </form>
        </div>
    }
}
