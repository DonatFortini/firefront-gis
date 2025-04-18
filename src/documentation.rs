use yew::prelude::*;

#[function_component(Documentation)]
pub fn documentation() -> Html {
    html! {
        <div class="documentation-view">
            <h2>{"Documentation"}</h2>
            <div class="doc-section">
                <h3>{"Getting Started"}</h3>
                <p>{"Firefront GIS allows you to create and manage geographic information projects for French departments. Start by creating a new project and selecting a department."}</p>
            </div>
            <div class="doc-section">
                <h3>{"Dependencies"}</h3>
                <p>{"Firefront requires GDAL, Python, and 7zip to be installed on your system."}</p>
                <ul>
                    <li>{"GDAL: For geospatial processing"}</li>
                    <li>{"Python: For additional processing scripts"}</li>
                    <li>{"7zip: For extracting data archives"}</li>
                </ul>
            </div>
            <div class="doc-section">
                <h3>{"Creating Projects"}</h3>
                <p>{"To create a new project, click the 'Create New Project' button, select a department, enter a project name, and specify the coordinates if needed."}</p>
                <p>{"The application will download the necessary data from the IGN (Institut national de l'information géographique et forestière) and create the project for you."}</p>
            </div>
            <div class="doc-section">
                <h3>{"Map Layers"}</h3>
                <p>{"Firefront GIS automatically adds several layers to your project:"}</p>
                <ul>
                    <li>{"Topographic features (roads, buildings, etc.)"}</li>
                    <li>{"Vegetation and forest data"}</li>
                    <li>{"Regional boundaries"}</li>
                    <li>{"Agricultural parcels (RPG data)"}</li>
                </ul>
            </div>
            <div class="doc-section">
                <h3>{"Exporting"}</h3>
                <p>{"Projects can be exported as JPEG images for use in reports or presentations. Both map data and satellite imagery are available."}</p>
            </div>
        </div>
    }
}
