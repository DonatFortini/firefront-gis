pub mod app;
pub mod documentation;
pub mod home;
pub mod new_project;
pub mod settings;
pub mod sidebar;
pub mod types;

use crate::app::App;

fn main() {
    console_error_panic_hook::set_once();
    let document = web_sys::window().unwrap().document().unwrap();
    let head = document.head().unwrap();

    let style = document.create_element("style").unwrap();
    style.set_inner_html(include_str!("../styles.css"));
    head.append_child(&style).unwrap();

    yew::Renderer::<App>::new().render();
}
