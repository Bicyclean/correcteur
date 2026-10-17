//! Application de bureau du correcteur.
//!
//! Le moteur est la bibliothèque Rust native, chargée au démarrage à partir des
//! données incluses dans l'application. Un raccourci global ouvre la fenêtre
//! avec le contenu du presse-papiers ; le texte corrigé y est recopié à la
//! demande. Aucun accès réseau.

// Pas de console sous Windows en version publiée.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::OnceLock;

use correcteur_core::interop;
use correcteur_core::{Category, Engine, Issue, Options};
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const WINDOW: &str = "principale";
/// Événement envoyé à l'interface avec le texte du presse-papiers.
const CLIPBOARD_EVENT: &str = "texte-presse-papiers";

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
    Ok(RawJson(engine.conjugate(&verbe).map_or_else(
        || "null".into(),
        |c| interop::conjugation_to_json(&c),
    )))
}

/// Synonymes et antonymes (JSON du moteur), ou `null`.
#[tauri::command]
fn synonymes(state: tauri::State<EngineState>, mot: String) -> Result<RawJson, String> {
    let engine = state.get()?;
    Ok(RawJson(
        interop::lookup_to_json(engine, &mot).unwrap_or_else(|| "null".into()),
    ))
}

#[tauri::command]
fn lisibilite(state: tauri::State<EngineState>, texte: String) -> Result<RawJson, String> {
    Ok(RawJson(interop::readability_to_json(
        &state.get()?.readability(&texte),
    )))
}

/// JSON déjà produit par le moteur, renvoyé tel quel à l'interface.
struct RawJson(String);

impl Serialize for RawJson {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Relu en valeur générique pour être transmis comme objet, pas comme chaîne.
        let value: serde_json::Value =
            serde_json::from_str(&self.0).map_err(serde::ser::Error::custom)?;
        value.serialize(s)
    }
}

/// Nombre de formes du lexique.
#[tauri::command]
fn infos(state: tauri::State<EngineState>) -> Result<usize, String> {
    Ok(state.get()?.lexicon().len())
}

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
    if let Some(w) = app.get_webview_window(WINDOW) {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn masquer(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(WINDOW) {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
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
        Err(e) => eprintln!("presse-papiers illisible : {e}"),
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

fn main() {
    let shortcut = clipboard_shortcut();
    tauri::Builder::default()
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
        .invoke_handler(tauri::generate_handler![
            analyser,
            infos,
            lire_presse_papiers,
            ecrire_presse_papiers,
            copier_et_masquer,
            masquer,
            conjuguer,
            synonymes,
            lisibilite
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let engine = load_engine(&handle);
            if let Err(e) = &engine {
                eprintln!("moteur non chargé : {e}");
            }
            let _ = app.state::<EngineState>().0.set(engine);

            if let Err(e) = app.global_shortcut().register(shortcut) {
                // Raccourci déjà pris par une autre application : l'appli reste utilisable.
                eprintln!("raccourci global indisponible : {e}");
            }

            // Icône de la barre des menus / zone de notification : l'application
            // reste disponible quand la fenêtre est fermée.
            let open =
                MenuItem::with_id(app, "ouvrir", "Ouvrir le correcteur", true, None::<&str>)?;
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
                    "ouvrir" => show_window(app),
                    "presse-papiers" => correct_clipboard(app),
                    "quitter" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fermer la fenêtre la masque : le raccourci global reste actif.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("échec du démarrage de l'application");
}
