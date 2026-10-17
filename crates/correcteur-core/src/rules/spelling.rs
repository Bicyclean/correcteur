//! Mots absents du lexique.

use std::collections::{HashMap, HashSet};

use super::{Context, RawIssue};
use crate::suggest::suggest;
use crate::text::{is_all_upper, is_apostrophe, is_capitalized};

pub const UNKNOWN: &str = "ORTH_MOT_INCONNU";

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    // Un mot inconnu répété dans le texte n'est analysé qu'une fois.
    let mut cache: HashMap<&str, Option<Vec<String>>> = HashMap::new();
    // Noms propres du texte : mots à majuscule employés en milieu de phrase.
    let names: HashSet<&str> = ctx
        .words
        .iter()
        .filter(|w| !w.sentence_start && is_capitalized(w.text))
        .map(|w| w.text)
        .collect();
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
        // Abréviation suivie de son point (« Cf. », « env. »).
        if ctx.text[w.end..].starts_with('.') && ctx.data.is_abbreviation(&w.norm) {
            continue;
        }
        // Sigles (ONU, SNCF) : rarement dans un lexique, presque jamais fautifs.
        if is_all_upper(w.text) && ctx.lexicon.find(&w.norm).is_none() {
            continue;
        }
        // Noms propres probables : majuscule hors début de phrase, ou en début
        // de phrase quand le texte emploie le même mot ailleurs (« Raoul »).
        if is_capitalized(w.text)
            && (!w.sentence_start || names.contains(w.text))
            && !ctx.options.check_proper_nouns
        {
            continue;
        }
        if ctx.lexicon.find(&w.norm).is_some()
            || ctx.user_words.contains(&w.norm)
            || ctx.user_words.contains(&w.lower)
        {
            continue;
        }
        let suggestions = cache.entry(w.norm.as_str()).or_insert_with(|| {
            (!known_variant(ctx, &w.lower)).then(|| {
                suggest(
                    ctx.lexicon,
                    &ctx.data.elisions,
                    &w.norm,
                    ctx.options.max_suggestions,
                )
            })
        });
        let Some(suggestions) = suggestions.clone() else {
            continue;
        };
        let mut issue = RawIssue::new(w.start, w.end, UNKNOWN).arg(w.text);
        issue.suggestions = suggestions;
        out.push(issue);
    }
}

/// Mot absent du lexique mais correct : forme conjuguée rare que les tables de
/// conjugaison produisent (« rétablît »), graphie ancienne à apostrophe
/// (« grand’mère », « entr’ouvrir »), ou élision restée collée (« va-t’en »).
fn known_variant(ctx: &Context, lower: &str) -> bool {
    let known = |w: &str| ctx.lexicon.find(w).is_some() || ctx.data.conjugator.is_form(w);
    if let Some((head, tail)) = lower.split_once('’') {
        let elision = format!("{head}’");
        return (ctx.data.elisions.contains(&elision) && known(tail))
            || known(&format!("{head}-{tail}"))
            || known(&format!("{head}{tail}"));
    }
    lower.chars().count() >= 4 && ctx.data.conjugator.is_form(lower)
}
