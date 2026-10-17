//! Homophones grammaticaux : a/à, ou/où, ce/se, c’/s’, et/est, on/ont, sa/ça,
//! ses/ces, leur/leurs, quelle/qu’elle, peu/peut.
//!
//! Ces règles ne s'appuient que sur le contexte immédiat (mot précédent et mot
//! suivant) et sur les lectures du lexique. Elles sont volontairement prudentes :
//! en cas de doute, rien n'est signalé. Les cas figés (« à cause de »,
//! « d’où »…) sont traités par `locutions.rs`.

use super::grammar::noun_gender;
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
pub const ON_ONT: &str = "HOMO_ON_ONT";
pub const ONT_ON: &str = "HOMO_ONT_ON";
pub const SA_CA: &str = "HOMO_SA_CA";
pub const CA_SA: &str = "HOMO_CA_SA";
pub const SES_CES: &str = "HOMO_SES_CES";
pub const CES_SES: &str = "HOMO_CES_SES";
pub const LEURS_LEUR: &str = "HOMO_LEURS_LEUR";
pub const QUELLE_QU_ELLE: &str = "HOMO_QUELLE_QU_ELLE";
pub const QU_ELLE_QUELLE: &str = "HOMO_QU_ELLE_QUELLE";
pub const PEU_PEUT: &str = "HOMO_PEU_PEUT";
pub const PEUT_PEU: &str = "HOMO_PEUT_PEU";

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
/// Verbes qui introduisent une interrogation indirecte (« savoir quelle… »).
const INDIRECT_QUESTION_VERBS: &[&str] = &[
    "savoir",
    "ignorer",
    "demander",
    "comprendre",
    "voir",
    "deviner",
    "chercher",
    "imaginer",
    "oublier",
    "apprendre",
    "rappeler",
    "raconter",
    "expliquer",
    "montrer",
    "préciser",
    "indiquer",
    "découvrir",
    "connaître",
    "dire",
    "examiner",
    "vérifier",
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
            "on" => check_on(ctx, i).then_some((ON_ONT, "ont")),
            "ont" => check_ont(ctx, i).then_some((ONT_ON, "on")),
            "sa" => check_sa(ctx, i).then_some((SA_CA, "ça")),
            "ça" => check_ca(ctx, i).then_some((CA_SA, "sa")),
            "ses" => check_ses(ctx, i).then_some((SES_CES, "ces")),
            "ces" => check_ces(ctx, i).then_some((CES_SES, "ses")),
            "leurs" => check_leurs(ctx, i).then_some((LEURS_LEUR, "leur")),
            "quelle" => check_quelle(ctx, i).then_some((QUELLE_QU_ELLE, "qu’elle")),
            "quelles" => check_quelle(ctx, i).then_some((QUELLE_QU_ELLE, "qu’elles")),
            "peu" => check_peu(ctx, i).then_some((PEU_PEUT, "peut")),
            "peut" => check_peut(ctx, i).then_some((PEUT_PEU, "peu")),
            "qu’" => {
                check_qu_elle(ctx, i, out);
                None
            }
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
    let p = ctx.prev(i);
    let n = ctx.next(i);
    if lower_is(ctx, n, RELATIVES) {
        return false; // « c’est elle à qui je pense »
    }
    // « d’elle à ses amis », « malgré elle à tout instant » : pronom après une préposition.
    let before_pronoun = p.and_then(|p| ctx.prev(p));
    let after_subject = is_preverbal_subject(ctx, p)
        && !lower_is(ctx, before_pronoun, PREPOSITIONS_BEFORE_PRONOUN)
        && !form(ctx, before_pronoun).is_some_and(|f| f.has_pos(Pos::Pre));
    let after_il_y =
        lower_is(ctx, p, &["y"]) && lower_is(ctx, p.and_then(|p| ctx.prev(p)), &["il"]);
    // Mot qui ne peut être que participe ou adjectif : « lui » (aussi participe
    // de « luire ») est d'abord un pronom, « à lui » est correct.
    let before_participle = form(ctx, n).is_some_and(|f| {
        f.has(super::super::lexicon::Reading::is_past_participle)
            && f.only(|r| (r.pos.is_verb() && !r.is_infinitive()) || r.pos == Pos::Adj)
    });
    (after_subject && n.is_some()) || after_il_y || before_participle
}

/// « A Paris » en début de phrase → « À » ; « il commence a pleuvoir » → « à ».
fn check_a(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    let n = ctx.next(i);
    if w.sentence_start && !w.compound_part {
        // En tête de phrase, le verbe « a » n'a pas de sujet : c'est la préposition.
        return n.is_some_and(|n| ctx.words[n].lower != "t")
            && !form(ctx, n)
                .is_some_and(|f| f.only(super::super::lexicon::Reading::is_past_participle));
    }
    let p = ctx.prev(i);
    let after_a_verb = form(ctx, p)
        .is_some_and(|f| f.has(|r| r.pos.is_verb() && ctx.data.verbs_with_a.contains(&r.lemma)));
    after_a_verb
        && form(ctx, n).is_some_and(|f| f.has(super::super::lexicon::Reading::is_infinitive))
}

/// « Ou est-il ? » → « Où ».
fn check_ou(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    if !w.sentence_start {
        return false;
    }
    let n = ctx.next(i);
    form(ctx, n).is_some_and(|f| f.has(super::super::lexicon::Reading::is_finite_verb))
        && ends_with_question(ctx, w.end)
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
    let n = ctx.next(i);
    let Some(f) = form(ctx, n) else {
        return false;
    };
    (is_preverbal_subject(ctx, ctx.prev(i))
        && f.has(super::super::lexicon::Reading::is_finite_verb))
        || f.only(super::super::lexicon::Reading::is_infinitive)
}

/// « se que je veux » → « ce » ; « se matin » → « ce ».
fn check_se(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    lower_is(ctx, n, &["qui", "que", "qu’", "dont", "quoi"])
        || form(ctx, n).is_some_and(|f| f.only(|r| r.pos == Pos::Nom))
}

/// « il c’est trompé » → « s’ ».
fn check_c(ctx: &Context, i: usize) -> bool {
    is_preverbal_subject(ctx, ctx.prev(i)) && ctx.next(i).is_some()
}

/// « il et parti », « c’et vrai » → « est ».
fn check_et(ctx: &Context, i: usize) -> bool {
    let p = ctx.prev(i);
    if lower_is(ctx, p, &["c’"]) {
        return true;
    }
    if !lower_is(ctx, p, &["il", "elle", "on", "cela", "ça"]) {
        return false;
    }
    if lower_is(
        ctx,
        p.and_then(|p| ctx.prev(p)),
        PREPOSITIONS_BEFORE_PRONOUN,
    ) {
        return false;
    }
    form(ctx, ctx.next(i)).is_some_and(|f| {
        f.has(|r| r.pos == Pos::Adj || r.is_past_participle())
            && !f.has(|r| {
                matches!(
                    r.pos,
                    Pos::Pro | Pos::Det | Pos::NomPropre | Pos::Con | Pos::Pre | Pos::Adv
                )
            })
    })
}

/// Sujet pluriel juste avant `i` : « ils », « elles », ou groupe nominal
/// simple au pluriel en début de proposition (« les enfants », « mes vieux amis »).
fn plural_subject_before(ctx: &Context, i: usize) -> bool {
    let Some(p) = ctx.prev(i) else {
        return false;
    };
    let pw = &ctx.words[p];
    if pw.compound_part {
        return false;
    }
    if matches!(pw.lower.as_str(), "ils" | "elles") {
        return true;
    }
    if is_capitalized(pw.text) && !pw.sentence_start {
        return false;
    }
    let plural_noun = form(ctx, Some(p)).is_some_and(|f| {
        f.has(|r| r.pos == Pos::Nom)
            && f.only(|r| {
                matches!(r.pos, Pos::Nom | Pos::Adj)
                    && r.gn().is_some_and(|g| g.number == Some('p'))
            })
    });
    if !plural_noun {
        return false;
    }
    // Déterminant pluriel, éventuellement suivi d'un adjectif.
    let mut k = ctx.prev(p);
    for _ in 0..2 {
        let Some(d) = k else { return false };
        if let Some(det) = ctx.data.determiners.get(&ctx.words[d].lower) {
            let dw = &ctx.words[d];
            let clause_start = dw.sentence_start
                || !dw.joined_to_prev
                || ctx
                    .prev(d)
                    .is_some_and(|q| ctx.data.is(&ctx.words[q].lower, "charniere"));
            return det.gn.number == Some('p') && clause_start;
        }
        if !form(ctx, k).is_some_and(|f| f.only(|r| r.pos == Pos::Adj)) {
            return false;
        }
        k = ctx.prev(d);
    }
    false
}

/// « les enfants on visité » → « ont » ; « ils on faim » → « ont ».
fn check_on(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    if n.is_none() || !plural_subject_before(ctx, i) {
        return false;
    }
    let p = ctx.prev(i).map(|p| ctx.words[p].lower.as_str());
    if matches!(p, Some("ils" | "elles")) {
        return true;
    }
    form(ctx, n).is_some_and(|f| {
        f.has(super::super::lexicon::Reading::is_past_participle)
            && !f.has(|r| r.is_finite_verb() || matches!(r.pos, Pos::Nom | Pos::Pro | Pos::Det))
    })
}

/// « Ont dit que… » en tête de phrase → « On ».
fn check_ont(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    w.sentence_start
        && !w.compound_part
        && form(ctx, ctx.next(i)).is_some_and(|f| f.has(|r| r.person() == Some("3s")))
}

/// Le mot qui suit `i` est-il une ponctuation (ou la fin du texte) ?
fn before_punctuation(ctx: &Context, i: usize) -> bool {
    ctx.text[ctx.words[i].end..]
        .chars()
        .find(|c| !c.is_whitespace())
        .is_none_or(|c| matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '…' | ')' | '»'))
}

/// « c’est sa le problème », « sa va », « comme sa. » → « ça ».
fn check_sa(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    let Some(nj) = n else {
        return before_punctuation(ctx, i);
    };
    let nw = &ctx.words[nj];
    if nw.compound_part || matches!(nw.lower.as_str(), "tout" | "toute" | "tous" | "toutes") {
        return false;
    }
    if ctx.data.determiners.contains_key(&nw.lower)
        || ctx.data.is(&nw.lower, "clitique") && !form(ctx, n).is_some_and(|f| f.has_pos(Pos::Nom))
    {
        return true;
    }
    // Verbe conjugué qui ne peut pas être un nom féminin : « sa va », « sa dépend ».
    form(ctx, n).is_some_and(|f| {
        f.has(super::super::lexicon::Reading::is_finite_verb)
            && !f.has(|r| {
                r.pos == Pos::Adj
                    || r.is_past_participle()
                    || r.pos == Pos::Nom && noun_gender(ctx, &r.lemma) != Some('m')
            })
    })
}

/// « ça maison » → « sa » : nom féminin singulier sans autre lecture.
fn check_ca(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    let Some(nj) = n else { return false };
    let nw = &ctx.words[nj];
    if nw.compound_part || is_capitalized(nw.text) && !nw.sentence_start {
        return false;
    }
    // Devant une voyelle, ce serait « son » (son amie) : on s'abstient.
    if nw
        .lower
        .chars()
        .next()
        .is_none_or(|c| "aàâeéèêëiîïoôuûùyhœ".contains(c))
    {
        return false;
    }
    form(ctx, n).is_some_and(|f| {
        f.only(|r| {
            r.pos == Pos::Nom
                && r.gn().is_some_and(|g| g.number != Some('p'))
                && noun_gender(ctx, &r.lemma) == Some('f')
        })
    })
}

/// « ses jours-ci », « ses gens-là » → « ces ».
fn check_ses(ctx: &Context, i: usize) -> bool {
    let Some(n) = ctx.next(i) else { return false };
    let Some(after) = ctx.words.get(n + 1) else {
        return false;
    };
    form(ctx, Some(n)).is_some_and(|f| f.has_pos(Pos::Nom))
        && after.compound_part
        && matches!(after.lower.as_str(), "ci" | "là")
        && &ctx.text[ctx.words[n].end..after.start] == "-"
}

/// « ces propres enfants » → « ses » : « propre » placé avant le nom marque la possession.
fn check_ces(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    lower_is(ctx, n, &["propres"])
        && form(ctx, n.and_then(|n| ctx.next(n))).is_some_and(|f| f.only(|r| r.pos == Pos::Nom))
}

/// « je leurs ai dit », « pour leurs parler » → « leur », pronom invariable.
fn check_leurs(ctx: &Context, i: usize) -> bool {
    let p = ctx.prev(i);
    let after_subject = p.is_some_and(|p| {
        let w = &ctx.words[p];
        !w.compound_part
            && (ctx.data.subjects.contains_key(&w.lower) || matches!(w.lower.as_str(), "ne" | "n’"))
    });
    let n = ctx.next(i);
    let Some(f) = form(ctx, n) else { return false };
    // Sans sujet avant, le mot suivant ne doit pas pouvoir être un nom pluriel
    // (« leurs parler » : « parler » n'est nom ou adjectif qu'au singulier).
    let no_plural_noun = f.only(|r| {
        r.pos.is_verb()
            || matches!(r.pos, Pos::Nom | Pos::Adj) && r.gn().is_some_and(|g| g.number == Some('s'))
    });
    f.has(|r| r.is_finite_verb() || r.is_infinitive()) && (after_subject || no_plural_noun)
}

/// « je pense quelle viendra » → « qu’elle ».
fn check_quelle(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    if w.sentence_start || ctx.prev(i).is_none() {
        return false;
    }
    // « on savait quelles avaient été… », « ni quelle était sa vie » :
    // interrogation indirecte, après « ni » ou un verbe qui l'introduit
    // (mais « je pense quelle viendra » → « qu’elle »).
    let p = ctx.prev(i);
    let asks = form(ctx, p).is_some_and(|f| {
        f.has(|r| r.pos.is_verb() && INDIRECT_QUESTION_VERBS.contains(&r.lemma.as_str()))
    });
    if asks || lower_is(ctx, p, &["ni"]) {
        return false;
    }
    let n = ctx.next(i);
    // « quelle avait été la raison » : sujet inversé après le verbe.
    if lower_is(ctx, n.and_then(|n| ctx.next(n)), &["été"]) {
        return false;
    }
    form(ctx, n).is_some_and(|f| {
        f.has(super::super::lexicon::Reading::is_finite_verb)
            && f.only(|r| {
                r.pos.is_verb()
                    && !matches!(
                        r.lemma.as_str(),
                        "être" | "pouvoir" | "devoir" | "sembler" | "paraître"
                    )
            })
    })
}

/// « qu’elle chance ! » → « quelle » : « qu’elle » devant un nom.
fn check_qu_elle(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(e) = ctx.next(i) else { return };
    let replacement = match ctx.words[e].lower.as_str() {
        "elle" => "quelle",
        "elles" => "quelles",
        _ => return,
    };
    let Some(n) = ctx.next(e) else { return };
    let nw = &ctx.words[n];
    if nw.compound_part || is_capitalized(nw.text) && !nw.sentence_start {
        return;
    }
    if !form(ctx, Some(n)).is_some_and(|f| f.only(|r| r.pos == Pos::Nom)) {
        return;
    }
    let (start, end) = (ctx.words[i].start, ctx.words[e].end);
    let found = &ctx.text[start..end];
    let replacement = if is_capitalized(found) {
        capitalize(replacement)
    } else {
        replacement.to_string()
    };
    out.push(
        RawIssue::new(start, end, QU_ELLE_QUELLE)
            .arg(found)
            .arg(replacement.clone())
            .suggest(replacement),
    );
}

/// « il peu venir », « elle ne peu pas » → « peut ».
fn check_peu(ctx: &Context, i: usize) -> bool {
    let Some(mut p) = ctx.prev(i) else {
        return false;
    };
    let negated = matches!(ctx.words[p].lower.as_str(), "ne" | "n’");
    if negated {
        let Some(q) = ctx.prev(p) else { return false };
        p = q;
    }
    let pw = &ctx.words[p];
    if pw.compound_part || !matches!(pw.lower.as_str(), "il" | "elle" | "on" | "ça" | "cela") {
        return false;
    }
    if lower_is(ctx, ctx.prev(p), PREPOSITIONS_BEFORE_PRONOUN) {
        return false;
    }
    let n = ctx.next(i);
    form(ctx, n).is_some_and(|f| f.has(super::super::lexicon::Reading::is_infinitive))
        || negated && n.is_some_and(|n| ctx.data.is(&ctx.words[n].lower, "negation"))
}

/// « un peut de sel », « à peut près » → « peu ».
fn check_peut(ctx: &Context, i: usize) -> bool {
    if ctx.words[i].compound_part
        || !lower_is(
            ctx,
            ctx.prev(i),
            &["un", "très", "trop", "assez", "si", "à", "quelque"],
        )
    {
        return false;
    }
    match ctx.next(i) {
        None => before_punctuation(ctx, i),
        Some(n) => {
            matches!(
                ctx.words[n].lower.as_str(),
                "de" | "d’" | "près" | "plus" | "moins" | "trop"
            ) || form(ctx, Some(n))
                .is_some_and(|f| f.has_pos(Pos::Adj) && !f.has(|r| r.pos.is_verb()))
        }
    }
}
