use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use crate::types::AppView;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_without_args(cmd: &str) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
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

    let xmin_str = use_state(String::new);
    let ymin_str = use_state(String::new);
    let xmax_str = use_state(String::new);
    let ymax_str = use_state(String::new);

    let validation_errors = use_state(Vec::<String>::new);

    {
        let departments = departments.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                let result = invoke_without_args("get_dpts_list").await;
                if let Ok(depts) = serde_wasm_bindgen::from_value(result) {
                    departments.set(depts);
                }
            });
            || ()
        });
    }

    fn parse_coordinate(s: &str) -> Option<f64> {
        if s.trim().is_empty() {
            None
        } else {
            s.parse::<f64>().ok()
        }
    }

    let is_valid_shape = {
        let xmin = parse_coordinate(&xmin_str);
        let ymin = parse_coordinate(&ymin_str);
        let xmax = parse_coordinate(&xmax_str);
        let ymax = parse_coordinate(&ymax_str);

        if let (Some(xmin), Some(ymin), Some(xmax), Some(ymax)) = (xmin, ymin, xmax, ymax) {
            let width = xmax - xmin;
            let height = ymax - ymin;
            if width <= 0.0 || height <= 0.0 {
                "invalid"
            } else {
                let width_is_valid = (width / 10.0) % 500.0 == 0.0;
                let height_is_valid = (height / 10.0) % 500.0 == 0.0;

                if width_is_valid && height_is_valid {
                    if width - height == 0.0 {
                        "square"
                    } else {
                        "rectangle"
                    }
                } else {
                    "invalid"
                }
            }
        } else {
            "invalid"
        }
    };

    let create_coordinate_handler = |state: UseStateHandle<String>| {
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let value = input.value();

            if value.is_empty() {
                state.set(value);
                return;
            }

            let filtered_value: String = value
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.' || (state.is_empty() && *c == '-'))
                .collect();

            if filtered_value.len() != value.len() {
                input.set_value(&filtered_value);
            }

            if filtered_value.matches('.').count() <= 1
                && filtered_value.matches('-').count() <= 1
                && !filtered_value.trim_start_matches('-').contains('-')
            {
                state.set(filtered_value);
            }
        })
    };

    let on_department_change = {
        let selected_department = selected_department.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            selected_department.set(select.value());
        })
    };

    let on_project_name_change = {
        let project_name = project_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            project_name.set(input.value());
        })
    };

    let on_xmin_input = create_coordinate_handler(xmin_str.clone());
    let on_ymin_input = create_coordinate_handler(ymin_str.clone());
    let on_xmax_input = create_coordinate_handler(xmax_str.clone());
    let on_ymax_input = create_coordinate_handler(ymax_str.clone());

    let on_submit = {
        let is_loading = is_loading.clone();
        let validation_errors = validation_errors.clone();
        let on_view_change = props.on_view_change.clone();
        let project_name = project_name.clone();
        let selected_department = selected_department.clone();
        let xmin_str = xmin_str.clone();
        let ymin_str = ymin_str.clone();
        let xmax_str = xmax_str.clone();
        let ymax_str = ymax_str.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let mut errors = Vec::new();

            if (*selected_department).is_empty() {
                errors.push("Veuillez sélectionner un département".to_string());
            }

            if (*project_name).is_empty() {
                errors.push("Le nom du projet est requis".to_string());
            }

            let xmin = parse_coordinate(&xmin_str);
            let ymin = parse_coordinate(&ymin_str);
            let xmax = parse_coordinate(&xmax_str);
            let ymax = parse_coordinate(&ymax_str);

            if xmin.is_none() || ymin.is_none() || xmax.is_none() || ymax.is_none() {
                errors.push(
                    "Tous les champs de coordonnées doivent être remplis avec des nombres valides"
                        .to_string(),
                );
            } else if let (Some(xmin), Some(ymin), Some(xmax), Some(ymax)) =
                (xmin, ymin, xmax, ymax)
            {
                if xmin == 0.0 && ymin == 0.0 && xmax == 0.0 && ymax == 0.0 {
                    errors.push(
                        "Les coordonnées ne peuvent pas toutes être égales à zéro".to_string(),
                    );
                } else {
                    let width = xmax - xmin;
                    let height = ymax - ymin;

                    if width <= 0.0 || height <= 0.0 {
                        errors.push("La zone de coordonnées doit avoir des dimensions positives (xmax > xmin, ymax > ymin)".to_string());
                    } else {
                        let width_is_valid = (width / 10.0) % 500.0 == 0.0;
                        let height_is_valid = (height / 10.0) % 500.0 == 0.0;

                        if !width_is_valid || !height_is_valid {
                            errors.push(
                                "Les dimensions doivent être des multiples de 500".to_string(),
                            );
                        }
                    }
                }
            }

            if !errors.is_empty() {
                validation_errors.set(errors.clone());
                return;
            }

            validation_errors.set(Vec::new());
            is_loading.set(true);

            let args = NewProjectArgs {
                code: (*selected_department).clone(),
                name: (*project_name).clone(),
                xmin: xmin.unwrap(),
                ymin: ymin.unwrap(),
                xmax: xmax.unwrap(),
                ymax: ymax.unwrap(),
            };

            let project_name_clone = (*project_name).clone();
            let on_view_change = on_view_change.clone();
            let is_loading = is_loading.clone();
            let validation_errors = validation_errors.clone();

            on_view_change.emit(AppView::Loading(project_name_clone.clone()));

            spawn_local(async move {
                let serialized_args = serde_wasm_bindgen::to_value(&args).unwrap();
                let result = invoke("open_new_project", serialized_args).await;

                if let Err(e) = serde_wasm_bindgen::from_value::<()>(result) {
                    web_sys::console::log_1(&format!("Error: {:?}", e).into());
                    validation_errors.set(vec![
                        "Une erreur est survenue lors de la création du projet".to_string(),
                    ]);
                    is_loading.set(false);
                }
            });
        })
    };

    let sorted_departments = {
        let mut dept_list: Vec<(String, String)> = departments
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        dept_list.sort_by(|(a_key, _), (b_key, _)| a_key.cmp(b_key));
        dept_list
    };

    html! {
        <div class="new-project-view">
            <h2>{"Créer un nouveau projet"}</h2>

            if !validation_errors.is_empty() {
                <div class="validation-errors">
                    <ul>
                        {for validation_errors.iter().map(|error| html! {
                            <li class="error-message">{error}</li>
                        })}
                    </ul>
                </div>
            }

            <form onsubmit={on_submit}>
                <div class="form-group">
                    <label for="department">{"Département"}<span class="required">{"*"}</span></label>
                    <select
                        id="department"
                        value={(*selected_department).clone()}
                        onchange={on_department_change}
                    >
                        <option value="">{"-- Sélectionnez un département --"}</option>
                        {
                            for sorted_departments.iter().map(|(code, name)| html! {
                            <option value={code.clone()}>{format!("{} - {}", code, name)}</option>
                            })
                        }
                    </select>
                </div>

                <div class="form-group">
                    <label for="project-name">{"Nom du projet"}<span class="required">{"*"}</span></label>
                    <input
                        type="text"
                        id="project-name"
                        value={(*project_name).clone()}
                        oninput={on_project_name_change}
                        placeholder="Entrez le nom du projet"
                    />
                </div>

                <div class="form-group">
                    <label>{"Coordonnées"}<span class="required">{"*"}</span></label>
                    <div class="coordinates-cross">
                        <div class="coord-row">
                            <div></div>
                            <div>
                                <label for="ymax">{"Y-Max"}</label>
                                <input
                                    id="ymax"
                                    type="text"
                                    class="coordinate-input"
                                    placeholder="ymax"
                                    value={(*ymax_str).clone()}
                                    oninput={on_ymax_input}
                                    inputmode="decimal"
                                />
                            </div>
                            <div></div>
                        </div>
                        <div class="coord-row">
                            <div>
                                <label for="xmin">{"X-Min"}</label>
                                <input
                                    id="xmin"
                                    type="text"
                                    class="coordinate-input"
                                    placeholder="xmin"
                                    value={(*xmin_str).clone()}
                                    oninput={on_xmin_input}
                                    inputmode="decimal"
                                />
                            </div>
                            <div class="square-indicator">
                                {
                                    if is_valid_shape == "square" {
                                        html! { <span class="square-yes">{"Carré ✓"}</span> }
                                    } else if is_valid_shape == "rectangle" {
                                        html! { <span class="square-yes">{"Rectangle !"}</span> }
                                    } else {
                                        html! { <span class="square-no">{"Invalide ⚠"}</span> }
                                    }
                                }
                            </div>
                            <div>
                                <label for="xmax">{"X-Max"}</label>
                                <input
                                    id="xmax"
                                    type="text"
                                    class="coordinate-input"
                                    placeholder="xmax"
                                    value={(*xmax_str).clone()}
                                    oninput={on_xmax_input}
                                    inputmode="decimal"
                                />
                            </div>
                        </div>
                        <div class="coord-row">
                            <div></div>
                            <div>
                                <label for="ymin">{"Y-Min"}</label>
                                <input
                                    id="ymin"
                                    type="text"
                                    class="coordinate-input"
                                    placeholder="ymin"
                                    value={(*ymin_str).clone()}
                                    oninput={on_ymin_input}
                                    inputmode="decimal"
                                />
                            </div>
                            <div></div>
                        </div>
                    </div>
                    <div class="coordinate-note">
                        <p>{"Note : Les dimensions de la zone (largeur et hauteur) doivent être des multiples de 500"}</p>
                    </div>
                </div>

                <button
                    type="submit"
                    disabled={*is_loading}
                    class={if *is_loading { "disabled" } else { "" }}
                >
                    {if *is_loading {
                        "Création du projet..."
                    } else {
                        "Créer le projet"
                    }}
                </button>
            </form>
        </div>
    }
}
