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
use crate::text::{capitalize, is_all_upper, is_capitalized};

pub const A_ACCENT: &str = "HOMO_A_VERBE";
pub const A_SANS_ACCENT: &str = "HOMO_A_PREPOSITION";
pub const OU_ACCENT: &str = "HOMO_OU_RELATIF";
pub const OU_SANS_ACCENT: &str = "HOMO_OU_CONJONCTION";
pub const CE_SE: &str = "HOMO_CE_SE";
pub const SE_CE: &str = "HOMO_SE_CE";
pub const C_S: &str = "HOMO_C_S";
pub const S_C: &str = "HOMO_S_C";
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
pub const QUAND_QUANT: &str = "HOMO_QUAND_QUANT";
pub const QUANT_QUAND: &str = "HOMO_QUANT_QUAND";
pub const CES_C_EST: &str = "HOMO_CES_C_EST";
pub const SON_SONT: &str = "HOMO_SON_SONT";
pub const SONT_SON: &str = "HOMO_SONT_SON";
pub const QUELQUE_QUEL_QUE: &str = "HOMO_QUELQUE_QUEL_QUE";

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
        // « la SA », « OU logique » : sigles et mots en capitales.
        if w.ignored
            || (w.text.chars().filter(|c| c.is_alphabetic()).count() > 1 && is_all_upper(w.text))
        {
            continue;
        }
        let fix = match w.lower.as_str() {
            "à" => match first_person_before(ctx, i) {
                Some(form) => Some((A_ACCENT, form)),
                None => check_a_accent(ctx, i).then_some((A_ACCENT, "a")),
            },
            "a" => check_a(ctx, i).then_some((A_SANS_ACCENT, "à")),
            "ou" => check_ou(ctx, i).then_some((OU_ACCENT, "où")),
            "où" => check_ou_accent(ctx, i).then_some((OU_SANS_ACCENT, "ou")),
            "s’" => check_s(ctx, i).then_some((S_C, "c’")),
            "ce" => check_ce(ctx, i).then_some((CE_SE, "se")),
            "se" => check_se(ctx, i).then_some((SE_CE, "ce")),
            "c’" => check_c(ctx, i).then_some((C_S, "s’")),
            "et" => check_et(ctx, i).then_some((ET_EST, "est")),
            "on" => check_on(ctx, i).then_some((ON_ONT, "ont")),
            "ont" => check_ont(ctx, i).then_some((ONT_ON, "on")),
            "sa" => check_sa(ctx, i).then_some((SA_CA, "ça")),
            "ça" => check_ca(ctx, i).then_some((CA_SA, "sa")),
            "ses" => check_ses(ctx, i).then_some((SES_CES, "ces")),
            "ces" => match check_ces_cest(ctx, i) {
                true => Some((CES_C_EST, "c’est")),
                false => check_ces(ctx, i).then_some((CES_SES, "ses")),
            },
            "leurs" => check_leurs(ctx, i).then_some((LEURS_LEUR, "leur")),
            "quelle" => check_quelle(ctx, i).then_some((QUELLE_QU_ELLE, "qu’elle")),
            "quelles" => check_quelle(ctx, i).then_some((QUELLE_QU_ELLE, "qu’elles")),
            "peu" => check_peu(ctx, i),
            "quant" => check_quant(ctx, i).then_some((QUANT_QUAND, "quand")),
            "peut" => check_peut(ctx, i).then_some((PEUT_PEU, "peu")),
            "quand" => check_quand(ctx, i).then_some((QUAND_QUANT, "quant")),
            "son" => check_son(ctx, i).then_some((SON_SONT, "sont")),
            "sont" => check_sont(ctx, i).then_some((SONT_SON, "son")),
            "qu’" => {
                check_qu_elle(ctx, i, out);
                None
            }
            "quelque" | "quelques" => {
                check_quelque(ctx, i, out);
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

fn form<'a>(ctx: &'a Context, i: Option<usize>) -> Option<&'a Form> {
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
    // « a commencé à parlé » : après un verbe qui se construit avec « à », c'est la
    // préposition, même devant un participe (une autre règle relève le participe).
    if form(ctx, p).is_some_and(|f| {
        f.has(|r| r.pos.is_verb() && !r.is_infinitive() && ctx.data.verbs_with_a.contains(&r.lemma))
            && !f.has_pos(Pos::Nom)
    }) {
        return false;
    }
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
    // « à René d’Anjou » : nom propre, pas le participe de renaître.
    let before_participle = !n.is_some_and(|k| is_capitalized(ctx.words[k].text))
        && form(ctx, n).is_some_and(|f| {
            f.has(super::super::lexicon::Reading::is_past_participle)
                && f.only(|r| (r.pos.is_verb() && !r.is_infinitive()) || r.pos == Pos::Adj)
        });
    // « c’est elle qui à raison » : « qui » sujet, puis une locution avec avoir.
    let after_qui = lower_is(ctx, p, &["qui"])
        && lower_is(
            ctx,
            n,
            &[
                "raison", "tort", "faim", "soif", "peur", "besoin", "envie", "froid", "chaud",
                "honte", "sommeil", "hâte", "mal",
            ],
        );
    // « on vous à vu », « il en à fait », « il n’y à pas » : pronoms entre le
    // sujet et le verbe.
    let mut k = p;
    let mut clitics = 0;
    while k.is_some_and(|q| {
        matches!(
            ctx.words[q].lower.as_str(),
            "ne" | "n’"
                | "y"
                | "en"
                | "me"
                | "m’"
                | "te"
                | "t’"
                | "se"
                | "s’"
                | "nous"
                | "vous"
                | "lui"
                | "leur"
                | "le"
                | "la"
                | "l’"
                | "les"
        )
    }) {
        clitics += 1;
        k = k.and_then(|q| ctx.prev(q));
    }
    let subject_then_clitics = clitics > 0
        && is_preverbal_subject(ctx, k)
        && !k
            .and_then(|q| ctx.prev(q))
            .is_some_and(|q| form(ctx, Some(q)).is_some_and(|f| f.has_pos(Pos::Pre)));
    // « à été » n'existe pas : « a été ». Participe après un nom ou un pronom
    // sujet (« la promesse à été acceptée », « la mère Giry à repris »).
    let etre = lower_is(ctx, n, &["été"]);
    // Participe sans autre lecture de nom, pronom ou adverbe : « à vue », « à
    // mort », « quant à lui », « à plus de » sont des compléments.
    let pp_after_subject = form(ctx, n).is_some_and(|f| {
        f.has(super::super::lexicon::Reading::is_past_participle)
            && !f.has(|r| matches!(r.pos, Pos::Nom | Pos::Pro | Pos::Adv | Pos::Pre))
    }) && !n.is_some_and(|k| is_capitalized(ctx.words[k].text))
        && p.is_some_and(|q| {
            let qw = &ctx.words[q];
            (is_capitalized(qw.text) && !qw.sentence_start)
                || matches!(qw.lower.as_str(), "lui" | "eux" | "aussi" | "cela" | "ça")
                || form(ctx, Some(q)).is_some_and(|f| f.only(|r| r.pos == Pos::Nom))
        })
        && !lower_is(ctx, n.and_then(|n| ctx.next(n)), &["de", "d’", "à"]);
    (after_subject && n.is_some())
        || after_il_y
        || before_participle
        || after_qui
        || (subject_then_clitics && n.is_some())
        || etre
        || pp_after_subject
}

/// « A Paris » en début de phrase → « À » ; « il commence a pleuvoir » → « à ».
fn check_a(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    let n = ctx.next(i);
    // « A. W. Mackenzie » : initiale. « A History of Chile », « Tom Jones, a
    // Foundling » : article anglais devant un mot inconnu du lexique.
    // « a priori », « a contrario » : locutions latines, sans accent.
    // Titres anglais : « A Concise History », « Contrasts : a Concerto for… ».
    let n2 = n.and_then(|n| ctx.next(n));
    let unknown = |k: Option<usize>| k.is_some_and(|k| ctx.form(k).is_none());
    let english = lower_is(
        ctx,
        n2,
        &["of", "in", "the", "and", "for", "to", "from", "with", "on"],
    ) || (n.is_some_and(|k| is_capitalized(ctx.words[k].text))
        && n2.is_some_and(|k| is_capitalized(ctx.words[k].text) || ctx.form(k).is_none()));
    // « I don't have a gun » : citation anglaise.
    let english = english
        || lower_is(
            ctx,
            ctx.prev(i),
            &[
                "have", "has", "had", "is", "was", "be", "got", "get", "with", "for", "as",
            ],
        );
    if ctx.text[w.end..].starts_with('.')
        || unknown(n)
        || unknown(ctx.prev(i))
        || english
        || lower_is(
            ctx,
            n,
            &[
                "priori",
                "posteriori",
                "contrario",
                "fortiori",
                "minima",
                "maxima",
                "capella",
                "cappella",
            ],
        )
    {
        return false;
    }
    if w.sentence_start && !w.compound_part {
        // En tête de phrase, le verbe « a » n'a pas de sujet : c'est la préposition.
        // « la Warner Bros. a décidé » : point d'abréviation, puis le verbe.
        return n.is_some_and(|n| ctx.words[n].lower != "t")
            && !form(ctx, n)
                .is_some_and(|f| f.has(super::super::lexicon::Reading::is_past_participle));
    }
    // « il réalise A Carioca » : « A » en capitale au milieu d'une phrase, titre.
    if w.text == "A" {
        return false;
    }
    let p = ctx.prev(i);
    // « Suite a votre message » : en tête de phrase, devant un groupe nominal ;
    // pas « Suite a été publiée ».
    if p.is_some_and(|q| ctx.words[q].sentence_start && ctx.words[q].lower == "suite")
        && n.is_some_and(|k| {
            let nw = &ctx.words[k];
            ctx.data.determiners.contains_key(nw.lower.as_str())
                && !form(ctx, Some(k))
                    .is_some_and(|f| f.has(super::super::lexicon::Reading::is_past_participle))
        })
    {
        return true;
    }
    let after_a_verb = form(ctx, p)
        .is_some_and(|f| f.has(|r| r.pos.is_verb() && ctx.data.verbs_with_a.contains(&r.lemma)));
    if after_a_verb
        && form(ctx, n).is_some_and(|f| f.has(super::super::lexicon::Reading::is_infinitive))
    {
        return true;
    }
    // « Je reste a votre disposition », « il habite a Paris » : un verbe conjugué
    // précédé de son sujet, puis « a » devant un groupe nominal ou un nom propre.
    // Deux verbes conjugués ne se suivent pas : « a » est la préposition.
    // « il y a », « il en a » : pronom entre le sujet et le verbe avoir.
    if p.is_some_and(|p| ctx.data.is(&ctx.words[p].lower, "clitique") || ctx.words[p].lower == "en")
    {
        return false;
    }
    // « Ce qu’il dit a du sens » : après une relative, « a » est le verbe principal.
    let relative_before = p.is_some_and(|q| {
        (q.saturating_sub(4)..q).any(|k| {
            matches!(
                ctx.words[k].lower.as_str(),
                "qui" | "que" | "qu’" | "quoi" | "dont" | "où"
            )
        })
    });
    let verb_with_subject = !relative_before
        && form(ctx, p).is_some_and(|f| f.has(super::super::lexicon::Reading::is_finite_verb))
        && p.and_then(|p| ctx.prev(p)).is_some_and(|s| {
            ctx.data.subjects.contains_key(ctx.words[s].lower.as_str())
                && !ctx.words[s].compound_part
        });
    let nominal = n.is_some_and(|n| {
        let nw = &ctx.words[n];
        ctx.data.determiners.contains_key(nw.lower.as_str())
            || (is_capitalized(nw.text) && !nw.sentence_start)
            || matches!(
                nw.lower.as_str(),
                "moi" | "toi" | "lui" | "eux" | "elles" | "nous" | "vous"
            )
    });
    if verb_with_subject && nominal {
        return true;
    }
    // « il parle sans cesse a ses amis », « il écrit souvent a Marie » : adverbe
    // après un verbe, puis le complément introduit par « à ».
    let adverb_end = p.map(|q| {
        if lower_is(ctx, Some(q), &["cesse"]) && lower_is(ctx, ctx.prev(q), &["sans"]) {
            ctx.prev(q).unwrap_or(q)
        } else {
            q
        }
    });
    let adverb_then_a = adverb_end.is_some_and(|q| {
        let adverb = ctx.data.is(&ctx.words[q].lower, "adverbe")
            || form(ctx, Some(q)).is_some_and(|f| f.has_pos(Pos::Adv) && !f.has_pos(Pos::Nom))
            || lower_is(ctx, Some(q), &["sans"]);
        adverb
            && form(ctx, ctx.prev(q)).is_some_and(|f| {
                f.has(super::super::lexicon::Reading::is_finite_verb)
                    && !f.has(|r| matches!(r.pos, Pos::Det | Pos::Pre))
            })
            // Un sujet pronom ou un nom propre devant le verbe : pas « ce calme même a ».
            && ctx.prev(q).and_then(|v| ctx.prev(v)).is_some_and(|sub| {
                ctx.data.subjects.contains_key(ctx.words[sub].lower.as_str())
                    || (is_capitalized(ctx.words[sub].text) && !ctx.words[sub].sentence_start)
            })
    });
    if adverb_then_a && nominal {
        return true;
    }
    // Le verbe « a » est suivi d'un participe (« a été », « a fini ») ou d'une
    // locution avec avoir (« a raison », « a lieu ») : on s'abstient alors.
    let pp = |k: Option<usize>| {
        form(ctx, k).is_some_and(|f| f.has(super::super::lexicon::Reading::is_past_participle))
    };
    // « a longtemps été », « a déjà fini » : participe après un adverbe.
    let verbal_after = pp(n)
        || (form(ctx, n).is_some_and(|f| f.has_pos(Pos::Adv)) && pp(n2))
        || lower_is(ctx, n, AVOIR_NOUNS);
    if verbal_after {
        return false;
    }
    // Locutions prépositives ou adverbiales : « tout a coup », « a jamais »,
    // « a cause de », « a la mode »…
    let nn = n.and_then(|n| ctx.next(n));
    // Un sujet juste avant (« il a jamais vu », « Pierre a peine ») : verbe avoir.
    let subject_before = p.is_some_and(|q| {
        let qw = &ctx.words[q];
        ctx.data.subjects.contains_key(qw.lower.as_str())
            || (is_capitalized(qw.text) && !qw.sentence_start)
            || form(ctx, Some(q)).is_some_and(|f| f.has(|r| matches!(r.pos, Pos::Nom | Pos::Pro)))
    });
    let locution = !subject_before
        && lower_is(
            ctx,
            n,
            &[
                "jamais",
                "travers",
                "peine",
                "présent",
                "nouveau",
                "propos",
                "condition",
                "côté",
                "partir",
                "cause",
            ],
        )
        || (lower_is(ctx, p, &["tout"]) && lower_is(ctx, n, &["coup", "fait", "l’heure"]))
        || (lower_is(ctx, n, &["la"])
            && lower_is(ctx, nn, &["fois", "mode", "suite", "place", "fin"]))
        || (lower_is(ctx, n, &["peu"]) && lower_is(ctx, nn, &["près"]));
    // Après une forme d'être (« c’est a ce moment »), un infinitif (« se
    // dévouer a la science ») ou un verbe conjugué sans nom possible (« vous
    // aurez a manger ») : deux verbes conjugués ne se suivent pas.
    // « le musée de Vienne a longtemps été » : nom propre sujet (« Vienne »
    // n'est pas le verbe venir).
    let name_before =
        p.is_some_and(|q| is_capitalized(ctx.words[q].text) && !ctx.words[q].sentence_start);
    let after_verb = !relative_before
        && !name_before
        && form(ctx, p).is_some_and(|f| {
            let lone_verb = !f.has(|r| matches!(r.pos, Pos::Nom | Pos::Adj | Pos::Pro | Pos::Adv));
            f.has(|r| r.lemma == "être" && r.is_finite_verb())
                || (lone_verb && f.has(|r| r.is_finite_verb() || r.is_infinitive()))
        });
    // Nom propre après « a », sans sujet devant : « naturelle a Raoul ».
    let name_after = n
        .is_some_and(|k| is_capitalized(ctx.words[k].text) && !ctx.words[k].sentence_start)
        && p.is_some()
        && !p.is_some_and(|q| {
            is_capitalized(ctx.words[q].text)
                || ctx.data.subjects.contains_key(ctx.words[q].lower.as_str())
                || form(ctx, Some(q)).is_some_and(|f| f.has_pos(Pos::Nom))
        });
    // « nous sommes allés a la plage », « il a travaillé a Lyon » : temps
    // composé déjà complet (auxiliaire puis participe) ; pas « le gâteau mangé
    // a disparu », où le participe suit un nom.
    let after_compound = p.is_some_and(|q| {
        form(ctx, Some(q))
            .is_some_and(|f| f.has(super::super::lexicon::Reading::is_past_participle))
            && form(ctx, ctx.prev(q))
                .is_some_and(|f| f.has(|r| r.pos == Pos::Aux && r.is_finite_verb()))
    });
    locution || after_verb || name_after || after_compound
}

/// Noms qui forment avec avoir une locution sans déterminant (« il a raison »).
const AVOIR_NOUNS: &[&str] = &[
    "raison",
    "tort",
    "faim",
    "soif",
    "peur",
    "besoin",
    "envie",
    "froid",
    "chaud",
    "honte",
    "sommeil",
    "hâte",
    "mal",
    "lieu",
    "trait",
    "affaire",
    "droit",
    "coutume",
    "horreur",
    "confiance",
    "cours",
    "beau",
    "l’air",
    "pitié",
    "soin",
    "part",
    "recours",
    "accès",
];

/// « Ou est-il ? » → « Où ».
fn check_ou(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    if !w.sentence_start {
        // « l’âge ou j’entrerais », « Constantinople ou il était » : après un
        // nom, devant un pronom sujet, « ou » ne relie rien : c'est le relatif.
        let p = ctx.prev(i);
        let after_noun = p.is_some_and(|p| {
            let pw = &ctx.words[p];
            // « sortirons ensemble ou nous mourrons » : adverbe, « ou » coordonne ;
            // mais « je ne sais pas ou il habite » : négation.
            let adverb = !ctx.data.is(&pw.lower, "negation");
            (is_capitalized(pw.text) && !pw.sentence_start)
                || form(ctx, Some(p)).is_some_and(|f| {
                    f.has_pos(Pos::Nom)
                        && !f.has(|r| {
                            r.pos.is_verb() || r.pos == Pos::Adj || (adverb && r.pos == Pos::Adv)
                        })
                })
        });
        let n = ctx.next(i);
        let lowercase_next = n.is_some_and(|k| !is_capitalized(ctx.words[k].text));
        let subject = lowercase_next
            && lower_is(
                ctx,
                n,
                &[
                    "je", "j’", "tu", "il", "elle", "on", "nous", "vous", "ils", "elles", "l’on",
                ],
            );
        // « dites-moi ou en est le projet », « savoir ou il va » : interrogation
        // indirecte. Devant un pronom sujet, seulement après savoir, ignorer,
        // demander (« il faut le dire ou il se fâchera »).
        // « dites-moi ou en est », « dis-lui ou il va » : pronom entre les deux.
        let v = if lower_is(
            ctx,
            p,
            &["moi", "nous", "lui", "leur", "me", "m’", "vous", "te", "t’"],
        ) {
            p.and_then(|p| ctx.prev(p))
                .or_else(|| p.and_then(|p| p.checked_sub(1)))
        } else {
            p
        };
        let asks = |lemmas: &[&str]| {
            form(ctx, v)
                .is_some_and(|f| f.has(|r| r.pos.is_verb() && lemmas.contains(&r.lemma.as_str())))
        };
        // Pas « on sait ou on ne sait pas » : alternative.
        let alternative = lower_is(ctx, n.and_then(|n| ctx.next(n)), &["ne", "n’", "n'"])
            || ctx.words[i + 1..]
                .iter()
                .take(4)
                .any(|x| v.is_some_and(|v| ctx.words[v].lower == x.lower));
        if (lower_is(ctx, n, &["en", "y"]) && asks(INDIRECT_QUESTION_VERBS))
            || (subject && !alternative && asks(&["savoir", "ignorer", "demander"]))
        {
            return true;
        }
        // « partout où il va », mais « partout ou presque ».
        let partout = lower_is(ctx, p, &["partout"])
            && n.is_some_and(|k| !is_capitalized(ctx.words[k].text))
            && !lower_is(ctx, n, &["presque", "ailleurs", "nulle", "non"]);
        return partout
            || after_noun && subject && !lower_is(ctx, n.and_then(|n| ctx.next(n)), &["ou", "et"]);
    }
    let n = ctx.next(i);
    // « … du roman ? Ou serait-elle plutôt… ? » : seconde branche d'une
    // question alternative.
    let alternative = ctx.text[..w.start].trim_end().ends_with('?');
    form(ctx, n).is_some_and(|f| f.has(super::super::lexicon::Reading::is_finite_verb))
        && ends_with_question(ctx, w.end)
        && !alternative
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
    // « pour ce faire » : locution correcte.
    if lower_is(ctx, ctx.prev(i), &["pour"]) && lower_is(ctx, n, &["faire"]) {
        return false;
    }
    let Some(f) = form(ctx, n) else {
        return false;
    };
    // « Ma collègue ce charge de votre dossier » : un nom sujet, puis un mot qui
    // est à la fois forme verbale et nom féminin, donc impossible après « ce »
    // masculin (« ce poste de travail » reste correct).
    // La virgule d'incise (« Madame Petit, ma collègue, ce charge ») compte pour rien.
    let before = ctx.prev(i).or_else(|| {
        i.checked_sub(1)
            .filter(|&b| ctx.text[ctx.words[b].end..ctx.words[i].start].trim() == ",")
    });
    let after_noun = form(ctx, before).is_some_and(|p| {
        p.has_pos(Pos::Nom)
            && !p.has(|r| r.pos.is_verb() || matches!(r.pos, Pos::Adv | Pos::Pre | Pos::Con))
    });
    let feminine_only = f
        .readings
        .iter()
        .filter(|r| r.pos == Pos::Nom)
        .all(|r| noun_gender(ctx, &r.lemma) == Some('f'))
        && f.has_pos(Pos::Nom);
    let verb_form = f.has(|r| r.is_finite_verb() && r.person() == Some("3s"));
    (is_preverbal_subject(ctx, ctx.prev(i))
        && f.has(super::super::lexicon::Reading::is_finite_verb))
        || f.only(super::super::lexicon::Reading::is_infinitive)
        || (after_noun && feminine_only && verb_form)
}

/// « se que je veux » → « ce » ; « se matin » → « ce ».
fn check_se(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    // « se sont les miens » → « ce sont » : être suivi d'un déterminant, sans
    // sujet ni relatif devant (« ils se sont lavé », « qui se sont le plus aimés »).
    let det = n.and_then(|n| ctx.next(n));
    let etre_then_det = lower_is(
        ctx,
        n,
        &[
            "sont", "sera", "seront", "serait", "seraient", "fut", "furent",
        ],
    ) && form(ctx, det).is_some_and(|f| f.has_pos(Pos::Det))
        && !lower_is(
            ctx,
            det.and_then(|d| ctx.next(d)),
            &["plus", "moins", "mieux"],
        )
        // Pas de sujet devant (« la sécheresse se sont un jour abattues ») :
        // début de proposition seulement, et pas de participe ensuite.
        && ctx.prev(i).is_none_or(|p| matches!(ctx.words[p].lower.as_str(), "et" | "mais" | "car" | "donc" | "puis"))
        && !(0..4)
            .scan(det, |k, _| {
                let cur = *k;
                *k = k.and_then(|x| ctx.next(x));
                Some(cur)
            })
            .any(|k| form(ctx, k).is_some_and(|f| f.has(super::super::lexicon::Reading::is_past_participle)));
    // « en se suicidant » : gérondif. « se ré-approprier » : verbe composé
    // inconnu du lexique. « se déshérite, se spécule et se contrebande » :
    // suite de verbes pronominaux.
    let gerund = lower_is(ctx, ctx.prev(i), &["en"]);
    let compound = n.is_some_and(|n| ctx.text[ctx.words[n].end..].starts_with('-'));
    let after_comma = i > 0 && ctx.text[ctx.words[i - 1].end..ctx.words[i].start].contains(',');
    let series = (after_comma || lower_is(ctx, ctx.prev(i), &["et", "ou"]))
        && (i.saturating_sub(6)..i).any(|k| lower_is(ctx, Some(k), &["se", "s’", "s'"]));
    etre_then_det
        || lower_is(ctx, n, &["qui", "que", "qu’", "dont", "quoi"])
        || (form(ctx, n).is_some_and(|f| f.only(|r| r.pos == Pos::Nom))
            && !gerund
            && !compound
            && !series)
}

/// « Mes amis son venus » → « sont » : sujet pluriel, puis participe passé ou
/// adverbe (« son déjà partis »).
fn check_son(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    // « ne son pas », « elles son tristes » : jamais le possessif.
    if lower_is(ctx, ctx.prev(i), &["ne", "ils", "elles"]) {
        return true;
    }
    // Après un sujet pluriel : participe, adverbe ou article (« les mois son
    // les meilleurs »). « Son » est aussi un nom (« au son du tambour ») et un
    // possessif devant « pas », « plus » (« son pas est sûr », « son plus
    // proche parent ») : ces suites seules ne suffisent pas.
    let verbal = form(ctx, n)
        .is_some_and(|f| f.has(super::super::lexicon::Reading::is_past_participle))
        || lower_is(
            ctx,
            n,
            &[
                "pas", "plus", "jamais", "guère", "tous", "toutes", "très", "bien", "déjà",
                "encore", "toujours", "le", "la", "l’", "les", "un", "une", "des", "du",
            ],
        );
    verbal && plural_subject_before(ctx, i)
}

/// « Il a pris sont manteau » → « son » : après un verbe, devant un mot qui ne
/// peut être qu'un nom.
fn check_sont(ctx: &Context, i: usize) -> bool {
    let p = ctx.prev(i);
    // « Ce qu’il dit a du sens » : après une relative, « a » est le verbe principal.
    let relative_before = p.is_some_and(|q| {
        (q.saturating_sub(4)..q).any(|k| {
            matches!(
                ctx.words[k].lower.as_str(),
                "qui" | "que" | "qu’" | "quoi" | "dont" | "où"
            )
        })
    });
    // « le musée de Vienne a longtemps été » : nom propre sujet (« Vienne »
    // n'est pas le verbe venir).
    let name_before =
        p.is_some_and(|q| is_capitalized(ctx.words[q].text) && !ctx.words[q].sentence_start);
    let after_verb = !relative_before
        && !name_before
        && form(ctx, p).is_some_and(|f| {
            f.has(|r| r.pos.is_verb()) && !f.has(|r| matches!(r.pos, Pos::Nom | Pos::Pro))
        })
        && !lower_is(ctx, p, &["que", "qu’", "où"]);
    after_verb && form(ctx, ctx.next(i)).is_some_and(|f| f.only(|r| r.pos == Pos::Nom))
}

/// « il c’est trompé » → « s’ ».
/// « quatre où cinq », « de Spa où de Toeplitz », « où du moins » → « ou » :
/// deux éléments de même nature reliés.
fn check_ou_accent(ctx: &Context, i: usize) -> bool {
    let (p, n) = (ctx.prev(i), ctx.next(i));
    // Nombres écrits en lettres ou en chiffres, hors années (« les années 1960
    // où… » : relatif).
    let number = |k: Option<usize>| {
        k.is_some_and(|k| {
            let w = &ctx.words[k];
            (w.has_digit && w.text.chars().count() < 4)
                || matches!(
                    w.lower.as_str(),
                    "un" | "une"
                        | "deux"
                        | "trois"
                        | "quatre"
                        | "cinq"
                        | "six"
                        | "sept"
                        | "huit"
                        | "neuf"
                        | "dix"
                        | "onze"
                        | "douze"
                        | "quinze"
                        | "vingt"
                        | "trente"
                        | "quarante"
                        | "cinquante"
                        | "soixante"
                        | "cent"
                        | "cents"
                        | "mille"
                )
        })
    };
    let both_numbers = number(p) && number(n);
    // Même préposition de part et d'autre : « de Spa où de Toeplitz ».
    let k = p.and_then(|p| ctx.prev(p));
    let same_preposition = n.is_some_and(|n| {
        matches!(
            ctx.words[n].lower.as_str(),
            "de" | "d’" | "à" | "en" | "par" | "pour" | "avec" | "sans"
        ) && k.is_some_and(|k| ctx.words[k].lower == ctx.words[n].lower)
    });
    let du_moins =
        lower_is(ctx, n, &["du"]) && lower_is(ctx, n.and_then(|n| ctx.next(n)), &["moins"]);
    both_numbers || same_preposition || du_moins
}

/// « s’est beau », « s’est la vérité », « s’est que » → « c’est » : « s’est »
/// sans participe passé après (« il s’est levé », « s’est bien amusé »).
fn check_s(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    if !lower_is(ctx, n, &["est", "était"]) {
        return false;
    }
    // « ne s’est pas », « qui s’est », « il s’est » : construction pronominale.
    let p = ctx.prev(i);
    if lower_is(ctx, p, &["ne", "n’", "qui", "que", "qu’"]) || is_preverbal_subject(ctx, p) {
        return false;
    }
    // Un participe dans les mots suivants, éventuellement après des adverbes
    // ou une incise (« s’est peu à peu transformée », « s’est en effet
    // déplacé », « s’est, semble-t-il, perdue ») : correct.
    let Some(n) = n else { return false };
    if ctx.next(n).is_none() {
        return false;
    }
    !(n + 1..ctx.words.len())
        .take_while(|&j| !ctx.words[j].sentence_start)
        .take(6)
        .any(|j| {
            form(ctx, Some(j))
                .is_some_and(|f| f.has(super::super::lexicon::Reading::is_past_participle))
        })
}

fn check_c(ctx: &Context, i: usize) -> bool {
    if is_preverbal_subject(ctx, ctx.prev(i)) && ctx.next(i).is_some() {
        return true;
    }
    // « Ma collègue c’est chargée de… » → « s’est » : nom sujet, sans virgule,
    // puis « est » et un participe passé.
    let Some(p) = ctx.prev(i) else { return false };
    let noun = form(ctx, Some(p)).is_some_and(|f| {
        f.has_pos(Pos::Nom)
            && !f.has(|r| r.pos.is_verb() || matches!(r.pos, Pos::Adv | Pos::Pre | Pos::Con))
    }) && ctx.prev(p).is_some_and(|d| {
        ctx.data
            .determiners
            .contains_key(ctx.words[d].lower.as_str())
    });
    // « ma sœur, qui c’était absentée », « ce qui c’est passé » → « s’ ».
    let noun = noun || lower_is(ctx, Some(p), &["qui"]);
    let n = ctx.next(i);
    noun && lower_is(ctx, n, &["est", "était"])
        && form(ctx, n.and_then(|n| ctx.next(n)))
            .is_some_and(|f| f.has(super::super::lexicon::Reading::is_past_participle))
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
    // « Tout cela et plus encore » : adverbes de degré.
    if lower_is(
        ctx,
        ctx.next(i),
        &[
            "plus", "moins", "encore", "aussi", "autant", "tant", "beaucoup", "mieux", "bien",
            "tout", "très",
        ],
    ) {
        return false;
    }
    form(ctx, ctx.next(i)).is_some_and(|f| {
        f.has(|r| r.pos == Pos::Adj || r.is_past_participle())
            && !f.has(|r| {
                matches!(
                    r.pos,
                    Pos::Pro | Pos::Det | Pos::NomPropre | Pos::Con | Pos::Pre
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
    // « devant elles on trouve » : pronom complément d'une préposition.
    let after_preposition = ctx
        .prev(i)
        .and_then(|p| ctx.prev(p))
        .is_some_and(|q| form(ctx, Some(q)).is_some_and(|f| f.has_pos(Pos::Pre)));
    if matches!(p, Some("ils" | "elles")) {
        return !after_preposition;
    }
    // « Les enfants on faim » : locution avec avoir.
    if lower_is(
        ctx,
        n,
        &[
            "faim", "soif", "peur", "raison", "tort", "besoin", "envie", "chaud", "froid", "honte",
            "sommeil", "hâte", "mal", "lieu", "droit",
        ],
    ) {
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
    if w.compound_part {
        return false;
    }
    if w.sentence_start
        && form(ctx, ctx.next(i)).is_some_and(|f| f.has(|r| r.person() == Some("3s")))
    {
        return true;
    }
    // « ont distinguait », « si ont la lui avait ouverte » : après « ont »
    // (et ses pronoms), un verbe conjugué ; avoir serait suivi d'un participe.
    let mut n = ctx.next(i);
    while n.is_some_and(|k| {
        matches!(
            ctx.words[k].lower.as_str(),
            "le" | "la"
                | "l’"
                | "les"
                | "lui"
                | "leur"
                | "me"
                | "m’"
                | "te"
                | "t’"
                | "se"
                | "s’"
                | "nous"
                | "vous"
                | "y"
                | "en"
                | "ne"
                | "n’"
        )
    }) {
        n = n.and_then(|k| ctx.next(k));
    }
    form(ctx, n).is_some_and(|f| {
        f.has(super::super::lexicon::Reading::is_finite_verb)
            && !f.has(|r| r.is_past_participle() || matches!(r.pos, Pos::Nom | Pos::Adj | Pos::Adv))
    })
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
    // « sa Barra Flyting Toccata » : titre ou nom propre après le possessif.
    if nw.compound_part
        || (is_capitalized(nw.text) && !nw.sentence_start)
        || matches!(nw.lower.as_str(), "tout" | "toute" | "tous" | "toutes")
        // « le journal lui consacre sa une » : la une, première page.
        || nw.lower == "une"
    {
        return false;
    }
    if ctx.data.determiners.contains_key(&nw.lower)
        || ctx.data.is(&nw.lower, "clitique") && !form(ctx, n).is_some_and(|f| f.has_pos(Pos::Nom))
    {
        return true;
    }
    // « merci de vérifier sa et de… » : après un verbe, devant une conjonction.
    // Pas « sa et son manteau » : déterminants coordonnés.
    let after = ctx.next(nj).map(|k| ctx.words[k].lower.as_str());
    if matches!(nw.lower.as_str(), "et" | "ou" | "mais")
        && !after.is_some_and(|a| ctx.data.determiners.contains_key(a))
        && form(ctx, ctx.prev(i)).is_some_and(|f| {
            f.has(|r| r.pos == Pos::Ver && (r.is_infinitive() || r.is_finite_verb()))
        })
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
/// « Ces dommage que… », « ces vrai » → « c’est » : « ces » devant un adjectif
/// ou un nom au singulier suivi de « que », « de », « pour ».
fn check_ces_cest(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    if w.compound_part {
        return false;
    }
    let n = ctx.next(i);
    lower_is(
        ctx,
        n,
        &[
            "dommage",
            "normal",
            "vrai",
            "faux",
            "possible",
            "impossible",
            "évident",
            "clair",
            "certain",
            "sûr",
            "mieux",
            "inutile",
            "triste",
            "étonnant",
            "exact",
            "logique",
            "naturel",
            "important",
            "génial",
            "bête",
        ],
    ) && lower_is(
        ctx,
        n.and_then(|n| ctx.next(n)),
        &["que", "qu’", "de", "d’", "pour"],
    )
}

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
    // « je crois quelle est partie » : être suivi d'un participe ou d'un
    // adjectif (« quelle est la question » garde son déterminant).
    let etre = form(ctx, n).is_some_and(|f| f.has(|r| r.lemma == "être" && r.is_finite_verb()));
    let attribute = form(ctx, n.and_then(|n| ctx.next(n))).is_some_and(|f| {
        f.has(|r| r.pos == Pos::Adj || r.is_past_participle())
            && !f.has(|r| matches!(r.pos, Pos::Det | Pos::Pro))
    });
    if etre && attribute {
        return true;
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
/// « Quelque soit ton choix » → « Quel que soit » : devant être au subjonctif,
/// « quel » s'accorde avec le sujet qui suit (quelle que soit la raison).
fn check_quelque(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(v) = ctx.next(i) else { return };
    let plural_verb = match ctx.words[v].lower.as_str() {
        "soit" | "fût" | "fut" => false,
        "soient" | "fussent" => true,
        _ => return,
    };
    // Genre et nombre : déterminant du sujet, complété par le genre du nom.
    let det = ctx.next(v).and_then(|d| {
        ctx.data
            .determiners
            .get(ctx.words[d].lower.as_str())
            .map(|x| (d, x))
    });
    let mut gender = det.and_then(|(_, x)| x.gn.gender);
    if gender.is_none() {
        gender = det
            .and_then(|(d, _)| ctx.next(d))
            .and_then(|n| ctx.form(n))
            .and_then(|f| f.readings.iter().find(|r| r.pos == Pos::Nom))
            .and_then(|r| noun_gender(ctx, &r.lemma));
    }
    let plural = det
        .and_then(|(_, x)| x.gn.number)
        .map_or(plural_verb, |n| n == 'p');
    let forms: Vec<&str> = match (gender, plural) {
        (Some('f'), false) => vec!["quelle que"],
        (Some('f'), true) => vec!["quelles que"],
        (Some(_), false) => vec!["quel que"],
        (Some(_), true) => vec!["quels que"],
        (None, false) => vec!["quel que", "quelle que"],
        (None, true) => vec!["quels que", "quelles que"],
    };
    let w = &ctx.words[i];
    // « quelque soit vos remarques » → « quelles que soient » : le verbe
    // s'accorde aussi avec le sujet qui suit.
    let verb = (plural != plural_verb).then(|| match (ctx.words[v].lower.as_str(), plural) {
        ("soit", true) => "soient",
        ("fût" | "fut", true) => "fussent",
        ("soient", false) => "soit",
        _ => "fût",
    });
    let end = if verb.is_some() {
        ctx.words[v].end
    } else {
        w.end
    };
    let suggestions: Vec<String> = forms
        .iter()
        .map(|f| {
            let f = match verb {
                Some(v) => format!("{f} {v}"),
                None => (*f).to_string(),
            };
            if is_capitalized(w.text) {
                capitalize(&f)
            } else {
                f
            }
        })
        .collect();
    out.push(
        RawIssue::new(w.start, end, QUELQUE_QUEL_QUE)
            .arg(&ctx.text[w.start..end])
            .arg(suggestions[0].clone())
            .with_suggestions(suggestions),
    );
}

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
/// « quand à moi » → « quant à moi » ; « Quand aux autres, … » → « Quant aux ».
fn check_quand(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    let nn = n.and_then(|n| ctx.next(n));
    let stressed = lower_is(
        ctx,
        nn,
        &[
            "moi", "toi", "lui", "elle", "nous", "vous", "eux", "elles", "cela", "ça", "ceux",
            "celle", "celles", "celui",
        ],
    );
    if lower_is(ctx, n, &["à"]) && stressed {
        return true;
    }
    // « Quand à la répartition des tâches, elle sera précisée » : en tête de
    // phrase, groupe sans verbe puis virgule.
    if ctx.words[i].sentence_start
        && lower_is(ctx, n, &["à"])
        && form(ctx, nn).is_some_and(|f| f.has_pos(Pos::Det))
    {
        let mut k = nn;
        for _ in 0..8 {
            let Some(j) = k else { break };
            if form(ctx, Some(j)).is_some_and(|f| {
                f.has(super::super::lexicon::Reading::is_finite_verb)
                    && !f.has(|r| matches!(r.pos, Pos::Nom | Pos::Adj | Pos::Det))
            }) {
                break;
            }
            if ctx.text[ctx.words[j].end..].trim_start().starts_with(',') {
                return true;
            }
            k = ctx.next(j);
        }
    }
    // « Quand au(x) » en tête de phrase, puis une virgule après le groupe :
    // « Quant aux autres, ils attendront. »
    ctx.words[i].sentence_start
        && lower_is(ctx, n, &["au", "aux"])
        && nn.is_some_and(|k| {
            let end = ctx.next(k).map_or(ctx.words[k].end, |l| ctx.words[l].end);
            ctx.text[ctx.words[k].end..].trim_start().starts_with(',')
                || ctx.text[end..].trim_start().starts_with(',')
        })
}

fn check_peu(ctx: &Context, i: usize) -> Option<(&'static str, &'static str)> {
    let mut p = ctx.prev(i)?;
    let negated = matches!(ctx.words[p].lower.as_str(), "ne" | "n’");
    if negated {
        p = ctx.prev(p)?;
    }
    let pw = &ctx.words[p];
    if pw.compound_part {
        return None;
    }
    // « tu peu venir » → « peux », « il peu venir » → « peut ».
    let fixed = match pw.lower.as_str() {
        "il" | "elle" | "on" | "ça" | "cela" => "peut",
        "tu" | "je" | "j’" => "peux",
        _ => return None,
    };
    if lower_is(ctx, ctx.prev(p), PREPOSITIONS_BEFORE_PRONOUN) {
        return None;
    }
    // « je peu vous aider » : pronoms compléments avant l'infinitif.
    let mut n = ctx.next(i);
    while n.is_some_and(|k| {
        matches!(
            ctx.words[k].lower.as_str(),
            "vous"
                | "nous"
                | "le"
                | "la"
                | "l’"
                | "les"
                | "lui"
                | "leur"
                | "me"
                | "m’"
                | "te"
                | "t’"
                | "se"
                | "s’"
                | "y"
                | "en"
        )
    }) {
        n = n.and_then(|k| ctx.next(k));
    }
    (form(ctx, n).is_some_and(|f| f.has(super::super::lexicon::Reading::is_infinitive))
        || negated && n.is_some_and(|n| ctx.data.is(&ctx.words[n].lower, "negation")))
    .then_some((PEU_PEUT, fixed))
}

/// « Quant il pleut » : « quant » devant un sujet et son verbe est « quand ».
fn check_quant(ctx: &Context, i: usize) -> bool {
    let n = ctx.next(i);
    lower_is(
        ctx,
        n,
        &[
            "il", "elle", "on", "ils", "elles", "je", "j’", "tu", "nous", "vous",
        ],
    ) && form(ctx, n.and_then(|n| ctx.next(n)))
        .is_some_and(|f| f.has(super::super::lexicon::Reading::is_finite_verb))
}

/// Pronom sujet de la 1re ou 2e personne juste avant « à » : « tu à fait » →
/// « as », « j’à vu » → « ai » ; « ils à » → « ont ».
fn first_person_before(ctx: &Context, i: usize) -> Option<&'static str> {
    if ctx.words[i].compound_part {
        return None;
    }
    let mut p = ctx.prev(i)?;
    while matches!(
        ctx.words[p].lower.as_str(),
        "y" | "en"
            | "ne"
            | "n’"
            | "le"
            | "la"
            | "l’"
            | "les"
            | "lui"
            | "leur"
            | "me"
            | "m’"
            | "te"
            | "t’"
            | "se"
            | "s’"
    ) {
        p = ctx.prev(p)?;
    }
    ctx.next(i)?;
    // « pour tu à » n'existe pas, mais « de moi à toi » : après une préposition, le
    // pronom n'est pas sujet (« que » et « et » n'en sont pas).
    if ctx.words[p].compound_part
        || lower_is(
            ctx,
            ctx.prev(p),
            &[
                "à", "avec", "chez", "contre", "de", "entre", "par", "pour", "sans", "sur", "vers",
            ],
        )
    {
        return None;
    }
    match ctx.words[p].lower.as_str() {
        "tu" => Some("as"),
        "je" | "j’" => Some("ai"),
        _ => None,
    }
}

/// « un peut de sel », « à peut près » → « peu ».
fn check_peut(ctx: &Context, i: usize) -> bool {
    // « Il mange peut car il n’a pas faim » : après un verbe conjugué et devant une
    // conjonction ou une ponctuation, c'est l'adverbe.
    if !ctx.words[i].compound_part
        && form(ctx, ctx.prev(i)).is_some_and(|f| {
            f.has(super::super::lexicon::Reading::is_finite_verb)
                && !f.has(|r| matches!(r.pos, Pos::Nom | Pos::Adj | Pos::Pro | Pos::Det))
        })
        && !lower_is(ctx, ctx.prev(i), &["il", "elle", "on", "ne", "n’"])
        && match ctx.next(i) {
            None => before_punctuation(ctx, i),
            Some(n) => matches!(
                ctx.words[n].lower.as_str(),
                "car" | "mais" | "donc" | "que" | "qu’"
            ),
        }
    {
        return true;
    }
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
