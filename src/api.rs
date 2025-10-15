use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    pub async fn invoke_without_args(cmd: &str) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    pub async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    pub fn convertFileSrc(filePath: &str, protocol: Option<&str>) -> String;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "dialog"])]
    pub async fn open(args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], catch)]
    pub async fn listen(event: &str, handler: &js_sys::Function) -> Result<JsValue, JsValue>;
}

pub mod prelude {
    pub use super::{convertFileSrc, invoke, invoke_without_args};
}
