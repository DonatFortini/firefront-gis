use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "dialog"])]
    async fn open(args: JsValue) -> JsValue;
}

#[function_component(Settings)]
pub fn settings() -> Html {
    let os = use_state(|| String::from("Unknown"));
    let cache_location = use_state(|| String::from("projects/cache"));
    let gdal_path = use_state(|| String::from(""));
    let python_path = use_state(|| String::from(""));

    // Get OS on component mount
    {
        let os = os.clone();
        use_effect(move || {
            spawn_local(async move {
                if let Some(os_value) = invoke("get_os", JsValue::NULL).await.as_string() {
                    os.set(os_value);
                }
            });

            || () // Cleanup function
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
                // This would use Tauri's dialog API in a real implementation
                web_sys::console::log_1(&"Browsing for cache location".into());
                // Mock implementation
                cache_location.set(String::from("projects/cache"));
            });
        })
    };

    let on_browse_gdal = {
        let gdal_path = gdal_path.clone();
        Callback::from(move |_| {
            let gdal_path = gdal_path.clone();
            spawn_local(async move {
                // This would use Tauri's dialog API in a real implementation
                web_sys::console::log_1(&"Browsing for GDAL path".into());
                // Mock implementation
                gdal_path.set(String::from("/usr/local/bin/gdal"));
            });
        })
    };

    let on_browse_python = {
        let python_path = python_path.clone();
        Callback::from(move |_| {
            let python_path = python_path.clone();
            spawn_local(async move {
                // This would use Tauri's dialog API in a real implementation
                web_sys::console::log_1(&"Browsing for Python path".into());
                // Mock implementation
                python_path.set(String::from("/usr/bin/python3"));
            });
        })
    };

    let on_submit = Callback::from(|e: SubmitEvent| {
        e.prevent_default();
        // This would save settings in a real implementation
        web_sys::console::log_1(&"Saving settings".into());
    });

    html! {
        <div class="settings-view">
            <h2>{"Settings"}</h2>
            <div class="settings-info">
                <p>{format!("Detected Operating System: {}", *os)}</p>
            </div>
            <form onsubmit={on_submit}>
                <div class="form-group">
                    <label for="cache-location">{"Cache Location"}</label>
                    <div class="input-with-button">
                        <input
                            type="text"
                            id="cache-location"
                            value={(*cache_location).clone()}
                            onchange={on_cache_location_change}
                        />
                        <button type="button" onclick={on_browse_cache}>{"Browse"}</button>
                    </div>
                </div>
                <div class="form-group">
                    <label for="gdal-path">{"GDAL Installation Path"}</label>
                    <div class="input-with-button">
                        <input
                            type="text"
                            id="gdal-path"
                            placeholder="Auto-detected"
                            value={(*gdal_path).clone()}
                            onchange={on_gdal_path_change}
                        />
                        <button type="button" onclick={on_browse_gdal}>{"Browse"}</button>
                    </div>
                </div>
                <div class="form-group">
                    <label for="python-path">{"Python Installation Path"}</label>
                    <div class="input-with-button">
                        <input
                            type="text"
                            id="python-path"
                            placeholder="Auto-detected"
                            value={(*python_path).clone()}
                            onchange={on_python_path_change}
                        />
                        <button type="button" onclick={on_browse_python}>{"Browse"}</button>
                    </div>
                </div>
                <button type="submit">{"Save Settings"}</button>
            </form>
        </div>
    }
}
