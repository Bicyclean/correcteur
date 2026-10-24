//! Formes verbales employées à la place d'une autre : impératif écrit comme le
//! présent (« tu » absent), participe sans auxiliaire.

use super::grammar::{agree_forms, inflected, is_class, lower, same_case};
use super::{Context, RawIssue};
use crate::lexicon::{Form, Gn, Pos, Reading};

pub const IMPERATIVE: &str = "GRAM_IMPERATIF";
pub const BARE_PARTICIPLE: &str = "GRAM_PARTICIPE_SEUL";
pub const BARE_INFINITIVE: &str = "GRAM_INFINITIF_SEUL";
pub const PARTICIPLE_AGREEMENT: &str = "GRAM_ACCORD_PARTICIPE";

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    for i in 0..ctx.words.len() {
        if ctx.words[i].ignored || ctx.words[i].has_digit {
            continue;
        }
        imperative(ctx, i, out);
        bare_participle(ctx, i, out);
        bare_infinitive(ctx, i, out);
        etant_participle(ctx, i, out);
    }
}

/// Premier mot d'une phrase, y compris le premier élément d'un composé à trait
/// d'union (« Donnes-moi »), que l'analyse ne marque pas comme début de phrase.
fn initial(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    if w.sentence_start {
        return true;
    }
    if !w.compound_part {
        return false;
    }
    if i > 0 && ctx.words[i - 1].compound_part && ctx.words[i - 1].end + 1 == w.start {
        return false; // second élément
    }
    ctx.text[..w.start]
        .trim_end()
        .chars()
        .next_back()
        .is_none_or(|c| matches!(c, '.' | '!' | '?' | '…' | '\n' | '«' | '—' | '–' | '"'))
}

/// Le mot suivant, s'il est accolé par un trait d'union (« -moi », « -y »).
fn hyphen_clitic<'c>(ctx: &'c Context, i: usize) -> Option<&'c str> {
    let n = i + 1;
    let (w, next) = (ctx.words.get(i)?, ctx.words.get(n)?);
    (w.compound_part && next.compound_part && ctx.text[w.end..next.start].starts_with('-'))
        .then_some(next.lower.as_str())
}

fn plain_verb(form: &Form) -> bool {
    !form.has(|r| {
        matches!(
            r.pos,
            Pos::Nom | Pos::Adj | Pos::Adv | Pos::Pre | Pos::Con | Pos::Det | Pos::Pro
        )
    })
}

/// « Donnes-moi », « Écoutes bien », « dit-lui », « Soit gentil » : la forme de
/// l'impératif manque ; la 2e personne du singulier perd son « s » (sauf devant
/// « y » et « en »).
fn imperative(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    let Some(form) = ctx.form(i) else { return };
    // « (es) », « Je... ne... veux... pas... » : code de langue, points de suspension.
    let before = ctx.text[..w.start].trim_end();
    if w.text.chars().count() <= 2
        || before.ends_with(['(', '…'])
        || before.ends_with("..")
        || ctx.text[w.end..].starts_with(')')
        || form.has(|r| matches!(r.lemma.as_str(), "être" | "avoir") && r.person() == Some("3s"))
    {
        return;
    }
    let clitic = hyphen_clitic(ctx, i);
    // Sujet inversé (« dit-il », « parles-tu ») : pas un impératif.
    if clitic.is_some_and(|c| {
        matches!(
            c,
            "il" | "ils"
                | "elle"
                | "elles"
                | "on"
                | "je"
                | "tu"
                | "nous"
                | "vous"
                | "t"
                | "ce"
                | "ci"
        )
    }) {
        return;
    }
    let object = clitic.is_some_and(|c| {
        matches!(
            c,
            "moi" | "toi" | "lui" | "leur" | "le" | "la" | "les" | "m’" | "t’" | "l’" | "y" | "en"
        )
    });
    // « Soit gentil » : « sois ».
    if w.lower == "soit"
        && initial(ctx, i)
        && ctx.next(i).and_then(|n| ctx.form(n)).is_some_and(|f| {
            f.has_pos(Pos::Adj) && !f.has(|r| r.pos.is_verb() || r.pos == Pos::Nom)
        })
        && ctx
            .next(i)
            .is_some_and(|n| !ctx.data.determiners.contains_key(lower(ctx, n)))
    {
        let fixed = same_case(w.text, "sois");
        out.push(
            RawIssue::new(w.start, w.end, IMPERATIVE)
                .arg(w.text)
                .arg(fixed.clone())
                .suggest(fixed),
        );
        return;
    }
    if !object && !(initial(ctx, i) || after_ne(ctx, i) || imperative_continuation(ctx, i)) {
        return;
    }
    // Sans trait d'union, seul un mot qui n'est que verbe est un impératif sûr.
    if !object && !plain_verb(form) {
        return;
    }
    let mut lemmas: Vec<&str> = Vec::new();
    for r in form.readings.iter().filter(|r| {
        r.is_finite_verb()
            && match r.mode_tense() {
                Some("ind.pres") => {
                    r.person() == Some("2s") || (object && r.person() == Some("3s"))
                }
                Some("sub.pres") => r.person() == Some("2s"),
                _ => false,
            }
    }) {
        if !lemmas.contains(&r.lemma.as_str()) {
            lemmas.push(&r.lemma);
        }
    }
    // Le mot est déjà un impératif (« mange », « prends ») : rien à dire.
    if form.has(|r| r.mode_tense() == Some("imp.pres") && r.person().is_none()) && !object {
        let imp_2s = form.has(|r| r.traits == "imp.pres.2s");
        if imp_2s {
            return;
        }
    }
    let before_y_en = matches!(clitic, Some("y" | "en"));
    let mut suggestions: Vec<String> = Vec::new();
    for lemma in lemmas {
        for f in inflected(ctx, lemma, Pos::Ver, "imp.pres.2s") {
            // « parles-en », « vas-y » : le « s » se maintient devant y et en.
            let f = if before_y_en && f.ends_with(['e', 'a']) {
                format!("{f}s")
            } else {
                f
            };
            let f = same_case(w.text, &f);
            if f.to_lowercase() != w.lower && !suggestions.contains(&f) {
                suggestions.push(f);
            }
        }
    }
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    // « vas-y » est correct : la forme avec « s » devant y et en.
    if form.has(|r| r.traits == "imp.pres.2s") {
        return;
    }
    out.push(
        RawIssue::new(w.start, w.end, IMPERATIVE)
            .arg(w.text)
            .arg(first)
            .with_suggestions(suggestions),
    );
}

/// « Ne parles pas » : la négation ouvre la phrase.
fn after_ne(ctx: &Context, i: usize) -> bool {
    ctx.prev(i)
        .is_some_and(|p| matches!(lower(ctx, p), "ne" | "n’") && ctx.words[p].sentence_start)
}

/// « Il voulu partir », « vous travaillé » : un participe passé seul après le
/// pronom sujet, sans auxiliaire.
fn bare_participle(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    if w.compound_part {
        return;
    }
    let Some(form) = ctx.form(i) else { return };
    // « voulu » est aussi un adjectif : après un pronom sujet, c'est le participe.
    if !form.has(|r| r.is_past_participle())
        || form
            .readings
            .iter()
            .any(|r| !(r.is_past_participle() || r.pos == Pos::Adj))
        || form
            .readings
            .iter()
            .any(|r| matches!(r.lemma.as_str(), "être" | "avoir" | "luire"))
    {
        return;
    }
    // Pronom sujet, éventuellement suivi de compléments : « il y eu ».
    let mut p = match ctx.prev(i) {
        Some(p) => p,
        None => return,
    };
    while matches!(
        lower(ctx, p),
        "y" | "en"
            | "ne"
            | "n’"
            | "me"
            | "m’"
            | "te"
            | "t’"
            | "se"
            | "s’"
            | "le"
            | "la"
            | "l’"
            | "les"
            | "lui"
            | "leur"
    ) {
        match ctx.prev(p) {
            Some(q) => p = q,
            None => return,
        }
    }
    let Some(subject) = ctx.data.subjects.get(lower(ctx, p)) else {
        return;
    };
    // En tête de proposition : pas « l’une d’elles intitulée ».
    if !(ctx.words[p].sentence_start
        || !ctx.words[p].joined_to_prev
        || ctx.prev(p).is_some_and(|q| is_class(ctx, q, "charniere")))
    {
        return;
    }
    if ctx.words[p].compound_part
        || !matches!(
            lower(ctx, p),
            "je" | "j’" | "tu" | "il" | "elle" | "on" | "nous" | "vous" | "ils" | "elles"
        )
    {
        return;
    }
    // « nous » et « vous » compléments : « il nous reconnu ».
    if matches!(lower(ctx, p), "nous" | "vous")
        && !(ctx.words[p].sentence_start
            || !ctx.words[p].joined_to_prev
            || ctx.prev(p).is_some_and(|q| is_class(ctx, q, "charniere")))
    {
        return;
    }
    let Some(reading) = form.readings.first() else {
        return;
    };
    let person = subject.person.as_str();
    let lemma = reading.lemma.as_str();
    let etre = ctx.data.conjugator.takes_etre(lemma);
    let aux = if etre { "être" } else { "avoir" };
    // Participe au masculin singulier : « elle est allée » demande l'accord, mais
    // « vous faite » est surtout un « s » oublié.
    let participle = inflected(ctx, lemma, Pos::Ver, "ppas.m.s")
        .into_iter()
        .next()
        .unwrap_or_else(|| w.text.to_string());
    let mut suggestions: Vec<String> = Vec::new();
    if let Some(a) = inflected(ctx, aux, Pos::Aux, &format!("ind.pres.{person}"))
        .into_iter()
        .next()
    {
        suggestions.push(format!("{a} {participle}"));
    }
    let mut finite: Vec<String> = Vec::new();
    for tense in ["ind.pres", "ind.ps"] {
        if let Some(f) = inflected(ctx, lemma, Pos::Ver, &format!("{tense}.{person}"))
            .into_iter()
            .next()
        {
            if !finite.contains(&f) {
                finite.push(f);
            }
        }
    }
    // Une forme conjuguée à une lettre près (« rougi » → « rougis ») passe en tête.
    let (close, far): (Vec<String>, Vec<String>) =
        finite.into_iter().partition(|f| one_edit(&w.lower, f));
    suggestions.splice(0..0, close);
    suggestions.extend(far);
    if suggestions.is_empty() {
        return;
    }
    let first = suggestions[0].clone();
    out.push(
        RawIssue::new(w.start, w.end, BARE_PARTICIPLE)
            .arg(w.text)
            .arg(first)
            .with_suggestions(suggestions),
    );
}

/// Les deux mots diffèrent d'une seule lettre ajoutée, retirée ou remplacée.
fn one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let (ra, rb) = (&a[common..], &b[common..]);
    match ra.len().cmp(&rb.len()) {
        std::cmp::Ordering::Equal => ra[1..] == rb[1..],
        std::cmp::Ordering::Greater => ra[1..] == *rb,
        std::cmp::Ordering::Less => *ra == rb[1..],
    }
}

/// Le verbe d'indice `i` ouvre une proposition (après une virgule, « et », « ou »…)
/// dans une phrase qui commence elle-même par un impératif : « Reste ici et
/// attends-moi, ne bouges pas ».
fn imperative_continuation(ctx: &Context, i: usize) -> bool {
    let mut h = i;
    if h > 0 && matches!(lower(ctx, h - 1), "ne" | "n’") && ctx.words[h].joined_to_prev {
        h -= 1;
    }
    if h == 0 {
        return false;
    }
    let before = ctx.text[..ctx.words[h].start].trim_end();
    let conjunction = matches!(lower(ctx, h - 1), "et" | "ou" | "puis" | "mais" | "ni");
    if !(before.ends_with(',') || conjunction) {
        return false;
    }
    let mut s = h;
    while s > 0 && !ctx.words[s].sentence_start {
        s -= 1;
    }
    let first = if matches!(lower(ctx, s), "ne" | "n’") {
        s + 1
    } else {
        s
    };
    let Some(form) = ctx.form(first) else {
        return false;
    };
    form.has(|r| r.traits.starts_with("imp.pres"))
        && !(s..h).any(|k| {
            matches!(
                lower(ctx, k),
                "je" | "j’" | "tu" | "il" | "elle" | "on" | "ils" | "elles" | "qui"
            )
        })
}

/// Début de proposition : début de phrase, ponctuation ou mot charnière.
fn clause_head(ctx: &Context, p: usize) -> bool {
    ctx.words[p].sentence_start
        || !ctx.words[p].joined_to_prev
        || ctx.prev(p).is_some_and(|q| is_class(ctx, q, "charniere"))
}

/// « Vous aller où ? », « Il manger » : un infinitif seul après le pronom sujet.
fn bare_infinitive(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    if w.compound_part {
        return;
    }
    let Some(form) = ctx.form(i) else { return };
    if !form.has(Reading::is_infinitive)
        || form.has(|r| r.is_finite_verb())
        || form.has(|r| matches!(r.pos, Pos::Pre | Pos::Con | Pos::Det | Pos::Pro | Pos::Adv))
        || matches!(w.lower.as_str(), "être" | "avoir")
    {
        return;
    }
    let Some(mut p) = ctx.prev(i) else { return };
    while matches!(
        lower(ctx, p),
        "y" | "en"
            | "ne"
            | "n’"
            | "me"
            | "m’"
            | "te"
            | "t’"
            | "se"
            | "s’"
            | "le"
            | "la"
            | "l’"
            | "les"
            | "lui"
            | "leur"
    ) {
        match ctx.prev(p) {
            Some(q) => p = q,
            None => return,
        }
    }
    let pronoun = lower(ctx, p);
    let Some(subject) = ctx.data.subjects.get(pronoun) else {
        return;
    };
    let strong = matches!(
        pronoun,
        "je" | "j’" | "tu" | "il" | "elle" | "on" | "ils" | "elles"
    );
    // « Vous pardonner ! », « Nous convertir ? » : infinitif exclamatif ou
    // interrogatif ; seuls « nous » et « vous » en tête de phrase y échappent, et
    // pas dans une phrase qui finit par « ? », « ! » ou « … ».
    let ends_with_mark = ctx.text[w.end..]
        .chars()
        .find(|c| matches!(c, '.' | '?' | '!' | '…' | ';' | ':' | '\n'))
        .is_some_and(|c| matches!(c, '?' | '!' | '…'));
    if ctx.words[p].compound_part || !(strong && clause_head(ctx, p)) || ends_with_mark {
        return;
    }
    let Some(reading) = form.readings.iter().find(|r| r.is_infinitive()) else {
        return;
    };
    let person = subject.person.as_str();
    let mut suggestions: Vec<String> = Vec::new();
    for tense in ["ind.pres", "ind.impf", "ind.fut"] {
        for f in inflected(ctx, &reading.lemma, Pos::Ver, &format!("{tense}.{person}")) {
            if !suggestions.contains(&f) {
                suggestions.push(f);
            }
            break;
        }
    }
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    out.push(
        RawIssue::new(w.start, w.end, BARE_INFINITIVE)
            .arg(w.text)
            .arg(first)
            .with_suggestions(suggestions),
    );
}

/// « Étant arrivé trop tard, ils ont raté » : le participe après « étant » en tête
/// de phrase s'accorde avec le sujet de la principale.
fn etant_participle(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    if w.lower != "étant" || !w.sentence_start {
        return;
    }
    // Le participe, après d'éventuels adverbes.
    let mut t = i + 1;
    while ctx.words.get(t).is_some_and(|x| {
        matches!(
            x.lower.as_str(),
            "très" | "bien" | "déjà" | "toujours" | "tous" | "tout"
        )
    }) {
        t += 1;
    }
    let Some(tw) = ctx.words.get(t) else { return };
    let Some(form) = ctx.form(t) else { return };
    if !form.has(Reading::is_past_participle) || form.has(|r| r.is_finite_verb()) {
        return;
    }
    // « étant donné les circonstances », « étant entendu que » : locutions invariables.
    if matches!(
        tw.lower.as_str(),
        "donné" | "entendu" | "supposé" | "convenu" | "attendu"
    ) || ctx
        .next(t)
        .is_some_and(|n| ctx.data.determiners.contains_key(lower(ctx, n)))
    {
        return;
    }
    // La virgule qui ferme la proposition, puis le pronom sujet.
    let Some(comma) = (t + 1..(t + 12).min(ctx.words.len()))
        .find(|&k| ctx.text[ctx.words[k - 1].end..ctx.words[k].start].contains(','))
    else {
        return;
    };
    let target = match lower(ctx, comma) {
        "il" | "on" => Gn {
            gender: Some('m'),
            number: Some('s'),
        },
        "elle" => Gn {
            gender: Some('f'),
            number: Some('s'),
        },
        "ils" => Gn {
            gender: Some('m'),
            number: Some('p'),
        },
        "elles" => Gn {
            gender: Some('f'),
            number: Some('p'),
        },
        "nous" | "vous" => Gn {
            gender: None,
            number: Some('p'),
        },
        _ => return,
    };
    if form.has(|r| r.is_past_participle() && r.gn().is_some_and(|g| g.compatible(target))) {
        return;
    }
    let suggestions: Vec<String> = agree_forms(ctx, form, Reading::is_past_participle, target)
        .iter()
        .map(|s| same_case(tw.text, s))
        .collect();
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    out.push(
        RawIssue::new(tw.start, tw.end, PARTICIPLE_AGREEMENT)
            .arg(tw.text)
            .arg(first)
            .arg(ctx.words[comma].text)
            .with_suggestions(suggestions),
    );
}
