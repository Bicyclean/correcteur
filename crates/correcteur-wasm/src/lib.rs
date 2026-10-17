//! Module WebAssembly du moteur.
//!
//! Interface volontairement minimale, sans `wasm-bindgen` : le JavaScript
//! réserve de la mémoire avec [`cr_alloc`], y écrit des chaînes UTF-8, appelle
//! une fonction, puis lit le résultat (JSON ou message d'erreur) à l'adresse
//! [`cr_output_ptr`], sur [`cr_output_len`] octets. Les fonctions renvoient 0
//! en cas de succès, 1 en cas d'erreur.
//!
//! Déroulement : `cr_add_file` pour chaque fichier de `data/`, `cr_build`,
//! puis `cr_check` autant de fois que nécessaire.

use std::cell::RefCell;

use correcteur_core::interop::{
    conjugation_to_json, issues_to_json, parse_options, readability_to_json, synonyms_to_json,
};
use correcteur_core::{Engine, EngineData};

#[derive(Default)]
struct State {
    data: EngineData,
    engine: Option<Engine>,
    output: Vec<u8>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn finish(result: Result<String, String>) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        match result {
            Ok(out) => {
                s.output = out.into_bytes();
                0
            }
            Err(e) => {
                s.output = e.into_bytes();
                1
            }
        }
    })
}

/// # Safety
/// `ptr` doit désigner `len` octets lisibles.
unsafe fn read_str(ptr: *const u8, len: usize) -> Result<String, String> {
    if len == 0 {
        return Ok(String::new());
    }
    let bytes = std::slice::from_raw_parts(ptr, len);
    String::from_utf8(bytes.to_vec()).map_err(|_| "texte UTF-8 invalide".to_string())
}

/// Réserve `len` octets et renvoie leur adresse.
#[no_mangle]
pub extern "C" fn cr_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Libère une zone obtenue par [`cr_alloc`].
///
/// # Safety
/// `ptr` et `len` doivent provenir du même appel à [`cr_alloc`].
#[no_mangle]
pub unsafe extern "C" fn cr_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
}

#[no_mangle]
pub extern "C" fn cr_output_ptr() -> *const u8 {
    STATE.with(|s| s.borrow().output.as_ptr())
}

#[no_mangle]
pub extern "C" fn cr_output_len() -> usize {
    STATE.with(|s| s.borrow().output.len())
}

/// Ajoute un fichier de données (`path` relatif à `data/`).
///
/// # Safety
/// Les pointeurs doivent désigner des zones lisibles de la longueur indiquée.
#[no_mangle]
pub unsafe extern "C" fn cr_add_file(
    path_ptr: *const u8,
    path_len: usize,
    content_ptr: *const u8,
    content_len: usize,
) -> i32 {
    let result = (|| {
        let path = read_str(path_ptr, path_len)?;
        let content = read_str(content_ptr, content_len)?;
        STATE.with(|s| {
            s.borrow_mut()
                .data
                .add_file(&path, content)
                .map_err(|e| e.to_string())
        })?;
        Ok(String::new())
    })();
    finish(result)
}

/// Construit le moteur à partir des fichiers ajoutés. Sortie : `{"forms":N}`.
#[no_mangle]
pub extern "C" fn cr_build() -> i32 {
    let result = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let data = std::mem::take(&mut s.data);
        let engine = Engine::new(data).map_err(|e| e.to_string())?;
        let out = format!("{{\"forms\":{}}}", engine.lexicon().len());
        s.engine = Some(engine);
        Ok(out)
    });
    finish(result)
}

/// Analyse un texte. Sortie : tableau JSON de problèmes (voir [`issues_to_json`]).
///
/// # Safety
/// Les pointeurs doivent désigner des zones lisibles de la longueur indiquée.
#[no_mangle]
pub unsafe extern "C" fn cr_check(
    text_ptr: *const u8,
    text_len: usize,
    options_ptr: *const u8,
    options_len: usize,
) -> i32 {
    let result = (|| {
        let text = read_str(text_ptr, text_len)?;
        let options = parse_options(&read_str(options_ptr, options_len)?)?;
        STATE.with(|s| {
            let s = s.borrow();
            let engine = s.engine.as_ref().ok_or("moteur non construit")?;
            Ok(issues_to_json(&engine.check(&text, &options)))
        })
    })();
    finish(result)
}

/// Applique `f` au moteur et à une chaîne reçue ; `null` (JSON) si `f` ne trouve rien.
///
/// # Safety
/// `ptr` doit désigner `len` octets lisibles.
unsafe fn with_engine(
    ptr: *const u8,
    len: usize,
    f: impl Fn(&Engine, &str) -> Option<String>,
) -> i32 {
    let result = (|| {
        let input = read_str(ptr, len)?;
        STATE.with(|s| {
            let s = s.borrow();
            let engine = s.engine.as_ref().ok_or("moteur non construit")?;
            Ok(f(engine, &input).unwrap_or_else(|| "null".into()))
        })
    })();
    finish(result)
}

/// Tableau de conjugaison (JSON), ou `null` si le verbe est inconnu.
///
/// # Safety
/// `ptr` doit désigner `len` octets lisibles.
#[no_mangle]
pub unsafe extern "C" fn cr_conjugate(ptr: *const u8, len: usize) -> i32 {
    with_engine(ptr, len, |e, v| {
        e.conjugate(v).map(|c| conjugation_to_json(&c))
    })
}

/// Synonymes (JSON), ou `null`.
///
/// # Safety
/// `ptr` doit désigner `len` octets lisibles.
#[no_mangle]
pub unsafe extern "C" fn cr_synonyms(ptr: *const u8, len: usize) -> i32 {
    with_engine(ptr, len, |e, w| {
        e.synonyms(w).map(|(entry, g)| synonyms_to_json(&entry, &g))
    })
}

/// Lisibilité d'un texte (JSON).
///
/// # Safety
/// `ptr` doit désigner `len` octets lisibles.
#[no_mangle]
pub unsafe extern "C" fn cr_readability(ptr: *const u8, len: usize) -> i32 {
    with_engine(ptr, len, |e, t| {
        Some(readability_to_json(&e.readability(t)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_complete() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut files = correcteur_core::RULE_FILES.to_vec();
        files.push("conjugaison/verbes.tsv");
        files.push("lexique/00-amorce.tsv");
        for f in files {
            let content = std::fs::read_to_string(root.join(f)).unwrap();
            let r = unsafe { cr_add_file(f.as_ptr(), f.len(), content.as_ptr(), content.len()) };
            assert_eq!(r, 0, "{f}");
        }
        assert_eq!(cr_build(), 0);
        let text = "😀 Bonjour!";
        let opts = "espace_fine=1";
        assert_eq!(
            unsafe { cr_check(text.as_ptr(), text.len(), opts.as_ptr(), opts.len()) },
            0
        );
        let out = STATE.with(|s| String::from_utf8(s.borrow().output.clone()).unwrap());
        let verb = "chanter";
        assert_eq!(unsafe { cr_conjugate(verb.as_ptr(), verb.len()) }, 0);
        let conj = STATE.with(|s| String::from_utf8(s.borrow().output.clone()).unwrap());
        assert!(conj.contains("\"je chante\""), "{conj}");
        let unknown = "xyzir";
        assert_eq!(unsafe { cr_conjugate(unknown.as_ptr(), unknown.len()) }, 0);
        assert_eq!(STATE.with(|s| s.borrow().output.clone()), b"null");
        // « ! » est au 11e caractère UTF-16 (l'émoji en compte deux).
        assert!(
            out.starts_with("[{\"start\":10,\"end\":11,\"rule\":\"TYPO_ESPACE_AVANT_MANQUANTE\""),
            "{out}"
        );
    }
}
