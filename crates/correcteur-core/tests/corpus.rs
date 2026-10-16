//! Tests de règles à partir du corpus annoté (`corpus/fr/regles`).
//!
//! Chaque ligne annotée est un test positif, chaque ligne sans annotation un
//! test négatif. Chaque règle doit avoir au moins un test de chaque sorte.

use std::collections::BTreeMap;
use std::path::PathBuf;

use correcteur_core::corpus::{check_rule_line, parse, CorpusLine};
use correcteur_core::{Engine, Options};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn engine() -> Engine {
    Engine::from_dir(root().join("data")).expect("données valides")
}

fn rule_lines() -> Vec<CorpusLine> {
    let dir = root().join("corpus/fr/regles");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "tsv"))
        .collect();
    paths.sort();
    paths
        .iter()
        .flat_map(|p| {
            let content = std::fs::read_to_string(p).unwrap();
            parse(&p.display().to_string(), &content).unwrap()
        })
        .collect()
}

#[test]
fn corpus_de_regles() {
    let engine = engine();
    let options = Options::default();
    let failures: Vec<String> = rule_lines()
        .iter()
        .filter_map(|l| check_rule_line(&engine, l, &options).err())
        .collect();
    assert!(
        failures.is_empty(),
        "{} échec(s) :\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn chaque_regle_a_un_test_positif_et_un_negatif() {
    let engine = engine();
    let mut seen: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for l in rule_lines() {
        assert!(
            engine.rule(&l.label).is_some(),
            "{}:{} : règle inconnue « {} »",
            l.file,
            l.line,
            l.label
        );
        let e = seen.entry(l.label.clone()).or_default();
        if l.annotations.is_empty() {
            e.1 += 1;
        } else {
            e.0 += 1;
        }
    }
    let mut missing = Vec::new();
    for rule in engine.rules() {
        let (pos, neg) = seen.get(&rule.id).copied().unwrap_or_default();
        if pos == 0 || neg == 0 {
            missing.push(format!("{} (positifs : {pos}, négatifs : {neg})", rule.id));
        }
    }
    assert!(
        missing.is_empty(),
        "tests manquants :\n{}",
        missing.join("\n")
    );
}
