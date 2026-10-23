//! Locutions fautives reconnues mot à mot (`data/regles/locutions.tsv`).

use super::{Context, RawIssue};
use crate::data::LocToken;
use crate::text::{capitalize, is_capitalized};

/// Expressions figées : pléonasmes, confusions, mauvais assemblages.
pub const EXPRESSION: &str = "GRAM_EXPRESSION";
/// Forme verbale inventée par analogie (« vous faisez »).
pub const FORM: &str = "ORTH_FORME";

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    let words = &ctx.words;
    let empty: Vec<usize> = Vec::new();
    for i in 0..words.len() {
        let by_word = ctx
            .data
            .locution_by_first
            .get(&words[i].lower)
            .unwrap_or(&empty);
        for loc in by_word
            .iter()
            .chain(ctx.data.locution_lemma_first.iter())
            .map(|&k| &ctx.data.locutions[k])
        {
            let n = loc.words.len();
            if i + n > words.len() {
                continue;
            }
            let matches = loc.words.iter().enumerate().all(|(k, expected)| {
                let w = &words[i + k];
                let same = match expected {
                    LocToken::Word(e) => w.lower == *e,
                    LocToken::Lemma(l) => ctx.form(i + k).is_some_and(|f| f.has(|r| r.lemma == *l)),
                };
                same && !w.ignored && (k == 0 || w.joined_to_prev)
            });
            if !matches {
                continue;
            }
            let (start, end) = (words[i].start, words[i + n - 1].end);
            // Déjà signalé par une autre règle d'homophone : elle est plus précise.
            // Un signalement de la même règle sur un mot de la locution cède, lui,
            // la place à la locution entière (« a » dans « a cause de »).
            if out.iter().any(|o| {
                o.rule.starts_with("HOMO_") && o.rule != loc.rule && o.start < end && start < o.end
            }) {
                continue;
            }
            let found = &ctx.text[start..end];
            // « $1 », « $2 »… : texte du mot trouvé à cette place (de droite à gauche
            // pour que « $1 » ne remplace pas le début de « $12 »).
            let mut replacement = loc.replacement.clone();
            for k in (0..n).rev() {
                replacement = replacement.replace(&format!("${}", k + 1), words[i + k].text);
            }
            if is_capitalized(found) {
                replacement = capitalize(&replacement);
            }
            // La locution entière remplace le signalement d'un de ses mots par la
            // même règle (« a » dans « a cause de »).
            out.retain(|o| !(o.rule == loc.rule && o.start >= start && o.end <= end));
            // « un vis à vis » : l'alerte de genre sur une locution mal écrite cède.
            out.retain(|o| {
                !(o.rule == "GRAM_ACCORD_DETERMINANT" && o.start < end && start < o.end)
            });
            // Un mot inconnu remplacé par sa correction connue : la correction prime.
            out.retain(|o| !(o.rule == "ORTH_MOT_INCONNU" && o.start >= start && o.end <= end));
            out.push(
                RawIssue::new(start, end, loc.rule)
                    .arg(found)
                    .arg(replacement.clone())
                    .suggest(replacement),
            );
        }
    }
}
