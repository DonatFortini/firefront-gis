use crate::types::AppView;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SidebarProps {
    pub current_view: AppView,
    pub on_view_change: Callback<AppView>,
}

#[function_component(Sidebar)]
pub fn sidebar(props: &SidebarProps) -> Html {
    let on_home_click = {
        let on_view_change = props.on_view_change.clone();
        Callback::from(move |_| {
            on_view_change.emit(AppView::Home);
        })
    };

    let on_new_project_click = {
        let on_view_change = props.on_view_change.clone();
        Callback::from(move |_| {
            on_view_change.emit(AppView::NewProject);
        })
    };

    let on_settings_click = {
        let on_view_change = props.on_view_change.clone();
        Callback::from(move |_| {
            on_view_change.emit(AppView::Settings);
        })
    };

    let on_docs_click = {
        let on_view_change = props.on_view_change.clone();
        Callback::from(move |_| {
            on_view_change.emit(AppView::Documentation);
        })
    };

    html! {
        <div class="sidebar">
            <div class="sidebar-header">
                <img src="public/icon.png" alt="Firefront GIS Logo" />
                <h1>{"Firefront GIS"}</h1>
            </div>
            <div class="sidebar-content">
                <button
                    onclick={on_home_click.clone()}
                    class={if props.current_view == AppView::Home { "active" } else { "" }}
                >
                    {"Accueil"}
                </button>
                <button
                    onclick={on_new_project_click.clone()}
                    class={if props.current_view == AppView::NewProject { "active" } else { "" }}
                >
                    {"Créer un nouveau projet"}
                </button>
            </div>
            <div class="sidebar-footer">
                <button
                    onclick={on_docs_click.clone()}
                    class={if props.current_view == AppView::Documentation { "active" } else { "" }}
                >
                    {"Documentation"}
                </button>
                <button
                    onclick={on_settings_click.clone()}
                    class={if props.current_view == AppView::Settings { "active" } else { "" }}
                >
                    {"Paramètres"}
                </button>
            </div>
        </div>
    }
}
