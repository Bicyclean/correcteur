//! Mots absents du lexique.

use std::collections::{HashMap, HashSet};

use super::grammar::subject_gn;
use super::{Context, RawIssue};
use crate::lexicon::{skeleton, Gn, Pos};
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
        // « (fr) », « (pt) » : code de langue des bibliographies. « librarything.fr » :
        // nom de domaine.
        let before = &ctx.text[..w.start];
        let language_code = w.text.chars().count() <= 3
            && before.ends_with('(')
            && ctx.text[w.end..].starts_with(')');
        let after = &ctx.text[w.end..];
        let domain = (before.ends_with('.')
            && before[..before.len() - 1]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric))
            || (after.starts_with('.')
                && after[1..].chars().next().is_some_and(char::is_lowercase));
        if language_code || domain {
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
        agreeing_first(ctx, i, &mut suggestions);
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
    feminine_plural
        || regular_inflection(ctx, lower)
        || (lower.chars().count() >= 4 && ctx.data.conjugator.is_form(lower))
        || rectified(ctx, lower)
        || wiktionary_verb(ctx, lower)
}

/// Graphies rectifiées de 1990, correctes comme les traditionnelles :
/// - accent circonflexe facultatif sur i et u (« connait », « bruler »,
///   « chaine »), sauf dans les terminaisons du passé simple (« nous vîmes »)
///   et du subjonctif imparfait (« qu’il fût ») ; les mots où il distingue un
///   homonyme (« dû », « mûr », « sûr ») restent connus sans accent ;
/// - verbes en -eler, -eter avec « è » (« il amoncèle », « il étiquète »),
///   sauf appeler et jeter, et noms en -ement (« amoncèlement ») ;
/// - futur et conditionnel en « è » (« je cèderai ») ;
/// - mots isolés : tréma (« ambigüe »), soudures (« weekend »), etc.
fn rectified(ctx: &Context, lower: &str) -> bool {
    if ctx.data.rectified.contains(lower)
        || lower
            .strip_suffix('s')
            .is_some_and(|w| ctx.data.rectified.contains(w))
    {
        return true;
    }
    let form = |w: &str| ctx.lexicon.find(w);
    // Circonflexe : chaque i ou u peut l'avoir perdu (au plus six positions).
    let chars: Vec<char> = lower.chars().collect();
    let slots: Vec<usize> = (0..chars.len())
        .filter(|&i| matches!(chars[i], 'i' | 'u'))
        .collect();
    if !slots.is_empty() && slots.len() <= 6 {
        for mask in 1u32..(1 << slots.len()) {
            let mut v = chars.clone();
            for (b, &i) in slots.iter().enumerate() {
                if mask & (1 << b) != 0 {
                    v[i] = if v[i] == 'i' { 'î' } else { 'û' };
                }
            }
            let v: String = v.into_iter().collect();
            // « vîmes », « vîtes », « qu’il fût » : le circonflexe reste.
            let keeps = |r: &crate::lexicon::Reading| {
                r.pos.is_verb()
                    && match (r.mode_tense(), r.person()) {
                        (Some("ind.ps"), Some(p)) => p == "1p" || p == "2p",
                        (Some("sub.impf"), Some(p)) => p == "3s",
                        _ => false,
                    }
            };
            // Formes absentes du lexique (« mûrissons ») : le conjugueur, hors
            // terminaisons qui gardent l'accent (« -îmes », « -îtes », « -ît »).
            let kept_ending = ["îmes", "îtes", "ûmes", "ûtes", "ît", "ût"]
                .iter()
                .any(|e| v.ends_with(e));
            if form(&v).is_some_and(|f| f.readings.iter().any(|r| !keeps(r)))
                || (!kept_ending && form(&v).is_none() && ctx.data.conjugator.is_form(&v))
            {
                return true;
            }
        }
    }
    // -eler, -eter : « amoncèle » → « amoncelle », « étiquète » → « étiquette ».
    for (rect, trad) in [("èl", "ell"), ("èt", "ett")] {
        if let Some(i) = lower.rfind(rect) {
            let v = format!("{}{}{}", &lower[..i], trad, &lower[i + rect.len()..]);
            let ok = form(&v).is_some_and(|f| {
                f.readings.iter().any(|r| {
                    let verb = r.pos.is_verb()
                        && (r.lemma.ends_with("eler") || r.lemma.ends_with("eter"))
                        && !r.lemma.ends_with("appeler")
                        && !r.lemma.ends_with("jeter");
                    verb || (r.pos == Pos::Nom && r.lemma.ends_with("ement"))
                })
            });
            if ok {
                return true;
            }
        }
    }
    // Futur et conditionnel : « cèderai » → « céderai ».
    if let Some(i) = lower.rfind('è') {
        let v = format!("{}é{}", &lower[..i], &lower[i + 'è'.len_utf8()..]);
        if form(&v).is_some_and(|f| {
            f.has(|r| {
                r.pos.is_verb() && matches!(r.mode_tense(), Some("ind.fut") | Some("cond.pres"))
            })
        }) {
            return true;
        }
    }
    false
}

/// Forme d'un verbe absent de Lexique, ajouté du Wiktionnaire (« modéliser »,
/// « paramètre »). Ces verbes souvent rares masqueraient des fautes : la forme
/// est refusée si un mot du lexique ne s'en distingue que par les accents ou
/// une lettre doublée (« repondre » pour « répondre », « moitie » pour
/// « moitié »), ou si elle est courte (« lure », « vale »).
fn wiktionary_verb(ctx: &Context, lower: &str) -> bool {
    if lower.chars().count() < 5 {
        return false;
    }
    let Some(forms) = ctx.data.conjugator.extra_forms_of(lower) else {
        return false;
    };
    // « modélise » reste admis malgré « modélisé » : une forme du même verbe.
    ctx.lexicon
        .ids_for_skeleton(&skeleton(lower))
        .iter()
        .all(|&id| forms.contains(&ctx.lexicon.form(id).text))
}

/// « développeurs », « toccatas », « islandaises », « poitevine » : pluriel ou
/// féminin régulier d'un nom ou d'un adjectif dont Lexique n'a aucune forme au
/// pluriel (ou au féminin). Un lemme qui en a une garde la sienne : « chevals »
/// reste une faute, puisque « chevaux » est connu.
fn regular_inflection(ctx: &Context, lower: &str) -> bool {
    // Le lemme du mot `base` n'a aucune forme qui vérifie `pred`.
    let lacks = |base: &str, adjective_only: bool, pred: &dyn Fn(Gn) -> bool| {
        ctx.lexicon.find(base).is_some_and(|f| {
            f.readings
                .iter()
                .filter(|r| r.pos == Pos::Adj || (!adjective_only && r.pos == Pos::Nom))
                .any(|r| {
                    !ctx.lexicon
                        .lemma_forms(&r.lemma)
                        .any(|(_, x)| x.pos == r.pos && x.gn().is_some_and(pred))
                })
        })
    };
    let plural = |g: Gn| g.number == Some('p');
    let feminine = |g: Gn| g.gender == Some('f');
    // Pluriel en -s, sauf mots en -s, -x, -z, -al, -au, -eu.
    if let Some(stem) = lower.strip_suffix('s') {
        let regular = !stem.ends_with(['s', 'x', 'z'])
            && !["al", "au", "eu"].iter().any(|e| stem.ends_with(e));
        if regular && lacks(stem, false, &plural) {
            return true;
        }
    }
    // Féminin d'adjectif en -e (« islandais », « poitevin », « ogival ») ou à
    // consonne doublée (« -ien » → « -ienne », « -el » → « -elle »).
    let singular = lower.strip_suffix('s').unwrap_or(lower);
    let Some(stem) = singular.strip_suffix('e') else {
        return false;
    };
    let masculine = if let Some(m) = stem
        .strip_suffix("ienn")
        .map(|m| format!("{m}ien"))
        .or_else(|| stem.strip_suffix("éenn").map(|m| format!("{m}éen")))
        .or_else(|| stem.strip_suffix("ell").map(|m| format!("{m}el")))
    {
        m
    } else if [
        "ais", "ois", "ain", "in", "ant", "ent", "ard", "and", "aud", "al", "é", "i", "u",
    ]
    .iter()
    .any(|e| stem.ends_with(e))
    {
        stem.to_string()
    } else {
        return false;
    };
    lacks(&masculine, true, &feminine)
}

/// « la forme blance » → « blanche » avant « blanc » : quand la première
/// suggestion est un adjectif et que d'autres formes du même adjectif suivent,
/// celle qui s'accorde avec le nom qui précède passe en tête.
fn agreeing_first(ctx: &Context, i: usize, suggestions: &mut [String]) {
    let Some(p) = ctx.prev(i) else { return };
    let Some(noun) = ctx.form(p) else { return };
    // Nom précédé de son déterminant (« la forme », « ces immensités ») ; pas
    // « une corde à boyau attacheé », où l'adjectif peut viser un autre nom.
    let after_determiner = ctx.prev(p).is_some_and(|d| {
        ctx.data
            .determiners
            .contains_key(ctx.words[d].lower.as_str())
    });
    // « une si brûlante actualité » : « si » adverbe, pas la note.
    let grammatical =
        noun.has(|r| matches!(r.pos, Pos::Adv | Pos::Con | Pos::Pre | Pos::Det | Pos::Pro));
    if !noun.has_pos(Pos::Nom) || !after_determiner || grammatical {
        return;
    }
    let gns: Vec<Gn> = noun
        .readings
        .iter()
        .filter(|r| r.pos == Pos::Nom)
        .filter_map(|r| r.gn())
        .collect();
    let Some(&target) = gns.first() else { return };
    if gns.iter().any(|&g| g != target) {
        return;
    }
    let adjective_lemma = |s: &str| {
        ctx.lexicon
            .get(s)
            .and_then(|f| f.readings.iter().find(|r| r.pos == Pos::Adj))
            .map(|r| r.lemma.clone())
    };
    let Some(lemma) = suggestions.first().and_then(|s| adjective_lemma(s)) else {
        return;
    };
    let agrees = |s: &str| {
        ctx.lexicon.get(s).is_some_and(|f| {
            f.has(|r| {
                r.pos == Pos::Adj
                    && r.lemma == lemma
                    && r.gn().is_some_and(|g| g.compatible(target))
            })
        })
    };
    if agrees(&suggestions[0]) {
        return;
    }
    if let Some(k) = suggestions.iter().position(|s| agrees(s)) {
        suggestions[..=k].rotate_right(1);
    }
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
