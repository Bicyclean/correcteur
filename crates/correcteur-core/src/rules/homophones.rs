//! Homophones grammaticaux : a/à, ou/où, ce/se, c’/s’, et/est.
//!
//! Ces règles ne s'appuient que sur le contexte immédiat (mot précédent et mot
//! suivant) et sur les lectures du lexique. Elles sont volontairement prudentes :
//! en cas de doute, rien n'est signalé. Les cas figés (« à cause de »,
//! « d’où »…) sont traités par `locutions.rs`.

use super::{Context, RawIssue};
use crate::lexicon::{Form, Pos};
use crate::text::{capitalize, is_capitalized};

pub const A_ACCENT: &str = "HOMO_A_VERBE";
pub const A_SANS_ACCENT: &str = "HOMO_A_PREPOSITION";
pub const OU_ACCENT: &str = "HOMO_OU_RELATIF";
pub const OU_SANS_ACCENT: &str = "HOMO_OU_CONJONCTION";
pub const CE_SE: &str = "HOMO_CE_SE";
pub const SE_CE: &str = "HOMO_SE_CE";
pub const C_S: &str = "HOMO_C_S";
pub const ET_EST: &str = "HOMO_ET_EST";

const SUBJECTS: &[&str] = &["il", "elle", "on", "ils", "elles"];
const RELATIVES: &[&str] = &[
    "qui",
    "quoi",
    "que",
    "qu’",
    "dont",
    "lequel",
    "laquelle",
    "lesquels",
    "lesquelles",
];
const PREPOSITIONS_BEFORE_PRONOUN: &[&str] = &[
    "à", "avec", "chez", "comme", "contre", "de", "entre", "et", "ou", "par", "pour", "sans",
    "sur", "vers", "que",
];

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    for i in 0..ctx.words.len() {
        let w = &ctx.words[i];
        if w.ignored {
            continue;
        }
        let fix = match w.lower.as_str() {
            "à" => check_a_accent(ctx, i).then_some((A_ACCENT, "a")),
            "a" => check_a(ctx, i).then_some((A_SANS_ACCENT, "à")),
            "ou" => check_ou(ctx, i).then_some((OU_ACCENT, "où")),
            "ce" => check_ce(ctx, i).then_some((CE_SE, "se")),
            "se" => check_se(ctx, i).then_some((SE_CE, "ce")),
            "c’" => check_c(ctx, i).then_some((C_S, "s’")),
            "et" => check_et(ctx, i).then_some((ET_EST, "est")),
            _ => None,
        };
        if let Some((rule, replacement)) = fix {
            let replacement = if is_capitalized(w.text) {
                capitalize(replacement)
            } else {
                replacement.to_string()
            };
            out.push(
                RawIssue::new(w.start, w.end, rule)
                    .arg(w.text)
                    .arg(replacement.clone())
                    .suggest(replacement),
            );
        }
    }
}

/// Indice du mot précédent, s'il n'en est séparé que par des blancs.
fn prev(ctx: &Context, i: usize) -> Option<usize> {
    (i > 0 && ctx.words[i].joined_to_prev).then(|| i - 1)
}

/// Indice du mot suivant, s'il n'en est séparé que par des blancs.
fn next(ctx: &Context, i: usize) -> Option<usize> {
    (i + 1 < ctx.words.len() && ctx.words[i + 1].joined_to_prev).then_some(i + 1)
}

fn lower_is(ctx: &Context, i: Option<usize>, set: &[&str]) -> bool {
    i.is_some_and(|i| set.contains(&ctx.words[i].lower.as_str()))
}

fn form<'a>(ctx: &Context<'a>, i: Option<usize>) -> Option<&'a Form> {
    i.and_then(|i| ctx.form(i))
}

/// Sujet placé avant le verbe (pas « dit-il »).
fn is_preverbal_subject(ctx: &Context, i: Option<usize>) -> bool {
    i.is_some_and(|i| {
        !ctx.words[i].compound_part && SUBJECTS.contains(&ctx.words[i].lower.as_str())
    })
}

/// « il à mangé » → « a » ; « il y à » → « a ».
fn check_a_accent(ctx: &Context, i: usize) -> bool {
    let p = prev(ctx, i);
    let n = next(ctx, i);
    if lower_is(ctx, n, RELATIVES) {
        return false; // « c’est elle à qui je pense »
    }
    let after_subject = is_preverbal_subject(ctx, p)
        && !lower_is(
            ctx,
            p.and_then(|p| prev(ctx, p)),
            PREPOSITIONS_BEFORE_PRONOUN,
        );
    let after_il_y =
        lower_is(ctx, p, &["y"]) && lower_is(ctx, p.and_then(|p| prev(ctx, p)), &["il"]);
    let before_participle = form(ctx, n).is_some_and(|f| {
        f.has(|r| r.is_past_participle())
            && !f.has(|r| matches!(r.pos, Pos::Nom | Pos::NomPropre) || r.is_infinitive())
    });
    (after_subject && n.is_some()) || after_il_y || before_participle
}

/// « A Paris » en début de phrase → « À » ; « il commence a pleuvoir » → « à ».
fn check_a(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    let n = next(ctx, i);
    if w.sentence_start && !w.compound_part {
        // En tête de phrase, le verbe « a » n'a pas de sujet : c'est la préposition.
        return n.is_some_and(|n| ctx.words[n].lower != "t")
            && !form(ctx, n).is_some_and(|f| f.only(|r| r.is_past_participle()));
    }
    let p = prev(ctx, i);
    let after_a_verb = form(ctx, p)
        .is_some_and(|f| f.has(|r| r.pos.is_verb() && ctx.data.verbs_with_a.contains(&r.lemma)));
    after_a_verb && form(ctx, n).is_some_and(|f| f.has(|r| r.is_infinitive()))
}

/// « Ou est-il ? » → « Où ».
fn check_ou(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    if !w.sentence_start {
        return false;
    }
    let n = next(ctx, i);
    form(ctx, n).is_some_and(|f| f.has(|r| r.is_finite_verb())) && ends_with_question(ctx, w.end)
}

/// La phrase qui contient la position `from` se termine-t-elle par « ? » ?
fn ends_with_question(ctx: &Context, from: usize) -> bool {
    ctx.text[from..]
        .chars()
        .find(|c| matches!(c, '.' | '!' | '?' | '\n'))
        .is_some_and(|c| c == '?')
}

/// « il ce lave » → « se » ; « pour ce laver » → « se ».
fn check_ce(ctx: &Context, i: usize) -> bool {
    let n = next(ctx, i);
    let Some(f) = form(ctx, n) else {
        return false;
    };
    (is_preverbal_subject(ctx, prev(ctx, i)) && f.has(|r| r.is_finite_verb()))
        || f.only(|r| r.is_infinitive())
}

/// « se que je veux » → « ce » ; « se matin » → « ce ».
fn check_se(ctx: &Context, i: usize) -> bool {
    let n = next(ctx, i);
    lower_is(ctx, n, &["qui", "que", "qu’", "dont", "quoi"])
        || form(ctx, n).is_some_and(|f| f.only(|r| r.pos == Pos::Nom))
}

/// « il c’est trompé » → « s’ ».
fn check_c(ctx: &Context, i: usize) -> bool {
    is_preverbal_subject(ctx, prev(ctx, i)) && next(ctx, i).is_some()
}

/// « il et parti », « c’et vrai » → « est ».
fn check_et(ctx: &Context, i: usize) -> bool {
    let p = prev(ctx, i);
    if lower_is(ctx, p, &["c’"]) {
        return true;
    }
    if !lower_is(ctx, p, &["il", "elle", "on", "cela", "ça"]) {
        return false;
    }
    if lower_is(
        ctx,
        p.and_then(|p| prev(ctx, p)),
        PREPOSITIONS_BEFORE_PRONOUN,
    ) {
        return false;
    }
    form(ctx, next(ctx, i)).is_some_and(|f| {
        f.has(|r| r.pos == Pos::Adj || r.is_past_participle())
            && !f.has(|r| {
                matches!(
                    r.pos,
                    Pos::Pro | Pos::Det | Pos::NomPropre | Pos::Con | Pos::Pre | Pos::Adv
                )
            })
    })
}
