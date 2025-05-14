# Firefront GIS

<div align="center">
   <img src="public/icon.png" alt="Firefront GIS Logo" width="200">
</div>

## Solution SIG pour la lutte contre les incendies

Firefront GIS est une application de Système d'Information Géographique (SIG) spécialisée, conçue pour les services d'incendie et de secours. Elle permet la création, la visualisation et l'exportation de données géographiques dans le format attendu par le logiciel Vulcain (CRISE).

## Présentation

Développée spécifiquement pour répondre aux besoins du SIS2B, Firefront GIS génère des cartographies de terrain détaillées en exploitant les données géographiques officielles de l'Institut National de l'Information Géographique et Forestière (IGN). L'application traite ces données pour mettre en évidence les éléments stratégiques essentiels à la lutte contre les incendies : typologie de végétation, infrastructures routières, ressources hydrauliques et batiments.

## Fonctionnalités principales

- **Intégration de sources officielles** : Téléchargement et traitement automatisés des données IGN (BD TOPO®, BD FORÊT®, RPG)
- **Analyse multicouche avancée** :
  - Couverture végétale avec différenciation des essences (feuillus, conifères, etc.)
  - Éléments topographiques (bâtiments, routes, voies ferrées)
  - Ressources hydrauliques (cours d'eau, plans d'eau, réservoirs)
  - Parcelles agricoles
- **Double mode de visualisation** : Basculement entre végétation et imagerie satellite
- **Fonctionnalité d'export** : Export des projets au format attendu par Vulcain, ainsi que les geopackage associés pour importation dans d'autres systèmes SIG
- **Analyse régionale intelligente** : Détermination automatique des départements et régions administratives intersectant la zone d'intérêt
- **Interface utilisateur intuitive** : Conçue pour une prise en main rapide par les utilisateurs non techniques
- **Compatibilité multiplateforme** : Fonctionne sur Windows, macOS et Linux
- **Système de cache** : Gestion efficace des données pour éviter les téléchargements redondants

## Prérequis techniques

L'application nécessite l'installation préalable des composants externes suivants :

- **GDAL** (Geospatial Data Abstraction Library) : Bibliothèque de traitement des données géospatiales
- **7-Zip** : Outil d'extraction d'archives
- **ImageMagick** : Suite logicielle de traitement d'images (pseudo-dépendance de Vulcain)

⚠️ **IMPORTANT** : GDAL doit impérativement être installé sur votre système **avant la compilation** du projet, pas seulement pour son exécution. Cette dépendance est requise lors du processus de build.

## Installation

### À partir des sources

1. **Installez les dépendances requises** :

   - GDAL avec les bibliothèques de développement (headers)
   - 7-Zip
   - ImageMagick

   **Note** : Sur Linux, vous pouvez installer GDAL via :

   # Ubuntu/Debian

   ```bash
   sudo apt-get install libgdal-dev gdal-bin
   ```

   # Fedora

   ```bash
   sudo dnf install gdal-devel gdal
   ```

   # Arch Linux

   ```bash
   sudo pacman -S gdal
   ```

   # macOS

   ```bash
   brew install gdal
   ```

   Sur Windows, utilisez les installateurs officiels de GDAL comme OSGEO4W et assurez-vous que les variables d'environnement sont correctement configurées.

2. Assurez-vous que Rust et Cargo et trunk sont installés sur votre système ainsi que Tauri CLI et la chaîne d'outils webassembly.

   - Installez Rust et Cargo via rustup :

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Installez la chaîne d'outils WebAssembly

   ```bash
   rustup target add wasm32-unknown-unknown
   ```

4. Installez Tauri CLI

   ```bash
   cargo install tauri-cli
   ```

5. Installez Trunk

   ```bash
   cargo install trunk
   ```

6. Clonez le dépôt

   ```bash
   git clone https://github.com/DonatFortini/firefront-gis.git
   cd firefront-gis
   ```

7. Démarrez le serveur de développement

   ```bash
   cargo tauri dev
   ```

   ou pour le mode de production

   ```bash
   cargo tauri build
   ```

L'application compilée sera disponible dans le répertoire `target/release`.

## Guide d'utilisation

### Création d'un nouveau projet opérationnel

1. Lancez l'application
2. Cliquez sur "Créer un nouveau projet" dans la barre latérale
3. Saisissez un nom de projet (ex: nom du secteur opérationnel)
4. Spécifiez les coordonnées Lambert-93 de votre zone d'intérêt (les dimensions doivent être des multiples de 500 px)
5. Cliquez sur "Créer le projet" et patientez pendant le téléchargement et le traitement des données

### Visualisation des données cartographiques

- Alternez entre la vue d'analyse de végétation et l'imagerie satellite à l'aide des boutons dans la barre latérale
- plus d'options de visualisation seront ajoutées dans les versions futures

### Exportation des données pour utilisation terrain

1. Ouvrez le projet à exporter
2. Cliquez sur le bouton "Exporter"
3. Le projet sera exporté vers l'emplacement de sortie configuré (paramétrable dans la section "Paramètres" de l'application)

## Configuration système

Accédez au panneau de configuration pour :

- Modifier le répertoire de sortie des projets exportés
- Spécifier le chemin d'installation de GDAL si non détecté automatiquement
- Vider le cache de données pour libérer de l'espace disque

## Architecture technique

- **src-tauri/** : Code backend (Rust avec Tauri)
  - **src/** : Fonctionnalités principales pour les opérations SIG
  - **commands.rs** : Gestionnaires de commandes Tauri
  - **gis_operation/** : Logique de traitement géospatial
- **src/** : Code frontend (Rust avec Yew)
  - **app.rs** : Composant principal de l'application
  - **home.rs** : Vue de liste des projets
  - **project.rs** : Visualiseur de projets
  - **new_project.rs** : Interface de création de projets

## Développement

Ce projet utilise :

- **Tauri** : Pour la création de l'application desktop multi-plateforme
- **Yew** : Pour les composants d'interface utilisateur basés sur WebAssembly
- **GDAL** : Pour le traitement des données géospatiales
- **Tokio** : Pour les opérations asynchrones

## Spécifications techniques

### Système de coordonnées

L'application utilise le système de coordonnées Lambert-93 (EPSG:2154) pour toutes les opérations géographiques, conformément aux standards nationaux français ainsi qu'une résolution de 10m par pixel pour les données raster.

### Traitement des données

1. L'application détermine quelles régions administratives intersectent avec la zone définie par l'utilisateur
2. Télécharge les fichiers de données nécessaires depuis les serveurs de l'IGN pour ces régions
3. Traite et découpe les données selon la zone d'intérêt
4. Crée diverses couches avec différents styles de visualisation adaptés aux besoins opérationnels
5. Génère un fichier de projet unifié qui peut être visualisé et exporté

### Processus d'exportation

Lors de l'exportation, l'application :

1. Découpe le projet en tuiles de 500x500 px, Orthographique et Végetation comme attendu par Vulcain
2. Exporte les données utilisées dans un format geopackage dans le dossier ressources
3. fournit le fichier au format TIFF qui est le fichier projet original
4. Conditionne toutes les données dans un format compressé
5. Enregistre l'ensemble dans l'emplacement de sortie spécifié

## Licence

GPL-3.0
