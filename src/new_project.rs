use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::types::AppView;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_without_args(cmd: &str) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

}

#[wasm_bindgen]
extern "C" {
    type HtmlSelectElement;

    #[wasm_bindgen(method, getter)]
    fn value(this: &HtmlSelectElement) -> String;
}

#[derive(Serialize, Deserialize)]
struct NewProjectArgs {
    code: String,
    name: String,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
}

#[derive(Properties, PartialEq)]
pub struct NewProjectProps {
    pub on_view_change: Callback<AppView>,
}

#[function_component(NewProject)]
pub fn new_project(props: &NewProjectProps) -> Html {
    let departments = use_state(HashMap::<String, String>::new);
    let is_loading = use_state(|| false);
    let project_name = use_state(String::new);
    let selected_department = use_state(String::new);
    let xmin = use_state(|| 0.0);
    let ymin = use_state(|| 0.0);
    let xmax = use_state(|| 0.0);
    let ymax = use_state(|| 0.0);
    let error_message = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);
    let is_square = use_state(|| false);

    {
        let departments = departments.clone();
        let error_message = error_message.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                let dp = invoke_without_args("get_dpts_list").await;
                let result: Result<HashMap<String, String>, _> =
                    serde_wasm_bindgen::from_value(dp.clone());

                match result {
                    Ok(depts) => departments.set(depts),
                    Err(err) => {
                        error_message.set(Some(format!("Failed to fetch departments: {}", err)))
                    }
                }
            });

            || () // Cleanup function
        });
    }

    let on_project_name_change = {
        let project_name = project_name.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            project_name.set(input.value());
        })
    };

    let on_ymax_change = {
        let ymax = ymax.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                if let Ok(value) = input.value().parse::<f64>() {
                    ymax.set(value);
                }
            }
        })
    };

    let on_xmax_change = {
        let xmax = xmax.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                if let Ok(value) = input.value().parse::<f64>() {
                    xmax.set(value);
                }
            }
        })
    };

    let on_xmin_change = {
        let xmin = xmin.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                if let Ok(value) = input.value().parse::<f64>() {
                    xmin.set(value);
                }
            }
        })
    };

    let on_ymin_change = {
        let ymin = ymin.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                if let Ok(value) = input.value().parse::<f64>() {
                    ymin.set(value);
                }
            }
        })
    };

    let on_department_change = {
        let selected_department = selected_department.clone();
        Callback::from(move |e: Event| {
            let target = e.target().unwrap();
            let select = wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlElement>(target).unwrap();
            let value = js_sys::Reflect::get(&select, &JsValue::from_str("value"))
                .unwrap()
                .as_string()
                .unwrap_or_default();
            selected_department.set(value);
        })
    };

    let on_submit = {
        let project_name = project_name.clone();
        let selected_department = selected_department.clone();
        let xmin = xmin.clone();
        let ymin = ymin.clone();
        let xmax = xmax.clone();
        let ymax = ymax.clone();
        let is_loading = is_loading.clone();
        let error_message = error_message.clone();
        let on_view_change = props.on_view_change.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            if project_name.is_empty() {
                error_message.set(Some("Project name is required".to_string()));
                return;
            }

            if selected_department.is_empty() {
                error_message.set(Some("Please select a department".to_string()));
                return;
            }

            if *xmin == 0.0 || *ymin == 0.0 || *xmax == 0.0 || *ymax == 0.0 {
                error_message.set(Some("All coordinates must be non-zero.".to_string()));
                return;
            }

            let width = ((*xmax as f64) - (*xmin as f64)).abs();
            let height = ((*ymax as f64) - (*ymin as f64)).abs();
            if (width - height).abs() >= 0.001 {
                error_message.set(Some("The coordinate box must be a square.".to_string()));
                return;
            }

            error_message.set(None);
            is_loading.set(true);

            let args = NewProjectArgs {
                code: (*selected_department).clone(),
                name: (*project_name).clone(),
                xmin: *xmin,
                ymin: *ymin,
                xmax: *xmax,
                ymax: *ymax,
            };

            let project_name_clone = (*project_name).clone();
            let on_view_change = on_view_change.clone();

            on_view_change.emit(AppView::Loading(project_name_clone.clone()));

            spawn_local(async move {
                let serialized_args = serde_wasm_bindgen::to_value(&args).unwrap();
                let _ = invoke("open_new_project", serialized_args).await;
            });
        })
    };

    html! {
        <div class="new-project-view">
            <h2>{"Create New Project"}</h2>

            if let Some(error) = &*error_message {
                <div class="error-message">{error}</div>
            }

            if let Some(success) = &*success_message {
                <div class="success-message">{success}</div>
            }

            <form onsubmit={on_submit}>
                <div class="form-group">
                    <label for="department">{"Department"}</label>
                    <select id="department" value={(*selected_department).clone()} onchange={on_department_change}>
                    if departments.is_empty() {
                        <option value="">{"Chargement..."}</option>
                    } else {
                        <option value="">{"-- Choisir un département --"}</option>
                    }
                    {
                        departments.iter().map(|(code, name)| {
                            html! {
                                <option value={code.clone()}>{format!("{} - {}", code, name)}</option>
                            }
                        }).collect::<Html>()
                    }
                    </select>
                </div>
                <div class="form-group">
                    <label for="project-name">{"Project Name"}</label>
                    <input
                        type="text"
                        id="project-name"
                        value={(*project_name).clone()}
                        onchange={on_project_name_change}
                        placeholder="Enter project name"
                    />
                </div>
                <div class="form-group">
                    <label>{"Coordinates"}</label>
                    <div class="coordinates-cross">
                        <div class="coord-row">
                            <div></div>
                            <div>
                                <label for="ymax">{"Y-Max"}</label>
                                <input
                                    id="ymax"
                                    type="number"
                                    placeholder="ymax"
                                    value={(*ymax).to_string()}
                                    onchange={on_ymax_change}
                                />
                            </div>
                            <div></div>
                        </div>
                        <div class="coord-row">
                            <div>
                                <label for="xmin">{"X-Min"}</label>
                                <input
                                    id="xmin"
                                    type="number"
                                    placeholder="xmin"
                                    value={(*xmin).to_string()}
                                    onchange={on_xmin_change}
                                />
                            </div>
                            <div class="square-indicator">
                                {
                                    if *is_square {
                                        html! { <span class="square-yes">{"Carré ✓"}</span> }
                                    } else {
                                        html! { <span class="square-no">{"Rectangle ⚠"}</span> }
                                    }
                                }
                            </div>
                            <div>
                                <label for="xmax">{"X-Max"}</label>
                                <input
                                    id="xmax"
                                    type="number"
                                    placeholder="xmax"
                                    value={(*xmax).to_string()}
                                    onchange={on_xmax_change}
                                />
                            </div>
                        </div>
                        <div class="coord-row">
                            <div></div>
                            <div>
                                <label for="ymin">{"Y-Min"}</label>
                                <input
                                    id="ymin"
                                    type="number"
                                    placeholder="ymin"
                                    value={(*ymin).to_string()}
                                    onchange={on_ymin_change}
                                />
                            </div>
                            <div></div>
                        </div>
                    </div>
                </div>
                <button type="submit" disabled={*is_loading}>
                    if *is_loading {
                        {"Creating Project..."}
                    } else {
                        {"Create Project"}
                    }
                </button>
            </form>
        </div>
    }
}
