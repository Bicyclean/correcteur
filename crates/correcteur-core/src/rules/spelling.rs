//! Mots absents du lexique.

use std::collections::{HashMap, HashSet};

use super::grammar::subject_gn;
use super::{Context, RawIssue};
use crate::lexicon::{Gn, Pos};
use crate::suggest::{suggest, weighted_distance};
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
    for (i, w) in ctx.words.iter().enumerate() {
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
        let Some(mut suggestions) = suggestions.clone() else {
            continue;
        };
        participles_first(ctx, i, &mut suggestions);
        let mut issue = RawIssue::new(w.start, w.end, UNKNOWN).arg(w.text);
        issue.suggestions = suggestions;
        out.push(issue);
    }
}

/// Mot absent du lexique mais correct : forme conjuguée rare que les tables de
/// conjugaison produisent (« rétablît »), graphie ancienne à apostrophe
/// (« grand’mère », « entr’ouvrir »), élision restée collée (« va-t’en »), ou
/// pluriel d'un participe féminin attesté (« souvenues »).
fn known_variant(ctx: &Context, lower: &str) -> bool {
    let known = |w: &str| ctx.lexicon.find(w).is_some() || ctx.data.conjugator.is_form(w);
    if let Some((head, tail)) = lower.split_once('’') {
        let elision = format!("{head}’");
        return (ctx.data.elisions.contains(&elision) && known(tail))
            || known(&format!("{head}-{tail}"))
            || known(&format!("{head}{tail}"));
    }
    // « souvenues » : pluriel d'un participe féminin attesté (« souvenue »), que
    // Lexique n'a pas toujours. Limité aux participes : « chevals » reste une faute.
    let feminine_plural = lower.strip_suffix("es").is_some_and(|stem| {
        ctx.lexicon.find(&format!("{stem}e")).is_some_and(|f| {
            f.has(|r| r.is_past_participle() && r.gn().is_some_and(|g| g.gender == Some('f')))
        })
    });
    feminine_plural || (lower.chars().count() >= 4 && ctx.data.conjugator.is_form(lower))
}

/// Ordre des suggestions selon le mot qui précède : celles qui conviennent à la
/// place du mot passent en tête, dans leur ordre d'origine.
///
/// - Après un auxiliaire (« elles sont partient ») : participe passé accordé
///   avec le sujet (être) ou au masculin singulier (avoir), adjectif accordé,
///   préposition ou adverbe (« il était parmis » → « parmi »). Les participes
///   des verbes suggérés s'ajoutent (« partent » → « parties »).
/// - Après un pronom sujet (« nous avont ») : verbe conjugué à sa personne.
fn participles_first(ctx: &Context, i: usize, suggestions: &mut Vec<String>) {
    let Some(p) = ctx.prev(i) else { return };
    if let Some(subject) = ctx.data.subjects.get(ctx.words[p].lower.as_str()) {
        if !ctx.words[p].compound_part {
            let person = subject.person.as_str();
            let fits = |s: &String| {
                ctx.lexicon
                    .get(s)
                    .is_some_and(|f| f.has(|r| r.person() == Some(person)))
            };
            let (good, bad): (Vec<String>, Vec<String>) = suggestions.drain(..).partition(fits);
            suggestions.extend(good.into_iter().chain(bad));
        }
        return;
    }
    let mut aux = p;
    for _ in 0..2 {
        let lower = ctx.words[aux].lower.as_str();
        if ctx.data.is(lower, "adverbe") || ctx.data.is(lower, "negation") {
            match ctx.prev(aux) {
                Some(k) => aux = k,
                None => return,
            }
        }
    }
    let Some(form) = ctx.form(aux) else { return };
    let is_aux =
        |lemma: &str| form.has(|r| r.pos == Pos::Aux && r.lemma == lemma && r.is_finite_verb());
    let target = if is_aux("être") {
        subject_gn(ctx, aux).unwrap_or_default()
    } else if is_aux("avoir") {
        Gn {
            gender: Some('m'),
            number: Some('s'),
        }
    } else {
        return;
    };
    let fits = |s: &str| {
        ctx.lexicon.get(s).is_some_and(|f| {
            f.has(|r| {
                matches!(r.pos, Pos::Pre | Pos::Adv)
                    || ((r.pos == Pos::Adj || r.is_past_participle())
                        && r.gn().is_some_and(|g| g.compatible(target)))
            })
        })
    };
    // Participes des verbes suggérés, du plus proche au plus éloigné du mot écrit.
    let word = &ctx.words[i].lower;
    let mut lemmas: Vec<&str> = Vec::new();
    for s in suggestions.iter() {
        for r in ctx
            .lexicon
            .get(s)
            .map_or(&[][..], |f| f.readings.as_slice())
        {
            if r.pos.is_verb() && !lemmas.contains(&r.lemma.as_str()) {
                lemmas.push(&r.lemma);
            }
        }
    }
    let mut extra: Vec<(f32, String)> = Vec::new();
    for lemma in lemmas {
        for f in ctx.lexicon.inflect(lemma, |r| {
            r.is_past_participle() && r.gn().is_some_and(|g| g.compatible(target))
        }) {
            let d = weighted_distance(word, f);
            if d <= 3.0 && !suggestions.iter().any(|s| s == f) && !extra.iter().any(|(_, x)| x == f)
            {
                extra.push((d, f.to_string()));
            }
        }
    }
    extra.sort_by(|a, b| a.0.total_cmp(&b.0));
    let max = suggestions.len().max(ctx.options.max_suggestions.max(1));
    let (good, bad): (Vec<String>, Vec<String>) = suggestions.drain(..).partition(|s| fits(s));
    let mut merged: Vec<String> = good;
    merged.extend(extra.into_iter().map(|(_, f)| f));
    merged.extend(bad);
    merged.truncate(max);
    *suggestions = merged;
}
