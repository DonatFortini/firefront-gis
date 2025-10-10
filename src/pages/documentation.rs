use stylist::yew::styled_component;
use yew::prelude::*;

#[styled_component(Documentation)]
pub fn documentation() -> Html {
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
            display: flex;
            align-items: center;
            gap: 12px;
        }
        
        @media (max-width: 768px) {
            left: 70px;
        }
        "#
    );

    let container_style = css!(
        r#"
        min-height: calc(100vh - 58px);
        padding: 24px;
        
        .content {
            max-width: 900px;
            margin: 0 auto;
        }
        "#
    );

    html! {
        <>
            <div class={header_style}>
                <h2>
                    <span>{"📚"}</span>
                    {"Documentation"}
                </h2>
            </div>

            <div class={container_style}>
                <div class="content">
                    <DocSection
                        icon="🚀"
                        title="Création de projets"
                        items={vec![
                            AttrValue::from("Cliquez sur 'Créer un nouveau projet' dans le menu latéral"),
                            AttrValue::from("Entrez un nom unique pour votre projet"),
                            AttrValue::from("Spécifiez les coordonnées de la zone d'étude (système Lambert 93)"),
                            AttrValue::from("Les dimensions doivent être des multiples de 500 mètres"),
                            AttrValue::from("L'application télécharge automatiquement les données depuis l'IGN")
                        ]}
                    />

                    <DocSection
                        icon="🗺️"
                        title="Couches cartographiques"
                        description="Firefront GIS ajoute automatiquement plusieurs couches à votre projet :"
                        items={vec![
                            AttrValue::from("🏗️ Éléments topographiques (routes, bâtiments, infrastructures)"),
                            AttrValue::from("🌲 Données de végétation et forestières"),
                            AttrValue::from("🗺️ Frontières régionales et administratives"),
                            AttrValue::from("🌾 Parcelles agricoles (données RPG)"),
                            AttrValue::from("⛰️ Modèle numérique de terrain (MNT)"),
                            AttrValue::from("🛰️ Orthophotographies aériennes")
                        ]}
                    />

                    <DocSection
                        icon="📦"
                        title="Exportation"
                        items={vec![
                            AttrValue::from("Ouvrez votre projet depuis la page d'accueil"),
                            AttrValue::from("Cliquez sur le bouton 'Exporter le projet'"),
                            AttrValue::from("Un fichier ZIP est créé contenant toutes les données"),
                            AttrValue::from("Le ZIP inclut : cartes de végétation, orthophotos, fichiers GPKG, photos originales"),
                            AttrValue::from("Configurez l'emplacement d'exportation dans les paramètres")
                        ]}
                    />

                    <DocSection
                        icon="⚙️"
                        title="Paramètres et configuration"
                        items={vec![
                            AttrValue::from("Configurez le dossier de destination pour les exports"),
                            AttrValue::from("Videz le cache pour libérer de l'espace disque, ou pour forcer le re-téléchargement des données"),
                        ]}
                    />

                    <DocSection
                        icon="💡"
                        title="Conseils d'utilisation"
                        items={vec![
                            AttrValue::from("Utilisez des noms de projet descriptifs et uniques"),
                            AttrValue::from("Vérifiez les coordonnées avant de créer un projet"),
                            AttrValue::from("La création peut prendre plusieurs minutes selon la zone")
                        ]}
                    />
                </div>
            </div>
        </>
    }
}

#[derive(Properties, PartialEq)]
struct DocSectionProps {
    icon: AttrValue,
    title: AttrValue,
    #[prop_or_default]
    description: Option<AttrValue>,
    items: Vec<AttrValue>,
}

#[styled_component(DocSection)]
fn doc_section(props: &DocSectionProps) -> Html {
    let style = css!(
        r#"
        margin-bottom: 32px;
        padding: 28px;
        background: linear-gradient(135deg, #242424 0%, #1c1c1c 100%);
        border-radius: 12px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
        box-shadow: 0 2px 10px rgba(0, 0, 0, 0.2);
        
        &:hover {
            border-color: rgba(255, 65, 65, 0.3);
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
            transform: translateY(-2px);
        }
        
        h3 {
            margin-bottom: 16px;
            color: #ffffff;
            display: flex;
            align-items: center;
            gap: 12px;
            font-size: 1.3rem;
            font-weight: 600;
        }
        
        .icon {
            font-size: 1.8rem;
            display: flex;
            align-items: center;
            justify-content: center;
            width: 48px;
            height: 48px;
            background: rgba(255, 65, 65, 0.1);
            border-radius: 8px;
        }
        
        .description {
            color: #cccccc;
            line-height: 1.6;
            margin-bottom: 16px;
            font-size: 1rem;
        }
        
        ul {
            list-style: none;
            padding: 0;
            margin: 0;
        }
        
        li {
            padding: 12px 0 12px 36px;
            color: #cccccc;
            font-size: 0.95rem;
            line-height: 1.6;
            position: relative;
            border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        }
        
        li:last-child {
            border-bottom: none;
        }
        
        li::before {
            content: "";
            position: absolute;
            left: 0;
            top: 50%;
            transform: translateY(-50%);
            width: 8px;
            height: 8px;
            background-color: #ff4141;
            border-radius: 50%;
            box-shadow: 0 0 8px rgba(255, 65, 65, 0.5);
        }
        
        li:hover {
            color: #ffffff;
            padding-left: 40px;
            transition: all 0.2s;
        }
        "#
    );

    html! {
        <div class={style}>
            <h3>
                <div class="icon">{&props.icon}</div>
                {&props.title}
            </h3>

            {if let Some(desc) = &props.description {
                html! { <p class="description">{desc}</p> }
            } else {
                html! {}
            }}

            <ul>
                {props.items.iter().map(|item| html! {
                    <li>{item}</li>
                }).collect::<Html>()}
            </ul>
        </div>
    }
}
