//! Mots absents du lexique.

use super::{Context, RawIssue};
use crate::suggest::suggest;
use crate::text::{is_all_upper, is_apostrophe, is_capitalized};

pub const UNKNOWN: &str = "ORTH_MOT_INCONNU";

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    for w in &ctx.words {
        if w.ignored
            || w.has_digit
            || w.text.chars().count() < 2
            || ctx.data.elisions.contains(&w.lower)
        {
            continue;
        }
        // « qu’ il » : le découpage a laissé l'apostrophe hors du mot.
        if ctx.text[w.end..].starts_with(is_apostrophe)
            && ctx.data.elisions.contains(&format!("{}’", w.lower))
        {
            continue;
        }
        // Sigles (ONU, SNCF) : rarement dans un lexique, presque jamais fautifs.
        if is_all_upper(w.text) && ctx.lexicon.find(&w.norm).is_none() {
            continue;
        }
        // Noms propres probables : majuscule hors début de phrase.
        if is_capitalized(w.text) && !w.sentence_start && !ctx.options.check_proper_nouns {
            continue;
        }
        if ctx.lexicon.find(&w.norm).is_some()
            || ctx.user_words.contains(&w.norm)
            || ctx.user_words.contains(&w.lower)
        {
            continue;
        }
        let suggestions = suggest(
            ctx.lexicon,
            &ctx.data.elisions,
            &w.norm,
            ctx.options.max_suggestions,
        );
        let mut issue = RawIssue::new(w.start, w.end, UNKNOWN).arg(w.text);
        issue.suggestions = suggestions;
        out.push(issue);
    }
}
