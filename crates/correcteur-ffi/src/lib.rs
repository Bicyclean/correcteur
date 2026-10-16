//! Bibliothèque native à interface C.
//!
//! ```c
//! CrEngine *cr_engine_new(const uint8_t *dir, size_t dir_len);    // NULL si erreur
//! char     *cr_check(const CrEngine *e, const uint8_t *text, size_t text_len,
//!                    const uint8_t *options, size_t options_len); // JSON, NULL si erreur
//! void      cr_string_free(char *s);
//! void      cr_engine_free(CrEngine *e);
//! const char *cr_last_error(void);  // message de la dernière erreur du fil courant
//! const char *cr_version(void);
//! ```
//!
//! Les chaînes d'entrée sont en UTF-8, passées avec leur longueur. Les
//! options suivent le format `clé=valeur` et le résultat le format JSON de
//! [`correcteur_core::interop`] ; les positions sont en unités UTF-16, comme
//! dans LibreOffice. Une chaîne renvoyée par `cr_check` se libère avec
//! `cr_string_free`, un moteur avec `cr_engine_free`.

use std::cell::RefCell;
use std::ffi::{c_char, CString};

use correcteur_core::interop::{issues_to_json, parse_options};
use correcteur_core::Engine;

/// Moteur opaque pour l'appelant.
pub struct CrEngine(Engine);

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn set_error(message: impl Into<String>) {
    let text = message.into().replace('\0', " ");
    LAST_ERROR.with(|e| *e.borrow_mut() = CString::new(text).unwrap_or_default());
}

/// # Safety
/// `ptr` doit désigner `len` octets lisibles (ou être nul si `len` vaut 0).
unsafe fn read_str(ptr: *const u8, len: usize) -> Result<String, String> {
    if len == 0 {
        return Ok(String::new());
    }
    if ptr.is_null() {
        return Err("pointeur nul".into());
    }
    let bytes = std::slice::from_raw_parts(ptr, len);
    String::from_utf8(bytes.to_vec()).map_err(|_| "texte UTF-8 invalide".to_string())
}

/// Charge le moteur depuis un dossier `data/`.
///
/// # Safety
/// `dir` doit désigner `dir_len` octets lisibles.
#[no_mangle]
pub unsafe extern "C" fn cr_engine_new(dir: *const u8, dir_len: usize) -> *mut CrEngine {
    let result =
        read_str(dir, dir_len).and_then(|d| Engine::from_dir(&d).map_err(|e| e.to_string()));
    match result {
        Ok(engine) => Box::into_raw(Box::new(CrEngine(engine))),
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

/// Analyse un texte ; renvoie un tableau JSON de problèmes.
///
/// # Safety
/// `engine` doit provenir de [`cr_engine_new`] ; les autres pointeurs doivent
/// désigner des zones lisibles de la longueur indiquée.
#[no_mangle]
pub unsafe extern "C" fn cr_check(
    engine: *const CrEngine,
    text: *const u8,
    text_len: usize,
    options: *const u8,
    options_len: usize,
) -> *mut c_char {
    let result = (|| {
        let engine = engine.as_ref().ok_or("moteur nul")?;
        let text = read_str(text, text_len)?;
        let options = parse_options(&read_str(options, options_len)?)?;
        let json = issues_to_json(&engine.0.check(&text, &options));
        CString::new(json).map_err(|_| "résultat invalide".to_string())
    })();
    match result {
        Ok(s) => s.into_raw(),
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

/// # Safety
/// `s` doit provenir de [`cr_check`] et ne pas avoir déjà été libérée.
#[no_mangle]
pub unsafe extern "C" fn cr_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// # Safety
/// `engine` doit provenir de [`cr_engine_new`] et ne pas avoir déjà été libéré.
#[no_mangle]
pub unsafe extern "C" fn cr_engine_free(engine: *mut CrEngine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

/// Message de la dernière erreur survenue sur ce fil (chaîne vide sinon).
/// Valable jusqu'au prochain appel en erreur sur le même fil.
#[no_mangle]
pub extern "C" fn cr_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn cr_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn interface_complete() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");
        let engine = unsafe { cr_engine_new(dir.as_ptr(), dir.len()) };
        assert!(!engine.is_null());
        let text = "😀 Il à faim!";
        let opts = "categorie_off=orthographe";
        let out = unsafe { cr_check(engine, text.as_ptr(), text.len(), opts.as_ptr(), opts.len()) };
        assert!(!out.is_null());
        let json = unsafe { CStr::from_ptr(out) }.to_str().unwrap().to_string();
        assert!(
            json.contains(r#""start":6,"end":7,"rule":"HOMO_A_VERBE""#),
            "{json}"
        );
        unsafe {
            cr_string_free(out);
            cr_engine_free(engine);
        }
    }

    #[test]
    fn erreurs() {
        let bad = "/dossier/inexistant";
        let engine = unsafe { cr_engine_new(bad.as_ptr(), bad.len()) };
        assert!(engine.is_null());
        let msg = unsafe { CStr::from_ptr(cr_last_error()) }
            .to_str()
            .unwrap()
            .to_string();
        assert!(msg.contains("inexistant"), "{msg}");
        let out = unsafe { cr_check(std::ptr::null(), std::ptr::null(), 0, std::ptr::null(), 0) };
        assert!(out.is_null());
        assert_eq!(
            unsafe { CStr::from_ptr(cr_version()) }.to_str().unwrap(),
            env!("CARGO_PKG_VERSION")
        );
    }
}
