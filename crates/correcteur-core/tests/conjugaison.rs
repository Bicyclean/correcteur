//! Tables de conjugaison : formes attendues de `corpus/fr/conjugaison/formes.tsv`.
//!
//! Chaque ligne donne un infinitif, un trait (`ind.ps.3s`, `imp.pres.2p`, `ppre`…)
//! et une forme que la table du verbe doit contenir.

use std::path::PathBuf;

use correcteur_core::Engine;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn formes_attendues() {
    let engine = Engine::from_dir(root().join("data")).expect("données valides");
    let content = std::fs::read_to_string(root().join("corpus/fr/conjugaison/formes.tsv")).unwrap();
    let mut failures = Vec::new();
    let mut total = 0;
    for (n, line) in content.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 3, "ligne {} mal formée : {line}", n + 1);
        total += 1;
        let forms = engine.conjugator().forms_for(cols[0], cols[1]);
        if !forms.iter().any(|f| f == cols[2]) {
            failures.push(format!(
                "{} {} : attendu « {} », obtenu {:?}",
                cols[0], cols[1], cols[2], forms
            ));
        }
    }
    assert!(total > 1000, "trop peu de formes vérifiées : {total}");
    assert!(
        failures.is_empty(),
        "{} écart(s) :\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn lines(c: &correcteur_core::Conjugation, mode: &str, tense: &str) -> Vec<String> {
    c.modes
        .iter()
        .find(|m| m.name == mode)
        .and_then(|m| m.tenses.iter().find(|t| t.name == tense))
        .map(|t| t.lines.clone())
        .unwrap_or_default()
}

#[test]
fn verbe_pronominal_avec_en() {
    let engine = Engine::from_dir(root().join("data")).expect("données valides");
    let c = engine.conjugate("s’en aller").expect("s’en aller");
    assert_eq!(c.infinitive, "s’en aller");
    assert_eq!(lines(&c, "Indicatif", "Présent")[0], "je m’en vais");
    assert_eq!(
        lines(&c, "Indicatif", "Passé composé")[3],
        "nous nous en sommes allé(e)s"
    );
    assert_eq!(
        lines(&c, "Impératif", "Présent"),
        ["va-t’en", "allons-nous-en", "allez-vous-en"]
    );
    assert_eq!(
        lines(&c, "Participe", "Passé composé"),
        ["s’en étant allé(e)"]
    );
}

#[test]
fn verbes_impersonnels_complets() {
    let engine = Engine::from_dir(root().join("data")).expect("données valides");
    let c = engine.conjugate("pleuvoir").expect("pleuvoir");
    assert_eq!(lines(&c, "Indicatif", "Imparfait"), ["il pleuvait"]);
    assert_eq!(lines(&c, "Subjonctif", "Présent"), ["qu’il pleuve"]);
    assert_eq!(lines(&c, "Subjonctif", "Imparfait"), ["qu’il plût"]);
    assert_eq!(lines(&c, "Participe", "Présent"), ["pleuvant"]);
}
