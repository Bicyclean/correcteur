//! Extraction des antonymes du Wiktionnaire français (CC BY-SA 4.0).
//!
//! L'export XML est lu ligne à ligne, en flux. Dans chaque page, seule la
//! section de langue française (`== {{langue|fr}} ==`) est examinée ; dans
//! celle-ci, les sections `{{S|antonymes}}` donnent des listes `* [[mot]]`,
//! rattachées à la catégorie de la section de mot qui les contient
//! (`=== {{S|adjectif|fr}} ===`).

use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;

pub const SOURCE: &str = "WIKTIONNAIRE";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub pages: usize,
    pub entries: usize,
    pub antonyms: usize,
}

/// (mot, catégorie) → antonymes.
pub type Antonyms = BTreeMap<(String, String), BTreeSet<String>>;

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&amp;", "&")
}

/// Catégorie d'un titre de section de mot : `{{S|adjectif|fr}}` → `adjectif`.
fn section_name(line: &str) -> Option<String> {
    let inner = line.trim().trim_matches('=').trim();
    let rest = inner.strip_prefix("{{S|")?;
    let name = rest.split(['|', '}']).next()?.trim();
    Some(name.to_string())
}

/// Mots liés d'une ligne de liste : `[[mot]]`, `[[mot|affiché]]`, `{{lien|mot|fr}}`.
fn linked_words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find("[[") {
        let after = &rest[i + 2..];
        let Some(end) = after.find("]]") else { break };
        let target = after[..end]
            .split('|')
            .next()
            .unwrap_or("")
            .split('#')
            .next()
            .unwrap_or("");
        out.push(target.trim().to_string());
        rest = &after[end + 2..];
    }
    let mut rest = line;
    while let Some(i) = rest.find("{{lien|") {
        let after = &rest[i + 7..];
        let Some(end) = after.find("}}") else { break };
        let target = after[..end].split('|').next().unwrap_or("");
        out.push(target.trim().to_string());
        rest = &after[end + 2..];
    }
    out.retain(|w| {
        !w.is_empty()
            && !w.contains(':')
            && w.chars().any(char::is_alphabetic)
            && w.chars().count() <= 40
    });
    out
}

/// Catégories de mots retenues, et leur nom dans le fichier produit.
fn category(section: &str) -> Option<&'static str> {
    Some(match section {
        "nom" | "nom commun" => "Nom",
        "adjectif" | "adj" => "Adjectif",
        "verbe" => "Verbe",
        "adverbe" | "adv" => "Adverbe",
        "préposition" => "Préposition",
        "locution-phrase" | "locution adverbiale" => "Locution",
        "interjection" => "Interjection",
        _ => return None,
    })
}

struct Page {
    title: String,
    namespace_main: bool,
    text: Vec<String>,
}

fn process(page: &Page, out: &mut Antonyms, report: &mut Report) {
    report.pages += 1;
    if !page.namespace_main || page.title.contains(':') {
        return;
    }
    let mut in_french = false;
    let mut current: Option<&'static str> = None;
    let mut in_antonyms = false;
    for line in &page.text {
        let t = line.trim();
        if t.starts_with("== ") || (t.starts_with("==") && !t.starts_with("===")) {
            in_french = t.contains("{{langue|fr}}");
            current = None;
            in_antonyms = false;
            continue;
        }
        if !in_french {
            continue;
        }
        if t.starts_with("===") {
            let name = section_name(t);
            in_antonyms = matches!(name.as_deref(), Some("antonymes" | "antonyme" | "ant"));
            if !in_antonyms {
                if let Some(cat) = name.as_deref().and_then(category) {
                    current = Some(cat);
                } else if t.starts_with("=== ") {
                    current = None;
                }
            }
            continue;
        }
        if in_antonyms && t.starts_with('*') {
            let Some(cat) = current else { continue };
            for w in linked_words(t) {
                if w != page.title {
                    let set = out
                        .entry((page.title.clone(), cat.to_string()))
                        .or_default();
                    if set.insert(w) {
                        report.antonyms += 1;
                    }
                }
            }
        }
    }
}

/// Lit un export XML du Wiktionnaire et en extrait les antonymes.
pub fn extract(input: impl BufRead) -> std::io::Result<(Antonyms, Report)> {
    let mut out = Antonyms::new();
    let mut report = Report::default();
    let mut page: Option<Page> = None;
    let mut in_text = false;
    for line in input.lines() {
        let line = line?;
        let t = line.trim_start();
        if t.starts_with("<page>") {
            page = Some(Page {
                title: String::new(),
                namespace_main: false,
                text: Vec::new(),
            });
            continue;
        }
        let Some(p) = page.as_mut() else { continue };
        if let Some(rest) = t.strip_prefix("<title>") {
            p.title = unescape(rest.trim_end_matches("</title>"));
        } else if t.starts_with("<ns>") {
            p.namespace_main = t == "<ns>0</ns>";
        } else if t.starts_with("<text") {
            // <text bytes="…" xml:space="preserve">première ligne
            let content = t.split_once('>').map_or("", |(_, c)| c);
            if let Some(c) = content.strip_suffix("</text>") {
                p.text.push(unescape(c));
            } else if !t.ends_with("/>") {
                p.text.push(unescape(content));
                in_text = true;
            }
        } else if in_text {
            match line.strip_suffix("</text>") {
                Some(c) => {
                    p.text.push(unescape(c));
                    in_text = false;
                }
                None => p.text.push(unescape(&line)),
            }
        } else if t.starts_with("</page>") {
            if let Some(p) = page.take() {
                process(&p, &mut out, &mut report);
            }
        }
    }
    report.entries = out.len();
    Ok((out, report))
}

/// Fichier TSV : `mot`, `catégorie`, antonymes séparés par `|`, `source`.
pub fn render(antonyms: &Antonyms) -> String {
    let mut s = String::from(
        "# Antonymes extraits du Wiktionnaire français (https://fr.wiktionary.org), contributeurs du Wiktionnaire.\n\
         # Licence : CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/) ; ce fichier dérivé est distribué\n\
         # sous la même licence. Généré par `correcteur importer-antonymes` : ne pas modifier à la main.\n\
         # Colonnes : mot, catégorie, antonymes (séparés par |), source.\n",
    );
    for ((word, cat), set) in antonyms {
        let list: Vec<&str> = set.iter().map(String::as_str).collect();
        s.push_str(&format!("{word}\t{cat}\t{}\t{SOURCE}\n", list.join("|")));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<mediawiki>
  <page>
    <title>grand</title>
    <ns>0</ns>
    <revision>
      <text bytes="10" xml:space="preserve">== {{langue|fr}} ==
=== {{S|adjectif|fr}} ===
'''grand'''
# De taille élevée.
==== {{S|antonymes}} ====
* [[petit]]
* {{lien|minuscule|fr}}, [[nain|naine]]
=== {{S|nom|fr}} ===
==== {{S|antonymes}} ====
* [[enfant]] &amp; [[petit]]
== {{langue|en}} ==
=== {{S|adjectif|en}} ===
==== {{S|antonymes}} ====
* [[small]]
</text>
    </revision>
  </page>
  <page>
    <title>Modèle:test</title>
    <ns>10</ns>
    <revision><text xml:space="preserve">== {{langue|fr}} ==</text></revision>
  </page>
</mediawiki>
"#;

    #[test]
    fn extraction() {
        let (a, report) = extract(SAMPLE.as_bytes()).unwrap();
        assert_eq!(report.pages, 2);
        let adj: Vec<&str> = a[&("grand".to_string(), "Adjectif".to_string())]
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(adj, ["minuscule", "nain", "petit"]);
        let nom: Vec<&str> = a[&("grand".to_string(), "Nom".to_string())]
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(nom, ["enfant", "petit"]);
        assert_eq!(a.len(), 2, "la section anglaise est ignorée");
        assert!(render(&a).contains("grand\tAdjectif\tminuscule|nain|petit\tWIKTIONNAIRE"));
    }
}
