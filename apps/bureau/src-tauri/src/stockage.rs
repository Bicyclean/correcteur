//! Fichiers de l'application et documents de l'utilisateur.
//!
//! - Réglages : `reglages.json` et `dictionnaire.txt` (un mot par ligne,
//!   modifiable à la main) dans le dossier de configuration.
//! - Brouillon : le texte en cours, pour le retrouver au lancement suivant.
//! - Journal : erreurs de l'application, pour le support.
//! - Réglages de l'application : mises à jour automatiques.
//! - Documents texte : décodage UTF-8 ou Windows-1252, fins de ligne gardées.
//!
//! Aucune dépendance à Tauri : tout se teste en isolation.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Map, Value};

/// Taille maximale d'un document ouvert.
pub const MAX_DOCUMENT: u64 = 10 * 1024 * 1024;
/// Au-delà, le journal est renommé en `journal.1.txt` et recommencé.
const MAX_JOURNAL: u64 = 512 * 1024;

/// Écrit un fichier en entier ou pas du tout : fichier temporaire voisin, puis
/// renommage. Une coupure pendant l'écriture laisse l'ancien contenu intact.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    // Lien symbolique : écrire la cible, sans remplacer le lien par un fichier.
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = dir.join(format!(".{name}.correcteur-tmp"));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
        fs::rename(&tmp, &path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

// ---------------------------------------------------------------------------
// Réglages

const SETTINGS: &str = "reglages.json";
const DICTIONARY: &str = "dictionnaire.txt";
const DICTIONARY_HEADER: &str = "# Dictionnaire personnel du correcteur : un mot par ligne.\n\
# Les lignes vides et celles qui commencent par # sont ignorées.\n";

/// Réglages enregistrés, avec la liste `userWords` lue dans le dictionnaire.
/// `Ok(None)` : aucun réglage enregistré (premier lancement).
pub fn read_settings(dir: &Path) -> io::Result<Option<Value>> {
    let json = read_optional(&dir.join(SETTINGS))?;
    let dict = read_optional(&dir.join(DICTIONARY))?;
    if json.is_none() && dict.is_none() {
        return Ok(None);
    }
    // Un fichier illisible vaut réglages par défaut : l'interface complète.
    let mut map = json
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default();
    let words = dict.as_deref().map(parse_dictionary).unwrap_or_default();
    map.insert(
        "userWords".into(),
        Value::Array(words.into_iter().map(Value::String).collect()),
    );
    Ok(Some(Value::Object(map)))
}

/// Enregistre les réglages ; `userWords` va dans le dictionnaire.
pub fn write_settings(dir: &Path, settings: Value) -> io::Result<()> {
    let mut map: Map<String, Value> = match settings {
        Value::Object(m) => m,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "réglages invalides",
            ))
        }
    };
    let words: Vec<String> = match map.remove("userWords") {
        Some(Value::Array(a)) => a
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    write_atomic(&dir.join(DICTIONARY), format_dictionary(&words).as_bytes())?;
    let json = serde_json::to_string_pretty(&Value::Object(map))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_atomic(&dir.join(SETTINGS), json.as_bytes())
}

fn parse_dictionary(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for line in text.lines() {
        let w = line.trim().trim_start_matches('\u{feff}');
        if !w.is_empty() && !w.starts_with('#') && !words.iter().any(|x| x == w) {
            words.push(w.to_string());
        }
    }
    words
}

fn format_dictionary(words: &[String]) -> String {
    let mut sorted: Vec<&str> = words
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty() && !w.contains('\n'))
        .collect();
    sorted.sort_by_key(|w| w.to_lowercase());
    sorted.dedup();
    let mut out = String::from(DICTIONARY_HEADER);
    for w in sorted {
        out.push_str(w);
        out.push('\n');
    }
    out
}

fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Réglages de l'application (hors correction)

const APP_SETTINGS: &str = "application.json";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Recherche des mises à jour au démarrage (une fois par jour). Coupée par défaut.
    pub mises_a_jour_auto: bool,
    /// Dernière recherche, en secondes depuis 1970.
    pub derniere_verification: u64,
}

/// Réglages de l'application ; valeurs par défaut si le fichier manque ou est illisible.
pub fn read_app_settings(dir: &Path) -> AppSettings {
    read_optional(&dir.join(APP_SETTINGS))
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_app_settings(dir: &Path, settings: &AppSettings) -> io::Result<()> {
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_atomic(&dir.join(APP_SETTINGS), json.as_bytes())
}

/// Secondes depuis 1970.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Brouillon

const DRAFT: &str = "brouillon.json";

/// Brouillon tel qu'enregistré par l'interface (JSON opaque), s'il est valide.
pub fn read_draft(dir: &Path) -> Option<String> {
    let text = read_optional(&dir.join(DRAFT)).ok()??;
    serde_json::from_str::<Value>(&text).ok()?;
    Some(text)
}

pub fn write_draft(dir: &Path, json: &str) -> io::Result<()> {
    write_atomic(&dir.join(DRAFT), json.as_bytes())
}

// ---------------------------------------------------------------------------
// Documents texte

/// Encodage d'un document, conservé à l'enregistrement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum Encoding {
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "utf-8-bom")]
    Utf8Bom,
    #[serde(rename = "windows-1252")]
    Windows1252,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TextFile {
    /// Texte avec des fins de ligne `\n`.
    pub text: String,
    /// Fins de ligne Windows dans le fichier d'origine.
    pub crlf: bool,
    pub encoding: Encoding,
}

/// Caractères 0x80–0x9F de Windows-1252 ; le reste coïncide avec Latin-1.
/// Les cinq positions non attribuées sont gardées telles quelles.
const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// Décode un fichier texte. Refuse les fichiers binaires (traitement de texte,
/// image…), reconnus à la présence d'octets nuls.
pub fn decode(bytes: &[u8]) -> Result<TextFile, String> {
    if bytes.contains(&0) {
        return Err(
            "ce fichier n'est pas du texte brut. Seuls les fichiers texte (.txt, .md…) sont pris en charge ; \
             pour un document Word, utilisez le complément Word ou copiez-collez son texte."
                .into(),
        );
    }
    let (text, encoding) = if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        (
            String::from_utf8(rest.to_vec()).map_err(|_| "texte UTF-8 invalide".to_string())?,
            Encoding::Utf8Bom,
        )
    } else {
        match std::str::from_utf8(bytes) {
            Ok(s) => (s.to_string(), Encoding::Utf8),
            Err(_) => (
                bytes
                    .iter()
                    .map(|&b| match b {
                        0x80..=0x9F => CP1252_HIGH[(b - 0x80) as usize],
                        _ => b as char,
                    })
                    .collect(),
                Encoding::Windows1252,
            ),
        }
    };
    let crlf = text.contains("\r\n");
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    Ok(TextFile {
        text,
        crlf,
        encoding,
    })
}

/// Encode un document. Un texte que Windows-1252 ne peut pas représenter
/// (espace fine insécable, emoji…) est enregistré en UTF-8 : l'encodage
/// réellement utilisé est renvoyé.
pub fn encode(text: &str, crlf: bool, encoding: Encoding) -> (Vec<u8>, Encoding) {
    let text = if crlf {
        text.replace('\n', "\r\n")
    } else {
        text.to_string()
    };
    match encoding {
        Encoding::Utf8 => (text.into_bytes(), Encoding::Utf8),
        Encoding::Utf8Bom => {
            let mut out = b"\xEF\xBB\xBF".to_vec();
            out.extend_from_slice(text.as_bytes());
            (out, Encoding::Utf8Bom)
        }
        Encoding::Windows1252 => {
            let bytes: Option<Vec<u8>> = text
                .chars()
                .map(|c| match c as u32 {
                    0..=0x7F | 0xA0..=0xFF => Some(c as u8),
                    _ => CP1252_HIGH
                        .iter()
                        .position(|&x| x == c)
                        .map(|i| 0x80 + i as u8),
                })
                .collect();
            match bytes {
                Some(b) => (b, Encoding::Windows1252),
                None => (text.into_bytes(), Encoding::Utf8),
            }
        }
    }
}

pub fn read_document(path: &Path) -> Result<TextFile, String> {
    let meta = fs::metadata(path).map_err(|e| describe(&e))?;
    if meta.is_dir() {
        return Err("c'est un dossier".into());
    }
    if meta.len() > MAX_DOCUMENT {
        return Err(format!(
            "fichier trop volumineux ({} Mo, au plus {} Mo)",
            meta.len() / (1024 * 1024),
            MAX_DOCUMENT / (1024 * 1024)
        ));
    }
    decode(&fs::read(path).map_err(|e| describe(&e))?)
}

pub fn write_document(
    path: &Path,
    text: &str,
    crlf: bool,
    encoding: Encoding,
) -> Result<Encoding, String> {
    let (bytes, used) = encode(text, crlf, encoding);
    write_atomic(path, &bytes).map_err(|e| describe(&e))?;
    Ok(used)
}

/// Message d'erreur de fichier compréhensible.
pub fn describe(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::NotFound => "fichier introuvable".into(),
        io::ErrorKind::PermissionDenied => "accès refusé".into(),
        _ => e.to_string(),
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// Journal

/// Ajoute une ligne horodatée (UTC) au journal. Ne fait jamais échouer l'appelant.
pub fn log(dir: &Path, message: &str) {
    let path = dir.join("journal.txt");
    let _ = (|| -> io::Result<()> {
        fs::create_dir_all(dir)?;
        if fs::metadata(&path)
            .map(|m| m.len() > MAX_JOURNAL)
            .unwrap_or(false)
        {
            fs::rename(&path, dir.join("journal.1.txt"))?;
        }
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(f, "{} {message}", timestamp(SystemTime::now()))
    })();
    #[cfg(debug_assertions)]
    eprintln!("{message}");
}

/// `AAAA-MM-JJ HH:MM:SS` en temps universel.
fn timestamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // Jours depuis 1970 → date civile (algorithme de H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    /// Dossier temporaire propre à un test.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("correcteur-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reglages_et_dictionnaire() {
        let dir = scratch("reglages");
        assert_eq!(read_settings(&dir).unwrap(), None, "premier lancement");
        let settings = serde_json::json!({
            "narrowNbsp": false,
            "userWords": ["Tauri", "zzz", "Tauri", " ", "abc"],
        });
        write_settings(&dir, settings).unwrap();
        let dict = fs::read_to_string(dir.join(DICTIONARY)).unwrap();
        assert!(dict.starts_with('#'));
        assert!(
            dict.ends_with("abc\nTauri\nzzz\n"),
            "trié, sans doublon : {dict}"
        );
        let back = read_settings(&dir).unwrap().unwrap();
        assert_eq!(back["narrowNbsp"], false);
        assert_eq!(
            back["userWords"],
            serde_json::json!(["abc", "Tauri", "zzz"])
        );

        // Dictionnaire modifié à la main, réglages corrompus.
        fs::write(
            dir.join(DICTIONARY),
            "\u{feff}mot\r\n# commentaire\n\nautre\n",
        )
        .unwrap();
        fs::write(dir.join(SETTINGS), "{pas du json").unwrap();
        let back = read_settings(&dir).unwrap().unwrap();
        assert_eq!(back, serde_json::json!({ "userWords": ["mot", "autre"] }));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reglages_application() {
        let dir = scratch("application");
        assert_eq!(
            read_app_settings(&dir),
            AppSettings::default(),
            "coupé par défaut"
        );
        let s = AppSettings {
            mises_a_jour_auto: true,
            derniere_verification: 42,
        };
        write_app_settings(&dir, &s).unwrap();
        assert_eq!(read_app_settings(&dir), s);
        fs::write(dir.join(APP_SETTINGS), "{\"misesAJourAuto\": true}").unwrap();
        assert!(
            read_app_settings(&dir).mises_a_jour_auto,
            "champ manquant : valeur par défaut"
        );
        fs::write(dir.join(APP_SETTINGS), "illisible").unwrap();
        assert_eq!(read_app_settings(&dir), AppSettings::default());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn brouillon() {
        let dir = scratch("brouillon");
        assert_eq!(read_draft(&dir), None);
        write_draft(&dir, r#"{"texte":"Bonjour"}"#).unwrap();
        assert_eq!(read_draft(&dir).as_deref(), Some(r#"{"texte":"Bonjour"}"#));
        write_draft(&dir, "tronqué {").unwrap();
        assert_eq!(read_draft(&dir), None, "brouillon invalide ignoré");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn decodage() {
        let f = decode("Été\r\nhiver\r\n".as_bytes()).unwrap();
        assert_eq!(
            f,
            TextFile {
                text: "Été\nhiver\n".into(),
                crlf: true,
                encoding: Encoding::Utf8
            }
        );
        let f = decode(b"\xEF\xBB\xBFa\rb").unwrap();
        assert_eq!(
            (f.text.as_str(), f.crlf, f.encoding),
            ("a\nb", false, Encoding::Utf8Bom)
        );
        // « Œuvre – été … » en Windows-1252.
        let f = decode(b"\xABx\xBB \x8Cuvre \x96 \xE9t\xE9 \x85").unwrap();
        assert_eq!(f.text, "«x» Œuvre – été …");
        assert_eq!(f.encoding, Encoding::Windows1252);
        assert!(decode(b"PK\x03\x04\x00\x00").is_err(), "binaire refusé");
    }

    #[test]
    fn encodage() {
        assert_eq!(encode("a\nb", true, Encoding::Utf8).0, b"a\r\nb");
        assert_eq!(
            encode("é", false, Encoding::Utf8Bom).0,
            b"\xEF\xBB\xBF\xC3\xA9"
        );
        assert_eq!(
            encode("Œuvre – été", false, Encoding::Windows1252),
            (b"\x8Cuvre \x96 \xE9t\xE9".to_vec(), Encoding::Windows1252)
        );
        // L'espace fine insécable n'existe pas en Windows-1252 : UTF-8.
        let (bytes, used) = encode("a\u{202f}!", false, Encoding::Windows1252);
        assert_eq!(used, Encoding::Utf8);
        assert_eq!(String::from_utf8(bytes).unwrap(), "a\u{202f}!");
        // Aller-retour sur tous les octets représentables.
        let all: Vec<u8> = (1..=255u8).collect();
        let f = decode(&all).unwrap();
        let (back, _) = encode(&f.text, false, f.encoding);
        let expected: Vec<u8> = all
            .iter()
            .map(|&b| if b == b'\r' { b'\n' } else { b })
            .collect();
        assert_eq!(back, expected);
    }

    #[test]
    fn documents() {
        let dir = scratch("documents");
        let path = dir.join("lettre.txt");
        write_document(&path, "Bonjour\nà tous", true, Encoding::Utf8).unwrap();
        assert_eq!(fs::read(&path).unwrap(), "Bonjour\r\nà tous".as_bytes());
        let f = read_document(&path).unwrap();
        assert!(f.crlf);
        assert_eq!(f.text, "Bonjour\nà tous");
        assert!(read_document(&dir.join("absent.txt"))
            .unwrap_err()
            .contains("introuvable"));
        assert!(read_document(&dir).is_err());
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "aucun fichier temporaire laissé"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn journal() {
        let dir = scratch("journal");
        log(&dir, "premier");
        log(&dir, "second");
        let text = fs::read_to_string(dir.join("journal.txt")).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.lines().nth(1).unwrap().ends_with(" second"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn horodatage() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01 00:00:00");
        let t = UNIX_EPOCH + Duration::from_secs(1_709_210_096); // 29 février 2024
        assert_eq!(timestamp(t), "2024-02-29 12:34:56");
    }
}
