//! Contrôle systématique des accords sujet-verbe : pour des verbes courants de tous les
//! groupes, chaque forme de l'indicatif et du conditionnel avec son pronom ne doit
//! produire aucune alerte, et la forme d'une autre personne doit être relevée.

use std::path::PathBuf;

use correcteur_core::{Engine, Options};

const VERBS: &[&str] = &[
    "être",
    "avoir",
    "aller",
    "faire",
    "dire",
    "voir",
    "pouvoir",
    "vouloir",
    "savoir",
    "devoir",
    "venir",
    "tenir",
    "prendre",
    "mettre",
    "écrire",
    "lire",
    "boire",
    "croire",
    "recevoir",
    "vivre",
    "suivre",
    "partir",
    "sortir",
    "dormir",
    "ouvrir",
    "courir",
    "mourir",
    "connaître",
    "conduire",
    "battre",
    "rendre",
    "finir",
    "chanter",
    "manger",
    "placer",
    "appeler",
    "jeter",
    "acheter",
    "payer",
    "nettoyer",
    "espérer",
    "tuer",
    "étudier",
    "craindre",
    "résoudre",
    "vaincre",
    "asseoir",
    "envoyer",
    "croître",
];

const PRONOUNS: [&str; 6] = ["je", "tu", "il", "nous", "vous", "ils"];

/// « j’ai » → « ai » : retire le pronom d'une ligne de table.
fn strip(line: &str) -> String {
    for p in ["j’", "je ", "tu ", "il ", "nous ", "vous ", "ils "] {
        if let Some(rest) = line.strip_prefix(p) {
            return rest.split(" / ").next().unwrap_or(rest).to_string();
        }
    }
    line.to_string()
}

fn sentence(person: usize, form: &str) -> String {
    let p = PRONOUNS[person];
    let vowel = form.starts_with(|c: char| "aeiouyéèêâîôûh".contains(c));
    let s = if p == "je" && vowel {
        format!("j’{form}")
    } else {
        format!("{p} {form}")
    };
    let mut c = s.chars();
    let first = c.next().unwrap().to_uppercase().collect::<String>();
    format!("{first}{}.", c.as_str())
}

#[test]
fn accord_de_toutes_les_personnes() {
    let engine =
        Engine::from_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let options = Options::default();
    let (mut false_alarms, mut right, mut wrong, mut found) = (Vec::new(), 0, 0, 0);
    for verb in VERBS {
        let table = engine.conjugate(verb).expect("verbe connu");
        for mode in table
            .modes
            .iter()
            .filter(|m| matches!(m.name, "Indicatif" | "Conditionnel"))
        {
            for tense in &mode.tenses {
                if tense.lines.len() != 6 {
                    continue;
                }
                let forms: Vec<String> = tense.lines.iter().map(|l| strip(l)).collect();
                for i in 0..6 {
                    // « (e) » : accord facultatif dans la table, pas une graphie.
                    // « Vous dîtes » seul est relevé : au présent on écrit « dites », et le
                    // passé simple de la 2e personne est rare.
                    if forms[i].contains('(') || forms[i] == "dîtes" {
                        continue;
                    }
                    right += 1;
                    let s = sentence(i, &forms[i]);
                    let issues: Vec<_> = engine
                        .check(&s, &options)
                        .into_iter()
                        .filter(|x| !x.rule.starts_with("TYPO"))
                        .collect();
                    if !issues.is_empty() {
                        false_alarms.push(format!("{s} → {}", issues[0].rule));
                    }
                    let j = (i + 1 + (i % 3) * 2) % 6;
                    if forms[j] != forms[i] && !forms[j].contains('(') {
                        wrong += 1;
                        let s = sentence(i, &forms[j]);
                        if engine
                            .check(&s, &options)
                            .iter()
                            .any(|x| !x.rule.starts_with("TYPO"))
                        {
                            found += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(right > 1500, "trop peu de formes : {right}");
    assert!(
        false_alarms.is_empty(),
        "{} fausse(s) alerte(s) sur {right} formes justes :\n{}",
        false_alarms.len(),
        false_alarms.join("\n")
    );
    let rate = found as f64 / wrong as f64;
    assert!(rate > 0.97, "détection {found}/{wrong} = {rate:.3}");
}
