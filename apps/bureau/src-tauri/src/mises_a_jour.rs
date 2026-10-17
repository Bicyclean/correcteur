//! Mises à jour de l'application.
//!
//! Chaque publication porte un fichier `latest.json` (version, notes,
//! paquets signés). La recherche le télécharge depuis GitHub ; le paquet
//! n'est installé qu'à la demande de l'utilisateur, après vérification de sa
//! signature par la clé publique de `tauri.conf.json`. Aucun texte, réglage
//! ni identifiant n'est envoyé : seule la requête HTTPS elle-même.
//!
//! Recherche automatique : coupée par défaut ; activée, au plus une fois par
//! jour, au démarrage.

use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{stockage, Dirs, WINDOW};

/// Événement envoyé à l'interface quand la recherche automatique trouve une version.
const AVAILABLE_EVENT: &str = "mise-a-jour-disponible";
/// Délai minimal entre deux recherches automatiques.
const DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Serialize)]
pub struct UpdateJs {
    version: String,
    notes: Option<String>,
}

/// Mise à jour trouvée, en attente d'installation.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

impl Pending {
    pub fn info(&self) -> Option<UpdateJs> {
        self.0.lock().ok()?.as_ref().map(info)
    }
}

fn info(u: &Update) -> UpdateJs {
    UpdateJs {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
    }
}

/// Vrai si la recherche automatique est activée et n'a pas eu lieu depuis un jour.
pub fn due(config: &Path) -> bool {
    let s = stockage::read_app_settings(config);
    s.mises_a_jour_auto && stockage::now_secs().saturating_sub(s.derniere_verification) >= DAY
}

/// Interroge le serveur ; garde la mise à jour trouvée pour l'installation.
async fn check(app: &AppHandle) -> Result<Option<UpdateJs>, String> {
    let dirs = app.state::<Dirs>();
    let result = async {
        app.updater()
            .map_err(|e| e.to_string())?
            .check()
            .await
            .map_err(|e| e.to_string())
    }
    .await;
    let mut settings = stockage::read_app_settings(&dirs.config);
    settings.derniere_verification = stockage::now_secs();
    if let Err(e) = stockage::write_app_settings(&dirs.config, &settings) {
        dirs.log(&format!("réglages de l'application : {e}"));
    }
    let update = result.map_err(|e| {
        dirs.log(&format!("recherche de mise à jour : {e}"));
        format!("Recherche impossible : {e}")
    })?;
    let found = update.as_ref().map(info);
    if let Ok(mut pending) = app.state::<Pending>().0.lock() {
        *pending = update;
    }
    Ok(found)
}

/// Recherche automatique au démarrage ; l'interface est prévenue si une version est trouvée.
pub fn check_in_background(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Ok(Some(found)) = check(&app).await {
            let _ = app.emit_to(WINDOW, AVAILABLE_EVENT, found);
        }
    });
}

/// Recherche demandée par l'utilisateur. `null` : l'application est à jour.
#[tauri::command]
pub async fn chercher_mise_a_jour(app: AppHandle) -> Result<Option<UpdateJs>, String> {
    check(&app).await
}

/// Télécharge, vérifie et installe la mise à jour trouvée, puis relance
/// l'application (sous Windows, l'installateur s'en charge).
#[tauri::command]
pub async fn installer_mise_a_jour(app: AppHandle) -> Result<(), String> {
    let update = app
        .state::<Pending>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Aucune mise à jour en attente.")?;
    if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
        let msg = format!("Installation impossible : {e}");
        app.state::<Dirs>().log(&msg);
        return Err(msg);
    }
    app.restart();
}

#[tauri::command]
pub fn regler_mises_a_jour_auto(dirs: tauri::State<Dirs>, actif: bool) -> Result<(), String> {
    let mut settings = stockage::read_app_settings(&dirs.config);
    settings.mises_a_jour_auto = actif;
    stockage::write_app_settings(&dirs.config, &settings).map_err(|e| {
        let msg = format!("réglages de l'application : {}", stockage::describe(&e));
        dirs.log(&msg);
        msg
    })
}
