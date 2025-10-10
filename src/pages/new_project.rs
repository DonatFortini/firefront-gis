use crate::{
    api::invoke,
    types::{NewProjectArgs, ProjectBoundingBox, Route},
};
use std::rc::Rc;
use stylist::yew::styled_component;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

const PREDEFINED_BB: ProjectBoundingBox = ProjectBoundingBox {
    xmin: 1210000.0,
    xmax: 1235000.0,
    ymin: 6070000.0,
    ymax: 6095000.0,
};

#[styled_component(NewProject)]
pub fn new_project() -> Html {
    let navigator = use_navigator().unwrap();
    let is_loading = use_state(|| false);
    let project_name = use_state(String::new);
    let coordinates = use_state(CoordinateState::default);
    let validation_errors = use_state(Vec::<Rc<String>>::new);

    let validation_result = use_memo(coordinates.clone(), |coords| validate_coordinates(coords));

    let on_test_button_click = {
        let project_name = project_name.clone();
        let coordinates = coordinates.clone();
        Callback::from(move |_e: MouseEvent| {
            project_name.set("Test Project".to_string());
            coordinates.set(CoordinateState {
                xmin: PREDEFINED_BB.xmin.to_string(),
                ymin: PREDEFINED_BB.ymin.to_string(),
                xmax: PREDEFINED_BB.xmax.to_string(),
                ymax: PREDEFINED_BB.ymax.to_string(),
            });
        })
    };

    let on_project_name_change = {
        let project_name = project_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            project_name.set(input.value());
        })
    };

    let on_submit = {
        let is_loading = is_loading.clone();
        let validation_errors = validation_errors.clone();
        let navigator = navigator.clone();
        let project_name = project_name.clone();
        let coordinates = coordinates.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let errors = validate_form(&project_name, &coordinates);

            if !errors.is_empty() {
                validation_errors.set(errors);
                return;
            }

            validation_errors.set(Vec::new());
            is_loading.set(true);

            let args = NewProjectArgs {
                name: (*project_name).clone(),
                project_bb: ProjectBoundingBox {
                    xmin: coordinates.xmin.parse().unwrap(),
                    ymin: coordinates.ymin.parse().unwrap(),
                    xmax: coordinates.xmax.parse().unwrap(),
                    ymax: coordinates.ymax.parse().unwrap(),
                },
            };

            let project_name_clone = (*project_name).clone();
            let navigator = navigator.clone();

            navigator.push(&Route::Loading {
                project_name: project_name_clone.clone(),
            });

            spawn_local(async move {
                let serialized = serde_wasm_bindgen::to_value(&args).unwrap();
                let _ = invoke("create_project", serialized).await;
            });
        })
    };

    let page_style = css!(
        r#"
        min-height: calc(100vh - 58px);
        padding: 12px;
        "#
    );

    let header_style = css!(
        r#"
        position: fixed;
        top: 0;
        left: 260px;
        right: 0;
        padding: 16px 20px;
        background-color: #0e0e0e;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        z-index: 100;
        height: 58px;
        display: flex;
        align-items: center;
        
        h2 {
            color: #ffffff;
            font-weight: 600;
            font-size: 1.25rem;
            margin: 0;
        }
        
        @media (max-width: 768px) {
            left: 70px;
        }
        "#
    );

    let form_style = css!(
        r#"
        background-color: #242424;
        padding: 32px;
        border-radius: 8px;
        box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.1);
        max-width: 800px;
        margin: 0 auto;
        
        .form-group {
            margin-bottom: 24px;
        }
        
        label {
            display: block;
            margin-bottom: 8px;
            font-weight: 500;
            color: #cccccc;
            font-size: 0.95rem;
        }
        
        .required {
            color: #ff4141;
            margin-left: 4px;
        }
        
        input {
            width: 100%;
            padding: 12px;
            background-color: #1c1c1c;
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 4px;
            color: #ffffff;
            font-size: 0.95rem;
            transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
        }
        
        input:focus {
            outline: none;
            border-color: #ff4141;
            box-shadow: 0 0 0 3px rgba(255, 65, 65, 0.1);
        }
        
        input::placeholder {
            color: #666666;
        }
        
        button {
            background-color: #ff4141;
            color: white;
            border: none;
            padding: 14px 24px;
            border-radius: 4px;
            font-size: 1rem;
            cursor: pointer;
            font-weight: 600;
            transition: all 0.15s;
            width: 100%;
            margin-top: 8px;
        }
        
        button:hover:not(:disabled) {
            background-color: #ff5757;
            transform: translateY(-1px);
        }
        
        button:disabled {
            background-color: #2a2a2a;
            color: #666666;
            cursor: not-allowed;
            transform: none;
        }
        
        button[type="button"] {
            background-color: #2a2a2a;
            margin-bottom: 16px;
        }
        
        button[type="button"]:hover {
            background-color: #333333;
        }
        "#
    );

    html! {
        <>
            <div class={header_style}>
                <h2>{"Créer un nouveau projet"}</h2>
            </div>

            <div class={page_style}>
                {if !validation_errors.is_empty() {
                    html! { <ErrorList errors={(*validation_errors).clone()} /> }
                } else {
                    html! {}
                }}

                <form class={form_style} onsubmit={on_submit}>
                    <div class="form-group">
                        <label for="project-name">
                            {"Nom du projet"}
                            <span class="required">{"*"}</span>
                        </label>
                        <input
                            type="text"
                            id="project-name"
                            value={(*project_name).clone()}
                            oninput={on_project_name_change}
                            placeholder="Entrez le nom du projet"
                        />
                    </div>

                    <div class="form-group">
                        <label>
                            {"Coordonnées"}
                            <span class="required">{"*"}</span>
                        </label>
                        <CoordinateInput
                            coordinates={(*coordinates).clone()}
                            on_change={Callback::from({
                                let coordinates = coordinates.clone();
                                move |(field, e): (CoordinateField, InputEvent)| {
                                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    let value = input.value();

                                    if value.is_empty() {
                                        update_coordinate_field(&coordinates, field, value);
                                        return;
                                    }

                                    let filtered: String = value
                                        .chars()
                                        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                                        .collect();

                                    if filtered.len() != value.len() {
                                        input.set_value(&filtered);
                                    }

                                    if filtered.matches('.').count() <= 1 && filtered.matches('-').count() <= 1 {
                                        update_coordinate_field(&coordinates, field, filtered);
                                    }
                                }
                            })}
                            validation_result={(*validation_result).clone()}
                        />
                    </div>

                    <button type="button" onclick={on_test_button_click}>
                        {"Charger des coordonnées de test"}
                    </button>

                    <button type="submit" disabled={*is_loading}>
                        {if *is_loading {
                            "Création du projet..."
                        } else {
                            "Créer le projet"
                        }}
                    </button>
                </form>
            </div>
        </>
    }
}

#[derive(Clone, PartialEq, Default)]
struct CoordinateState {
    xmin: String,
    ymin: String,
    xmax: String,
    ymax: String,
}

#[derive(Clone, Copy)]
enum CoordinateField {
    XMin,
    YMin,
    XMax,
    YMax,
}

fn update_coordinate_field(
    coordinates: &UseStateHandle<CoordinateState>,
    field: CoordinateField,
    value: String,
) {
    let mut new_coords = (**coordinates).clone();
    match field {
        CoordinateField::XMin => new_coords.xmin = value,
        CoordinateField::YMin => new_coords.ymin = value,
        CoordinateField::XMax => new_coords.xmax = value,
        CoordinateField::YMax => new_coords.ymax = value,
    }
    coordinates.set(new_coords);
}

#[derive(Clone, PartialEq)]
enum ValidationResult {
    Valid(ShapeType),
    Invalid,
}

#[derive(Clone, PartialEq)]
enum ShapeType {
    Square,
    Rectangle,
}

fn validate_coordinates(coords: &CoordinateState) -> ValidationResult {
    let xmin = coords.xmin.parse::<f64>().ok();
    let ymin = coords.ymin.parse::<f64>().ok();
    let xmax = coords.xmax.parse::<f64>().ok();
    let ymax = coords.ymax.parse::<f64>().ok();

    if let (Some(xmin), Some(ymin), Some(xmax), Some(ymax)) = (xmin, ymin, xmax, ymax) {
        let width = xmax - xmin;
        let height = ymax - ymin;

        if width <= 0.0 || height <= 0.0 {
            return ValidationResult::Invalid;
        }

        let width_valid = (width / 10.0) % 500.0 == 0.0;
        let height_valid = (height / 10.0) % 500.0 == 0.0;

        if width_valid && height_valid {
            if (width - height).abs() < f64::EPSILON {
                return ValidationResult::Valid(ShapeType::Square);
            } else {
                return ValidationResult::Valid(ShapeType::Rectangle);
            }
        }
    }

    ValidationResult::Invalid
}

fn validate_form(project_name: &str, coordinates: &CoordinateState) -> Vec<Rc<String>> {
    let mut errors = Vec::new();

    if project_name.is_empty() {
        errors.push(Rc::new("Le nom du projet est requis".to_string()));
    }

    let xmin = coordinates.xmin.parse::<f64>().ok();
    let ymin = coordinates.ymin.parse::<f64>().ok();
    let xmax = coordinates.xmax.parse::<f64>().ok();
    let ymax = coordinates.ymax.parse::<f64>().ok();

    if xmin.is_none() || ymin.is_none() || xmax.is_none() || ymax.is_none() {
        errors.push(Rc::new(
            "Tous les champs de coordonnées doivent être remplis".to_string(),
        ));
        return errors;
    }

    if let (Some(xmin), Some(ymin), Some(xmax), Some(ymax)) = (xmin, ymin, xmax, ymax) {
        if xmin == 0.0 && ymin == 0.0 && xmax == 0.0 && ymax == 0.0 {
            errors.push(Rc::new(
                "Les coordonnées ne peuvent pas toutes être zéro".to_string(),
            ));
        } else {
            let width = xmax - xmin;
            let height = ymax - ymin;

            if width <= 0.0 || height <= 0.0 {
                errors.push(Rc::new(
                    "Les dimensions doivent être positives (xmax > xmin, ymax > ymin)".to_string(),
                ));
            } else {
                let width_valid = (width / 10.0) % 500.0 == 0.0;
                let height_valid = (height / 10.0) % 500.0 == 0.0;

                if !width_valid || !height_valid {
                    errors.push(Rc::new(
                        "Les dimensions doivent être des multiples de 500".to_string(),
                    ));
                }
            }
        }
    }

    errors
}

#[derive(Properties, PartialEq)]
struct ErrorListProps {
    errors: Vec<Rc<String>>,
}

#[styled_component(ErrorList)]
fn error_list(props: &ErrorListProps) -> Html {
    let style = css!(
        r#"
        margin-bottom: 24px;
        
        ul {
            list-style: none;
            padding: 0;
            margin: 0;
        }
        
        li {
            background-color: rgba(231, 76, 60, 0.1);
            color: #e74c3c;
            padding: 12px 16px;
            border-radius: 4px;
            margin-bottom: 8px;
            border-left: 4px solid #e74c3c;
            font-size: 0.9rem;
        }
        "#
    );

    html! {
        <div class={style}>
            <ul>
                {props.errors.iter().map(|error| html! {
                    <li>{&**error}</li>
                }).collect::<Html>()}
            </ul>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct CoordinateInputProps {
    coordinates: CoordinateState,
    on_change: Callback<(CoordinateField, InputEvent)>,
    validation_result: ValidationResult,
}

#[styled_component(CoordinateInput)]
fn coordinate_input(props: &CoordinateInputProps) -> Html {
    let style = css!(
        r#"
        .coordinate-cross {
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 16px;
            margin: 24px 0;
        }
        
        .coord-row {
            display: grid;
            grid-template-columns: 1fr 1fr 1fr;
            gap: 16px;
            width: 100%;
            align-items: center;
            justify-items: center;
        }
        
        .coord-row > div {
            display: flex;
            flex-direction: column;
            align-items: center;
            width: 100%;
        }
        
        .coord-input {
            font-family: 'Fira Code', monospace;
            text-align: center;
            width: 100%;
            max-width: 150px;
        }
        
        .shape-indicator {
            font-weight: 600;
            margin: 16px 0;
            text-align: center;
            font-size: 1rem;
            padding: 12px;
            border-radius: 4px;
            background-color: rgba(255, 255, 255, 0.05);
        }
        
        .valid-square {
            color: #2ecc71;
        }
        
        .valid-rectangle {
            color: #3498db;
        }
        
        .invalid {
            color: #e74c3c;
        }
        
        .coordinate-note {
            margin-top: 16px;
            color: #999999;
            font-size: 0.9rem;
            text-align: center;
            padding: 16px;
            background-color: rgba(255, 255, 255, 0.03);
            border-radius: 4px;
        }
        .coordinate-note p {
            margin: 8px 0;
        }
        
        @media (max-width: 768px) {
            .coord-row {
                grid-template-columns: 1fr;
                gap: 12px;
            }
        }
        "#
    );

    let on_change = props.on_change.clone();

    html! {
        <div class={style}>
            <div class="coordinate-cross">
                <div class="coord-row">
                    <div></div>
                    <div>
                        <label for="ymax">{"Y-Max"}</label>
                        <input
                            id="ymax"
                            type="text"
                            class="coord-input"
                            placeholder="ymax"
                            value={props.coordinates.ymax.clone()}
                            oninput={on_change.reform(|e| (CoordinateField::YMax, e))}
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
                            class="coord-input"
                            placeholder="xmin"
                            value={props.coordinates.xmin.clone()}
                            oninput={on_change.reform(|e| (CoordinateField::XMin, e))}
                            inputmode="decimal"
                        />
                    </div>

                    <div class="shape-indicator">
                        {match &props.validation_result {
                            ValidationResult::Valid(ShapeType::Square) =>
                                html! { <span class="valid-square">{"Carré ✓"}</span> },
                            ValidationResult::Valid(ShapeType::Rectangle) =>
                                html! { <span class="valid-rectangle">{"Rectangle ✓"}</span> },
                            ValidationResult::Invalid =>
                                html! { <span class="invalid">{"Invalide ⚠"}</span> },
                        }}
                    </div>

                    <div>
                        <label for="xmax">{"X-Max"}</label>
                        <input
                            id="xmax"
                            type="text"
                            class="coord-input"
                            placeholder="xmax"
                            value={props.coordinates.xmax.clone()}
                            oninput={on_change.reform(|e| (CoordinateField::XMax, e))}
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
                            class="coord-input"
                            placeholder="ymin"
                            value={props.coordinates.ymin.clone()}
                            oninput={on_change.reform(|e| (CoordinateField::YMin, e))}
                            inputmode="decimal"
                        />
                    </div>
                    <div></div>
                </div>
            </div>

            <div class="coordinate-note">
                <p>{"💡 Les dimensions (largeur et hauteur) doivent être des multiples de 500"}</p>
                <p>{"🗺️ Le système déterminera automatiquement les régions qui intersectent cette zone"}</p>
            </div>
        </div>
    }
}
