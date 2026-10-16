//! Grammaire : accord sujet-verbe, accords dans le groupe nominal, participes
//! passés avec être et avoir, infinitif à la place du participe, négation.
//!
//! Le sujet n'est reconnu que dans des configurations simples et sûres : pronom
//! personnel placé juste avant le verbe (éventuellement séparé par des
//! pronoms compléments ou « ne »), ou groupe nominal simple (déterminant,
//! adjectif, nom, adjectif) en début de proposition. Dans les autres cas, rien
//! n'est signalé. Les constructions prépositionnelles fautives sont décrites
//! dans `locutions.tsv` (règle `GRAM_PREPOSITION`).

use super::{Context, RawIssue};
use crate::data::{Determiner, Subject};
use crate::lexicon::{Form, Gn, Pos, Reading};
use crate::text::{base_letter, capitalize, is_capitalized};

pub const SUBJECT_VERB: &str = "GRAM_ACCORD_SUJET_VERBE";
pub const DETERMINER: &str = "GRAM_ACCORD_DETERMINANT";
pub const ADJECTIVE: &str = "GRAM_ACCORD_ADJECTIF";
pub const PARTICIPLE_ETRE: &str = "GRAM_PARTICIPE_ETRE";
pub const PARTICIPLE_AVOIR: &str = "GRAM_PARTICIPE_AVOIR";
pub const INFINITIVE: &str = "GRAM_INFINITIF_PARTICIPE";
pub const NEGATION: &str = "GRAM_NEGATION_NE";
pub const PREPOSITION: &str = "GRAM_PREPOSITION";

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    for i in 0..ctx.words.len() {
        if ctx.words[i].ignored || ctx.words[i].has_digit {
            continue;
        }
        noun_phrase(ctx, i, out);
        subject_verb(ctx, i, out);
        auxiliary(ctx, i, out);
        negation(ctx, i, out);
    }
}

// ---------------------------------------------------------------------------
// Outils

fn lower<'c>(ctx: &'c Context, i: usize) -> &'c str {
    &ctx.words[i].lower
}

fn is_class(ctx: &Context, i: usize, class: &str) -> bool {
    ctx.data.is(lower(ctx, i), class)
}

/// Début de proposition : début de phrase, après une ponctuation ou après un
/// mot charnière (et, que, quand…).
fn clause_start(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    w.sentence_start
        || !w.joined_to_prev
        || ctx.prev(i).is_some_and(|p| is_class(ctx, p, "charniere"))
}

/// Initiale vocalique, ou h muet : déclenche l'élision (l’école, n’habite).
fn elides_before(ctx: &Context, word: &str, lemma: Option<&str>) -> bool {
    let lower = word.to_lowercase();
    match lower.chars().next().map(base_letter) {
        Some('a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'œ' | 'æ') => true,
        Some('h') => {
            !ctx.data.h_aspire.contains(&lower)
                && !lemma.is_some_and(|l| ctx.data.h_aspire.contains(l))
        }
        _ => false,
    }
}

/// Reprend la majuscule initiale de `like`.
fn same_case(like: &str, s: &str) -> String {
    if is_capitalized(like) {
        capitalize(s)
    } else {
        s.to_string()
    }
}

/// Genre d'un nom, d'après toutes les formes de son lemme ; `None` s'il
/// s'emploie aux deux genres (« un/une élève », « le/la somme ») ou si le
/// lexique ne le précise pas. Les lectures sans genre sont ignorées quand
/// d'autres formes en donnent un (Lexique note « maison » sans genre au
/// singulier mais au féminin au pluriel).
fn noun_gender(ctx: &Context, lemma: &str) -> Option<char> {
    if ctx.data.two_gender_nouns.contains(lemma) {
        return None;
    }
    let (mut m, mut f) = (false, false);
    for r in ctx.lexicon.lemma_readings(lemma) {
        if r.pos == Pos::Nom {
            match r.gn().and_then(|g| g.gender) {
                Some('m') => m = true,
                Some(_) => f = true,
                None => {}
            }
        }
    }
    match (m, f) {
        (true, false) => Some('m'),
        (false, true) => Some('f'),
        _ => None,
    }
}

/// Genre et nombre d'une lecture ; nombre libre pour les mots invariables
/// en -s, -x, -z (« un avis », « des avis » : Lexique ne donne qu'un nombre).
fn reading_gn(r: &Reading) -> Option<Gn> {
    let mut gn = r.gn()?;
    if r.lemma.ends_with(['s', 'x', 'z']) {
        gn.number = None;
    }
    Some(gn)
}

/// Genres et nombres possibles d'un nom.
fn noun_gns(ctx: &Context, form: &Form) -> Vec<(Gn, String)> {
    form.readings
        .iter()
        .filter(|r| r.pos == Pos::Nom)
        .filter_map(|r| {
            let mut gn = reading_gn(r)?;
            gn.gender = noun_gender(ctx, &r.lemma);
            Some((gn, r.lemma.clone()))
        })
        .collect()
}

/// Lecture adjectivale (adjectif ou participe passé).
fn adjectival(r: &Reading) -> bool {
    r.pos == Pos::Adj || r.is_past_participle()
}

/// Formes du même lemme qui s'accordent avec `target` ; les formes de genre et
/// de nombre exactement connus d'abord.
fn agree_forms<'a>(
    ctx: &'a Context,
    form: &Form,
    pred: impl Fn(&Reading) -> bool,
    target: Gn,
) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    let mut lemmas: Vec<&str> = form
        .readings
        .iter()
        .filter(|r| pred(r))
        .map(|r| r.lemma.as_str())
        .collect();
    lemmas.dedup();
    for exact in [true, false] {
        for lemma in &lemmas {
            for f in ctx.lexicon.inflect(lemma, |r| {
                pred(r)
                    && r.gn().is_some_and(|g| {
                        if exact {
                            g == target || (target.gender.is_none() && g.number == target.number)
                        } else {
                            g.compatible(target)
                        }
                    })
            }) {
                if !out.contains(&f) {
                    out.push(f);
                }
            }
        }
    }
    out.truncate(ctx.options.max_suggestions.max(1));
    out
}

/// Participes passés accordés avec `target`, pour les verbes dont `form` a
/// une lecture satisfaisant `from` (infinitif ou forme conjuguée employés à tort).
fn participles(
    ctx: &Context,
    form: &Form,
    from: impl Fn(&Reading) -> bool,
    target: Gn,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in form.readings.iter().filter(|r| r.pos.is_verb() && from(r)) {
        let mut forms = ctx.lexicon.inflect(&r.lemma, |p| {
            p.is_past_participle() && p.gn().is_some_and(|g| g.compatible(target))
        });
        // Forme exacte d'abord : « allée » plutôt que « allé » pour « elle ».
        forms.sort_by_key(|f| {
            !ctx.lexicon.get(f).is_some_and(|x| {
                x.has(|p| p.is_past_participle() && p.gn().is_some_and(|g| g == target))
            })
        });
        for f in forms {
            if !out.iter().any(|x| x == f) {
                out.push(f.to_string());
            }
        }
    }
    out.truncate(ctx.options.max_suggestions.max(1));
    out
}

// ---------------------------------------------------------------------------
// Groupe nominal

struct NounPhrase<'d> {
    det: usize,
    determiner: &'d Determiner,
    pre: Vec<usize>,
    noun: usize,
    post: Option<usize>,
}

/// Groupe nominal simple commençant par le déterminant `det`.
fn find_noun_phrase<'c>(ctx: &'c Context, det: usize) -> Option<NounPhrase<'c>> {
    let w = &ctx.words[det];
    if w.ignored || w.compound_part {
        return None;
    }
    let determiner = ctx.data.determiners.get(&w.lower)?;
    let mut j = ctx.next(det)?;
    // « il les mange » : pronom complément, pas déterminant.
    let pronoun_reading = ctx.form(det).is_some_and(|f| f.has_pos(Pos::Pro));
    let after_subject = ctx.prev(det).is_some_and(|p| {
        ctx.data.subjects.contains_key(lower(ctx, p)) || is_class(ctx, p, "clitique")
    });
    if pronoun_reading && after_subject {
        return None;
    }

    // Adjectifs antéposés : « une grande maison ».
    let mut pre = Vec::new();
    while pre.len() < 2 {
        let (Some(fj), Some(n)) = (ctx.form(j), ctx.next(j)) else {
            break;
        };
        let Some(fnext) = ctx.form(n) else { break };
        let next_is_plain_noun =
            fnext.has_pos(Pos::Nom) && !fnext.has(|r| r.pos == Pos::Adj || r.is_finite_verb());
        if fj.has_pos(Pos::Adj) && next_is_plain_noun {
            pre.push(j);
            j = n;
        } else {
            break;
        }
    }

    let noun = j;
    let nw = &ctx.words[noun];
    if nw.ignored || nw.has_digit || (is_capitalized(nw.text) && !nw.sentence_start) {
        return None;
    }
    let noun_form = ctx.form(noun)?;
    // « un peu », « le mieux » : mot surtout adverbial, pas un nom.
    if !noun_form.has_pos(Pos::Nom) || noun_form.has_pos(Pos::Adv) {
        return None;
    }
    let post = ctx.next(noun).filter(|&p| {
        !ctx.data.determiners.contains_key(lower(ctx, p))
            && !is_class(ctx, p, "clitique")
            && ctx.form(p).is_some_and(|f| {
                // « les tables pendant que » : préposition, pas adjectif.
                f.has(adjectival)
                    && !f.has(|r| {
                        r.is_finite_verb()
                            || matches!(r.pos, Pos::Pre | Pos::Con | Pos::Adv | Pos::Pro | Pos::Det)
                    })
            })
    });
    Some(NounPhrase {
        det,
        determiner,
        pre,
        noun,
        post,
    })
}

/// Genre et nombre du groupe : ceux du déterminant complétés par ceux du nom.
/// `None` si déterminant et nom ne s'accordent pas.
fn phrase_gn(ctx: &Context, np: &NounPhrase) -> Option<Gn> {
    let d = np.determiner.gn;
    noun_gns(ctx, ctx.form(np.noun)?)
        .into_iter()
        .find(|(g, _)| g.compatible(d))
        .map(|(g, _)| d.merge(g))
}

/// Texte de `from` à `to` (mots compris), avec certains mots remplacés.
fn rebuild(ctx: &Context, from: usize, to: usize, replace: &[(usize, String)]) -> String {
    let mut out = String::new();
    for k in from..=to {
        if k > from {
            out.push_str(&ctx.text[ctx.words[k - 1].end..ctx.words[k].start]);
        }
        match replace.iter().find(|(i, _)| *i == k) {
            Some((_, s)) => out.push_str(s),
            None => out.push_str(ctx.words[k].text),
        }
    }
    out
}

/// Déterminant de remplacement adapté au mot qui suit (le → l’, ce → cet, ma → mon).
fn adapt_determiner(ctx: &Context, det: &str, next: usize) -> String {
    let next_word = &ctx.words[next];
    let lemma = ctx
        .form(next)
        .and_then(|f| f.readings.first())
        .map(|r| r.lemma.as_str());
    if !elides_before(ctx, next_word.text, lemma) {
        return det.to_string();
    }
    match det {
        "le" | "la" => "l’".into(),
        "de la" => "de l’".into(),
        "à la" => "à l’".into(),
        "ce" => "cet".into(),
        "ma" => "mon".into(),
        "ta" => "ton".into(),
        "sa" => "son".into(),
        _ => det.to_string(),
    }
}

/// Texte du groupe avec un nouveau déterminant (gère l'élision : « l’école »).
fn with_determiner(ctx: &Context, np: &NounPhrase, det: &str, end: usize) -> String {
    let first = np.det + 1;
    let det = same_case(ctx.words[np.det].text, &adapt_determiner(ctx, det, first));
    let sep = if det.ends_with('’') {
        ""
    } else if ctx.words[np.det].text.ends_with(['’', '\'']) {
        " "
    } else {
        &ctx.text[ctx.words[np.det].end..ctx.words[first].start]
    };
    format!("{det}{sep}{}", rebuild(ctx, first, end, &[]))
}

fn noun_phrase(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(np) = find_noun_phrase(ctx, i) else {
        return;
    };
    let Some(noun_form) = ctx.form(np.noun) else {
        return;
    };
    let d = np.determiner.gn;
    let nouns = noun_gns(ctx, noun_form);
    let (start, end) = (ctx.words[np.det].start, ctx.words[np.noun].end);
    let found = &ctx.text[start..end];

    match phrase_gn(ctx, &np) {
        Some(target) => {
            // Adjectifs du groupe.
            for adj in np.pre.iter().copied().chain(np.post) {
                let Some(f) = ctx.form(adj) else { continue };
                let ok = f
                    .readings
                    .iter()
                    .filter(|r| adjectival(r))
                    .any(|r| reading_gn(r).is_some_and(|g| g.compatible(target)));
                if ok {
                    continue;
                }
                let suggestions = agree_forms(ctx, f, adjectival, target);
                let w = &ctx.words[adj];
                let mut issue = RawIssue::new(w.start, w.end, ADJECTIVE)
                    .arg(w.text)
                    .arg(suggestions.first().copied().unwrap_or(""))
                    .arg(ctx.words[np.noun].text);
                issue.suggestions = suggestions.iter().map(|s| same_case(w.text, s)).collect();
                out.push(issue);
            }
        }
        None if nouns
            .iter()
            .all(|(g, _)| g.number.is_some() && d.number.is_some() && g.number != d.number) =>
        {
            // Nombre : « les chat » → « les chats » ou « le chat ».
            let mut suggestions = Vec::new();
            if let Some((g, lemma)) = nouns.first() {
                let target = Gn {
                    gender: g.gender,
                    number: d.number,
                };
                if let Some(fixed) = ctx
                    .lexicon
                    .inflect(lemma, |r| {
                        r.pos == Pos::Nom && r.gn().is_some_and(|x| x.compatible(target))
                    })
                    .first()
                {
                    let mut repl = vec![(np.noun, same_case(ctx.words[np.noun].text, fixed))];
                    for &a in &np.pre {
                        if let Some(fa) = ctx.form(a) {
                            if let Some(s) =
                                agree_forms(ctx, fa, adjectival, d.merge(target)).first()
                            {
                                repl.push((a, s.to_string()));
                            }
                        }
                    }
                    suggestions.push(rebuild(ctx, np.det, np.noun, &repl));
                }
            }
            if let Some(other) = &np.determiner.other_number {
                suggestions.push(with_determiner(ctx, &np, other, np.noun));
            }
            if suggestions.is_empty() {
                return;
            }
            out.push(
                RawIssue::new(start, end, DETERMINER)
                    .arg(found)
                    .arg(suggestions[0].clone())
                    .arg(ctx.words[np.det].text)
                    .with_suggestions(suggestions),
            );
        }
        None => {
            // Genre : « le maison » → « la maison ».
            let Some(other) = &np.determiner.other_gender else {
                return;
            };
            let fixed = with_determiner(ctx, &np, other, np.noun);
            out.push(
                RawIssue::new(start, end, DETERMINER)
                    .arg(found)
                    .arg(fixed.clone())
                    .arg(ctx.words[np.det].text)
                    .suggest(fixed),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Sujet

struct FoundSubject {
    person: String,
    gn: Gn,
    /// Premier mot du sujet.
    start: usize,
    /// Pronoms et négation entre le sujet et le verbe.
    chain: Vec<usize>,
    pronoun: bool,
}

fn from_pronoun(s: &Subject, start: usize, chain: Vec<usize>) -> FoundSubject {
    FoundSubject {
        person: s.person.clone(),
        gn: s.gn,
        start,
        chain,
        pronoun: true,
    }
}

/// Sujet du verbe d'indice `verb`, s'il est reconnaissable avec certitude.
/// Un sujet coordonné (« lui et elle », « le chat et la souris ») est écarté :
/// seul son dernier élément serait vu.
fn subject_of(ctx: &Context, verb: usize) -> Option<FoundSubject> {
    let s = simple_subject(ctx, verb)?;
    let coordinated = ctx
        .prev(s.start)
        .is_some_and(|p| matches!(lower(ctx, p), "et" | "ou" | "ni"));
    (!coordinated).then_some(s)
}

fn simple_subject(ctx: &Context, verb: usize) -> Option<FoundSubject> {
    if ctx.words[verb].compound_part {
        return None; // « dit-il » : sujet inversé
    }
    let mut k = verb;
    let mut chain = Vec::new();
    while let Some(p) = ctx.prev(k) {
        if is_class(ctx, p, "clitique") && !ctx.words[p].compound_part {
            chain.push(p);
            k = p;
        } else {
            break;
        }
    }
    if let Some(p) = ctx.prev(k) {
        if let Some(s) = ctx.data.subjects.get(lower(ctx, p)) {
            if !ctx.words[p].compound_part {
                chain.reverse();
                return Some(from_pronoun(s, p, chain));
            }
        }
    }
    // « nous », « vous » : sujets seulement en tête de proposition.
    if let Some(&left) = chain.last() {
        if matches!(lower(ctx, left), "nous" | "vous") && clause_start(ctx, left) {
            let s = ctx.data.subjects.get(lower(ctx, left))?;
            chain.pop();
            chain.reverse();
            return Some(from_pronoun(s, left, chain));
        }
    }
    // Groupe nominal en début de proposition : « Les enfants jouent ».
    let last = ctx.prev(k)?;
    for det in (last.saturating_sub(4)..last).rev() {
        let Some(np) = find_noun_phrase(ctx, det) else {
            continue;
        };
        if np.post.unwrap_or(np.noun) != last || !clause_start(ctx, det) {
            continue;
        }
        let gn = phrase_gn(ctx, &np)?;
        let person = match gn.number? {
            's' => "3s",
            _ => "3p",
        };
        chain.reverse();
        return Some(FoundSubject {
            person: person.into(),
            gn,
            start: det,
            chain,
            pronoun: false,
        });
    }
    None
}

fn subject_text(ctx: &Context, s: &FoundSubject, verb: usize) -> String {
    let end = s.chain.first().map_or(verb, |&c| c) - 1;
    rebuild(ctx, s.start, end, &[])
}

fn subject_verb(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(form) = ctx.form(i) else { return };
    let finite: Vec<&Reading> = form
        .readings
        .iter()
        .filter(|r| r.person().is_some())
        .collect();
    if finite.is_empty() {
        return;
    }
    let Some(subject) = subject_of(ctx, i) else {
        return;
    };
    // Après un groupe nominal, un mot qui peut être adjectif est ambigu.
    if !subject.pronoun && form.has(adjectival) {
        return;
    }
    if finite
        .iter()
        .any(|r| r.person() == Some(subject.person.as_str()))
    {
        return;
    }
    let mut readings = finite.clone();
    readings.sort_by_key(|r| r.mode_tense() != Some("ind.pres"));
    let mut suggestions: Vec<String> = Vec::new();
    for r in readings {
        let wanted = format!("{}.{}", r.mode_tense().unwrap_or(""), subject.person);
        for f in ctx
            .lexicon
            .inflect(&r.lemma, |x| x.pos == r.pos && x.traits == wanted)
        {
            let f = same_case(ctx.words[i].text, f);
            if !suggestions.contains(&f) {
                suggestions.push(f);
            }
        }
    }
    if suggestions.is_empty() {
        return;
    }
    suggestions.truncate(ctx.options.max_suggestions.max(1));
    let w = &ctx.words[i];
    out.push(
        RawIssue::new(w.start, w.end, SUBJECT_VERB)
            .arg(w.text)
            .arg(suggestions[0].clone())
            .arg(subject_text(ctx, &subject, i))
            .with_suggestions(suggestions),
    );
}

// ---------------------------------------------------------------------------
// Auxiliaires : participe passé, infinitif

fn auxiliary(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(form) = ctx.form(i) else { return };
    let aux =
        |lemma: &str| form.has(|r| r.pos == Pos::Aux && r.lemma == lemma && r.is_finite_verb());
    let (etre, avoir) = (aux("être"), aux("avoir"));
    if !etre && !avoir {
        return;
    }
    let Some(subject) = subject_of(ctx, i) else {
        return;
    };

    // Mot qui suit l'auxiliaire, après d'éventuels adverbes (« n’est pas encore parti »).
    let mut t = ctx.next(i);
    for _ in 0..2 {
        match t {
            Some(j) if is_class(ctx, j, "adverbe") => t = ctx.next(j),
            _ => break,
        }
    }
    let Some(t) = t else { return };
    let Some(ft) = ctx.form(t) else { return };
    let w = &ctx.words[t];
    let has_object = subject.chain.iter().any(|&c| is_class(ctx, c, "objet"));

    // « il a manger », « elle est aller » → participe passé.
    if ft.has(Reading::is_infinitive) && !ft.has(adjectival) {
        let target = if avoir {
            Gn {
                gender: Some('m'),
                number: Some('s'),
            }
        } else {
            subject.gn
        };
        let suggestions = participles(ctx, ft, Reading::is_infinitive, target);
        if let Some(first) = suggestions.first().cloned() {
            out.push(
                RawIssue::new(w.start, w.end, INFINITIVE)
                    .arg(w.text)
                    .arg(first)
                    .with_suggestions(suggestions),
            );
        }
        return;
    }

    if etre && !has_object {
        // Attribut ou participe : accord avec le sujet (« elles sont parties »).
        if subject.gn == Gn::default() {
            return;
        }
        let adj: Vec<&Reading> = ft.readings.iter().filter(|r| adjectival(r)).collect();
        let fits = adj
            .iter()
            .any(|r| reading_gn(r).is_some_and(|g| g.compatible(subject.gn)));
        // « elle est partit » : forme conjuguée à la place du participe.
        let finite_instead =
            adj.is_empty() && !ft.has_pos(Pos::Nom) && ft.has(Reading::is_finite_verb);
        if fits || (adj.is_empty() && !finite_instead) {
            return;
        }
        let pred = |r: &Reading| {
            if finite_instead {
                r.is_past_participle()
            } else {
                adjectival(r)
            }
        };
        let suggestions: Vec<String> = if finite_instead {
            participles(ctx, ft, Reading::is_finite_verb, subject.gn)
        } else {
            agree_forms(ctx, ft, pred, subject.gn)
                .iter()
                .map(|s| s.to_string())
                .collect()
        };
        if let Some(first) = suggestions.first().cloned() {
            out.push(
                RawIssue::new(w.start, w.end, PARTICIPLE_ETRE)
                    .arg(w.text)
                    .arg(first)
                    .arg(subject_text(ctx, &subject, i))
                    .with_suggestions(suggestions),
            );
        }
        return;
    }

    if avoir && !has_object && !ft.has_pos(Pos::Nom) {
        // Avec avoir, sans complément placé avant, le participe reste au masculin singulier.
        let pp: Vec<&Reading> = ft
            .readings
            .iter()
            .filter(|r| r.is_past_participle())
            .collect();
        let ms = Gn {
            gender: Some('m'),
            number: Some('s'),
        };
        if pp.is_empty() || pp.iter().any(|r| r.gn().is_some_and(|g| g.compatible(ms))) {
            return;
        }
        // « la pomme qu’il a mangée » : complément placé avant, accord correct.
        if (0..subject.start)
            .rev()
            .take_while(|&k| !ctx.words[k + 1].sentence_start)
            .any(|k| is_class(ctx, k, "antecedent"))
        {
            return;
        }
        let suggestions: Vec<String> = agree_forms(ctx, ft, |r| r.is_past_participle(), ms)
            .iter()
            .map(|s| s.to_string())
            .collect();
        if let Some(first) = suggestions.first().cloned() {
            out.push(
                RawIssue::new(w.start, w.end, PARTICIPLE_AVOIR)
                    .arg(w.text)
                    .arg(first)
                    .with_suggestions(suggestions),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Négation

fn negation(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(form) = ctx.form(i) else { return };
    if !form.has(Reading::is_finite_verb) {
        return;
    }
    let Some(n) = ctx.next(i) else { return };
    if !is_class(ctx, n, "negation") {
        return;
    }
    // « c’est pas » → « ce n’est pas ».
    if let Some(p) = ctx.prev(i).filter(|&p| lower(ctx, p) == "c’") {
        let verb = ctx.words[i].text;
        let fixed = format!("{} n’{verb}", same_case(ctx.words[p].text, "ce"));
        let (start, end) = (ctx.words[p].start, ctx.words[i].end);
        out.push(
            RawIssue::new(start, end, NEGATION)
                .arg(&ctx.text[start..end])
                .arg(fixed.clone())
                .suggest(fixed),
        );
        return;
    }
    let Some(subject) = subject_of(ctx, i) else {
        return;
    };
    if subject
        .chain
        .iter()
        .any(|&c| matches!(lower(ctx, c), "ne" | "n’"))
    {
        return;
    }
    let first = subject.chain.first().copied().unwrap_or(i);
    let (start, end) = (ctx.words[first].start, ctx.words[i].end);
    let span = &ctx.text[start..end];
    let lemma = ctx
        .form(first)
        .and_then(|f| f.readings.first())
        .map(|r| r.lemma.as_str());
    let fixed = if elides_before(ctx, ctx.words[first].text, lemma) {
        format!("n’{span}")
    } else {
        format!("ne {span}")
    };
    out.push(
        RawIssue::new(start, end, NEGATION)
            .arg(span)
            .arg(fixed.clone())
            .arg(ctx.words[n].text)
            .suggest(fixed),
    );
}
