//! Format d'échange avec les interfaces non Rust (WebAssembly, bibliothèque C) :
//! options en texte `clé=valeur`, problèmes en JSON. Sans dépendance.

use crate::{Category, Issue, Options};

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
}
