//! Locutions fautives reconnues mot à mot (`data/regles/locutions.tsv`).

use super::{Context, RawIssue};
use crate::data::LocToken;
use crate::text::{capitalize, is_capitalized};

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    let words = &ctx.words;
    for i in 0..words.len() {
        for loc in &ctx.data.locutions {
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
            out.push(
                RawIssue::new(start, end, loc.rule)
                    .arg(found)
                    .arg(replacement.clone())
                    .suggest(replacement),
            );
        }
    }
}
