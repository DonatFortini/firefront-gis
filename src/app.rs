use wasm_bindgen::prelude::*;
use yew::prelude::*;

use crate::documentation::Documentation;
use crate::home::Home;
use crate::new_project::NewProject;
use crate::settings::Settings;
use crate::sidebar::Sidebar;
use crate::types::AppView;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[function_component(App)]
pub fn app() -> Html {
    let app_view = use_state(|| AppView::Home);

    let on_view_change = {
        let app_view = app_view.clone();
        Callback::from(move |view: AppView| {
            app_view.set(view);
        })
    };

    html! {
        <div class="app-container">
            <Sidebar current_view={(*app_view).clone()} on_view_change={on_view_change.clone()} />
            <div class="main-content">
                {
                    match *app_view {
                        AppView::Home => html! { <Home /> },
                        AppView::NewProject => html! { <NewProject /> },
                        AppView::Settings => html! { <Settings /> },
                        AppView::Documentation => html! { <Documentation /> },
                    }
                }
            </div>
        </div>
    }
}
