//! Comportement de l'interface publique : options, positions, dictionnaire personnel.

use std::path::PathBuf;

use correcteur_core::{Category, Engine, Options};

fn engine() -> Engine {
    Engine::from_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

#[test]
fn positions_utf16() {
    let text = "Le café 😀 du chein.";
    let issues = engine().check(text, &Options::default());
    let i = issues
        .iter()
        .find(|i| i.rule == "ORTH_MOT_INCONNU")
        .unwrap();
    assert_eq!(&text[i.start..i.end], "chein");
    let utf16: Vec<u16> = text.encode_utf16().collect();
    assert_eq!(
        String::from_utf16(&utf16[i.start_utf16..i.end_utf16]).unwrap(),
        "chein"
    );
}

#[test]
fn dictionnaire_personnel() {
    let e = engine();
    let mut options = Options::default();
    assert_eq!(e.check("Le chein dort.", &options).len(), 1);
    options.user_words.push("chein".into());
    assert!(e.check("Le chein dort.", &options).is_empty());
}

#[test]
fn categories_et_regles_desactivees() {
    let e = engine();
    let text = "Le chein dort !";
    let mut options = Options::default();
    assert_eq!(e.check(text, &options).len(), 2);
    options.disabled_categories.push(Category::Typographie);
    assert_eq!(e.check(text, &options).len(), 1);
    options.disabled_categories.clear();
    options.disabled_rules.push("ORTH_MOT_INCONNU".into());
    let issues = e.check(text, &options);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].category, Category::Typographie);
}

#[test]
fn espace_insecable_normale_si_demandee() {
    let e = engine();
    let options = Options {
        narrow_nbsp: false,
        ..Options::default()
    };
    let issues = e.check("Combien?", &options);
    assert_eq!(issues[0].suggestions, ["\u{00A0}?"]);
}

#[test]
fn options_typographiques() {
    let e = engine();
    let options = Options {
        typographic_apostrophe: false,
        french_quotes: false,
        ellipsis_character: false,
        ..Options::default()
    };
    assert!(e.check("Il dit \"l'enfant dort...\"", &options).is_empty());
}

#[test]
fn messages_remplis() {
    let issues = engine().check("Le chein dort.", &Options::default());
    assert_eq!(issues[0].message, "Mot inconnu : « chein ».");
    assert!(!issues[0].explanation.contains('{'));
}

#[test]
fn texte_vide_et_blancs() {
    let e = engine();
    assert!(e.check("", &Options::default()).is_empty());
    assert!(e.check("   \n\n", &Options::default()).is_empty());
}

#[test]
fn conjugaison() {
    let e = engine();
    let c = e.conjugate("haïr").unwrap();
    let present = &c.modes[0].tenses[0].lines;
    assert_eq!(present[0], "je hais", "h aspiré");
    let c = e.conjugate("habiter").unwrap();
    assert_eq!(c.modes[0].tenses[0].lines[0], "j’habite", "h muet");
    // Forme conjuguée → infinitif.
    assert_eq!(e.conjugate("mangeons").unwrap().infinitive, "manger");
    assert_eq!(e.conjugate("allé").unwrap().infinitive, "aller");
    assert!(e.conjugate("maison").is_none());
}
