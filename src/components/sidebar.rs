use crate::types::Route;
use stylist::yew::styled_component;
use yew::prelude::*;
use yew_router::prelude::*;

#[styled_component(Sidebar)]
pub fn sidebar() -> Html {
    let current_route = use_route::<Route>().unwrap_or(Route::Home);

    let sidebar_style = css!(
        r#"
        width: 260px;
        background: #151515;
        border-right: 1px solid rgba(255, 255, 255, 0.1);
        display: flex;
        flex-direction: column;
        position: fixed;
        height: 100vh;
        top: 0;
        left: 0;
        z-index: 1;
        
        &::before {
            content: "";
            position: absolute;
            top: 0;
            left: 0;
            width: 3px;
            height: 100%;
            background: linear-gradient(to bottom, #ff4141 0%, transparent 100%);
        }
        
        @media (max-width: 768px) {
            width: 70px;
            
            .sidebar-text {
                display: none;
            }
        }
        "#
    );

    let header_style = css!(
        r#"
        padding: 16px 20px;
        text-align: left;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        background: #242424;
        height: 58px;
        display: flex;
        align-items: center;
        gap: 12px;
        
        img {
            width: 30px;
            height: 30px;
            object-fit: contain;
        }
        
        h1 {
            font-size: 1.25rem;
            font-weight: 600;
            color: #ffffff;
            letter-spacing: -0.01em;
            margin: 0;
        }
        
        @media (max-width: 768px) {
            padding: 16px 8px;
            justify-content: center;
            
            h1 {
                display: none;
            }
            
            img {
                width: 36px;
                height: 36px;
            }
        }
        "#
    );

    let content_style = css!(
        r#"
        padding: 20px 16px;
        display: flex;
        flex-direction: column;
        gap: 8px;
        flex: 1;
        overflow-y: auto;
        "#
    );

    let footer_style = css!(
        r#"
        padding: 20px 16px;
        display: flex;
        flex-direction: column;
        gap: 8px;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
        background: #242424;
        "#
    );

    html! {
        <div class={sidebar_style}>
            <div class={header_style}>
                <img src="public/icon.png" alt="Firefront GIS Logo" />
                <h1>{"Firefront GIS"}</h1>
            </div>

            <div class={content_style}>
                <NavLink route={Route::Home} current={current_route.clone()}>
                    {"Accueil"}
                </NavLink>
                <NavLink route={Route::NewProject} current={current_route.clone()}>
                    {"Créer un nouveau projet"}
                </NavLink>
            </div>

            <div class={footer_style}>
                <NavLink route={Route::Documentation} current={current_route.clone()}>
                    {"Documentation"}
                </NavLink>
                <NavLink route={Route::Settings} current={current_route.clone()}>
                    {"Paramètres"}
                </NavLink>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct NavLinkProps {
    pub route: Route,
    pub current: Route,
    pub children: Children,
}

#[styled_component(NavLink)]
fn nav_link(props: &NavLinkProps) -> Html {
    let is_active = std::mem::discriminant(&props.route) == std::mem::discriminant(&props.current);

    let link_style = css!(
        r#"
        background-color: transparent;
        color: #cccccc;
        border: none;
        padding: 10px 14px;
        text-align: left;
        font-size: 0.9rem;
        cursor: pointer;
        transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
        border-radius: 4px;
        position: relative;
        overflow: hidden;
        font-weight: 500;
        text-transform: none;
        letter-spacing: normal;
        text-decoration: none;
        display: block;
        
        &:hover {
            background-color: rgba(255, 65, 65, 0.1);
            color: #ffffff;
        }
        
        &.active {
            background-color: #ff4141;
            color: white;
            font-weight: 600;
        }
        
        &.active::before {
            content: "";
            position: absolute;
            left: 0;
            top: 50%;
            transform: translateY(-50%);
            width: 3px;
            height: 60%;
            background-color: white;
            border-radius: 0 3px 3px 0;
        }
        
        @media (max-width: 768px) {
            padding: 12px 8px;
            text-align: center;
            justify-content: center;
        }
        "#
    );

    html! {
        <Link<Route>
            to={props.route.clone()}
            classes={classes!(link_style, is_active.then_some("active"))}
        >
            <span class="sidebar-text">
                { for props.children.iter() }
            </span>
        </Link<Route>>
    }
}
