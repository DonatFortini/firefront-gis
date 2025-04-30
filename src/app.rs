use yew::prelude::*;

use crate::documentation::Documentation;
use crate::home::Home;
use crate::loading::Loading;
use crate::new_project::NewProject;
use crate::project::Project;
use crate::settings::SettingsComponent as Settings;
use crate::sidebar::Sidebar;
use crate::types::AppView;

#[function_component(App)]
pub fn app() -> Html {
    let app_view = use_state(|| AppView::Home);

    let on_view_change = {
        let app_view = app_view.clone();
        Callback::from(move |view: AppView| {
            app_view.set(view);
        })
    };

    let show_sidebar = match *app_view {
        AppView::Loading(_) | AppView::Project(_) => false,
        AppView::Home | AppView::Settings | AppView::Documentation | AppView::NewProject => true,
    };

    html! {
        <div class="app-container">
            if show_sidebar {
                <Sidebar current_view={(*app_view).clone()} on_view_change={on_view_change.clone()} />
            }
            <div class={if show_sidebar { "main-content" } else { "full-content" }}>
                {
                    match (*app_view).clone() {
                        AppView::Home => html! { <Home on_view_change={on_view_change.clone()} /> },
                        AppView::NewProject => html! { <NewProject on_view_change={on_view_change.clone()} /> },
                        AppView::Settings => html! { <Settings /> },
                        AppView::Documentation => html! { <Documentation /> },
                        AppView::Loading(project_name) => html! {
                            <Loading project_name={project_name} on_view_change={on_view_change.clone()} />
                        },
                        AppView::Project(project_data) => html! {
                            <Project project_data={project_data} on_view_change={on_view_change.clone()} />
                        },
                    }
                }
            </div>
        </div>
    }
}
