use yew::prelude::*;

#[function_component(Documentation)]
pub fn documentation() -> Html {
    html! {
        <div class="documentation-view">
            <h2>{"Documentation"}</h2>
            <div class="doc-section">
                <h3>{"Pour commencer"}</h3>
                <p>{"Firefront GIS vous permet de créer et de gérer des projets d'information géographique pour les départements français. Commencez par créer un nouveau projet et sélectionner un département."}</p>
            </div>
            <div class="doc-section">
                <h3>{"Dépendances"}</h3>
                <p>{"Firefront nécessite l'installation de GDAL, Python et 7zip sur votre système."}</p>
                <ul>
                    <li>{"GDAL : Pour le traitement géospatial"}</li>
                    <li>{"Python : Pour les scripts de traitement supplémentaires"}</li>
                    <li>{"7zip : Pour extraire les archives de données"}</li>
                </ul>
            </div>
            <div class="doc-section">
                <h3>{"Création de projets"}</h3>
                <p>{"Pour créer un nouveau projet, cliquez sur le bouton 'Créer un nouveau projet', sélectionnez un département, entrez un nom de projet et spécifiez les coordonnées si nécessaire."}</p>
                <p>{"L'application téléchargera les données nécessaires depuis l'IGN (Institut national de l'information géographique et forestière) et créera le projet pour vous."}</p>
            </div>
            <div class="doc-section">
                <h3>{"Couches cartographiques"}</h3>
                <p>{"Firefront GIS ajoute automatiquement plusieurs couches à votre projet :"}</p>
                <ul>
                    <li>{"Éléments topographiques (routes, bâtiments, etc.)"}</li>
                    <li>{"Données de végétation et forestières"}</li>
                    <li>{"Frontières régionales"}</li>
                    <li>{"Parcelles agricoles (données RPG)"}</li>
                </ul>
            </div>
            <div class="doc-section">
                <h3>{"Exportation"}</h3>
                <p>{"Les projets peuvent être exportés en images JPEG pour une utilisation dans des rapports ou des présentations. Les données cartographiques et les images satellites sont disponibles."}</p>
            </div>
        </div>
    }
}
