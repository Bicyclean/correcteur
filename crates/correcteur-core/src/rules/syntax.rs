//! Règles de syntaxe et d'écriture des mots composés : élision obligatoire
//! (« je ai » → « j’ai »), contractions (« à le » → « au »), traits d'union
//! des inversions (« peut on » → « peut-on », « est ce que » → « est-ce que »),
//! « sensé » et « près » pour « censé » et « prêt ».

use super::{Context, RawIssue};
use crate::lexicon::{Pos, Reading};
use crate::text::{capitalize, is_capitalized};

pub const ELISION: &str = "ORTH_ELISION";
pub const CONTRACTION: &str = "GRAM_CONTRACTION";
pub const HYPHEN: &str = "ORTH_TRAIT_UNION";
pub const SENSE: &str = "GRAM_CENSE";
pub const PARONYM: &str = "GRAM_PARONYME";
pub const DONT: &str = "GRAM_DONT";
pub const NOUN_EXPECTED: &str = "GRAM_NOM_ATTENDU";

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    for i in 0..ctx.words.len() {
        if ctx.words[i].ignored {
            continue;
        }
        elision(ctx, i, out);
        contraction(ctx, i, out);
        inversion(ctx, i, out);
        est_ce(ctx, i, out);
        sense(ctx, i, out);
        paronym(ctx, i, out);
        dont(ctx, i, out);
        hyphenated_wrongly(ctx, i, out);
        noun_expected(ctx, i, out);
    }
}

fn lower<'a>(ctx: &'a Context, i: usize) -> &'a str {
    ctx.words[i].lower.as_str()
}

fn same_case(model: &str, s: &str) -> String {
    if is_capitalized(model) {
        capitalize(s)
    } else {
        s.to_string()
    }
}

/// Mots devant lesquels « le », « la », « de »… s'élident malgré leur initiale
/// (semi-consonnes qui se comportent comme des consonnes).
const NO_ELISION: &[&str] = &[
    "oui",
    "onze",
    "onzième",
    "onzièmes",
    "ouate",
    "ouistiti",
    "ouistitis",
    "uhlan",
    "ululer",
];

/// « Je ai » → « J’ai », « le arbre » → « l’arbre », « que il » → « qu’il ».
fn elision(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    let base = match w.lower.as_str() {
        "je" => "j’",
        "me" => "m’",
        "te" => "t’",
        "se" => "s’",
        "le" | "la" => "l’",
        "de" => "d’",
        "ne" => "n’",
        "que" => "qu’",
        "ce" => "c’",
        "si" => "s’",
        "lorsque" => "lorsqu’",
        "puisque" => "puisqu’",
        "quoique" => "quoiqu’",
        _ => return,
    };
    if w.compound_part {
        return;
    }
    let Some(n) = ctx.next(i) else { return };
    let nw = &ctx.words[n];
    if nw.compound_part || nw.ignored || nw.has_digit || nw.text.is_empty() {
        return;
    }
    // Un seul caractère : « de A à Z » ; seul « y » s'élide (« je y vais »).
    if nw.text.chars().count() == 1 && nw.lower != "y" {
        return;
    }
    let elides = ctx.data.elides(&nw.lower, None)
        && !NO_ELISION.contains(&nw.lower.as_str())
        && (nw.lower == "y" || !nw.lower.starts_with('y'));
    if !elides {
        return;
    }
    // « ce » seulement devant être (« c’est »), « si » devant « il(s) ».
    match w.lower.as_str() {
        "ce" if !matches!(nw.lower.as_str(), "est" | "était" | "étaient" | "eût") => return,
        "si" if !matches!(nw.lower.as_str(), "il" | "ils") => return,
        // « Le Havre », « La Anne » : titres et noms propres.
        "le" | "la" if is_capitalized(nw.text) => return,
        _ => {}
    }
    // Le pronom après un impératif est lié par un trait d'union : déjà écarté
    // (compound_part). Un « la » ou « le » suivi de « a » peut être la lettre.
    let fixed = same_case(w.text, &format!("{base}{}", nw.text));
    let found = &ctx.text[w.start..nw.end];
    // « le école » : l'élision règle aussi le genre (« l’ » n'en a pas).
    out.retain(|o| !(o.rule == "GRAM_ACCORD_DETERMINANT" && o.start < nw.end && w.start < o.end));
    out.push(
        RawIssue::new(w.start, nw.end, ELISION)
            .arg(found)
            .arg(fixed.clone())
            .suggest(fixed),
    );
}

/// « à le » → « au », « de les » → « des », devant un nom.
fn contraction(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    if !matches!(w.lower.as_str(), "à" | "de") || w.compound_part {
        return;
    }
    let Some(a) = ctx.next(i) else { return };
    let article = lower(ctx, a);
    let fixed = match (w.lower.as_str(), article) {
        ("à", "le") => "au",
        ("à", "les") => "aux",
        ("de", "le") => "du",
        ("de", "les") => "des",
        _ => return,
    };
    let Some(n) = ctx.next(a) else { return };
    let noun_only = ctx.form(n).is_some_and(|f| {
        f.has_pos(Pos::Nom)
            && !f.has(|r| {
                r.pos.is_verb() || matches!(r.pos, Pos::Adv | Pos::Pre | Pos::Pro | Pos::Con)
            })
    });
    if !noun_only || ctx.words[n].compound_part {
        return;
    }
    let found = &ctx.text[w.start..ctx.words[a].end];
    let fixed = same_case(w.text, fixed);
    out.push(
        RawIssue::new(w.start, ctx.words[a].end, CONTRACTION)
            .arg(found)
            .arg(fixed.clone())
            .suggest(fixed),
    );
}

const SUBJECT_PRONOUNS: &[&str] = &[
    "il", "elle", "on", "ils", "elles", "je", "tu", "nous", "vous",
];
const INTERROGATIVES: &[&str] = &[
    "où",
    "quand",
    "comment",
    "pourquoi",
    "combien",
    "que",
    "qu’",
    "qui",
    "quoi",
    "quel",
    "quels",
    "quelle",
    "quelles",
    "lequel",
    "laquelle",
    "lesquels",
    "lesquelles",
];

/// La phrase qui contient la position `from` se termine-t-elle par « ? » ?
fn ends_with_question(ctx: &Context, from: usize) -> bool {
    ctx.text[from..]
        .chars()
        .find(|c| matches!(c, '.' | '!' | '?' | '\n'))
        .is_some_and(|c| c == '?')
}

/// Le mot d'indice `i` commence-t-il une phrase ? (Les parties d'un mot composé
/// n'ont pas l'indicateur : on regarde le texte qui précède.)
fn opens_sentence(ctx: &Context, i: usize) -> bool {
    let before = ctx.text[..ctx.words[i].start].trim_end_matches(|c: char| {
        c.is_whitespace() || matches!(c, '«' | '"' | '“' | '(' | '—' | '–' | '-')
    });
    before.is_empty() || before.ends_with(['.', '!', '?', '…', '\n'])
}

/// « Peut on » → « Peut-on », « Que dit il » → « Que dit-il », « A-t il » → « A-t-il ».
fn inversion(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let p = &ctx.words[i];
    if !SUBJECT_PRONOUNS.contains(&p.lower.as_str()) || p.compound_part {
        return;
    }
    let Some(v) = ctx.prev(i) else { return };
    let vw = &ctx.words[v];
    if vw.ignored || !ends_with_question(ctx, p.end) {
        return;
    }
    // Le verbe ouvre la question, ou suit un mot interrogatif.
    let opens = vw.sentence_start
        || ctx.prev(v).is_some_and(|q| {
            INTERROGATIVES.contains(&lower(ctx, q))
                || ctx.words[q].sentence_start && INTERROGATIVES.contains(&lower(ctx, q))
        });
    // « A-t il » : « t » est la dernière partie d'un composé commencé par le verbe.
    let t_part = vw.compound_part && vw.lower == "t";
    let compound_start = t_part
        && v >= 1
        && (opens_sentence(ctx, v - 1)
            || ctx
                .prev(v - 1)
                .is_some_and(|q| INTERROGATIVES.contains(&lower(ctx, q))));
    if !(opens && !vw.compound_part || compound_start) {
        return;
    }
    let verb_ok = t_part
        || ctx
            .form(v)
            .is_some_and(|f| f.has(Reading::is_finite_verb) || f.has(|r| r.pos == Pos::Aux));
    if !verb_ok {
        return;
    }
    // Le pronom doit être séparé du verbe par des espaces seulement.
    if !ctx.text[vw.end..p.start].chars().all(char::is_whitespace) {
        return;
    }
    let vowel_end =
        vw.lower.ends_with(['a', 'e']) && matches!(p.lower.as_str(), "il" | "elle" | "on");
    let joiner = if vowel_end && !t_part { "-t-" } else { "-" };
    let start = if t_part {
        ctx.words[v - 1].start
    } else {
        vw.start
    };
    let found = &ctx.text[start..p.end];
    let fixed = format!("{}{joiner}{}", &ctx.text[start..vw.end], p.text);
    out.push(
        RawIssue::new(start, p.end, HYPHEN)
            .arg(found)
            .arg(fixed.clone())
            .suggest(fixed),
    );
}

/// « Est ce que » → « Est-ce que », « n’est ce pas » → « n’est-ce pas ».
fn est_ce(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let c = &ctx.words[i];
    if c.lower != "ce" || c.compound_part {
        return;
    }
    let Some(e) = ctx.prev(i) else { return };
    if lower(ctx, e) != "est" || ctx.words[e].compound_part {
        return;
    }
    let Some(n) = ctx.next(i) else { return };
    if !matches!(lower(ctx, n), "que" | "qu’" | "qui" | "pas") {
        return;
    }
    // « Il est ce que je cherchais » : après un sujet, c'est correct.
    let before = ctx.prev(e);
    let opens = ctx.words[e].sentence_start
        || before.is_some_and(|q| {
            INTERROGATIVES.contains(&lower(ctx, q)) || matches!(lower(ctx, q), "n’" | "ne")
        });
    if !opens {
        return;
    }
    let found = &ctx.text[ctx.words[e].start..c.end];
    let fixed = format!("{}-{}", ctx.words[e].text, c.text);
    out.push(
        RawIssue::new(ctx.words[e].start, c.end, HYPHEN)
            .arg(found)
            .arg(fixed.clone())
            .suggest(fixed),
    );
}

/// « Il est sensé le savoir » → « censé » ; « près à partir » → « prêt à partir ».
fn sense(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    let fixed = match w.lower.as_str() {
        "sensé" => "censé",
        "sensée" => "censée",
        "sensés" => "censés",
        "sensées" => "censées",
        "près" => "prêt",
        _ => return,
    };
    // Le mot qui suit (après d'éventuels pronoms) est un infinitif.
    let mut k = ctx.next(i);
    if w.lower == "près" {
        // « près à partir » : « à » puis un infinitif, après une forme d'être.
        let after_etre = ctx
            .prev(i)
            .and_then(|p| ctx.form(p))
            .is_some_and(|f| f.has(|r| r.lemma == "être"));
        if !after_etre || k.is_none_or(|a| lower(ctx, a) != "à") {
            return;
        }
        k = k.and_then(|a| ctx.next(a));
        if !k
            .and_then(|n| ctx.form(n))
            .is_some_and(|f| f.has(Reading::is_infinitive))
        {
            return;
        }
    } else {
        while let Some(j) = k {
            let clitic = matches!(
                lower(ctx, j),
                "le" | "la"
                    | "les"
                    | "l’"
                    | "y"
                    | "en"
                    | "me"
                    | "te"
                    | "se"
                    | "nous"
                    | "vous"
                    | "lui"
                    | "leur"
                    | "ne"
                    | "n’"
                    | "pas"
            );
            if clitic {
                k = ctx.next(j);
            } else {
                break;
            }
        }
        if !k
            .and_then(|n| ctx.form(n))
            .is_some_and(|f| f.has(Reading::is_infinitive))
        {
            return;
        }
    }
    let fixed = same_case(w.text, fixed);
    out.push(
        RawIssue::new(w.start, w.end, SENSE)
            .arg(w.text)
            .arg(fixed.clone())
            .suggest(fixed),
    );
}

/// « une effraction au code de la route » → « infraction » : mot proche employé
/// pour un autre, trahi par un mot du voisinage (`paronymes.tsv`).
fn paronym(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    let Some((right, triggers)) = ctx.data.paronyms.get(&w.lower) else {
        return;
    };
    // Fenêtre de quatre mots de part et d'autre, sans sortir de la phrase.
    let mut hit = false;
    for dir in [-1i64, 1] {
        let mut k = i as i64;
        for _ in 0..4 {
            k += dir;
            if k < 0 || k as usize >= ctx.words.len() {
                break;
            }
            let u = k as usize;
            let (a, b) = if dir < 0 { (u, i) } else { (i, u) };
            if ctx.text[ctx.words[a].end..ctx.words[b].start].contains(['.', '!', '?', '\n']) {
                break;
            }
            if dir > 0 && ctx.words[u].sentence_start {
                break;
            }
            if triggers.contains(&ctx.words[u].lower) {
                hit = true;
            }
            if dir < 0 && ctx.words[u].sentence_start {
                break;
            }
        }
    }
    if !hit {
        return;
    }
    let fixed = same_case(w.text, right);
    out.push(
        RawIssue::new(w.start, w.end, PARONYM)
            .arg(w.text)
            .arg(fixed.clone())
            .suggest(fixed),
    );
}

/// Verbes qui se construisent avec « de » : « le livre que je t’ai parlé » →
/// « dont ».
const VERBS_WITH_DE: &[&str] = &[
    "parler", "profiter", "souvenir", "moquer", "plaindre", "servir",
];

/// « Le livre que je t’ai parlé » → « dont ».
fn dont(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let q = &ctx.words[i];
    if !matches!(q.lower.as_str(), "que" | "qu’") || q.sentence_start {
        return;
    }
    // L'antécédent est un nom (« le livre que ») ou « ce » ; pas « il faut que »,
    // « parce que », « bien que ».
    let Some(a) = ctx.prev(i) else { return };
    // « montrer aux autres qu’elle se moquait » : « que » conjonction après un
    // complément introduit par « à », « aux » ; pas non plus « du tout qu’on ».
    const ARTICLES: &[&str] = &[
        "le", "la", "l’", "les", "ce", "cet", "cette", "ces", "mon", "ma", "mes", "ton", "ta",
        "tes", "son", "sa", "ses", "notre", "nos", "votre", "vos", "un", "une",
    ];
    let antecedent = lower(ctx, a) == "ce"
        || ctx.form(a).is_some_and(|f| f.has_pos(Pos::Nom))
            && !matches!(lower(ctx, a), "tout" | "rien" | "fois")
            && ctx
                .prev(a)
                .is_some_and(|d| ARTICLES.contains(&lower(ctx, d)));
    if !antecedent {
        return;
    }
    // Sujet pronom, pronoms compléments, puis le verbe (ou son participe).
    let mut k = ctx.next(i);
    let Some(s) = k else { return };
    if !matches!(
        lower(ctx, s),
        "je" | "j’" | "tu" | "il" | "elle" | "on" | "nous" | "vous" | "ils" | "elles"
    ) {
        return;
    }
    k = ctx.next(s);
    let mut found = None;
    for _ in 0..5 {
        let Some(j) = k else { break };
        let Some(f) = ctx.form(j) else { break };
        if f.has(|r| {
            r.pos.is_verb()
                && VERBS_WITH_DE.contains(&r.lemma.as_str())
                && (r.is_finite_verb() || r.is_past_participle())
        }) {
            // « la langue que je parle » : parler est transitif ; « le livre que
            // je t’ai parlé » a un pronom datif.
            let dative = (s + 1..j)
                .any(|x| matches!(lower(ctx, x), "me" | "m’" | "te" | "t’" | "lui" | "leur"));
            if !f.has(|r| r.lemma == "parler") || dative {
                found = Some(j);
                break;
            }
        }
        // « ce que j’ai besoin » : avoir besoin, envie, honte… de.
        if matches!(lower(ctx, j), "besoin" | "envie" | "honte" | "horreur")
            && ctx
                .next(j)
                .is_none_or(|n| !matches!(lower(ctx, n), "de" | "d’" | "du" | "des"))
            && ctx
                .prev(j)
                .and_then(|p| ctx.form(p))
                .is_some_and(|f| f.has(|r| r.lemma == "avoir"))
        {
            found = Some(j);
            break;
        }
        // Pronoms, négation, auxiliaire : on continue.
        let skip = matches!(
            lower(ctx, j),
            "me" | "m’"
                | "te"
                | "t’"
                | "se"
                | "s’"
                | "lui"
                | "leur"
                | "nous"
                | "vous"
                | "en"
                | "y"
                | "ne"
                | "n’"
                | "pas"
                | "jamais"
                | "déjà"
                | "toujours"
                | "souvent"
                | "encore"
        ) || f.has(|r| r.pos == Pos::Aux && r.is_finite_verb());
        if !skip {
            break;
        }
        k = ctx.next(j);
    }
    let Some(v) = found else { return };
    // « qu’on m’en parlât » : « en » reprend déjà le complément en « de ».
    if (s + 1..v).any(|x| matches!(lower(ctx, x), "en" | "y")) {
        return;
    }
    // « Le plat que je sers » : servir est transitif ; seulement « se servir de ».
    if ctx.form(v).is_some_and(|f| f.has(|r| r.lemma == "servir")) {
        let pronominal =
            (i..v).any(|j| matches!(lower(ctx, j), "me" | "m’" | "te" | "t’" | "se" | "s’"));
        if !pronominal {
            return;
        }
    }
    let fixed = if q.lower == "qu’" { "dont " } else { "dont" };
    out.push(
        RawIssue::new(q.start, q.end, DONT)
            .arg(q.text)
            .arg(fixed)
            .suggest(fixed),
    );
}

/// « Quand-même » : le trait d'union est fautif (« quand même »).
fn hyphenated_wrongly(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let a = &ctx.words[i];
    if a.lower == "quand-même" {
        let fixed = same_case(a.text, "quand même");
        out.push(
            RawIssue::new(a.start, a.end, HYPHEN)
                .arg(a.text)
                .arg(fixed.clone())
                .suggest(fixed),
        );
        return;
    }
    if a.lower != "quand" || !a.compound_part {
        return;
    }
    let Some(b) = ctx
        .next(i)
        .filter(|&b| lower(ctx, b) == "même" && ctx.words[b].compound_part)
    else {
        return;
    };
    if &ctx.text[a.end..ctx.words[b].start] != "-" {
        return;
    }
    let found = &ctx.text[a.start..ctx.words[b].end];
    let fixed = same_case(a.text, "quand même");
    out.push(
        RawIssue::new(a.start, ctx.words[b].end, HYPHEN)
            .arg(found)
            .arg(fixed.clone())
            .suggest(fixed),
    );
}

/// « un restaurent » : après un déterminant sûr, un mot qui n'est qu'une forme
/// verbale est une faute d'écriture d'un nom proche (« restaurant »).
fn noun_expected(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    if w.compound_part || w.has_digit || is_capitalized(w.text) || w.text.chars().count() < 4 {
        return;
    }
    let Some(d) = ctx.prev(i) else { return };
    // Déterminants qui ne peuvent pas être des pronoms (« le », « la », « les »,
    // « leur » précèdent un verbe : « il le mange »).
    let det = lower(ctx, d);
    const SAFE_DETERMINERS: &[&str] = &[
        "un",
        "une",
        "des",
        "cet",
        "cette",
        "ces",
        "mon",
        "ma",
        "mes",
        "ton",
        "ta",
        "tes",
        "son",
        "sa",
        "ses",
        "notre",
        "votre",
        "nos",
        "vos",
        "quelques",
        "plusieurs",
        "chaque",
    ];
    if !SAFE_DETERMINERS.contains(&det) {
        return;
    }
    // « et un font douze » : somme.
    if matches!(det, "un" | "une") && ctx.prev(d).is_some_and(|p| lower(ctx, p) == "et") {
        return;
    }
    let Some(f) = ctx.form(i) else { return };
    if !f.readings.iter().all(|r| r.is_finite_verb()) {
        return;
    }
    // Déjà signalé par une règle plus précise (« un peut de sel » → « peu »).
    if out.iter().any(|o| o.start < w.end && w.start < o.end) {
        return;
    }
    // Un mot de remplacement : nom proche, accordé avec le déterminant.
    let gn = ctx.data.determiners.get(det).map(|x| x.gn);
    let suggestions: Vec<String> =
        crate::suggest::suggest(ctx.lexicon, &ctx.data.elisions, &w.norm, 6)
            .into_iter()
            .filter(|s| {
                ctx.lexicon.get(&s.to_lowercase()).is_some_and(|x| {
                    x.has(|r| {
                        r.pos == Pos::Nom
                            && gn.is_none_or(|g| r.gn().is_none_or(|n| n.compatible(g)))
                    }) && !x.has(|r| r.is_finite_verb())
                })
            })
            .collect();
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    out.push(
        RawIssue::new(w.start, w.end, NOUN_EXPECTED)
            .arg(w.text)
            .arg(first)
            .arg(ctx.words[d].text)
            .with_suggestions(suggestions),
    );
}
