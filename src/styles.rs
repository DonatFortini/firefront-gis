use stylist::yew::Global;
use stylist::{Style, css, style};
use yew::prelude::*;

#[function_component(GlobalStyles)]
pub fn global_styles() -> Html {
    html! {
        <Global css={css!(r#"
            /* CSS Variables */
            :root {
                --background-primary: #0e0e0e;
                --background-secondary: #151515;
                --background-tertiary: #1c1c1c;
                --surface-primary: #242424;
                --surface-secondary: #2a2a2a;
                --surface-elevated: #333333;
                --accent-primary: #ff4141;
                --accent-secondary: #ff5757;
                --accent-tertiary: #ff2c2c;
                --accent-soft: rgba(255, 65, 65, 0.1);
                --accent-subtle: rgba(255, 65, 65, 0.05);
                --text-primary: #ffffff;
                --text-secondary: #cccccc;
                --text-tertiary: #999999;
                --text-muted: #666666;
                --success-color: #2ecc71;
                --warning-color: #f39c12;
                --error-color: #e74c3c;
                --info-color: #3498db;
                --border-color: rgba(255, 255, 255, 0.1);
                --border-color-lighter: rgba(255, 255, 255, 0.15);
                --border-radius: 4px;
                --border-radius-lg: 8px;
                --box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
                --box-shadow-hover: 0 4px 20px rgba(0, 0, 0, 0.4);
                --transition-speed: 0.15s;
                --transition-timing: cubic-bezier(0.4, 0, 0.2, 1);
                --font-sans: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                --font-mono: "Fira Code", "Jetbrains Mono", monospace;
            }
            
            /* Reset CSS */
            * {
                box-sizing: border-box;
                margin: 0;
                padding: 0;
            }
            
            /* Body */
            body {
                font-family: var(--font-sans);
                background-color: var(--background-primary);
                color: var(--text-primary);
                line-height: 1.5;
                -webkit-font-smoothing: antialiased;
                -moz-osx-font-smoothing: grayscale;
            }
            
            html {
                scroll-behavior: smooth;
            }
            
            /* Headings */
            h1, h2, h3, h4, h5, h6 {
                color: var(--text-primary);
            }
            
            h2 {
                font-size: 1.25rem;
                font-weight: 600;
                line-height: 1.2;
            }
            
            h3 {
                font-size: 1.25rem;
                font-weight: 600;
            }
            
            /* Form elements */
            button, input, textarea, select {
                font-family: inherit;
            }
            
            button {
                cursor: pointer;
            }
            
            input::placeholder {
                color: var(--text-tertiary);
            }
            
            /* Links */
            a {
                color: inherit;
                text-decoration: none;
            }
            
            /* Images */
            img {
                max-width: 100%;
                height: auto;
                display: block;
            }
            
            /* Scrollbar */
            ::-webkit-scrollbar {
                width: 12px;
                height: 12px;
            }
            
            ::-webkit-scrollbar-track {
                background: var(--background-tertiary);
            }
            
            ::-webkit-scrollbar-thumb {
                background: var(--surface-secondary);
                border-radius: 6px;
                border: 3px solid var(--background-tertiary);
            }
            
            ::-webkit-scrollbar-thumb:hover {
                background: var(--accent-primary);
            }
            
            /* Selection */
            ::selection {
                background: rgba(255, 65, 65, 0.3);
                color: #ffffff;
            }
            
            ::-moz-selection {
                background: rgba(255, 65, 65, 0.3);
                color: #ffffff;
            }
            
            /* Focus */
            button:focus-visible,
            input:focus-visible,
            select:focus-visible {
                outline: 2px solid var(--accent-primary);
                outline-offset: 2px;
            }
            
            /* Disable tap highlight */
            * {
                -webkit-tap-highlight-color: transparent;
            }
            
            /* Animations */
            @keyframes spin {
                to { transform: rotate(360deg); }
            }
            
            @keyframes pulse {
                0%, 100% { opacity: 1; }
                50% { opacity: 0.5; }
            }
            
            @keyframes fadeIn {
                from { opacity: 0; }
                to { opacity: 1; }
            }
            
            @keyframes slideUp {
                from {
                    transform: translateY(20px);
                    opacity: 0;
                }
                to {
                    transform: translateY(0);
                    opacity: 1;
                }
            }
            
            @keyframes slideIn {
                from {
                    opacity: 0;
                    transform: translateY(-10px);
                }
                to {
                    opacity: 1;
                    transform: translateY(0);
                }
            }
            
            @keyframes shimmer {
                0% { transform: translateX(-100%); }
                100% { transform: translateX(100%); }
            }
            
            /* Responsive */
            @media (max-width: 768px) {
                h2 { font-size: 1.1rem; }
            }
            
            /* Print */
            @media print {
                button { display: none; }
                body {
                    background-color: white;
                    color: black;
                }
            }
            
            /* Utility */
            .sr-only {
                position: absolute;
                width: 1px;
                height: 1px;
                padding: 0;
                margin: -1px;
                overflow: hidden;
                clip: rect(0, 0, 0, 0);
                white-space: nowrap;
                border-width: 0;
            }
        "#)} />
    }
}

pub const BG_PRIMARY: &str = "#0e0e0e";
pub const BG_SECONDARY: &str = "#151515";
pub const SURFACE_PRIMARY: &str = "#242424";
pub const ACCENT_PRIMARY: &str = "#ff4141";
pub const ACCENT_SECONDARY: &str = "#ff5757";
pub const TEXT_PRIMARY: &str = "#ffffff";
pub const TEXT_SECONDARY: &str = "#cccccc";
pub const BORDER_COLOR: &str = "rgba(255, 255, 255, 0.1)";

pub fn app_container() -> Style {
    style!(
        r#"
        display: flex;
        height: 100vh;
        width: 100%;
        overflow: hidden;
        background: ${bg_primary};
        "#,
        bg_primary = BG_PRIMARY
    )
    .unwrap()
}

pub fn main_content() -> Style {
    style!(
        r#"
        flex: 1;
        overflow-y: auto;
        background-color: ${bg_primary};
        position: relative;
        height: calc(100vh - 58px);
        padding: 0;
        margin-top: 58px;
        margin-left: 260px;
        
        @media (max-width: 768px) {
            margin-left: 70px;
        }
        "#,
        bg_primary = BG_PRIMARY
    )
    .unwrap()
}

pub fn full_content() -> Style {
    style!(
        r#"
        flex: 1;
        overflow-y: auto;
        background-color: ${bg_primary};
        position: relative;
        height: 100vh;
        padding: 0;
        margin-top: 0;
        margin-left: 0;
        "#,
        bg_primary = BG_PRIMARY
    )
    .unwrap()
}

pub fn button_primary() -> Style {
    style!(
        r#"
        background-color: ${accent};
        color: white;
        border: none;
        padding: 12px 20px;
        border-radius: 4px;
        font-size: 0.95rem;
        cursor: pointer;
        font-weight: 600;
        transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
        text-transform: uppercase;
        letter-spacing: 0.5px;
        
        &:hover {
            background-color: ${accent_hover};
            transform: translateY(-1px);
        }
        
        &:disabled {
            background-color: ${disabled};
            color: #666666;
            cursor: not-allowed;
            transform: none;
        }
        "#,
        accent = ACCENT_PRIMARY,
        accent_hover = ACCENT_SECONDARY,
        disabled = "#2a2a2a"
    )
    .unwrap()
}

pub fn input_field() -> Style {
    style!(
        r#"
        width: 100%;
        padding: 10px 12px;
        background-color: #1c1c1c;
        border: 1px solid ${border_color};
        border-radius: 4px;
        font-size: 0.95rem;
        color: ${text};
        transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
        
        &::placeholder {
            color: #999999;
        }
        
        &:focus {
            outline: none;
            border-color: ${accent};
            box-shadow: 0 0 0 3px rgba(255, 65, 65, 0.1);
        }
        "#,
        border_color = BORDER_COLOR,
        text = TEXT_PRIMARY,
        accent = ACCENT_PRIMARY
    )
    .unwrap()
}

pub fn card() -> Style {
    style!(
        r#"
        background-color: ${surface};
        border-radius: 8px;
        padding: 24px;
        box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
        border: 1px solid ${border};
        transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
        
        &:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
        }
        "#,
        surface = SURFACE_PRIMARY,
        border = BORDER_COLOR
    )
    .unwrap()
}

pub fn page_header() -> Style {
    style!(
        r#"
        position: fixed;
        top: 0;
        left: 260px;
        right: 0;
        padding: 16px 20px;
        background-color: ${bg};
        border-bottom: 1px solid ${border};
        z-index: 100;
        margin: 0;
        color: ${text};
        font-weight: 600;
        font-size: 1.25rem;
        line-height: 1.2;
        height: 58px;
        display: flex;
        align-items: center;
        
        @media (max-width: 768px) {
            left: 70px;
        }
        "#,
        bg = BG_PRIMARY,
        border = BORDER_COLOR,
        text = TEXT_PRIMARY
    )
    .unwrap()
}

pub fn error_message() -> Style {
    style!(
        r#"
        background-color: rgba(231, 76, 60, 0.1);
        color: #e74c3c;
        padding: 14px 16px;
        border-radius: 4px;
        margin-bottom: 20px;
        border-left: 4px solid #e74c3c;
        font-size: 0.9rem;
        "#
    )
    .unwrap()
}

pub fn form_group() -> Style {
    style!(
        r#"
        margin-bottom: 20px;
        
        label {
            display: block;
            margin-bottom: 8px;
            font-weight: 500;
            color: ${text_secondary};
            font-size: 0.9rem;
            letter-spacing: 0.01em;
        }
        
        .required {
            color: ${accent};
            margin-left: 4px;
        }
        "#,
        text_secondary = TEXT_SECONDARY,
        accent = ACCENT_PRIMARY
    )
    .unwrap()
}

pub fn grid_container(min_width: usize) -> Style {
    style!(
        r#"
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(${min_width}px, 1fr));
        gap: 16px;
        margin-top: 8px;
        "#,
        min_width = min_width
    )
    .unwrap()
}
