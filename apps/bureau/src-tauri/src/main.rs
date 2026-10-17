//! Application de bureau du correcteur.
//!
//! Le moteur est la bibliothèque Rust native, chargée au démarrage à partir des
//! données incluses dans l'application. Un raccourci global ouvre la fenêtre
//! avec le contenu du presse-papiers ; le texte corrigé y est recopié à la
//! demande. Les réglages, le dictionnaire personnel et le brouillon sont des
//! fichiers locaux (voir `stockage`). Seule connexion réseau : la recherche
//! des mises à jour (`mises_a_jour`), coupée par défaut ; aucun texte n'est envoyé.

// Pas de console sous Windows en version publiée.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod mises_a_jour;
mod stockage;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use correcteur_core::interop;
use correcteur_core::{Category, Engine, Issue, Options};
use mises_a_jour::UpdateJs;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::Value;
use stockage::Encoding;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_window_state::StateFlags;

pub(crate) const WINDOW: &str = "principale";
/// Événement envoyé à l'interface avec le texte du presse-papiers.
const CLIPBOARD_EVENT: &str = "texte-presse-papiers";
/// Événement envoyé avec le chemin d'un fichier déposé sur la fenêtre.
const DROP_EVENT: &str = "fichier-depose";
/// Événement envoyé avec l'identifiant d'une commande du menu Fichier.
const MENU_EVENT: &str = "menu";
/// Argument passé au lancement automatique : la fenêtre reste masquée.
const AUTOSTART_ARG: &str = "--demarrage";

/// Moteur chargé au démarrage, ou message d'erreur.
struct EngineState(OnceLock<Result<Engine, String>>);

impl EngineState {
    fn get(&self) -> Result<&Engine, String> {
        match self.0.get() {
            Some(Ok(e)) => Ok(e),
            Some(Err(e)) => Err(e.clone()),
            None => Err("moteur en cours de chargement".into()),
        }
    }
}

/// Réglages envoyés par l'interface (mêmes noms que `CheckOptions` en TypeScript).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OptionsJs {
    disabled_categories: Vec<String>,
    disabled_rules: Vec<String>,
    user_words: Vec<String>,
    narrow_nbsp: bool,
    typographic_apostrophe: bool,
    french_quotes: bool,
    ellipsis_character: bool,
    check_proper_nouns: bool,
    max_suggestions: usize,
    long_sentence_words: usize,
    repetition_window: usize,
}

impl From<OptionsJs> for Options {
    fn from(o: OptionsJs) -> Options {
        Options {
            disabled_categories: o
                .disabled_categories
                .iter()
                .filter_map(|c| Category::parse(c))
                .collect(),
            disabled_rules: o.disabled_rules,
            user_words: o.user_words,
            narrow_nbsp: o.narrow_nbsp,
            typographic_apostrophe: o.typographic_apostrophe,
            french_quotes: o.french_quotes,
            ellipsis_character: o.ellipsis_character,
            check_proper_nouns: o.check_proper_nouns,
            max_suggestions: o.max_suggestions,
            long_sentence_words: o.long_sentence_words,
            repetition_window: o.repetition_window,
        }
    }
}

/// Problème au format de l'interface : positions en UTF-16, comme les chaînes JavaScript.
#[derive(Serialize)]
struct IssueJs {
    start: usize,
    end: usize,
    rule: String,
    category: &'static str,
    message: String,
    explanation: String,
    suggestions: Vec<String>,
}

impl From<Issue> for IssueJs {
    fn from(i: Issue) -> IssueJs {
        IssueJs {
            start: i.start_utf16,
            end: i.end_utf16,
            rule: i.rule,
            category: i.category.as_str(),
            message: i.message,
            explanation: i.explanation,
            suggestions: i.suggestions,
        }
    }
}

#[tauri::command]
fn analyser(
    state: tauri::State<EngineState>,
    texte: String,
    options: OptionsJs,
) -> Result<Vec<IssueJs>, String> {
    let engine = state.get()?;
    Ok(engine
        .check(&texte, &options.into())
        .into_iter()
        .map(IssueJs::from)
        .collect())
}

/// Tableau de conjugaison (JSON du moteur), ou `null` si le verbe est inconnu.
#[tauri::command]
fn conjuguer(state: tauri::State<EngineState>, verbe: String) -> Result<RawJson, String> {
    let engine = state.get()?;
    raw(engine
        .conjugate(&verbe)
        .map_or_else(|| "null".into(), |c| interop::conjugation_to_json(&c)))
}

/// Synonymes et antonymes (JSON du moteur), ou `null`.
#[tauri::command]
fn synonymes(state: tauri::State<EngineState>, mot: String) -> Result<RawJson, String> {
    let engine = state.get()?;
    raw(interop::lookup_to_json(engine, &mot).unwrap_or_else(|| "null".into()))
}

#[tauri::command]
fn lisibilite(state: tauri::State<EngineState>, texte: String) -> Result<RawJson, String> {
    raw(interop::readability_to_json(
        &state.get()?.readability(&texte),
    ))
}

/// JSON déjà produit par le moteur, transmis tel quel à l'interface.
type RawJson = Box<RawValue>;

fn raw(json: String) -> Result<RawJson, String> {
    RawValue::from_string(json).map_err(|e| e.to_string())
}

/// Nombre de formes du lexique.
#[tauri::command]
fn infos(state: tauri::State<EngineState>) -> Result<usize, String> {
    Ok(state.get()?.lexicon().len())
}

// ---------------------------------------------------------------------------
// Presse-papiers

#[tauri::command]
fn lire_presse_papiers(app: AppHandle) -> Result<String, String> {
    app.clipboard().read_text().map_err(|e| e.to_string())
}

#[tauri::command]
fn ecrire_presse_papiers(app: AppHandle, texte: String) -> Result<(), String> {
    app.clipboard().write_text(texte).map_err(|e| e.to_string())
}

/// Recopie le texte dans le presse-papiers et masque la fenêtre : il ne reste
/// qu'à coller dans l'application d'origine.
#[tauri::command]
fn copier_et_masquer(app: AppHandle, texte: String) -> Result<(), String> {
    app.clipboard()
        .write_text(texte)
        .map_err(|e| e.to_string())?;
    masquer(app)
}

#[tauri::command]
fn masquer(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(WINDOW) {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Fichiers de l'application

/// Dossiers de l'application.
pub(crate) struct Dirs {
    /// Réglages et dictionnaire personnel.
    pub(crate) config: PathBuf,
    /// Brouillon.
    data: PathBuf,
    /// Journal.
    logs: PathBuf,
}

impl Dirs {
    fn new(app: &AppHandle) -> tauri::Result<Dirs> {
        let path = app.path();
        Ok(Dirs {
            config: path.app_config_dir()?,
            data: path.app_data_dir()?,
            logs: path.app_log_dir()?,
        })
    }

    pub(crate) fn log(&self, message: &str) {
        stockage::log(&self.logs, message);
    }
}

/// Vrai si le raccourci global a pu être enregistré.
struct ShortcutStatus(AtomicBool);

/// Erreur journalisée puis renvoyée à l'interface.
fn logged<T>(dirs: &Dirs, what: &str, r: std::io::Result<T>) -> Result<T, String> {
    r.map_err(|e| {
        let msg = format!("{what} : {}", stockage::describe(&e));
        dirs.log(&msg);
        msg
    })
}

// ---------------------------------------------------------------------------
// Réglages et brouillon

/// Réglages enregistrés, ou `null` au premier lancement.
#[tauri::command]
fn lire_reglages(dirs: tauri::State<Dirs>) -> Result<Option<Value>, String> {
    logged(
        &dirs,
        "lecture des réglages",
        stockage::read_settings(&dirs.config),
    )
}

#[tauri::command]
fn ecrire_reglages(dirs: tauri::State<Dirs>, reglages: Value) -> Result<(), String> {
    logged(
        &dirs,
        "enregistrement des réglages",
        stockage::write_settings(&dirs.config, reglages),
    )
}

/// Brouillon enregistré par l'interface (JSON), ou `null`.
#[tauri::command]
fn lire_brouillon(dirs: tauri::State<Dirs>) -> Option<RawJson> {
    stockage::read_draft(&dirs.data).and_then(|s| RawValue::from_string(s).ok())
}

#[tauri::command]
fn ecrire_brouillon(dirs: tauri::State<Dirs>, brouillon: String) -> Result<(), String> {
    logged(
        &dirs,
        "enregistrement du brouillon",
        stockage::write_draft(&dirs.data, &brouillon),
    )
}

// ---------------------------------------------------------------------------
// Documents

#[derive(Serialize)]
struct DocumentJs {
    chemin: String,
    nom: String,
    texte: String,
    crlf: bool,
    encodage: Encoding,
}

#[derive(Serialize)]
struct SavedJs {
    chemin: String,
    nom: String,
    encodage: Encoding,
}

const TEXT_EXTENSIONS: &[&str] = &["txt", "md", "markdown", "text"];

/// Ouvre un document texte : `chemin` donné (fichier déposé, brouillon), ou
/// choisi dans la fenêtre d'ouverture. `null` si l'utilisateur annule.
#[tauri::command]
async fn ouvrir_document(
    app: AppHandle,
    window: WebviewWindow,
    chemin: Option<String>,
) -> Result<Option<DocumentJs>, String> {
    let path = match chemin {
        Some(c) => PathBuf::from(c),
        None => {
            let picked = app
                .dialog()
                .file()
                .set_title("Ouvrir un texte")
                .add_filter("Texte", TEXT_EXTENSIONS)
                .set_parent(&window)
                .blocking_pick_file();
            match picked {
                Some(p) => p.into_path().map_err(|e| e.to_string())?,
                None => return Ok(None),
            }
        }
    };
    let f = stockage::read_document(&path).map_err(|e| {
        format!(
            "Impossible d'ouvrir « {} » : {e}",
            stockage::file_name(&path)
        )
    })?;
    Ok(Some(DocumentJs {
        chemin: path.to_string_lossy().into_owned(),
        nom: stockage::file_name(&path),
        texte: f.text,
        crlf: f.crlf,
        encodage: f.encoding,
    }))
}

/// Enregistre le document ; sans `chemin`, demande où. `null` si l'utilisateur annule.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn enregistrer_document(
    app: AppHandle,
    window: WebviewWindow,
    dirs: tauri::State<'_, Dirs>,
    chemin: Option<String>,
    nom: Option<String>,
    texte: String,
    crlf: bool,
    encodage: Encoding,
) -> Result<Option<SavedJs>, String> {
    let path = match chemin {
        Some(c) => PathBuf::from(c),
        None => {
            let picked = app
                .dialog()
                .file()
                .set_title("Enregistrer le texte")
                .add_filter("Texte", TEXT_EXTENSIONS)
                .set_file_name(nom.unwrap_or_else(|| "Sans titre.txt".into()))
                .set_parent(&window)
                .blocking_save_file();
            match picked {
                Some(p) => p.into_path().map_err(|e| e.to_string())?,
                None => return Ok(None),
            }
        }
    };
    let used = stockage::write_document(&path, &texte, crlf, encodage).map_err(|e| {
        let msg = format!(
            "Impossible d'enregistrer « {} » : {e}",
            stockage::file_name(&path)
        );
        dirs.log(&msg);
        msg
    })?;
    Ok(Some(SavedJs {
        chemin: path.to_string_lossy().into_owned(),
        nom: stockage::file_name(&path),
        encodage: used,
    }))
}

/// Demande une confirmation (fenêtre native). Vrai si l'utilisateur accepte.
#[tauri::command]
async fn confirmer(
    app: AppHandle,
    window: WebviewWindow,
    titre: String,
    message: String,
    ok: String,
) -> bool {
    app.dialog()
        .message(message)
        .title(titre)
        .kind(MessageDialogKind::Warning)
        .parent(&window)
        .buttons(MessageDialogButtons::OkCancelCustom(ok, "Annuler".into()))
        .blocking_show()
}

#[tauri::command]
fn titre(window: WebviewWindow, titre: String) -> Result<(), String> {
    window.set_title(&titre).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Application

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfoJs {
    version: String,
    raccourci: bool,
    demarrage_auto: bool,
    dossier_reglages: String,
    mises_a_jour_auto: bool,
    /// Mise à jour trouvée et pas encore installée.
    mise_a_jour: Option<UpdateJs>,
}

#[tauri::command]
fn infos_application(
    app: AppHandle,
    dirs: tauri::State<Dirs>,
    shortcut: tauri::State<ShortcutStatus>,
    updates: tauri::State<'_, mises_a_jour::Pending>,
) -> AppInfoJs {
    AppInfoJs {
        version: app.package_info().version.to_string(),
        raccourci: shortcut.0.load(Ordering::Relaxed),
        demarrage_auto: app.autolaunch().is_enabled().unwrap_or(false),
        dossier_reglages: dirs.config.to_string_lossy().into_owned(),
        mises_a_jour_auto: stockage::read_app_settings(&dirs.config).mises_a_jour_auto,
        mise_a_jour: updates.info(),
    }
}

#[tauri::command]
fn regler_demarrage_auto(
    app: AppHandle,
    dirs: tauri::State<Dirs>,
    actif: bool,
) -> Result<(), String> {
    let manager = app.autolaunch();
    let r = if actif {
        manager.enable()
    } else {
        manager.disable()
    };
    r.map_err(|e| {
        let msg = format!("lancement au démarrage : {e}");
        dirs.log(&msg);
        msg
    })
}

/// Montre le dossier des réglages dans le Finder ou l'Explorateur.
#[tauri::command]
fn afficher_reglages(dirs: tauri::State<Dirs>) -> Result<(), String> {
    std::fs::create_dir_all(&dirs.config).map_err(|e| e.to_string())?;
    reveal(&dirs.config).map_err(|e| e.to_string())
}

fn reveal(dir: &Path) -> std::io::Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(dir)
        .spawn()
        .map(|_| ())
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Ouvre la fenêtre et y envoie le texte du presse-papiers.
fn correct_clipboard(app: &AppHandle) {
    show_window(app);
    match app.clipboard().read_text() {
        Ok(text) => {
            let _ = app.emit_to(WINDOW, CLIPBOARD_EVENT, text);
        }
        Err(e) => app
            .state::<Dirs>()
            .log(&format!("presse-papiers illisible : {e}")),
    }
}

/// ⌘⇧E sur macOS, Ctrl+Maj+E ailleurs.
fn clipboard_shortcut() -> Shortcut {
    let modifier = if cfg!(target_os = "macos") {
        Modifiers::SUPER
    } else {
        Modifiers::CONTROL
    };
    Shortcut::new(Some(modifier | Modifiers::SHIFT), Code::KeyE)
}

fn load_engine(app: &AppHandle) -> Result<Engine, String> {
    let dir = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("data");
    Engine::from_dir(&dir).map_err(|e| e.to_string())
}

/// Barre de menus de macOS. Sous Windows, les mêmes commandes passent par la
/// barre d'outils et les raccourcis clavier de la fenêtre.
#[cfg(target_os = "macos")]
fn app_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    use tauri::menu::{AboutMetadata, PredefinedMenuItem as P, Submenu};
    let item =
        |id: &str, text: &str, accel: &str| MenuItem::with_id(app, id, text, true, Some(accel));
    let about = AboutMetadata {
        name: Some("Correcteur".into()),
        version: Some(app.package_info().version.to_string()),
        copyright: Some("Logiciel libre sous licence GPL-3.0-or-later".into()),
        ..Default::default()
    };
    let main = Submenu::with_items(
        app,
        "Correcteur",
        true,
        &[
            &P::about(app, Some("À propos de Correcteur"), Some(about))?,
            &P::separator(app)?,
            &P::services(app, Some("Services"))?,
            &P::separator(app)?,
            &P::hide(app, Some("Masquer Correcteur"))?,
            &P::hide_others(app, Some("Masquer les autres"))?,
            &P::show_all(app, Some("Tout afficher"))?,
            &P::separator(app)?,
            &P::quit(app, Some("Quitter Correcteur"))?,
        ],
    )?;
    let file = Submenu::with_items(
        app,
        "Fichier",
        true,
        &[
            &item("nouveau", "Nouveau", "CmdOrCtrl+N")?,
            &item("ouvrir", "Ouvrir…", "CmdOrCtrl+O")?,
            &P::separator(app)?,
            &item("enregistrer", "Enregistrer", "CmdOrCtrl+S")?,
            &item("enregistrer-sous", "Enregistrer sous…", "CmdOrCtrl+Shift+S")?,
            &P::separator(app)?,
            &P::close_window(app, Some("Fermer la fenêtre"))?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Édition",
        true,
        &[
            &P::undo(app, Some("Annuler"))?,
            &P::redo(app, Some("Rétablir"))?,
            &P::separator(app)?,
            &P::cut(app, Some("Couper"))?,
            &P::copy(app, Some("Copier"))?,
            &P::paste(app, Some("Coller"))?,
            &P::select_all(app, Some("Tout sélectionner"))?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Fenêtre",
        true,
        &[
            &P::minimize(app, Some("Placer dans le Dock"))?,
            &P::maximize(app, Some("Réduire/agrandir"))?,
            &P::separator(app)?,
            &P::fullscreen(app, Some("Plein écran"))?,
        ],
    )?;
    Menu::with_items(app, &[&main, &file, &edit, &window])
}

fn main() {
    let shortcut = clipboard_shortcut();
    tauri::Builder::default()
        // En premier : un second lancement ramène la fenêtre existante.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_window(app)
        }))
        .plugin(
            // Taille et position retenues ; la visibilité, elle, dépend du lancement.
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARG]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, s, event| {
                    if *s == shortcut && event.state() == ShortcutState::Pressed {
                        correct_clipboard(app);
                    }
                })
                .build(),
        )
        .manage(EngineState(OnceLock::new()))
        .manage(ShortcutStatus(AtomicBool::new(false)))
        .manage(mises_a_jour::Pending::default())
        .invoke_handler(tauri::generate_handler![
            analyser,
            infos,
            conjuguer,
            synonymes,
            lisibilite,
            lire_presse_papiers,
            ecrire_presse_papiers,
            copier_et_masquer,
            masquer,
            lire_reglages,
            ecrire_reglages,
            lire_brouillon,
            ecrire_brouillon,
            ouvrir_document,
            enregistrer_document,
            confirmer,
            titre,
            infos_application,
            regler_demarrage_auto,
            afficher_reglages,
            mises_a_jour::chercher_mise_a_jour,
            mises_a_jour::installer_mise_a_jour,
            mises_a_jour::regler_mises_a_jour_auto
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let dirs = Dirs::new(&handle)?;
            let engine = load_engine(&handle);
            if let Err(e) = &engine {
                dirs.log(&format!("moteur non chargé : {e}"));
            }
            let _ = app.state::<EngineState>().0.set(engine);

            match app.global_shortcut().register(shortcut) {
                Ok(()) => app
                    .state::<ShortcutStatus>()
                    .0
                    .store(true, Ordering::Relaxed),
                // Raccourci déjà pris par une autre application : l'appli reste
                // utilisable, et les Réglages le signalent.
                Err(e) => dirs.log(&format!("raccourci global indisponible : {e}")),
            }
            let auto_check = mises_a_jour::due(&dirs.config);
            app.manage(dirs);
            if auto_check {
                mises_a_jour::check_in_background(handle.clone());
            }

            #[cfg(target_os = "macos")]
            {
                app.set_menu(app_menu(&handle)?)?;
                app.on_menu_event(|app, event| {
                    let id = event.id.as_ref();
                    if ["nouveau", "ouvrir", "enregistrer", "enregistrer-sous"].contains(&id) {
                        show_window(app);
                        let _ = app.emit_to(WINDOW, MENU_EVENT, id);
                    }
                });
            }

            // Icône de la barre des menus / zone de notification : l'application
            // reste disponible quand la fenêtre est fermée.
            let open = MenuItem::with_id(
                app,
                "ouvrir-fenetre",
                "Ouvrir le correcteur",
                true,
                None::<&str>,
            )?;
            let clip = MenuItem::with_id(
                app,
                "presse-papiers",
                "Corriger le presse-papiers",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quitter", "Quitter", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &clip, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Correcteur")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "ouvrir-fenetre" => show_window(app),
                    "presse-papiers" => correct_clipboard(app),
                    "quitter" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // Lancé à l'ouverture de session : seulement l'icône, pas la fenêtre.
            if !std::env::args().any(|a| a == AUTOSTART_ARG) {
                show_window(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Fermer la fenêtre la masque : le raccourci global reste actif.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) => {
                if let Some(p) = paths.first() {
                    let _ = window.emit_to(WINDOW, DROP_EVENT, p.to_string_lossy());
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("échec du démarrage de l'application");
}
