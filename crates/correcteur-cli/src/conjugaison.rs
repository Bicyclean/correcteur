//! Rattachement des verbes du lexique aux modèles de conjugaison.
//!
//! Pour chaque verbe, chaque modèle de terminaison compatible engendre toutes
//! les formes simples ; on retient celui qui ne contredit aucune forme
//! attestée dans le lexique et en reproduit le plus. Une contradiction est une
//! forme attestée pour un temps et une personne où le modèle produit autre chose.

use std::collections::{BTreeMap, HashMap, HashSet};

use correcteur_core::conjugation::{traits_forms, Models};
use correcteur_core::{Lexicon, Pos};

pub struct Assignment {
    pub verb: String,
    pub model: String,
    pub matches: usize,
    pub contradictions: usize,
    /// Lectures contredites par le modèle : (forme, traits tels que dans le lexique).
    pub contradicted: Vec<(String, String)>,
}

/// Formes attestées d'un verbe : traits normalisés → (forme, traits d'origine).
fn attested(lexicon: &Lexicon, verb: &str) -> HashMap<String, HashSet<(String, String)>> {
    let mut out: HashMap<String, HashSet<(String, String)>> = HashMap::new();
    for (text, r) in lexicon.lemma_forms(verb) {
        if !matches!(r.pos, Pos::Ver | Pos::Aux) {
            continue;
        }
        // Lexique note parfois le participe sans genre ni nombre : masculin singulier.
        let traits = match r.traits.as_str() {
            "ppas" | "ppas.e.e" | "ppas.e.s" | "ppas.m.e" => "ppas.m.s".to_string(),
            t => t.to_string(),
        };
        out.entry(traits)
            .or_default()
            .insert((text.to_string(), r.traits.clone()));
    }
    out
}

pub fn assign(lexicon: &Lexicon, models: &Models) -> Vec<Assignment> {
    let mut verbs: Vec<String> = lexicon
        .forms()
        .filter(|f| f.has(|r| r.pos.is_verb() && r.traits == "inf" && r.lemma == f.text))
        .map(|f| f.text.clone())
        .collect();
    verbs.sort();
    verbs.dedup();

    let mut out = Vec::new();
    for verb in verbs {
        let seen = attested(lexicon, &verb);
        let mut best: Option<Assignment> = None;
        for model in models.candidates(&verb) {
            let Some(forms) = model.conjugate(&verb) else {
                continue;
            };
            let mut generated: HashMap<String, HashSet<String>> = HashMap::new();
            for (t, f) in traits_forms(&forms) {
                generated.entry(t).or_default().insert(f);
            }
            let (mut matches, mut contradicted) = (0, Vec::new());
            for (traits, texts) in &seen {
                let Some(gen) = generated.get(traits) else {
                    continue;
                };
                for (t, original) in texts {
                    if gen.contains(t) {
                        matches += 1;
                    } else {
                        contradicted.push((t.clone(), original.clone()));
                    }
                }
            }
            contradicted.sort();
            let contradictions = contradicted.len();
            let better = best.as_ref().is_none_or(|b| {
                (contradictions, std::cmp::Reverse(matches))
                    < (b.contradictions, std::cmp::Reverse(b.matches))
            });
            if better {
                best = Some(Assignment {
                    verb: verb.clone(),
                    model: model.name.clone(),
                    matches,
                    contradictions,
                    contradicted,
                });
            }
        }
        if let Some(b) = best {
            out.push(b);
        }
    }
    out
}

/// Fichier `verbes.tsv` et résumé.
pub fn render(assignments: &[Assignment]) -> (String, String) {
    let mut text = String::from(
        "# Verbes et modèle de conjugaison : verbe, modèle, formes attestées reproduites, formes contredites.\n\
         # Généré par `correcteur attribuer-conjugaisons` à partir de Lexique 3.83 (CC BY-SA 4.0) :\n\
         # ne pas modifier à la main. Ce fichier dérivé est distribué sous la même licence.\n",
    );
    let mut per_model: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut total_m, mut total_c, mut clean) = (0, 0, 0);
    for a in assignments {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            a.verb, a.model, a.matches, a.contradictions
        ));
        *per_model.entry(a.model.as_str()).or_default() += 1;
        total_m += a.matches;
        total_c += a.contradictions;
        clean += usize::from(a.contradictions == 0);
    }
    let summary = format!(
        "{} verbes rattachés à {} modèles ; {} sans aucune contradiction ({:.1} %) ; \
         formes attestées reproduites : {} sur {} ({:.2} %)",
        assignments.len(),
        per_model.len(),
        clean,
        100.0 * clean as f64 / assignments.len().max(1) as f64,
        total_m,
        total_m + total_c,
        100.0 * total_m as f64 / (total_m + total_c).max(1) as f64,
    );
    (text, summary)
}
