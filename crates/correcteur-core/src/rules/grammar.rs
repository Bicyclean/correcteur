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
use crate::text::{capitalize, is_capitalized};

pub const SUBJECT_VERB: &str = "GRAM_ACCORD_SUJET_VERBE";
pub const DETERMINER: &str = "GRAM_ACCORD_DETERMINANT";
pub const ADJECTIVE: &str = "GRAM_ACCORD_ADJECTIF";
pub const PARTICIPLE_ETRE: &str = "GRAM_PARTICIPE_ETRE";
pub const PARTICIPLE_AVOIR: &str = "GRAM_PARTICIPE_AVOIR";
pub const PARTICIPLE_AVOIR_COD: &str = "GRAM_PARTICIPE_AVOIR_COD";
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

/// Reprend la majuscule initiale de `like`.
fn same_case(like: &str, s: &str) -> String {
    if is_capitalized(like) {
        capitalize(s)
    } else {
        s.to_string()
    }
}

/// Participe d'un verbe pronominal qui s'accorde avec le sujet (« elles se
/// sont levées ») : pronom réfléchi de la personne du sujet, verbe dont le
/// pronom est complément direct, et rien après le participe qui puisse être
/// son complément (« elle s'est lavé les mains » : invariable).
fn reflexive_agrees(ctx: &Context, subject: &FoundSubject, t: usize, ft: &Form) -> bool {
    let person = subject.person.as_str();
    let mut reflexive = false;
    for &c in &subject.chain {
        match (lower(ctx, c), person) {
            ("se" | "s’", p) if p.starts_with('3') => reflexive = true,
            ("me" | "m’", "1s") | ("te" | "t’", "2s") | ("nous", "1p") | ("vous", "2p") => {
                reflexive = true;
            }
            ("ne" | "n’" | "y", _) => {}
            // « s’en », « se le » : autre complément, accord incertain.
            _ => return false,
        }
    }
    let invariable =
        ft.has(|r| r.is_past_participle() && ctx.data.is(&r.lemma, "pronominal_invariable"));
    // « la comédie qu’il s’était jouée » : « que » est le complément direct.
    let after_que = ctx
        .prev(subject.start)
        .is_some_and(|q| matches!(lower(ctx, q), "que" | "qu’" | "qu'"));
    if !reflexive || invariable || after_que || !ft.has(Reading::is_past_participle) {
        return false;
    }
    // Après le participe : fin de proposition, adverbe ou préposition seulement.
    ctx.next(t).is_none_or(|n| {
        is_class(ctx, n, "adverbe")
            || ctx.form(n).is_some_and(|f| {
                (f.has_pos(Pos::Adv) || f.has_pos(Pos::Pre))
                    && !f.has(|r| matches!(r.pos, Pos::Det | Pos::Nom) || r.is_infinitive())
            })
    })
}

/// « Les fleurs qu’il a cueilli » : participe avec avoir dont le complément
/// direct, « que », reprend le groupe nominal qui précède. Signale le
/// participe non accordé ; vrai si la construction a été reconnue.
fn preceding_object(
    ctx: &Context,
    subject: &FoundSubject,
    t: usize,
    ft: &Form,
    out: &mut Vec<RawIssue>,
) -> bool {
    let Some(q) = ctx.prev(subject.start) else {
        return false;
    };
    if !matches!(lower(ctx, q), "que" | "qu’" | "qu'") {
        return false;
    }
    let Some(head) = ctx.prev(q) else {
        return false;
    };
    let np = (head.saturating_sub(4)..head)
        .rev()
        .filter_map(|det| find_noun_phrase(ctx, det))
        .find(|np| np.post.unwrap_or(np.noun) == head);
    let Some(np) = np else { return false };
    // Antécédent sûr seulement : article, démonstratif ou possessif (pas
    // « trois mois que »), groupe qui ne suit ni une préposition ni « de »
    // (« les inspirations de sa jeunesse que… », « c’est de cette façon
    // que… »), et qui n'est pas coordonné (« l’intérêt, la curiosité que… »).
    let plain_det = matches!(
        lower(ctx, np.det),
        "le" | "la"
            | "l’"
            | "l'"
            | "les"
            | "ce"
            | "cet"
            | "cette"
            | "ces"
            | "mon"
            | "ma"
            | "mes"
            | "ton"
            | "ta"
            | "tes"
            | "son"
            | "sa"
            | "ses"
            | "notre"
            | "nos"
            | "votre"
            | "vos"
            | "leur"
            | "leurs"
            | "un"
            | "une"
    );
    // « contre tous ces défauts que… » : passer « tous » pour trouver la préposition.
    let mut before = ctx.prev(np.det);
    if before.is_some_and(|p| matches!(lower(ctx, p), "tout" | "tous" | "toute" | "toutes")) {
        before = before.and_then(|p| ctx.prev(p));
    }
    let after_preposition = before.is_some_and(|p| {
        ctx.form(p).is_some_and(|f| f.has_pos(Pos::Pre)) || matches!(lower(ctx, p), "d’" | "d'")
    });
    let coordinated = np.det.checked_sub(1).is_some_and(|b| {
        matches!(lower(ctx, b), "et" | "ou")
            || ctx.text[ctx.words[b].end..ctx.words[np.det].start].contains(',')
    });
    if !plain_det || after_preposition || coordinated {
        return false;
    }
    let Some(target) = phrase_gn(ctx, &np) else {
        return false;
    };
    // « la fois qu’il a parlé » : « que » n'est pas complément d'objet.
    if is_class(ctx, np.noun, "nom_adverbial") || !ft.has(Reading::is_past_participle) {
        return false;
    }
    // « les années qu’il a vécu », « la maison qu’il a fait construire » : invariable.
    let invariable =
        ft.has(|r| r.is_past_participle() && ctx.data.is(&r.lemma, "participe_invariable"));
    // Après le participe, un infinitif, une préposition ou « que » changent la
    // construction : « qu’il a fait construire », « que j’avais fini par »,
    // « que j’aurais aimé qu’elle ».
    let construction = ctx.next(t).is_some_and(|n| {
        matches!(lower(ctx, n), "que" | "qu’" | "qu'")
            || ctx
                .form(n)
                .is_some_and(|f| f.has(Reading::is_infinitive) || f.has_pos(Pos::Pre))
    });
    if invariable || construction {
        return true;
    }
    let agrees = ft
        .readings
        .iter()
        .filter(|r| r.is_past_participle())
        .any(|r| r.gn().is_some_and(|g| g.compatible(target)));
    if agrees {
        return true;
    }
    let suggestions: Vec<String> = agree_forms(ctx, ft, Reading::is_past_participle, target)
        .iter()
        .map(|s| s.to_string())
        .collect();
    if let Some(first) = suggestions.first().cloned() {
        let w = &ctx.words[t];
        out.push(
            RawIssue::new(w.start, w.end, PARTICIPLE_AVOIR_COD)
                .arg(w.text)
                .arg(first)
                .arg(rebuild(ctx, np.det, head, &[]))
                .with_suggestions(suggestions),
        );
    }
    true
}

/// Vrai si l'adjectif placé après le groupe peut se rapporter à un autre nom :
/// celui que le groupe complète (« une partie du monde éloignée »), ou une
/// suite de noms coordonnés (« une animation, un zèle extraordinaires »).
fn other_head(ctx: &Context, np: &NounPhrase, adj: &Form) -> bool {
    let agrees_with = |i: usize| {
        ctx.form(i).is_some_and(|f| {
            f.readings.iter().filter(|r| r.pos == Pos::Nom).any(|n| {
                n.gn().is_some_and(|gn| {
                    adj.readings
                        .iter()
                        .filter(|r| adjectival(r))
                        .any(|r| reading_gn(r).is_some_and(|g| g.compatible(gn)))
                })
            })
        })
    };
    // Préposition « de » devant le groupe, ou contractée dans « du », « des ».
    if let Some(before) = ctx.prev(np.det) {
        let head_end = match lower(ctx, np.det) {
            "du" | "des" => Some(before),
            _ if matches!(lower(ctx, before), "de" | "d’" | "d'") => ctx.prev(before),
            _ => None,
        };
        // Le nom complété, éventuellement suivi d'un adjectif.
        if head_end.is_some_and(|h| agrees_with(h) || ctx.prev(h).is_some_and(agrees_with)) {
            return true;
        }
    }
    // Coordination : un adjectif pluriel après « …, un zèle » ou « … et un zèle ».
    let plural = adj
        .readings
        .iter()
        .filter(|r| adjectival(r))
        .any(|r| r.gn().is_some_and(|g| g.number == Some('p')));
    let coordinated = np.det.checked_sub(1).is_some_and(|b| {
        matches!(lower(ctx, b), "et" | "ou" | "ni")
            || ctx.text[ctx.words[b].end..ctx.words[np.det].start].contains(',')
    });
    plural && coordinated
}

/// Vrai si le mot `adj` peut être un adjectif accordé avec le nom qui le suit.
fn follows_noun(ctx: &Context, adj: usize) -> bool {
    let (Some(fa), Some(fnext)) = (ctx.form(adj), ctx.next(adj).and_then(|n| ctx.form(n))) else {
        return false;
    };
    fnext
        .readings
        .iter()
        .filter(|r| r.pos == Pos::Nom)
        .any(|n| {
            n.gn().is_some_and(|gn| {
                fa.readings
                    .iter()
                    .filter(|r| adjectival(r))
                    .any(|r| reading_gn(r).is_some_and(|g| g.compatible(gn)))
            })
        })
}

/// Mot élidé (« d’ », « l’ ») : jamais un nom.
fn elided(ctx: &Context, i: usize) -> bool {
    ctx.words[i].text.ends_with(['’', '\''])
}

/// Genre d'un nom, d'après toutes les formes de son lemme ; `None` s'il
/// s'emploie aux deux genres (« un/une élève », « le/la somme ») ou si le
/// lexique ne le précise pas.
///
/// Lexique note « e » (épicène) des noms qui ne le sont pas (« maison » au
/// singulier, féminin au pluriel) : une forme « e » ne compte pour l'autre
/// genre que si le lemme a, au même nombre, une forme genrée d'orthographe
/// différente (« professeur » à côté de « professeure »).
pub(super) fn noun_gender(ctx: &Context, lemma: &str) -> Option<char> {
    if ctx.data.two_gender_nouns.contains(lemma) {
        return None;
    }
    let nouns: Vec<(&str, &Reading)> = ctx
        .lexicon
        .lemma_forms(lemma)
        .filter(|(_, r)| r.pos == Pos::Nom)
        .collect();
    let (mut m, mut f) = (false, false);
    for &(text, r) in &nouns {
        match r.gn().and_then(|g| g.gender) {
            Some('m') => m = true,
            Some(_) => f = true,
            None if r.traits.starts_with('e') => {
                let number = r.gn().and_then(|g| g.number);
                for &(t2, r2) in &nouns {
                    let Some(g2) = r2.gn() else { continue };
                    let same_number =
                        number.is_none() || g2.number.is_none() || g2.number == number;
                    if t2 != text && same_number {
                        match g2.gender {
                            Some('m') => f = true,
                            Some(_) => m = true,
                            None => {}
                        }
                    }
                }
            }
            None => {}
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

/// Genre et nombre d'une lecture adjectivale. Un adjectif en -e dont le
/// lexique n'atteste qu'un genre est épicène : Lexique ne note souvent que la
/// forme rencontrée (« giratoire » féminin seulement, « sixième » masculin).
fn adjective_gn(ctx: &Context, r: &Reading) -> Option<Gn> {
    let mut gn = reading_gn(r)?;
    if gn.gender.is_some() && r.lemma.ends_with('e') {
        let other = ctx.lexicon.lemma_readings(&r.lemma).any(|x| {
            x.pos == r.pos
                && x.gn()
                    .is_some_and(|g| g.gender.is_some() && g.gender != gn.gender)
        });
        if !other {
            gn.gender = None;
        }
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
        let next_is_plain_noun = fnext.has_pos(Pos::Nom)
            && !fnext.has(|r| r.pos == Pos::Adj || r.is_finite_verb())
            && !elided(ctx, n)
            && !is_class(ctx, n, "clitique");
        // « les neuf heures » : nombre, pas adjectif.
        if fj.has_pos(Pos::Adj) && !fj.has_pos(Pos::Det) && next_is_plain_noun {
            pre.push(j);
            j = n;
        } else {
            break;
        }
    }

    let noun = j;
    let nw = &ctx.words[noun];
    if nw.ignored
        || nw.has_digit
        || (is_capitalized(nw.text) && !nw.sentence_start)
        || elided(ctx, noun)
        // « ce je ne sais quoi », « mes amis me parlent » : pronom, pas un nom.
        || is_class(ctx, noun, "clitique")
        || ctx.data.subjects.contains_key(nw.lower.as_str())
    {
        return None;
    }
    let noun_form = ctx.form(noun)?;
    // « un peu », « le mieux » : mot surtout adverbial, pas un nom. « le
    // sept », « les mille » : nombre.
    if !noun_form.has_pos(Pos::Nom) || noun_form.has_pos(Pos::Adv) || noun_form.has_pos(Pos::Det) {
        return None;
    }
    // « pour la revoir », « les avoir vus », « qui les fait rire » : pronom
    // complément devant un verbe.
    let object_pronoun = pronoun_reading && is_class(ctx, det, "clitique");
    if object_pronoun
        && noun_form.has(|r| r.pos.is_verb() && (r.is_infinitive() || r.is_finite_verb()))
    {
        return None;
    }
    // « sa propre personne », « une double enquête » : adjectif devant un nom
    // que la recherche des adjectifs antéposés n'a pas retenu.
    if noun_form.has_pos(Pos::Adj) && follows_noun(ctx, noun) {
        return None;
    }
    // « la pire de toutes », « une étrange » : adjectif employé comme nom,
    // accordé avec le déterminant. « la portant » : participe présent.
    let adjective_agrees = noun_form.readings.iter().any(|r| {
        (r.pos == Pos::Adj && reading_gn(r).is_some_and(|g| g.compatible(determiner.gn)))
            || (r.pos.is_verb() && r.traits == "ppre")
    });
    if adjective_agrees {
        return None;
    }
    // « vingt et un ans », « mille et une manières » : nombre composé.
    if matches!(lower(ctx, det), "un" | "une")
        && ctx.prev(det).is_some_and(|p| lower(ctx, p) == "et")
    {
        return None;
    }
    // « vers les minuit », « sur les midi » : tour ancien, toujours au singulier.
    if lower(ctx, det) == "les"
        && matches!(lower(ctx, noun), "midi" | "minuit")
        && ctx
            .prev(det)
            .is_some_and(|p| matches!(lower(ctx, p), "vers" | "sur"))
    {
        return None;
    }
    // « des contre » : préposition.
    if noun_form.has_pos(Pos::Pre) {
        return None;
    }
    // « Quelle est ta couleur ? » : forme conjuguée d'être ou d'avoir, pas le
    // nom « est » ou « as ».
    if noun_form.has(|r| r.is_finite_verb() && matches!(r.lemma.as_str(), "être" | "avoir")) {
        return None;
    }
    // « Elle est tout le temps fatiguée » : « tout le temps » est une locution
    // adverbiale, le mot suivant ne s'y rattache pas.
    let adverbial = ctx
        .prev(det)
        .is_some_and(|p| matches!(lower(ctx, p), "tout" | "toute" | "tous" | "toutes"));
    // « une fois arrivés » : le mot suivant se rapporte au sujet, pas à « fois ».
    let adverbial = adverbial || is_class(ctx, noun, "nom_adverbial");
    let post = ctx.next(noun).filter(|&p| {
        let pw = &ctx.words[p];
        !adverbial
            && !(is_capitalized(pw.text) && !pw.sentence_start)
            && !ctx.data.determiners.contains_key(lower(ctx, p))
            && !is_class(ctx, p, "clitique")
            // « un chapeau haute forme » : l'adjectif se rapporte au nom suivant,
            // avec lequel il s'accorde.
            && !follows_noun(ctx, p)
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
    if !ctx.data.elides(next_word.text, lemma) {
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
    // « sa va » → « ça va » : homophone déjà signalé, ce n'est pas un groupe nominal.
    let det = &ctx.words[np.det];
    if out
        .iter()
        .any(|o| o.rule.starts_with("HOMO_") && o.start == det.start && o.end == det.end)
    {
        return;
    }
    let Some(noun_form) = ctx.form(np.noun) else {
        return;
    };
    let d = np.determiner.gn;
    let nouns = noun_gns(ctx, noun_form);
    // Ni genre ni nombre connus (« au fur ») : rien à vérifier.
    if nouns.is_empty() {
        return;
    }
    let (start, end) = (ctx.words[np.det].start, ctx.words[np.noun].end);
    let found = &ctx.text[start..end];

    let target = phrase_gn(ctx, &np);
    // « du jaune coquille d’œuf », « d’un gris agate » : le premier mot est le
    // nom (une couleur), le suivant lui est apposé.
    let apposition = np.pre.first().and_then(|&a| ctx.form(a)).is_some_and(|f| {
        f.readings
            .iter()
            .any(|r| r.pos == Pos::Nom && r.gn().is_some_and(|g| g.compatible(d)))
    });
    if target.is_none() && apposition {
        return;
    }
    match target {
        Some(target) => {
            // Adjectifs du groupe.
            for adj in np.pre.iter().copied().chain(np.post) {
                let Some(f) = ctx.form(adj) else { continue };
                let ok = f
                    .readings
                    .iter()
                    .filter(|r| adjectival(r))
                    .any(|r| adjective_gn(ctx, r).is_some_and(|g| g.compatible(target)));
                // « les paysannes entrant » : participe présent, invariable.
                if ok || f.has(|r| r.pos.is_verb() && r.traits == "ppre") {
                    continue;
                }
                // L'adjectif qui suit peut se rapporter à un autre nom.
                if Some(adj) == np.post && other_head(ctx, &np, f) {
                    continue;
                }
                let suggestions = agree_forms(ctx, f, adjectival, target);
                // Pas de forme accordée connue : l'analyse est sans doute fausse.
                if suggestions.is_empty() {
                    continue;
                }
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
            // « chacune d’elles », « près d’elle » : pronom après une préposition.
            let after_preposition = ctx
                .prev(p)
                .is_some_and(|q| ctx.form(q).is_some_and(|f| f.has_pos(Pos::Pre)));
            if after_preposition {
                return None;
            }
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
    // « que nous montre le film », « il nous voit, nous aborde » : hors du début
    // de phrase, « nous » et « vous » devant un verbe qui ne s'accorde pas sont
    // plutôt compléments.
    let start = &ctx.words[subject.start];
    if matches!(start.lower.as_str(), "nous" | "vous") && !start.sentence_start {
        return;
    }
    // « Le lendemain nous avions… », « Quelles promenades nous eussions faites » :
    // le groupe nominal n'est pas sujet, « nous » ou « vous » l'est.
    let pronoun_subject = subject.chain.iter().any(|&c| {
        ctx.data
            .subjects
            .get(lower(ctx, c))
            .is_some_and(|s| finite.iter().any(|r| r.person() == Some(s.person.as_str())))
    });
    if !subject.pronoun && pronoun_subject {
        return;
    }
    // « la vulgarité, l’utilité reprenaient » : sujets juxtaposés.
    let juxtaposed = subject.start.checked_sub(1).is_some_and(|b| {
        ctx.text[ctx.words[b].end..start.start].contains(',')
            && ctx.form(b).is_some_and(|f| f.has_pos(Pos::Nom))
    });
    if !subject.pronoun && juxtaposed {
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

    // Mot qui suit l'auxiliaire, après d'éventuels adverbes (« n’est pas encore parti »)
    // ou « tous », « toutes » (« ils étaient tous très contents »).
    let mut t = ctx.next(i);
    for _ in 0..3 {
        match t {
            Some(j) if is_class(ctx, j, "adverbe") => t = ctx.next(j),
            _ => break,
        }
    }
    let Some(t) = t else { return };
    let Some(ft) = ctx.form(t) else { return };
    // « elle était sur le pas », « comme s’il était entre guillemets » : préposition.
    if ft.has_pos(Pos::Pre) {
        return;
    }
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

    if etre && (!has_object || reflexive_agrees(ctx, &subject, t, ft)) {
        // Attribut ou participe : accord avec le sujet (« elles sont parties »).
        // « elle est tout contente », « elle est tout le temps » : « tout »,
        // « fort », « bien »… peuvent être adverbes, donc invariables.
        if subject.gn == Gn::default() || ft.has_pos(Pos::Adv) {
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
                .map(std::string::ToString::to_string)
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

    if avoir && subject.chain.is_empty() && preceding_object(ctx, &subject, t, ft, out) {
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
        let suggestions: Vec<String> = agree_forms(
            ctx,
            ft,
            super::super::lexicon::Reading::is_past_participle,
            ms,
        )
        .iter()
        .map(std::string::ToString::to_string)
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
    // Sens positif de « jamais », « rien », « aucun »… : « le mieux qu’elle eût
    // jamais vu », « s’il revient jamais », « sans qu’on eût rien dit ».
    if lower(ctx, n) != "pas" {
        // « que l’on ait jamais vu » : passer l'article de « l’on ».
        let mut b = ctx.prev(subject.start);
        if b.is_some_and(|k| matches!(lower(ctx, k), "l’" | "l'")) {
            b = b.and_then(|k| ctx.prev(k));
        }
        let before = b.map(|k| lower(ctx, k));
        // « comment il pourrait jamais sortir », « si oui ou non tu as jamais… ».
        let positive = matches!(
            before,
            Some("que" | "qu’" | "qu'" | "si" | "s’" | "s'" | "comment" | "pourquoi" | "non")
        ) || (subject.start.saturating_sub(6)..subject.start)
            .any(|k| lower(ctx, k) == "sans");
        if positive {
            return;
        }
    }
    let first = subject.chain.first().copied().unwrap_or(i);
    let (start, end) = (ctx.words[first].start, ctx.words[i].end);
    let span = &ctx.text[start..end];
    let lemma = ctx
        .form(first)
        .and_then(|f| f.readings.first())
        .map(|r| r.lemma.as_str());
    let fixed = if ctx.data.elides(ctx.words[first].text, lemma) {
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
