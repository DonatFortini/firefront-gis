pub mod app;
pub mod documentation;
pub mod home;
pub mod loading;
pub mod new_project;
pub mod project;
pub mod settings;
pub mod sidebar;
pub mod types;

use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use wasm_bindgen_futures::spawn_local;

use crate::app::App;

fn main() {
    #[wasm_bindgen]
    unsafe extern "C" {
        #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
        async fn invoke_without_args(cmd: &str) -> JsValue;
    }

    console_error_panic_hook::set_once();
    let document = web_sys::window().unwrap().document().unwrap();
    let head = document.head().unwrap();

    spawn_local(async move {
        let _ = invoke_without_args("load_regions_graph").await;
    });

    let style = document.create_element("style").unwrap();
    style.set_inner_html(include_str!("../styles.css"));
    head.append_child(&style).unwrap();

    yew::Renderer::<App>::new().render();
}
