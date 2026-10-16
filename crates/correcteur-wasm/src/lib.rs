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

use correcteur_core::{Category, Engine, EngineData, Issue, Options};

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

/// Options au format `clé=valeur`, une par ligne. Clés :
/// `categorie_off`, `regle_off`, `mot` (répétables) ; `espace_fine`,
/// `apostrophe`, `guillemets`, `points`, `noms_propres` (0 ou 1) ;
/// `suggestions_max` (nombre).
pub fn parse_options(text: &str) -> Result<Options, String> {
    let mut o = Options::default();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("option sans « = » : {line}"))?;
        let flag = || match value {
            "1" => Ok(true),
            "0" => Ok(false),
            _ => Err(format!("{key} : 0 ou 1 attendu")),
        };
        match key {
            "categorie_off" => o.disabled_categories.push(
                Category::parse(value).ok_or_else(|| format!("catégorie inconnue : {value}"))?,
            ),
            "regle_off" => o.disabled_rules.push(value.to_string()),
            "mot" => o.user_words.push(value.to_string()),
            "espace_fine" => o.narrow_nbsp = flag()?,
            "apostrophe" => o.typographic_apostrophe = flag()?,
            "guillemets" => o.french_quotes = flag()?,
            "points" => o.ellipsis_character = flag()?,
            "noms_propres" => o.check_proper_nouns = flag()?,
            "suggestions_max" => {
                o.max_suggestions = value
                    .parse()
                    .map_err(|_| format!("suggestions_max : nombre attendu, « {value} »"))?
            }
            _ => return Err(format!("option inconnue : {key}")),
        }
    }
    Ok(o)
}

/// Positions en unités UTF-16, comme les chaînes JavaScript.
pub fn issues_to_json(issues: &[Issue]) -> String {
    let mut out = String::from("[");
    for (i, issue) in issues.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"start\":{},\"end\":{},\"rule\":",
            issue.start_utf16, issue.end_utf16
        ));
        json_string(&issue.rule, &mut out);
        out.push_str(",\"category\":");
        json_string(issue.category.as_str(), &mut out);
        out.push_str(",\"message\":");
        json_string(&issue.message, &mut out);
        out.push_str(",\"explanation\":");
        json_string(&issue.explanation, &mut out);
        out.push_str(",\"suggestions\":[");
        for (k, s) in issue.suggestions.iter().enumerate() {
            if k > 0 {
                out.push(',');
            }
            json_string(s, &mut out);
        }
        out.push_str("]}");
    }
    out.push(']');
    out
}

fn json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options() {
        let o = parse_options("espace_fine=0\nregle_off=X\nmot=chein\ncategorie_off=typographie\n")
            .unwrap();
        assert!(!o.narrow_nbsp);
        assert_eq!(o.disabled_rules, ["X"]);
        assert_eq!(o.user_words, ["chein"]);
        assert_eq!(o.disabled_categories, [Category::Typographie]);
        assert!(parse_options("inconnue=1").is_err());
        assert!(parse_options("apostrophe=oui").is_err());
    }

    #[test]
    fn echappement_json() {
        let mut s = String::new();
        json_string("a\"b\\c\nd\u{1}", &mut s);
        assert_eq!(s, r#""a\"b\\c\nd\u0001""#);
    }

    #[test]
    fn interface_complete() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut files = vec![
            "regles/regles.tsv",
            "regles/locutions.tsv",
            "regles/elisions.txt",
            "regles/abreviations.txt",
            "regles/verbes_a.txt",
        ];
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
        // « ! » est au 11e caractère UTF-16 (l'émoji en compte deux).
        assert!(
            out.starts_with("[{\"start\":10,\"end\":11,\"rule\":\"TYPO_ESPACE_AVANT_MANQUANTE\""),
            "{out}"
        );
    }
}
