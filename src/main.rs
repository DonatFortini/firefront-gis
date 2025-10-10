pub mod api;
pub mod components;
pub mod pages;
pub mod styles;
pub mod types;

use pages::*;
use types::Route;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::styles::GlobalStyles;

fn main() {
    console_error_panic_hook::set_once();
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <>
            <GlobalStyles />
            <BrowserRouter>
                <AppContent />
            </BrowserRouter>
        </>
    }
}

#[function_component(AppContent)]
fn app_content() -> Html {
    let route = use_route::<Route>().unwrap_or(Route::Home);

    let show_sidebar = !matches!(route, Route::Loading { .. } | Route::Project { .. });

    html! {
        <div class={styles::app_container()}>
            if show_sidebar {
                <components::Sidebar />
            }
            <main class={if show_sidebar {
                styles::main_content()
            } else {
                styles::full_content()
            }}>
                <Switch<Route> render={switch_route} />
            </main>
        </div>
    }
}

fn switch_route(route: Route) -> Html {
    match route {
        Route::Home => html! { <Home /> },
        Route::NewProject => html! { <NewProject /> },
        Route::Settings => html! { <Settings /> },
        Route::Documentation => html! { <Documentation /> },
        Route::Loading { project_name } => html! {
            <Loading {project_name} />
        },
        Route::Project {
            project_name,
            view_mode,
        } => html! {
            <Project {project_name} {view_mode} />
        },
        Route::NotFound => html! {
            <div>
                <h1>{"404 - Page Not Found"}</h1>
                <Link<Route> to={Route::Home}>{"Return Home"}</Link<Route>>
            </div>
        },
    }
}
