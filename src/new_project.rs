use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
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

#[function_component(NewProject)]
pub fn new_project() -> Html {
    let departments = use_state(|| HashMap::<String, String>::new());
    let is_loading = use_state(|| false);
    let project_name = use_state(|| String::new());
    let selected_department = use_state(|| String::new());
    let xmin = use_state(|| 0.0);
    let ymin = use_state(|| 0.0);
    let xmax = use_state(|| 0.0);
    let ymax = use_state(|| 0.0);
    let error_message = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);

    {
        let departments = departments.clone();
        use_effect(move || {
            spawn_local(async move {
                match invoke("get_dpts_list", JsValue::NULL).await.as_string() {
                    Some(deps_json) => {
                        match serde_json::from_str::<HashMap<String, String>>(&deps_json) {
                            Ok(deps) => {
                                departments.set(deps);
                            }
                            Err(e) => {
                                web_sys::console::error_1(
                                    &format!("Error parsing departments: {:?}", e).into(),
                                );
                            }
                        }
                    }
                    None => {
                        web_sys::console::error_1(&"Failed to get departments".into());
                    }
                }
            });

            || () // Cleanup function
        });
    }

    let on_submit = {
        let project_name = project_name.clone();
        let selected_department = selected_department.clone();
        let xmin = xmin.clone();
        let ymin = ymin.clone();
        let xmax = xmax.clone();
        let ymax = ymax.clone();
        let is_loading = is_loading.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            // Validation
            if project_name.is_empty() {
                error_message.set(Some("Project name is required".to_string()));
                return;
            }

            if selected_department.is_empty() {
                error_message.set(Some("Please select a department".to_string()));
                return;
            }

            // Reset messages
            error_message.set(None);
            success_message.set(None);
            is_loading.set(true);

            let args = NewProjectArgs {
                code: (*selected_department).clone(),
                name: (*project_name).clone(),
                xmin: *xmin,
                ymin: *ymin,
                xmax: *xmax,
                ymax: *ymax,
            };

            let project_name = project_name.clone();
            let is_loading = is_loading.clone();
            let error_message = error_message.clone();
            let success_message = success_message.clone();

            spawn_local(async move {
                let serialized_args = serde_wasm_bindgen::to_value(&args).unwrap();
                match invoke("open_new_project", serialized_args)
                    .await
                    .as_string()
                {
                    Some(result) => {
                        is_loading.set(false);
                        if result.contains("error") {
                            error_message.set(Some(result));
                        } else {
                            success_message.set(Some(format!(
                                "Project {} created successfully",
                                *project_name
                            )));
                            // Reset form
                            project_name.set(String::new());
                        }
                    }
                    None => {
                        is_loading.set(false);
                        error_message.set(Some("Failed to create project".to_string()));
                    }
                }
            });
        })
    };

    let on_project_name_change = {
        let project_name = project_name.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            project_name.set(input.value());
        })
    };

    let on_department_change = {
        let selected_department = selected_department.clone();
        Callback::from(move |e: Event| {
            // Use a more direct approach without relying on HtmlSelectElement
            let target = e.target().unwrap();
            let select = wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlElement>(target).unwrap();
            // Get value using JS property access
            let value = js_sys::Reflect::get(&select, &JsValue::from_str("value"))
                .unwrap()
                .as_string()
                .unwrap_or_default();
            selected_department.set(value);
        })
    };

    let on_xmin_change = {
        let xmin = xmin.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            if let Ok(value) = input.value().parse::<f64>() {
                xmin.set(value);
            }
        })
    };

    let on_ymin_change = {
        let ymin = ymin.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            if let Ok(value) = input.value().parse::<f64>() {
                ymin.set(value);
            }
        })
    };

    let on_xmax_change = {
        let xmax = xmax.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            if let Ok(value) = input.value().parse::<f64>() {
                xmax.set(value);
            }
        })
    };

    let on_ymax_change = {
        let ymax = ymax.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            if let Ok(value) = input.value().parse::<f64>() {
                ymax.set(value);
            }
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
                        <option value="">{"Select a department"}</option>
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
                    <div class="coordinates-grid">
                        <input
                            type="number"
                            placeholder="xmin"
                            value={(*xmin).to_string()}
                            onchange={on_xmin_change}
                        />
                        <input
                            type="number"
                            placeholder="ymin"
                            value={(*ymin).to_string()}
                            onchange={on_ymin_change}
                        />
                        <input
                            type="number"
                            placeholder="xmax"
                            value={(*xmax).to_string()}
                            onchange={on_xmax_change}
                        />
                        <input
                            type="number"
                            placeholder="ymax"
                            value={(*ymax).to_string()}
                            onchange={on_ymax_change}
                        />
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
