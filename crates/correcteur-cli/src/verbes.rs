//! Extraction des verbes français du Wiktionnaire (CC BY-SA 4.0).
//!
//! Une page dont la section française contient une section de verbe
//! (`=== {{S|verbe|fr}} ===`, hors flexions) donne un infinitif. Sont écartés
//! les titres en plusieurs mots ou à apostrophe (« s’en aller »), et les verbes
//! dont toutes les définitions sont marquées vieillies, archaïques, désuètes ou
//! rares : ils masqueraient des fautes de frappe sans servir.

use std::collections::BTreeMap;
use std::io::BufRead;

use crate::wiktionnaire::{read_pages, section_name, Page, SOURCE};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub pages: usize,
    pub verbs: usize,
    /// Verbes écartés parce que toutes leurs définitions sont vieillies ou rares.
    pub dated: usize,
}

/// Marques d'usage qui écartent une définition.
const DATED: &[&str] = &[
    "{{vieilli",
    "{{archaïque",
    "{{archaïsme",
    "{{désuet",
    "{{rare",
    "{{très rare",
    "{{hapax",
    "{{ancien",
];

fn infinitive(title: &str) -> bool {
    let ending = ["er", "ir", "re", "ïr"].iter().any(|e| title.ends_with(e));
    ending
        && title.chars().count() >= 3
        && title.chars().all(|c| c.is_alphabetic() || c == '-')
        && title.chars().all(|c| !c.is_uppercase())
}

/// Infinitif → groupe indiqué par le Wiktionnaire (`grp=1`), s'il l'est.
pub type Verbs = BTreeMap<String, Option<u8>>;

/// Groupe d'une ligne de forme : `{{conjugaison|fr|grp=1}}`, `{{conj|groupe=2|fr}}`.
fn group(line: &str) -> Option<u8> {
    ["grp=", "groupe="].iter().find_map(|key| {
        let i = line.find(key)?;
        line[i + key.len()..]
            .chars()
            .next()?
            .to_digit(10)
            .map(|d| d as u8)
    })
}

fn process(page: &Page, out: &mut Verbs, report: &mut Report) {
    if !page.namespace_main || !infinitive(&page.title) {
        return;
    }
    let mut in_french = false;
    let mut in_verb = false;
    let mut definitions = 0;
    let mut dated = 0;
    let mut grp = None;
    for line in &page.text {
        let t = line.trim();
        if t.starts_with("== ") || (t.starts_with("==") && !t.starts_with("===")) {
            in_french = t.contains("{{langue|fr}}");
            in_verb = false;
            continue;
        }
        if !in_french {
            continue;
        }
        if t.starts_with("===") {
            let name = section_name(t);
            // Sous-sections (synonymes, traductions…) : le verbe reste en cours.
            if t.starts_with("=== ") || t.starts_with("===") && !t.starts_with("====") {
                in_verb = name.as_deref() == Some("verbe") && !t.contains("flexion");
            }
            continue;
        }
        if in_verb && grp.is_none() {
            grp = group(t);
        }
        // Définitions : « # … », pas les exemples (« #* ») ni les sous-listes.
        if in_verb && t.starts_with('#') && !t.starts_with("#*") && !t.starts_with("#:") {
            definitions += 1;
            if DATED.iter().any(|m| t.contains(m)) {
                dated += 1;
            }
        }
    }
    if definitions == 0 {
        return;
    }
    if dated == definitions {
        report.dated += 1;
        return;
    }
    out.insert(page.title.clone(), grp);
}

/// Lit un export XML du Wiktionnaire et en extrait les infinitifs français.
pub fn extract(input: impl BufRead) -> std::io::Result<(Verbs, Report)> {
    let mut out = Verbs::new();
    let mut report = Report::default();
    report.pages = read_pages(input, |p| process(p, &mut out, &mut report))?;
    report.verbs = out.len();
    Ok((out, report))
}

/// Groupe d'un modèle : 1 pour les verbes en -er (sauf « aller »), 2 pour
/// « finir » et « haïr », 3 pour les autres.
fn model_group(name: &str, ending: &str) -> u8 {
    match name {
        "aller" => 3,
        "finir" | "haïr" => 2,
        _ if ending.ends_with("er") => 1,
        _ => 3,
    }
}

/// Modèle de chaque verbe par analogie : parmi les verbes déjà rattachés
/// (`known`, de Lexique), du même groupe quand le Wiktionnaire l'indique, ceux
/// qui partagent la plus longue terminaison avec le verbe ; le modèle le plus
/// fréquent parmi eux, s'il s'applique au verbe. Renvoie aussi les verbes
/// laissés de côté, faute de modèle.
pub fn assign(
    verbs: &Verbs,
    known: &[(String, String)],
    models: &correcteur_core::conjugation::Models,
) -> (Vec<(String, String)>, Vec<String>) {
    let ending_of = |m: &str| models.get(m).map(|x| x.ending.as_str()).unwrap_or("");
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for (verb, grp) in verbs {
        let chars: Vec<char> = verb.chars().collect();
        let mut chosen = None;
        for k in (2..chars.len()).rev() {
            let suffix: String = chars[chars.len() - k..].iter().collect();
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for (v, m) in known {
                if v.ends_with(&suffix)
                    && verb.ends_with(ending_of(m))
                    && grp.is_none_or(|g| model_group(m, ending_of(m)) == g)
                {
                    *counts.entry(m.as_str()).or_default() += 1;
                }
            }
            if let Some((m, _)) = counts
                .iter()
                .max_by_key(|(m, n)| (**n, std::cmp::Reverse(**m)))
            {
                chosen = Some(m.to_string());
                break;
            }
        }
        match chosen {
            Some(m) => out.push((verb.clone(), m)),
            None => skipped.push(verb.clone()),
        }
    }
    (out, skipped)
}

/// Fichier TSV : `verbe`, `modèle`, `source`.
pub fn render(verbs: &[(String, String)]) -> String {
    let mut s = String::from(
        "# Verbes du Wiktionnaire français (https://fr.wiktionary.org) absents de Lexique 3.83, avec le modèle de\n\
         # conjugaison du projet qui leur convient. Contributeurs du Wiktionnaire ; licence CC BY-SA 4.0\n\
         # (https://creativecommons.org/licenses/by-sa/4.0/) ; ce fichier dérivé est distribué sous la même licence.\n\
         # Généré par `correcteur importer-verbes` : ne pas modifier à la main.\n\
         # Colonnes : verbe, modèle, source.\n",
    );
    for (verb, model) in verbs {
        s.push_str(&format!("{verb}\t{model}\t{SOURCE}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<mediawiki>
  <page>
    <title>modéliser</title>
    <ns>0</ns>
    <revision>
      <text bytes="10" xml:space="preserve">== {{langue|fr}} ==
=== {{S|étymologie}} ===
: De ''[[modèle]]''.
=== {{S|verbe|fr}} ===
'''modéliser''' {{pron|mɔ.de.li.ze|fr}} {{t|fr}} {{conjugaison|fr|grp=1}}
# Établir un [[modèle]].
#* ''Exemple.''
==== {{S|synonymes}} ====
* [[modéler]]
</text>
    </revision>
  </page>
  <page>
    <title>ardoir</title>
    <ns>0</ns>
    <revision>
      <text bytes="10" xml:space="preserve">== {{langue|fr}} ==
=== {{S|verbe|fr}} ===
# {{vieilli|fr}} Brûler.
</text>
    </revision>
  </page>
  <page>
    <title>modélisent</title>
    <ns>0</ns>
    <revision>
      <text bytes="10" xml:space="preserve">== {{langue|fr}} ==
=== {{S|verbe|fr|flexion}} ===
# ''Troisième personne du pluriel du présent de'' [[modéliser]].
</text>
    </revision>
  </page>
  <page>
    <title>render</title>
    <ns>0</ns>
    <revision>
      <text bytes="10" xml:space="preserve">== {{langue|en}} ==
=== {{S|verbe|en}} ===
# Rendre.
</text>
    </revision>
  </page>
</mediawiki>
"#;

    #[test]
    fn extraction() {
        let (verbs, report) = extract(SAMPLE.as_bytes()).unwrap();
        assert_eq!(
            verbs.into_iter().collect::<Vec<_>>(),
            vec![("modéliser".to_string(), Some(1))]
        );
        assert_eq!(report.pages, 4);
        assert_eq!(report.dated, 1);
    }
}
