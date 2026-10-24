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
use crate::text::{capitalize, is_all_upper, is_capitalized};

pub const SUBJECT_VERB: &str = "GRAM_ACCORD_SUJET_VERBE";
pub const DETERMINER: &str = "GRAM_ACCORD_DETERMINANT";
pub const ADJECTIVE: &str = "GRAM_ACCORD_ADJECTIF";
pub const PARTICIPLE_ETRE: &str = "GRAM_PARTICIPE_ETRE";
pub const PARTICIPLE_AVOIR: &str = "GRAM_PARTICIPE_AVOIR";
pub const PARTICIPLE_AVOIR_COD: &str = "GRAM_PARTICIPE_AVOIR_COD";
pub const SI_CONDITIONAL: &str = "GRAM_SI_CONDITIONNEL";
pub const PARTICIPLE_AVOIR_LES: &str = "GRAM_PARTICIPE_AVOIR_LES";
pub const SUBJUNCTIVE: &str = "GRAM_SUBJONCTIF";
pub const CONCORDANCE: &str = "GRAM_CONCORDANCE";
pub const TOUT: &str = "GRAM_TOUT";
pub const PRONOMINAL_PARTICIPLE: &str = "GRAM_PARTICIPE_PRONOMINAL";
pub const AUXILIARY_ETRE: &str = "GRAM_AUXILIAIRE";
pub const PARTICIPLE_FOR_INFINITIVE: &str = "GRAM_PARTICIPE_POUR_INFINITIF";
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
        noun_for_verb(ctx, i, out);
        subjunctive(ctx, i, out);
        concordance(ctx, i, out);
        modal_finite(ctx, i, out);
        tout_agreement(ctx, i, out);
        auxiliary(ctx, i, out);
        negation(ctx, i, out);
        si_conditional(ctx, i, out);
        participle_for_infinitive(ctx, i, out);
    }
}

// ---------------------------------------------------------------------------
// Outils

pub(super) fn lower<'c>(ctx: &'c Context, i: usize) -> &'c str {
    &ctx.words[i].lower
}

pub(super) fn is_class(ctx: &Context, i: usize, class: &str) -> bool {
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
pub(super) fn same_case(like: &str, s: &str) -> String {
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

/// « Je les ai vu » → « vus » ou « vues » : « les » placé avant avoir est
/// toujours complément direct et pluriel ; son genre dépend de ce qu'il
/// remplace, les deux sont proposés. Vrai si la construction a été reconnue.
fn plural_object_pronoun(
    ctx: &Context,
    subject: &FoundSubject,
    t: usize,
    ft: &Form,
    out: &mut Vec<RawIssue>,
) -> bool {
    let chain: Vec<&str> = subject.chain.iter().map(|&c| lower(ctx, c)).collect();
    // « je les en ai… » : autre complément, accord incertain.
    if !chain.contains(&"les") || chain.contains(&"en") || !ft.has(Reading::is_past_participle) {
        return false;
    }
    // « je les ai fait venir », « les efforts qu’il a fallu » : invariable.
    let invariable =
        ft.has(|r| r.is_past_participle() && ctx.data.is(&r.lemma, "participe_invariable"));
    let before_infinitive = ctx
        .next(t)
        .and_then(|n| ctx.form(n))
        .is_some_and(|f| f.has(Reading::is_infinitive));
    let plural = ft
        .readings
        .iter()
        .filter(|r| r.is_past_participle())
        .any(|r| r.gn().is_some_and(|g| g.number != Some('s')));
    if invariable || before_infinitive || plural {
        return true;
    }
    let mut suggestions: Vec<String> = Vec::new();
    for gender in ['m', 'f'] {
        let target = Gn {
            gender: Some(gender),
            number: Some('p'),
        };
        for f in agree_forms(ctx, ft, Reading::is_past_participle, target) {
            if !suggestions.iter().any(|s| s == f) {
                suggestions.push(f.to_string());
            }
        }
    }
    suggestions.truncate(2);
    if let Some(first) = suggestions.first().cloned() {
        let w = &ctx.words[t];
        out.push(
            RawIssue::new(w.start, w.end, PARTICIPLE_AVOIR_LES)
                .arg(w.text)
                .arg(first)
                .with_suggestions(suggestions),
        );
    }
    true
}

/// Groupe nominal que reprend « que » devant le verbe : « les fleurs qu’il a »,
/// « mes collègues, que j’ai ». Antécédent sûr seulement (voir les gardes).
fn que_antecedent<'c>(ctx: &'c Context, subject: &FoundSubject) -> Option<(NounPhrase<'c>, Gn)> {
    let q = ctx.prev(subject.start)?;
    if !matches!(lower(ctx, q), "que" | "qu’" | "qu'") {
        return None;
    }
    // « mes collègues, que j’ai » : la virgule d'incise sépare « que » de son
    // antécédent, qui doit alors commencer la proposition (« sa voiture, qu’il
    // avait gardé » reprend un nom plus éloigné).
    let comma = ctx.prev(q).is_none();
    let head = ctx.prev(q).or_else(|| {
        q.checked_sub(1)
            .filter(|&b| ctx.text[ctx.words[b].end..ctx.words[q].start].trim() == ",")
    })?;
    let np = (head.saturating_sub(4)..head)
        .rev()
        .filter_map(|det| find_noun_phrase(ctx, det))
        .find(|np| np.post.unwrap_or(np.noun) == head);
    let np = np?;
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
            | "des"
    );
    // « contre tous ces défauts que… » : passer « tous » pour trouver la préposition.
    let mut before = ctx.prev(np.det);
    if before.is_some_and(|p| matches!(lower(ctx, p), "tout" | "tous" | "toute" | "toutes")) {
        before = before.and_then(|p| ctx.prev(p));
    }
    // Après « de », l'antécédent peut être le nom complété (« les inspirations
    // de sa jeunesse que… ») ; après une autre préposition, seulement dans
    // « c’est … que » (« c’est à cette époque que… »). « avec des amis que… »
    // reste vérifié. « la maison des voisins que… », « aucun des jours
    // qu’il lui avait dit » : « des » contracté, sauf après un verbe ou une
    // préposition.
    let cleft = before.is_some_and(|p| {
        (p.saturating_sub(3)..p).any(|k| matches!(lower(ctx, k), "c’" | "c'" | "ce"))
    });
    let after_preposition = before.is_some_and(|p| {
        matches!(lower(ctx, p), "de" | "d’" | "d'")
            || (ctx.form(p).is_some_and(|f| f.has_pos(Pos::Pre))
                && !matches!(lower(ctx, p), "voici" | "voilà")
                && cleft)
            || (matches!(lower(ctx, np.det), "du" | "des")
                && ctx
                    .form(p)
                    .is_none_or(|f| !f.has(|r| r.pos.is_verb() || r.pos == Pos::Pre)))
    });
    let coordinated = np.det.checked_sub(1).is_some_and(|b| {
        matches!(lower(ctx, b), "et" | "ou")
            || ctx.text[ctx.words[b].end..ctx.words[np.det].start].contains(',')
    });
    // « il y a des éternités qu’on ne vous a vu », « cela fait deux ans que… » :
    // « que » temporel, pas relatif.
    let duration = before.is_some_and(|p| {
        lower(ctx, p) == "fait" || (lower(ctx, p) == "a" && p > 0 && lower(ctx, p - 1) == "y")
    });
    if !plain_det || after_preposition || coordinated || duration {
        return None;
    }
    if comma && !clause_start(ctx, np.det) {
        return None;
    }
    let target = phrase_gn(ctx, &np)?;
    Some((np, target))
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
    let Some((np, target)) = que_antecedent(ctx, subject) else {
        return false;
    };
    let head = np.post.unwrap_or(np.noun);
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
    // « que je n’ai pas eu assez de volonté pour découvrir », « qu’il a eu le
    // temps de lire » : le participe a son propre complément direct.
    let construction = ctx.next(t).is_some_and(|n| {
        matches!(
            lower(ctx, n),
            "que" | "qu’" | "qu'" | "assez" | "beaucoup" | "peu" | "trop" | "tant" | "autant"
        ) || (ctx.data.determiners.contains_key(lower(ctx, n))
            // « que vous avez passé le 3 octobre », « passé le lendemain » : date
            // ou moment, pas un complément direct.
            // (« a eu le temps de lire » garde son complément direct.)
            && !ctx.next(n).is_some_and(|k| {
                ctx.words[k].has_digit
                    || (is_class(ctx, k, "nom_adverbial") && lower(ctx, k) != "temps")
            }))
            || ctx.form(n).is_some_and(|f| f.has(Reading::is_infinitive))
            // « que j’avais fini par », « qu’il a essayé de » ; mais « vus depuis
            // longtemps », « achetée chez lui » restent accordés.
            || matches!(lower(ctx, n), "de" | "d’" | "d'" | "à" | "par" | "pour" | "en")
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
    // « le cours de sa vie tracé » : Lexique ne note certains noms invariables
    // (« cours », « bras », « prix ») qu'au pluriel ; leur nombre est libre.
    let agrees_with = |i: usize| {
        let text = ctx.words[i].lower.as_str();
        !elided(ctx, i)
            && ctx.form(i).is_some_and(|f| {
                f.readings.iter().filter(|r| r.pos == Pos::Nom).any(|n| {
                    let invariable = n.lemma == text && text.ends_with(['s', 'x', 'z']);
                    n.gn()
                        .map(|g| {
                            if invariable {
                                Gn { number: None, ..g }
                            } else {
                                g
                            }
                        })
                        .is_some_and(|gn| {
                            adj.readings
                                .iter()
                                .filter(|r| adjectival(r))
                                .any(|r| reading_gn(r).is_some_and(|g| g.compatible(gn)))
                        })
                })
            })
    };
    // Préposition devant le groupe (« de », « à », « par »…, ou contractée dans
    // « du », « des », « au », « aux ») : l'adjectif peut se rapporter au nom
    // que le groupe complète (« une partie du monde éloignée », « une partie au
    // violoncelle jouée par… », « une étoile à six branches décorée »).
    let before = ctx.prev(np.det);
    // « de » et « à » seulement : après « sans », « avec », « pour »…, le
    // groupe est un complément du verbe (« sans cette préoccupation habituel »).
    let preposition = |k: usize| matches!(lower(ctx, k), "de" | "d’" | "d'" | "à");
    // Le nom complété, éventuellement suivi d'un adjectif, en remontant les
    // compléments successifs (« la nationalisation des biens du clergé
    // proposée », « la part de ce budget des ménages dévolue »). Un nom propre
    // (« l’Émilie au jardin représentée ») peut être de l'un ou l'autre genre.
    let mut det = np.det;
    for _ in 0..4 {
        let before = ctx.prev(det);
        let head_end = match lower(ctx, det) {
            "du" | "des" | "au" | "aux" => before,
            _ => before.filter(|&b| preposition(b)).and_then(|b| ctx.prev(b)),
        };
        let Some(h) = head_end else { break };
        let hw = &ctx.words[h];
        // « l’une de vos mains placée » : pronom de quantité.
        let pronoun = matches!(
            hw.lower.as_str(),
            "un" | "une"
                | "l’un"
                | "l’une"
                | "l'un"
                | "l'une"
                | "chacun"
                | "chacune"
                | "aucun"
                | "aucune"
                | "plusieurs"
                | "certains"
                | "certaines"
        );
        // « il a des fleurs rouge », « c’est des fleurs » : une forme d'être, avoir,
        // aller ou faire n'est pas la tête d'un groupe nominal.
        let head_ok = ctx.form(h).is_some_and(|f| {
            (f.has_pos(Pos::Nom) || f.has_pos(Pos::Adj) || f.has_pos(Pos::NomPropre))
                && !f.has(|r| {
                    r.is_finite_verb()
                        && matches!(r.lemma.as_str(), "être" | "avoir" | "aller" | "faire")
                })
        });
        // « dans la plupart des cas représenté » : « plupart » n'est noté qu'adverbe.
        if !pronoun && !head_ok && !collective(ctx, h) {
            break;
        }
        // « dans la plupart des cas représenté » : le participe s'accorde avec le
        // sujet de la phrase, pas avec le nom du complément.
        if collective(ctx, h)
            || pronoun
            || (is_capitalized(hw.text) && !hw.sentence_start)
            || agrees_with(h)
            || ctx.prev(h).is_some_and(agrees_with)
        {
            return true;
        }
        let up = (h.saturating_sub(4)..h)
            .rev()
            .filter_map(|d| find_noun_phrase(ctx, d))
            .find(|g| g.noun == h || g.post == Some(h));
        let Some(up) = up else { break };
        det = up.det;
    }
    // Autre préposition juste après un nom qui ne peut pas être verbe : le
    // groupe complète ce nom (« les dangers sur l’environnement liés à… »,
    // « son goût pour l’arabesque avivé »).
    let noun_only = |h: usize| {
        ctx.form(h)
            .is_some_and(|f| f.has_pos(Pos::Nom) && !f.has(|r| r.pos.is_verb()))
    };
    if before
        .filter(|&b| ctx.form(b).is_some_and(|f| f.has_pos(Pos::Pre)))
        .and_then(|b| ctx.prev(b))
        .is_some_and(|h| noun_only(h) && agrees_with(h))
    {
        return true;
    }
    // Coordination : un adjectif pluriel après « …, un zèle », « … et un zèle »
    // ou « … et de la musique ».
    let plural = adj
        .readings
        .iter()
        .filter(|r| adjectival(r))
        .any(|r| r.gn().is_some_and(|g| g.number == Some('p')));
    let mut first = np.det;
    if let Some(b) = before.filter(|&b| matches!(lower(ctx, b), "de" | "d’" | "d'")) {
        first = b;
    }
    let coordinated = first.checked_sub(1).is_some_and(|b| {
        matches!(lower(ctx, b), "et" | "ou" | "ni")
            || ctx.text[ctx.words[b].end..ctx.words[first].start].contains(',')
    });
    plural && coordinated
}

/// Adjectif placé après le groupe qui ne s'accorde pas avec lui par nature :
/// distributif (« les domaines industriel, agricole et maritime », « les versions
/// indienne et irlandaise », « les empires ottoman puis russe »), « seul » rapporté au sujet (« il prend ses repas
/// seul »), couleur nuancée invariable (« la peau brun foncé », « une livrée gris métallisé »).
fn free_adjective(ctx: &Context, np: &NounPhrase, adj: usize, f: &Form) -> bool {
    let singular = f
        .readings
        .iter()
        .filter(|r| adjectival(r))
        .all(|r| r.gn().is_none_or(|g| g.number != Some('p')));
    // « les pays éduen, sénon et lingon » : nom invariable, déterminant pluriel.
    let noun_plural = np.determiner.gn.number == Some('p')
        || ctx.form(np.noun).is_some_and(|nf| {
            nf.has(|r| r.pos == Pos::Nom && r.gn().is_some_and(|g| g.number == Some('p')))
        });
    let next = ctx.next(adj);
    let enumerated = next.is_some_and(|n| matches!(lower(ctx, n), "et" | "ou" | "puis"))
        || ctx.text[ctx.words[adj].end..]
            .trim_start()
            .starts_with([',', '('])
        || ctx.next(adj).is_none() && ctx.text[ctx.words[adj].end..].trim_start().starts_with(',');
    let distributive = singular && noun_plural && enumerated;
    let seul = f.has(|r| r.lemma == "seul");
    let nuance = next.is_some_and(|n| {
        matches!(
            lower(ctx, n),
            "foncé"
                | "clair"
                | "vif"
                | "pâle"
                | "sombre"
                | "tendre"
                | "moyen"
                | "doré"
                | "métallisé"
                | "perle"
                | "mat"
                | "brillant"
        )
    });
    distributive || seul || nuance
}

/// Couleur composée, invariable : « les pattes sont gris-bleuâtre », « ils
/// sont blanc crémeux ».
fn nuanced_colour(ctx: &Context, t: usize, ft: &Form) -> bool {
    let colour = ft.has(|r| {
        matches!(
            r.lemma.as_str(),
            "blanc"
                | "noir"
                | "gris"
                | "brun"
                | "bleu"
                | "vert"
                | "rouge"
                | "jaune"
                | "roux"
                | "beige"
                | "rose"
                | "violet"
                | "orange"
        )
    });
    colour
        && (ctx.text[ctx.words[t].end..].starts_with('-')
            || ctx
                .next(t)
                .and_then(|n| ctx.form(n))
                .is_some_and(|f| f.has(|r| r.pos == Pos::Adj) && !f.has_pos(Pos::Con)))
}

/// Vrai si le mot `adj` peut être un adjectif accordé avec le nom qui le suit.
fn follows_noun(ctx: &Context, adj: usize) -> bool {
    let (Some(fa), Some(fnext)) = (ctx.form(adj), ctx.next(adj).and_then(|n| ctx.form(n))) else {
        return false;
    };
    // « une réunion important a lieu », « une table rond est là » : « a » et
    // « est » sont des verbes, pas des noms que l'adjectif précède.
    if fnext.has(|r| r.is_finite_verb() && matches!(r.lemma.as_str(), "être" | "avoir")) {
        return false;
    }
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

/// Mot suivi d'une énumération : « et », « ou », ou un nombre (« les dimanche 4
/// et mercredi 7 »).
fn enumerated(ctx: &Context, i: usize) -> bool {
    ctx.next(i)
        .is_some_and(|n| matches!(lower(ctx, n), "et" | "ou") || ctx.words[n].has_digit)
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
    // « la mi-décembre », « la mi-temps » : féminins que Lexique note masculins.
    if ctx.data.two_gender_nouns.contains(lemma) || lemma.starts_with("mi-") {
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
            gn.gender = noun_gender(ctx, &r.lemma).or_else(|| {
                if r.lemma.starts_with("mi-") {
                    return None;
                }
                // « amie », « cousine » : le lemme a les deux genres, mais cette
                // forme-ci n'en a qu'un (d'orthographe distincte de l'autre).
                let own = r.gn().and_then(|g| g.gender)?;
                (!ctx.data.two_gender_nouns.contains(&r.lemma)
                    && form
                        .readings
                        .iter()
                        .filter(|x| x.pos == Pos::Nom && x.lemma == r.lemma)
                        .all(|x| x.gn().and_then(|g| g.gender) == Some(own)))
                .then_some(own)
            });
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
    // Lexique sépare le masculin et le féminin de quelques adjectifs en deux
    // lemmes (« nouveau » / « nouvelle ») : on cherche dans les deux.
    const SIBLINGS: &[(&str, &str)] = &[
        ("nouveau", "nouvelle"),
        ("beau", "belle"),
        ("vieux", "vieille"),
        ("fou", "folle"),
        ("mou", "molle"),
        ("jumeau", "jumelle"),
    ];
    let siblings: Vec<&str> = lemmas
        .iter()
        .filter_map(|l| {
            SIBLINGS
                .iter()
                .find_map(|(a, b)| (a == l).then_some(*b).or_else(|| (b == l).then_some(*a)))
        })
        .collect();
    lemmas.extend(siblings);
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
    // Genre inconnu : le masculin d'abord, comme dans l'accord par défaut.
    if target.gender.is_none() {
        out.sort_by_key(|f| {
            ctx.lexicon.get(f).map_or(2, |x| {
                let feminine =
                    |r: &Reading| pred(r) && r.gn().is_some_and(|g| g.gender == Some('f'));
                let other = |r: &Reading| pred(r) && r.gn().is_some_and(|g| g.gender != Some('f'));
                if x.has(feminine) && !x.has(other) {
                    1
                } else {
                    0
                }
            })
        });
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
    // « la SA écrit » : sigle, pas déterminant.
    if w.ignored
        || w.compound_part
        || (w.text.chars().filter(|c| c.is_alphabetic()).count() > 1 && is_all_upper(w.text))
    {
        return None;
    }
    let determiner = ctx.data.determiners.get(&w.lower)?;
    let mut j = ctx.next(det)?;
    // « les deux sœurs », « ces trois enfants » : le nombre ne change pas le groupe.
    const NUMBERS: &[&str] = &[
        "deux",
        "trois",
        "quatre",
        "cinq",
        "six",
        "sept",
        "huit",
        "neuf",
        "dix",
        "onze",
        "douze",
        "treize",
        "quatorze",
        "quinze",
        "seize",
        "vingt",
        "trente",
        "quarante",
        "cinquante",
        "soixante",
        "cent",
        "mille",
    ];
    if determiner.gn.number == Some('p')
        && NUMBERS.contains(&lower(ctx, j))
        && !matches!(w.lower.as_str(), "deux" | "trois" | "des")
    {
        j = ctx.next(j)?;
    }
    // « il les mange » : pronom complément, pas déterminant.
    let pronoun_reading = ctx.form(det).is_some_and(|f| f.has_pos(Pos::Pro));
    let after_subject = ctx.prev(det).is_some_and(|p| {
        ctx.data.subjects.contains_key(lower(ctx, p)) || is_class(ctx, p, "clitique")
    });
    // « dont un porte un brûleur », « l’un parle » : pronom « un ».
    let pronoun_un = matches!(w.lower.as_str(), "un" | "une")
        && ctx
            .prev(det)
            .is_some_and(|p| matches!(lower(ctx, p), "dont" | "l’" | "l'" | "chacun"));
    if pronoun_un {
        return None;
    }
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
        // « l’alezan vole », « la cité conserve » : le premier mot est le nom, le
        // suivant un verbe ; « un belle voiture » : « belle » n'est pas un nom au
        // masculin, c'est bien un adjectif devant « voiture ».
        let first_is_noun = fj
            .readings
            .iter()
            .any(|r| r.pos == Pos::Nom && r.gn().is_some_and(|g| g.compatible(determiner.gn)));
        // Verbe seulement si un autre mot que ne suit pas un second verbe : « ces grands
        // tables sont » (« tables » est un nom, « sont » le verbe).
        let next_is_verb = fnext.has(Reading::is_finite_verb)
            && ctx
                .next(n)
                .is_some_and(|m| !ctx.form(m).is_some_and(|f| f.has(Reading::is_finite_verb)));
        let next_is_plain_noun = fnext.has_pos(Pos::Nom)
            && !(first_is_noun && next_is_verb)
            && !fnext.has(|r| r.pos == Pos::Adj)
            && !elided(ctx, n)
            && !is_class(ctx, n, "clitique");
        // « les neuf heures » : nombre, pas adjectif.
        // « le Pure race espagnole » : majuscule, nom propre ou titre.
        let capital = is_capitalized(ctx.words[j].text) && !ctx.words[j].sentence_start;
        // « une grande et belle maison » : deux adjectifs coordonnés avant le nom.
        if fj.has_pos(Pos::Adj) && !fj.has_pos(Pos::Det) && !capital && lower(ctx, n) == "et" {
            if let Some((m, n2)) = ctx.next(n).and_then(|m| ctx.next(m).map(|n2| (m, n2))) {
                let plain = |k: usize| {
                    ctx.form(k).is_some_and(|f| {
                        f.has_pos(Pos::Nom)
                            && !f.has_pos(Pos::Adj)
                            && !elided(ctx, k)
                            && !is_class(ctx, k, "clitique")
                    })
                };
                if ctx
                    .form(m)
                    .is_some_and(|f| f.has_pos(Pos::Adj) && !f.has_pos(Pos::Det))
                    && plain(n2)
                {
                    pre.push(j);
                    pre.push(m);
                    j = n2;
                    break;
                }
            }
        }
        if fj.has_pos(Pos::Adj) && !fj.has_pos(Pos::Det) && !capital && next_is_plain_noun {
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
        // « la mi-décembre », « les porte-parole » : premier élément d'un mot composé.
        || ctx.text[nw.end..].starts_with('-')
        || (nw.text.contains('-') && ctx.lexicon.find(&nw.norm).is_none())
        || (nw.compound_part && ctx.text[..nw.start].ends_with('-'))
        // « ce je ne sais quoi », « mes amis me parlent » : pronom, pas un nom.
        || is_class(ctx, noun, "clitique")
        || ctx.data.subjects.contains_key(nw.lower.as_str())
    {
        return None;
    }
    // « les saint Thomas », « le docteur Paul » : titre devant un nom propre.
    if ctx.next(noun).is_some_and(|n| {
        let nw = &ctx.words[n];
        is_capitalized(nw.text)
            && !nw.sentence_start
            && ctx.form(n).is_some_and(|f| f.has_pos(Pos::NomPropre))
    }) {
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
    // Devant un verbe conjugué : pronom possible (« une voix les fait se
    // retourner », « et les glisse »), sauf juste après un verbe ou une
    // préposition, où c'est un déterminant (« j’aime les voitures » : pas « que
    // tu voitures »), et en tête de phrase ou après une ponctuation.
    let before_verb = ctx.prev(det).is_some_and(|p| {
        // « tout les livres », « j’ai lu les livres » : après « tout » ou un
        // participe, c'est un déterminant.
        !matches!(lower(ctx, p), "tout" | "tous" | "toute" | "toutes")
            && !ctx.form(p).is_some_and(|f| {
                f.has_pos(Pos::Pre)
                    || f.has(Reading::is_past_participle)
                    || (f.has(|r| r.is_finite_verb() || r.is_infinitive()) && !f.has_pos(Pos::Nom))
            })
    });
    if object_pronoun
        && noun_form
            .has(|r| r.pos.is_verb() && (r.is_infinitive() || (before_verb && r.is_finite_verb())))
    {
        return None;
    }
    // « des première, deuxième, troisième et quatrième batteries » : ordinal.
    if matches!(
        noun_form.readings.first().map(|r| r.lemma.as_str()),
        Some(
            "premier"
                | "première"
                | "deuxième"
                | "second"
                | "seconde"
                | "troisième"
                | "dernier"
                | "dernière"
        )
    ) && ctx.text[ctx.words[noun].end..].starts_with(',')
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
    // Seulement si le nom, lui, ne s'accorde pas : « mes amis » reste un nom.
    let noun_agrees = noun_form
        .readings
        .iter()
        .any(|r| r.pos == Pos::Nom && r.gn().is_some_and(|g| g.compatible(determiner.gn)));
    if adjective_agrees && !noun_agrees {
        return None;
    }
    // « sans compensation aucune » : « aucun » placé après le nom.
    if matches!(lower(ctx, det), "aucun" | "aucune")
        && ctx
            .prev(det)
            .and_then(|p| ctx.form(p))
            .is_some_and(|f| f.only(|r| r.pos == Pos::Nom))
    {
        return None;
    }
    // « des cache-pot », « des porte-monnaie » : nom composé verbe-nom, souvent
    // laissé invariable avant les rectifications de 1990.
    if nw.text.contains('-') {
        let first = nw.text.split('-').next().unwrap_or("");
        if ctx
            .lexicon
            .find(&first.to_lowercase())
            .is_some_and(|f| f.has(Reading::is_finite_verb))
        {
            return None;
        }
    }
    // « vingt et un ans », « mille et une manières » : nombre composé.
    // « cent une synagogues », « mille et une nuits ».
    if matches!(lower(ctx, det), "un" | "une")
        && ctx.prev(det).is_some_and(|p| {
            matches!(
                lower(ctx, p),
                "et" | "cent"
                    | "mille"
                    | "vingt"
                    | "trente"
                    | "quarante"
                    | "cinquante"
                    | "soixante"
                    | "quatre-vingt"
            )
        })
    {
        return None;
    }
    // « la bémol majeur » : la note, pas l'article.
    if lower(ctx, det) == "la" && matches!(lower(ctx, noun), "bémol" | "dièse") {
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
    // « dès le départ conscients », « par la suite rebaptisées » : complément de temps.
    let adverbial = adverbial
        || is_class(ctx, noun, "nom_adverbial")
        || ctx
            .prev(det)
            .is_some_and(|p| matches!(lower(ctx, p), "dès" | "depuis"))
        || (lower(ctx, noun) == "suite" && ctx.prev(det).is_some_and(|p| lower(ctx, p) == "par"))
        // « de nos jours », « à l’origine », « du reste », « à cet endroit ».
        || {
            let p = ctx.prev(det).map_or("", |p| lower(ctx, p));
            matches!(
                (p, lower(ctx, det), lower(ctx, noun)),
                ("de", "nos", "jours")
                    | ("à", "l’" | "l'", "origine")
                    | (_, "du", "reste")
                    | ("à", "cet", "endroit")
                    | ("en", "quelque", "sorte")
                    | ("à", _, "yeux")
            )
        }
        // « elle a l’air intelligente » : accord avec le sujet.
        || (lower(ctx, noun) == "air"
            && ctx.prev(det).and_then(|p| ctx.form(p)).is_some_and(|f| f.has(|r| r.lemma == "avoir")));
    let post = ctx.next(noun).filter(|&p| {
        let pw = &ctx.words[p];
        !adverbial
            && !(is_capitalized(pw.text) && !pw.sentence_start)
            // « les ruines gallo-romaines », « la gestion péri-opératoire ».
            && !ctx.text[pw.end..].starts_with('-')
            // « pourvu qu’on les proclamât » : locution conjonctive.
            && !(pw.lower == "pourvu"
                && ctx.next(p).is_some_and(|q| matches!(lower(ctx, q), "que" | "qu’" | "qu'")))
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
                // « courant / tension » : deux termes séparés par une barre.
                if ctx.text[ctx.words[adj].end..].trim_start().starts_with('/') {
                    continue;
                }
                // « important », « charmant » s'accordent, sauf s'ils ont un complément
                // (« les maisons couvrant les trottoirs », « une femme ressemblant au
                // portrait » : participe présent, invariable).
                let complement = ctx.next(adj).is_some_and(|n| {
                    !matches!(lower(ctx, n), "et" | "ou" | "mais" | "qui")
                        && ctx.form(n).is_some_and(|g| {
                            // « a lieu », « est » : être et avoir ne sont pas un complément.
                            !g.has(|r| {
                                r.is_finite_verb() && matches!(r.lemma.as_str(), "être" | "avoir")
                            }) && (ctx.data.determiners.contains_key(lower(ctx, n))
                                || g.has(|r| {
                                    matches!(
                                        r.pos,
                                        Pos::Pre | Pos::Pro | Pos::Adv | Pos::Adj | Pos::Nom
                                    )
                                }))
                        })
                });
                if ok
                    || (f.has(|r| r.pos.is_verb() && r.traits == "ppre")
                        && (!f.has_pos(Pos::Adj) || complement))
                {
                    continue;
                }
                // L'adjectif qui suit peut se rapporter à un autre nom, ou ne pas
                // s'accorder par nature ; après un collectif, il peut s'accorder
                // avec le complément sous-entendu (« une douzaine réalisées »).
                if Some(adj) == np.post
                    && (other_head(ctx, &np, f)
                        || free_adjective(ctx, &np, adj, f)
                        || collective(ctx, np.noun))
                {
                    continue;
                }
                // « un finale intitulé », « des intellectuelles membres » : le
                // premier mot est le nom, le suivant un adjectif ou une apposition.
                // Seulement quand un complément suit (« membres du parti », « finale
                // intitulé Moto perpetuo ») : « ces grands tables » reste une faute.
                let complement_follows = ctx.next(np.noun).is_some_and(|n| {
                    is_capitalized(ctx.words[n].text)
                        || matches!(lower(ctx, n), "du" | "des" | "au" | "aux" | "de" | "d’")
                        || ctx.form(n).is_some_and(|g| g.has_pos(Pos::Pre))
                });
                if Some(adj) != np.post
                    && complement_follows
                    && f.has(|r| r.pos == Pos::Nom && r.gn().is_some_and(|g| g.compatible(d)))
                {
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
            coordinated_adjective(ctx, &np, target, out);
        }
        // « Elisabeth et Maria, les marque profondément », « un sort navrant le
        // place » : « le », « la », « les » devant une forme verbale peuvent être
        // pronoms ; sans accord possible, on s'abstient.
        None if is_class(ctx, np.det, "clitique") && noun_form.has(Reading::is_finite_verb) => {}
        // « les samedi et dimanche », « les dimanche 4 et mercredi 7 » : noms
        // énumérés au singulier. « six sur dix résident » : nombre puis verbe.
        None if enumerated(ctx, np.noun)
            || (np.determiner.gn.number == Some('p')
                && !matches!(
                    lower(ctx, np.det),
                    "les" | "des" | "ces" | "mes" | "tes" | "ses" | "nos" | "vos" | "leurs" | "aux"
                )
                && noun_form.has(Reading::is_finite_verb)) => {}
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
                // « au pommes » → « aux pommes » d'abord : un article contracté
                // devant un pluriel en -s est plus souvent la faute.
                let contracted = matches!(lower(ctx, np.det), "au" | "du");
                let plural_noun = nouns.iter().all(|(g, _)| g.number == Some('p'));
                let fixed = with_determiner(ctx, &np, other, np.noun);
                if contracted && plural_noun {
                    suggestions.insert(0, fixed);
                } else {
                    suggestions.push(fixed);
                }
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
        .filter(|&p| matches!(lower(ctx, p), "et" | "ou" | "ni"))
        .is_some_and(|p| {
            // « Il mange et ils boit » : un pronom sujet après « et » ouvre une
            // proposition, sauf après un pronom tonique (« lui et elle »).
            !s.pronoun
                || !matches!(
                    lower(ctx, s.start),
                    "je" | "j’" | "tu" | "il" | "ils" | "on"
                )
                || p.checked_sub(1).is_some_and(|b| {
                    matches!(
                        lower(ctx, b),
                        "moi" | "toi" | "lui" | "eux" | "elle" | "elles"
                    )
                })
        });
    (!coordinated).then_some(s)
}

/// Sujet coordonné à un autre placé avant lui : « aussi bien le poète que la
/// jeune femme sont devenus », « les matières comme le cuivre soient
/// produites », « la peste et la sécheresse se sont abattues ».
fn coordinated_subject(ctx: &Context, subject: &FoundSubject) -> bool {
    // « Ses manières, ses paroles, tout son air me disaient » : « tout »
    // résume l'énumération.
    let mut start = subject.start;
    if start
        .checked_sub(1)
        .is_some_and(|p| matches!(lower(ctx, p), "tout" | "toute" | "tous" | "toutes"))
    {
        start -= 1;
    }
    !subject.pronoun
        && start.checked_sub(1).is_some_and(|b| {
            matches!(lower(ctx, b), "comme" | "et" | "ou" | "ni" | "avec" | "puis")
                || ctx.text[ctx.words[b].end..ctx.words[start].start].contains(',')
                // « aussi bien X que Y », « ainsi que », « tant X que Y », « de
                // plus savants que son neveu eussent » ; pas « je pense que les
                // enfants… ».
                || (lower(ctx, b) == "que"
                    && (b.saturating_sub(8)..b).any(|k| {
                        matches!(
                            lower(ctx, k),
                            "aussi" | "ainsi" | "autant" | "tant" | "plus" | "moins"
                        )
                    }))
        })
}

/// « ils sont jaune, magenta et cyan » : attributs au singulier énumérés après
/// un sujet pluriel.
fn distributed_attribute(ctx: &Context, subject: &FoundSubject, t: usize, ft: &Form) -> bool {
    let singular = ft
        .readings
        .iter()
        .filter(|r| adjectival(r))
        .all(|r| r.gn().is_none_or(|g| g.number != Some('p')));
    let next_attribute = (t + 1 < ctx.words.len()
        && (ctx.text[ctx.words[t].end..ctx.words[t + 1].start].contains(',')
            || matches!(lower(ctx, t + 1), "et" | "ou"))
        && ctx.form(t + 1).is_some_and(|f| f.has(adjectival)))
        || (t + 2 < ctx.words.len()
            && matches!(lower(ctx, t + 1), "et" | "ou")
            && ctx.form(t + 2).is_some_and(|f| f.has(adjectival)));
    subject.gn.number == Some('p') && singular && next_attribute
}

/// « la majorité sont », « un grand nombre restent » : accord avec le
/// complément sous-entendu, admis après un nom collectif.
fn collective_subject(ctx: &Context, subject: &FoundSubject, verb: usize) -> bool {
    !subject.pronoun && (subject.start..verb).any(|k| collective(ctx, k))
}

/// Nom collectif ou de quantité, suivi d'un complément au pluriel avec lequel
/// l'accord peut se faire (« la majorité sont », « une douzaine réalisées »).
fn collective(ctx: &Context, k: usize) -> bool {
    matches!(
        lower(ctx, k),
        "majorité"
            | "plupart"
            | "nombre"
            | "minorité"
            | "moitié"
            | "totalité"
            | "quart"
            | "tiers"
            | "partie"
            | "reste"
            | "ensemble"
            | "foule"
            | "centaine"
            | "dizaine"
            | "douzaine"
            | "millier"
            | "infinité"
            | "multitude"
            | "série"
            | "majeure"
    )
}

/// Vrai si l'auxiliaire être d'indice `verb`, mal accordé, est à remplacer par
/// avoir : pronom complément d'une autre personne que le sujet (« je vous »)
/// et participe d'un verbe qui se conjugue avec avoir (« dit »).
fn wants_avoir(ctx: &Context, subject: &FoundSubject, verb: usize) -> bool {
    let Some(form) = ctx.form(verb) else {
        return false;
    };
    if !form.has(|r| r.lemma == "être" && r.is_finite_verb()) {
        return false;
    }
    let other_person_object = subject.chain.iter().any(|&c| {
        let person = match lower(ctx, c) {
            "me" | "m’" => "1s",
            "te" | "t’" => "2s",
            "nous" => "1p",
            "vous" => "2p",
            "lui" | "leur" | "le" | "la" | "l’" | "les" => "3",
            _ => return false,
        };
        !subject.person.starts_with(person)
    });
    // « je vous est déjà envoyé » : adverbes entre l'auxiliaire et le participe.
    let mut next = ctx.next(verb);
    for _ in 0..2 {
        match next {
            Some(k) if is_class(ctx, k, "adverbe") || is_class(ctx, k, "negation") => {
                next = ctx.next(k);
            }
            _ => break,
        }
    }
    let participle = next.and_then(|n| ctx.form(n)).is_some_and(|f| {
        f.has(|r| r.is_past_participle() && !ctx.data.conjugator.takes_etre(&r.lemma))
    });
    other_person_object && participle
}

/// « Si j’aurais su » → « si j’avais su » : après « si » de condition, en tête
/// de phrase, l'imparfait et non le conditionnel. « Je me demande si
/// j’aurais dû » (interrogation indirecte, après un verbe) n'est pas visé.
fn si_conditional(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(form) = ctx.form(i) else { return };
    let finite: Vec<&Reading> = form
        .readings
        .iter()
        .filter(|r| r.person().is_some())
        .collect();
    if finite.is_empty() || !finite.iter().all(|r| r.mode_tense() == Some("cond.pres")) {
        return;
    }
    let Some(subject) = subject_of(ctx, i) else {
        return;
    };
    if !subject.pronoun {
        return;
    }
    let Some(si) = ctx.prev(subject.start) else {
        return;
    };
    if !matches!(lower(ctx, si), "si" | "s’" | "s'") {
        return;
    }
    let sw = &ctx.words[si];
    // En tête de phrase seulement : après une virgule, « si » introduit souvent
    // une interrogation indirecte (« …, s’il pourrait continuer »).
    let clause_start = sw.sentence_start
        || ctx.prev(si).is_some_and(|p| {
            matches!(lower(ctx, p), "et" | "mais" | "même") && ctx.words[p].sentence_start
        });
    if !clause_start {
        return;
    }
    let wanted = format!("ind.impf.{}", subject.person);
    let mut suggestions: Vec<String> = Vec::new();
    for r in &finite {
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
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    let w = &ctx.words[i];
    out.push(
        RawIssue::new(w.start, w.end, SI_CONDITIONAL)
            .arg(w.text)
            .arg(first)
            .with_suggestions(suggestions),
    );
}

/// « pour serré », « de fermé », « il faut traversé » → infinitif : après une
/// préposition ou un verbe qui appelle un infinitif, un participe en -é d'un
/// verbe du premier groupe est une confusion de terminaison.
/// Mot qui impose l'infinitif au verbe d'indice `i` : « pour », « sans »,
/// « avant de », « afin de », « au lieu de », ou « à » après « commencer »,
/// « continuer »… Les pronoms compléments entre les deux sont sautés.
fn infinitive_introducer(ctx: &Context, i: usize) -> Option<usize> {
    let mut p = ctx.prev(i)?;
    while matches!(
        lower(ctx, p),
        "me" | "m’"
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
            | "pas"
            | "plus"
            | "jamais"
    ) {
        p = ctx.prev(p)?;
    }
    match lower(ctx, p) {
        // « tenir pour avéré », « laissé pour mort » : attribut.
        "pour" => (!ctx.prev(p).and_then(|q| ctx.form(q)).is_some_and(|f| {
            f.has(|r| {
                matches!(
                    r.lemma.as_str(),
                    "tenir" | "laisser" | "donner" | "prendre" | "considérer" | "passer" | "avoir"
                )
            })
        }))
        .then_some(p),
        "sans" => Some(p),
        "de" | "d’" => {
            let q = ctx.prev(p)?;
            let before = ctx.prev(q).map(|b| lower(ctx, b));
            (matches!(lower(ctx, q), "avant" | "afin" | "histoire" | "plutôt")
                || (lower(ctx, q) == "lieu" && before == Some("au")))
            .then_some(p)
        }
        "à" => {
            let q = ctx.prev(p)?;
            ctx.form(q)
                .is_some_and(|f| {
                    f.has(|r| {
                        r.is_finite_verb()
                            && matches!(
                                r.lemma.as_str(),
                                "commencer"
                                    | "continuer"
                                    | "apprendre"
                                    | "réussir"
                                    | "hésiter"
                                    | "parvenir"
                                    | "tarder"
                                    | "s’habituer"
                                    | "renoncer"
                            )
                    }) && !f.has(|r| matches!(r.pos, Pos::Nom | Pos::Adj))
                })
                .then_some(p)
        }
        _ => None,
    }
}

fn participle_for_infinitive(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    // « d’Honoré de Balzac », « Sablé-sur-Sarthe » : nom propre.
    if is_capitalized(w.text) {
        return;
    }
    // Après un verbe modal (« il faut travaillé », « tu peux m’aidé », « ils vont
    // arrivés »), le participe, même accordé, est un infinitif mal écrit.
    // « il se sait aimé », « je compte fermé » : savoir et compter n'ont pas ce sens.
    let modal_word = modal_before(ctx, i).filter(|&k| {
        !ctx.form(k)
            .is_some_and(|f| f.has(|r| matches!(r.lemma.as_str(), "savoir" | "compter")))
    });
    let modal = modal_word.is_some();
    // « pouvoir lui dire » : « lui » (participe de luire) est un pronom.
    let pronoun_like =
        is_class(ctx, i, "clitique") || ctx.data.subjects.contains_key(w.lower.as_str());
    // « nous allons parti », « je peux venu », « il doit finis » : tout participe
    // après un modal est un infinitif mal écrit.
    if let (Some(m), Some(form)) = (modal_word.filter(|_| !pronoun_like), ctx.form(i)) {
        if let Some(r) = form
            .readings
            .iter()
            .find(|r| r.is_past_participle() && !matches!(r.lemma.as_str(), "être" | "avoir"))
        {
            let infinitive = r.lemma.clone();
            if ctx
                .lexicon
                .get(&infinitive)
                .is_some_and(|f| f.has(Reading::is_infinitive))
                && !form.has(|x| matches!(x.pos, Pos::Adv | Pos::Pre | Pos::Con))
            {
                let fixed = same_case(w.text, &infinitive);
                out.push(
                    RawIssue::new(w.start, w.end, PARTICIPLE_FOR_INFINITIVE)
                        .arg(w.text)
                        .arg(fixed.clone())
                        .arg(ctx.words[m].text)
                        .suggest(fixed),
                );
                return;
            }
        }
    }
    if let Some(introducer) = infinitive_introducer(ctx, i) {
        if let Some(form) = ctx.form(i) {
            // Aucune lecture hors du verbe : « sans cesse », « pour sûr » restent corrects.
            // « Pour réussi, il faut » : seul « pour » accepte un adjectif (« pour sûr »,
            // « pour acquis »), sauf devant un complément.
            let lenient = lower(ctx, introducer) != "pour"
                || ctx.next(i).is_some_and(|n| {
                    is_class(ctx, n, "clitique") || ctx.data.determiners.contains_key(lower(ctx, n))
                })
                || (ctx.words[introducer].sentence_start
                    && ctx.text[ctx.words[i].end..].starts_with(','));
            let usable = |r: &Reading| {
                r.pos.is_verb()
                    && !r.is_infinitive()
                    && !matches!(r.lemma.as_str(), "être" | "avoir")
            };
            // « pour lui », « sans péché », « n’importe » : pronoms, noms et idiomes.
            let verbal_only = !form.has(Reading::is_infinitive)
                && !is_class(ctx, i, "clitique")
                && !ctx.data.subjects.contains_key(w.lower.as_str())
                && w.lower != "importe"
                && form.has(usable)
                && form
                    .readings
                    .iter()
                    .all(|r| usable(r) || (lenient && r.pos == Pos::Adj));
            let lemma = form
                .readings
                .iter()
                .find(|r| r.is_past_participle())
                .or_else(|| form.readings.first())
                .map(|r| r.lemma.clone());
            if let (true, Some(infinitive)) = (verbal_only, lemma) {
                if ctx
                    .lexicon
                    .get(&infinitive)
                    .is_some_and(|f| f.has(Reading::is_infinitive))
                {
                    let fixed = same_case(w.text, &infinitive);
                    out.push(
                        RawIssue::new(w.start, w.end, PARTICIPLE_FOR_INFINITIVE)
                            .arg(w.text)
                            .arg(fixed.clone())
                            .arg(ctx.words[introducer].text)
                            .suggest(fixed),
                    );
                    return;
                }
            }
        }
    }
    let stem = ["ées", "és", "ée", "é"]
        .iter()
        .filter(|e| modal || **e == "é")
        .find_map(|e| w.lower.strip_suffix(e));
    let Some(stem) = stem else {
        return;
    };
    let Some(form) = ctx.form(i) else { return };
    // « le passé », « un fermé » : nom possible, on s'abstient.
    if !form.has(|r| r.is_past_participle() && r.lemma.ends_with("er"))
        || (form.has_pos(Pos::Nom) && !modal)
    {
        return;
    }
    let infinitive = format!("{stem}er");
    if !ctx
        .lexicon
        .get(&infinitive)
        .is_some_and(|f| f.has(Reading::is_infinitive))
    {
        return;
    }
    // « heureux de vous rencontré » : pronoms compléments entre la préposition et
    // le participe ; l'infinitif est alors seul, sans complément à suivre.
    let mut clitics = false;
    let mut pp = ctx.prev(i);
    while let Some(k) = pp.filter(|&k| {
        matches!(
            lower(ctx, k),
            "me" | "m’" | "te" | "t’" | "se" | "s’" | "nous" | "vous" | "lui"
        )
    }) {
        clitics = true;
        pp = ctx.prev(k);
    }
    let Some(p) = pp else {
        return;
    };
    if modal {
        let fixed = same_case(w.text, &infinitive);
        out.push(
            RawIssue::new(w.start, w.end, PARTICIPLE_FOR_INFINITIVE)
                .arg(w.text)
                .arg(fixed.clone())
                .arg(ctx.words[modal_word.unwrap_or(p)].text)
                .suggest(fixed),
        );
        return;
    }
    // « un poste de chargé de mission », « le titre de délégué » : fonction.
    if matches!(lower(ctx, p), "de" | "d’" | "d'")
        && ctx.prev(p).is_some_and(|q| {
            matches!(
                lower(ctx, q),
                "poste"
                    | "postes"
                    | "fonction"
                    | "fonctions"
                    | "titre"
                    | "titres"
                    | "rôle"
                    | "rôles"
                    | "métier"
                    | "métiers"
                    | "statut"
                    | "qualité"
                    | "grade"
                    | "emploi"
                    | "profession"
                    | "mandat"
                    | "place"
                    | "situation"
                    | "position"
                    | "responsabilité"
                    | "poste-clé"
                    | "offre"
                    | "offres"
                    | "recrutement"
            )
        })
    {
        return;
    }
    let introduces = match lower(ctx, p) {
        // « rien de prévu », « quelque chose de cassé » : participe correct.
        // « rien de prévu », « quelqu’un d’autorisé », « quelque chose d’osseux
        // et de glacé » : adjectif après un pronom indéfini.
        "de" | "d’" => !(p.saturating_sub(6)..p).any(|q| {
            matches!(
                lower(ctx, q),
                "rien"
                    | "chose"
                    | "personne"
                    | "quoi"
                    | "plus"
                    | "moins"
                    | "quelqu’un"
                    | "quelqu'un"
                    | "quiconque"
            )
        }),
        // « tenir pour avéré », « laissé pour mort » : attribut.
        "pour" => !ctx.prev(p).and_then(|q| ctx.form(q)).is_some_and(|f| {
            f.has(|r| matches!(r.lemma.as_str(), "tenir" | "laisser" | "donner" | "prendre"))
        }),
        "sans" => true,
        _ => ctx.form(p).is_some_and(|f| {
            f.has(|r| {
                r.is_finite_verb()
                    && matches!(
                        r.lemma.as_str(),
                        "falloir"
                            | "pouvoir"
                            | "devoir"
                            | "vouloir"
                            | "aller"
                            | "oser"
                            | "espérer"
                            | "compter"
                            | "préférer"
                    )
            }) && !f.has(|r| matches!(r.pos, Pos::Adv | Pos::Con))
        }),
    };
    // Un complément doit suivre, comme après un infinitif (« pour fermer la
    // porte ») : « des rôles de roué et de snob » n'en a pas.
    let complement = ctx.next(i).is_some_and(|n| {
        is_class(ctx, n, "clitique")
            || ctx.data.determiners.contains_key(lower(ctx, n))
            || ctx
                .form(n)
                .is_some_and(|f| f.has(|r| matches!(r.pos, Pos::Det | Pos::Pre | Pos::Pro)))
    });
    if !introduces || !(complement || clitics) {
        return;
    }
    let fixed = same_case(w.text, &infinitive);
    out.push(
        RawIssue::new(w.start, w.end, PARTICIPLE_FOR_INFINITIVE)
            .arg(w.text)
            .arg(fixed.clone())
            .arg(ctx.words[p].text)
            .suggest(fixed),
    );
}

/// Genre et nombre du sujet du verbe d'indice `verb`, s'il est reconnu.
pub(super) fn subject_gn(ctx: &Context, verb: usize) -> Option<Gn> {
    subject_of(ctx, verb).map(|s| s.gn)
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
            // « ils veulent chacun leur part » : « chacun » flottant, pas un sujet.
            let floating = matches!(lower(ctx, p), "chacun" | "chacune") && !clause_start(ctx, p);
            if !ctx.words[p].compound_part && !floating {
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
    let Some(last) = ctx.prev(k) else {
        return incise_subject(ctx, k, &chain);
    };
    if lower(ctx, last) == "qui" {
        return antecedent_subject(ctx, last, verb, &chain);
    }
    noun_subject(ctx, last, &chain).or_else(|| relative_subject(ctx, k, &chain))
}

/// Groupe nominal sujet qui finit au mot `last`, en début de proposition.
/// `chain` : pronoms entre le sujet et le verbe, du verbe vers le sujet.
fn noun_subject(ctx: &Context, last: usize, chain: &[usize]) -> Option<FoundSubject> {
    // « Certains d’entre eux nécessite » : pronom de quantité, sujet au pluriel.
    if matches!(lower(ctx, last), "eux" | "elles") {
        let head = ctx
            .prev(last)
            .filter(|&e| lower(ctx, e) == "entre")
            .and_then(|e| ctx.prev(e))
            .filter(|&d| matches!(lower(ctx, d), "d’" | "d'"))
            .and_then(|d| ctx.prev(d))
            .filter(|&h| {
                matches!(
                    lower(ctx, h),
                    "certains"
                        | "certaines"
                        | "plusieurs"
                        | "beaucoup"
                        | "quelques-uns"
                        | "quelques-unes"
                ) && clause_start(ctx, h)
            });
        if let Some(h) = head {
            let mut chain = chain.to_vec();
            chain.reverse();
            return Some(FoundSubject {
                person: "3p".into(),
                gn: Gn {
                    gender: match lower(ctx, h) {
                        "certains" | "quelques-uns" => Some('m'),
                        "certaines" | "quelques-unes" => Some('f'),
                        _ => None,
                    },
                    number: Some('p'),
                },
                start: h,
                chain,
                pronoun: false,
            });
        }
    }
    // Groupe nominal en début de proposition : « Les enfants jouent ». Il peut
    // être suivi d'un complément en « de » : « La réunion de lundi a été ».
    let mut ends = vec![last];
    if let Some(d) = (last.saturating_sub(3)..last)
        .rev()
        .find(|&d| matches!(lower(ctx, d), "de" | "d’" | "d'" | "du" | "des"))
    {
        let nominal = ctx.form(last).is_some_and(|f| f.has_pos(Pos::Nom))
            && (d + 1..=last).all(|j| {
                ctx.form(j).is_some_and(|f| {
                    f.has(|r| matches!(r.pos, Pos::Nom | Pos::Adj | Pos::Det))
                        && !f.has(Reading::is_finite_verb)
                })
            });
        if let Some(head) = ctx.prev(d).filter(|_| nominal && ctx.next(d).is_some()) {
            // « Certaines de ces modifications reste » : le pronom de quantité
            // est le sujet, au pluriel.
            if matches!(lower(ctx, head), "certains" | "certaines" | "plusieurs")
                && clause_start(ctx, head)
            {
                let gender = match lower(ctx, head) {
                    "certains" => Some('m'),
                    "certaines" => Some('f'),
                    _ => None,
                };
                let mut chain = chain.to_vec();
                chain.reverse();
                return Some(FoundSubject {
                    person: "3p".into(),
                    gn: Gn {
                        gender,
                        number: Some('p'),
                    },
                    start: head,
                    chain,
                    pronoun: false,
                });
            }
            // « La plupart des gens sont » : Lexique ne note « plupart » que comme
            // adverbe ; le sujet est pluriel.
            if lower(ctx, head) == "plupart" {
                if let Some(la) = ctx
                    .prev(head)
                    .filter(|&l| lower(ctx, l) == "la" && clause_start(ctx, l))
                {
                    let mut chain = chain.to_vec();
                    chain.reverse();
                    let gender = find_noun_phrase(ctx, d)
                        .and_then(|np| phrase_gn(ctx, &np))
                        .and_then(|g| g.gender);
                    return Some(FoundSubject {
                        person: "3p".into(),
                        gn: Gn {
                            gender,
                            number: Some('p'),
                        },
                        start: la,
                        chain,
                        pronoun: false,
                    });
                }
            }
            ends.push(head);
        }
    }
    for (last, det) in ends.into_iter().flat_map(|last| {
        (last.saturating_sub(4)..last)
            .rev()
            .map(move |det| (last, det))
    }) {
        let Some(np) = find_noun_phrase(ctx, det) else {
            continue;
        };
        // « Tous ces événements sont connus » : « tous » devant le déterminant.
        let predet = ctx
            .prev(det)
            .filter(|&p| matches!(lower(ctx, p), "tout" | "toute" | "tous" | "toutes"));
        let first = predet.unwrap_or(det);
        // « La majeure partie (65 %) du parc est », « « L'esprit de corps » des
        // polytechniciens tend » : après une parenthèse ou un guillemet fermants,
        // le groupe complète ce qui précède.
        let closing = first > 0
            && ctx.text[ctx.words[first - 1].end..ctx.words[first].start]
                .trim_end()
                .ends_with([')', '»', '”', '"']);
        let starts =
            (clause_start(ctx, det) || predet.is_some_and(|p| clause_start(ctx, p))) && !closing;
        // « un génie qui, tous les soirs descendait », « La même année
        // paraissent deux comics » : complément de temps, le sujet est inversé.
        let time = (predet.is_some() && is_class(ctx, np.noun, "nom_adverbial"))
            || matches!(
                lower(ctx, np.noun),
                "année" | "semaine" | "mois" | "nuit" | "saison" | "veille" | "décennie"
            );
        // « Au village règnent » : complément introduit par « au », « aux ».
        let preposition = matches!(lower(ctx, det), "au" | "aux");
        if np.post.unwrap_or(np.noun) != last || !starts || time || preposition {
            continue;
        }
        let gn = phrase_gn(ctx, &np)?;
        let person = match gn.number? {
            's' => "3s",
            _ => "3p",
        };
        let mut chain = chain.to_vec();
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

/// « Ma sœur, qui s’était absentée quelques minutes, est revenue » : sujet
/// suivi d'une relative entre virgules. `k` : premier mot de la chaîne verbale.
fn incise_subject(ctx: &Context, k: usize, chain: &[usize]) -> Option<FoundSubject> {
    let gap = |j: usize| &ctx.text[ctx.words[j - 1].end..ctx.words[j].start];
    if k == 0 || ctx.words[k].sentence_start || gap(k).trim() != "," {
        return None;
    }
    // Virgule ouvrante : au plus 15 mots plus tôt, dans la même phrase.
    let mut j = k - 1;
    for _ in 0..15 {
        if j == 0 || ctx.words[j].sentence_start {
            return None;
        }
        let g = gap(j);
        if g.contains(',') {
            if g.trim() != "," {
                return None;
            }
            break;
        }
        if g.contains(['.', ';', ':', '!', '?', '(', ')', '«', '»', '—']) {
            return None;
        }
        j -= 1;
    }
    if !matches!(lower(ctx, j), "qui" | "que" | "qu’" | "qu'" | "dont") {
        return None;
    }
    noun_subject(ctx, j - 1, chain)
}

/// « Les chiffres qu’on nous a communiqués sont… », « L’homme dont les fils
/// sont partis est… » : sujet séparé du verbe par une relative qui ne contient
/// qu'un verbe conjugué. `k` : premier mot de la chaîne verbale.
fn relative_subject(ctx: &Context, k: usize, chain: &[usize]) -> Option<FoundSubject> {
    let mut r = k;
    for _ in 0..8 {
        r = ctx.prev(r)?;
        if matches!(lower(ctx, r), "que" | "qu’" | "qu'" | "qui" | "dont" | "où") {
            break;
        }
    }
    if !matches!(lower(ctx, r), "que" | "qu’" | "qu'" | "qui" | "dont" | "où") {
        return None;
    }
    let finite = (r + 1..k)
        .filter(|&j| {
            ctx.form(j).is_some_and(|f| {
                f.has(|x| x.pos == Pos::Aux && x.is_finite_verb())
                    || (f.has(Reading::is_finite_verb)
                        && !f.has_pos(Pos::Nom)
                        && !f.has_pos(Pos::Adj))
            })
        })
        .count();
    // « qu’il recherchait sans s’en rendre compte » : la relative ne doit
    // contenir ni préposition ni infinitif, sinon sa fin est incertaine.
    let simple = (r + 1..k).all(|j| {
        ctx.form(j).is_none_or(|f| {
            !(f.has_pos(Pos::Pre) && !f.has_pos(Pos::Pro)) && !f.only(Reading::is_infinitive)
        })
    });
    if finite != 1 || !simple {
        return None;
    }
    noun_subject(ctx, ctx.prev(r)?, chain)
}

/// « les délais de livraison, qui sont trop longs » : « qui » sujet, accordé
/// avec son antécédent. Après un complément en « de », l'antécédent est celui
/// des deux noms dont le nombre convient au verbe ; sinon, on s'abstient.
fn antecedent_subject(
    ctx: &Context,
    qui: usize,
    verb: usize,
    chain: &[usize],
) -> Option<FoundSubject> {
    let before = qui.checked_sub(1)?;
    let gap = &ctx.text[ctx.words[before].end..ctx.words[qui].start];
    if !gap.chars().all(|c| c.is_whitespace() || c == ',') {
        return None;
    }
    let ending = |end: usize| {
        (end.saturating_sub(4)..end)
            .rev()
            .filter_map(|d| find_noun_phrase(ctx, d))
            .find(|np| np.post.unwrap_or(np.noun) == end)
    };
    // Antécédent sûr seulement : groupe nominal qui n'est pas lui-même
    // complément (« sous la lampe qui… », « du changement qui… » : on
    // s'abstient), éventuellement suivi d'un complément sans déterminant
    // (« les délais de livraison, qui… »).
    let mut candidates = Vec::new();
    let mut bare = false;
    let np = match ending(before) {
        Some(near) => Some(near),
        None => {
            let noun = ctx
                .form(before)
                .is_some_and(|f| f.only(|r| r.pos == Pos::Nom));
            bare = true;
            ctx.prev(before)
                .filter(|&b| noun && matches!(lower(ctx, b), "de" | "d’" | "d'"))
                .and_then(|b| ctx.prev(b))
                .and_then(ending)
        }
    }?;
    let complement = matches!(lower(ctx, np.det), "du" | "des" | "au" | "aux")
        || ctx.prev(np.det).is_some_and(|b| {
            matches!(lower(ctx, b), "de" | "d’" | "d'")
                || ctx.form(b).is_some_and(|f| f.has_pos(Pos::Pre))
        });
    if complement {
        return None;
    }
    candidates.push(np);
    let persons: Vec<&str> = ctx
        .form(verb)?
        .readings
        .iter()
        .filter_map(Reading::person)
        .collect();
    let mut found = candidates.iter().filter_map(|np| {
        let gn = phrase_gn(ctx, np)?;
        let person = if gn.number? == 's' { "3s" } else { "3p" };
        Some((np.det, gn, person))
    });
    let (start, gn, person) = if candidates.len() == 1 && !bare {
        found.next()?
    } else {
        found.find(|(_, _, p)| persons.contains(p))?
    };
    let mut chain = chain.to_vec();
    chain.reverse();
    Some(FoundSubject {
        person: person.into(),
        gn,
        start,
        chain,
        pronoun: false,
    })
}

fn subject_text(ctx: &Context, s: &FoundSubject, verb: usize) -> String {
    let end = s.chain.first().map_or(verb, |&c| c) - 1;
    rebuild(ctx, s.start, end, &[])
}

/// Verbes (tous temps) qui veulent le subjonctif dans la complétive en « que ».
const SUBJUNCTIVE_VERBS: &[&str] = &[
    "falloir",
    "vouloir",
    "souhaiter",
    "désirer",
    "aimer",
    "préférer",
    "douter",
    "craindre",
    "regretter",
    "exiger",
    "demander",
    "ordonner",
    "interdire",
    "permettre",
    "empêcher",
    "attendre",
    "accepter",
    "éviter",
    "redouter",
    "déplorer",
    "veiller",
];
/// Verbes d'opinion : subjonctif seulement à la forme négative (« je ne pense pas que »).
const OPINION_VERBS: &[&str] = &["penser", "croire", "estimer", "imaginer", "supposer"];
/// Adjectifs après « il est », « c’est » qui veulent le subjonctif.
const SUBJUNCTIVE_ADJECTIVES: &[&str] = &[
    "possible",
    "impossible",
    "nécessaire",
    "important",
    "essentiel",
    "préférable",
    "normal",
    "dommage",
    "temps",
    "utile",
    "indispensable",
    "urgent",
    "bon",
    "rare",
    "juste",
    "regrettable",
    "souhaitable",
    "étonnant",
    "surprenant",
    "naturel",
    "inadmissible",
    "inutile",
];

/// Locution ou verbe qui demande le subjonctif devant la complétive en « que » :
/// renvoie son texte (« il faut que »), ou rien.
fn subjunctive_trigger(ctx: &Context, que: usize) -> Option<String> {
    let text = |from: usize| ctx.text[ctx.words[from].start..ctx.words[que].end].to_string();
    let w1 = ctx.prev(que)?;
    let l1 = lower(ctx, w1);
    let w2 = ctx.prev(w1);
    let l2 = w2.map(|k| lower(ctx, k));
    // Conjonctions : « pour que », « à moins que », « jusqu’à ce que »…
    match (l2, l1) {
        (_, "pour" | "afin" | "avant" | "sans" | "pourvu") => return Some(text(w1)),
        (Some("à"), "moins" | "condition" | "supposer") => return Some(text(w2?)),
        (Some("de"), "peur" | "crainte") => return Some(text(w2?)),
        (Some("en"), "attendant") => return Some(text(w2?)),
        (Some("jusqu’à" | "jusqu'à"), "ce") => return Some(text(w2?)),
        (Some("à"), "ce") => {
            let j = ctx.prev(w2?)?;
            if matches!(lower(ctx, j), "jusqu’" | "jusqu'") {
                return Some(text(j));
            }
        }
        _ => {}
    }
    // « bien que », « encore que » en tête de proposition ; pas « si bien que »,
    // « tant bien que mal » (conséquence, indicatif).
    if matches!(l1, "bien" | "encore")
        && clause_start(ctx, w1)
        && !matches!(
            l2,
            Some("si" | "tant" | "aussi" | "très" | "trop" | "assez" | "tellement")
        )
        && !(l2 == Some("et")
            && ctx
                .prev(w1)
                .and_then(|k| ctx.prev(k))
                .is_some_and(|b| lower(ctx, b) == "bel"))
    {
        return Some(text(w1));
    }
    // « quoi que », « où que », « qui que ».
    if matches!(l1, "quoi" | "où" | "qui")
        && ctx.prev(w1).is_none_or(|p| lower(ctx, p) != "ce")
        && clause_start(ctx, w1)
    {
        return Some(text(w1));
    }
    // « il se peut que », « il vaut mieux que ».
    if l1 == "peut" && l2 == Some("se") {
        return Some(text(ctx.prev(w1)?));
    }
    if l1 == "mieux"
        && ctx
            .prev(w1)
            .and_then(|k| ctx.form(k))
            .is_some_and(|f| f.has(|r| r.lemma == "valoir"))
    {
        return Some(text(ctx.prev(w1)?));
    }
    // « il est possible que », « c’est dommage que ».
    if SUBJUNCTIVE_ADJECTIVES.contains(&l1) {
        let e = ctx.prev(w1)?;
        let etre = ctx
            .form(e)
            .is_some_and(|f| f.has(|r| r.lemma == "être" && r.is_finite_verb()));
        if etre || matches!(lower(ctx, e), "c’" | "c'") {
            return Some(text(e));
        }
    }
    // « il faut que », « je veux que », « j’aimerais que »…
    let f1 = ctx.form(w1)?;
    // « hors de doute qu’il », « n’empêche que » : nom ou idiome, indicatif.
    let noun_use = ctx.prev(w1).is_some_and(|p| {
        matches!(
            lower(ctx, p),
            "de" | "d’"
                | "sans"
                | "le"
                | "un"
                | "ce"
                | "du"
                | "au"
                | "mon"
                | "ton"
                | "son"
                | "aucun"
                | "aucune"
                | "nul"
                | "tout"
        )
    });
    let idiom = l1 == "empêche"
        && ctx
            .prev(w1)
            .is_some_and(|p| matches!(lower(ctx, p), "n’" | "n'" | "ne"));
    if !noun_use
        && !idiom
        && f1.has(|r| {
            // « attendu que », « entendu que » : conjonctions de l'indicatif.
            (r.is_finite_verb()
                || (r.is_past_participle() && !matches!(r.lemma.as_str(), "attendre" | "entendre")))
                && SUBJUNCTIVE_VERBS.contains(&r.lemma.as_str())
        })
    {
        // Verbe lui-même précédé d'un mot qui le rend autre (« ce que je veux que »)
        // reste un déclencheur ; « ne … pas » sans effet.
        return Some(text(w1));
    }
    // « je ne pense pas que ».
    if l1 == "pas" {
        let v = ctx.prev(w1)?;
        if ctx.form(v).is_some_and(|f| {
            f.has(|r| r.is_finite_verb() && OPINION_VERBS.contains(&r.lemma.as_str()))
        }) {
            return Some(text(v));
        }
    }
    None
}

/// « Il faut que vous alliez au marché, que vous allez vite » : un « que » repris
/// après « et », « ou » ou une virgule garde le déclencheur du « que » précédent.
fn subjunctive_context(ctx: &Context, que: usize) -> Option<String> {
    if !matches!(lower(ctx, que), "que" | "qu’" | "qu'") {
        return None;
    }
    if let Some(t) = subjunctive_trigger(ctx, que) {
        return Some(t);
    }
    let before = que.checked_sub(1)?;
    let repeated = matches!(lower(ctx, before), "et" | "ou")
        || ctx.text[ctx.words[before].end..ctx.words[que].start].contains(',');
    if !repeated {
        return None;
    }
    let mut k = que;
    while k > 0 && !ctx.words[k].sentence_start && que - k < 12 {
        k -= 1;
        if matches!(lower(ctx, k), "que" | "qu’" | "qu'") {
            return subjunctive_trigger(ctx, k);
        }
        // Une autre conjonction de subordination, un « mais » ou un pronom relatif
        // (« le pouce … et que vous avez posé ») coupe l'enchaînement.
        if matches!(
            lower(ctx, k),
            "mais" | "car" | "donc" | "si" | "quand" | "lorsque" | "qui" | "dont" | "où"
        ) {
            return None;
        }
    }
    None
}

/// « Il faut que tu viens » → « viennes » : indicatif présent là où le subjonctif
/// est de rigueur, quand les deux formes diffèrent.
fn subjunctive(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(form) = ctx.form(i) else { return };
    let w = &ctx.words[i];
    if w.compound_part || w.ignored {
        return;
    }
    // Pré-filtre : seul un verbe à l'indicatif présent ou au passé simple peut
    // être fautif.
    if !form.has(|r| matches!(r.mode_tense(), Some("ind.pres" | "ind.ps"))) {
        return;
    }
    let Some(subject) = simple_subject(ctx, i) else {
        return;
    };
    let Some(que) = ctx.prev(subject.start) else {
        return;
    };
    // Le sujet touche le verbe : pas une proposition relative entre les deux
    // (« une bête qui n’a que cet âge »).
    if (subject.start..i).any(|k| matches!(lower(ctx, k), "qui" | "que" | "qu’" | "dont" | "où"))
    {
        return;
    }
    let trigger = if matches!(lower(ctx, que), "que" | "qu’" | "qu'") {
        subjunctive_context(ctx, que)
    } else if matches!(lower(ctx, que), "quoique" | "quoiqu’" | "quoiqu'") {
        Some(ctx.words[que].text.to_string())
    } else {
        None
    };
    let Some(trigger) = trigger else { return };
    let person = subject.person.as_str();
    // « bien qu’il eut raison » : le passé simple à la place du subjonctif imparfait.
    let simple_past = person == "3s"
        && form.has(|r| r.mode_tense() == Some("ind.ps") && r.person() == Some("3s"))
        && !form.has(|r| {
            matches!(r.mode_tense(), Some("sub.impf" | "sub.pres" | "ind.pres"))
                && r.person() == Some("3s")
        });
    if simple_past {
        let past_context = (que.saturating_sub(8)..que).any(|k| {
            ctx.form(k).is_some_and(|f| {
                f.has(|r| matches!(r.mode_tense(), Some("ind.impf" | "cond.pres" | "sub.impf")))
            })
        });
        let mut suggestions: Vec<String> = Vec::new();
        for r in form
            .readings
            .iter()
            .filter(|r| r.mode_tense() == Some("ind.ps") && r.person() == Some("3s"))
        {
            let mut tenses = ["sub.impf.3s", "sub.pres.3s"];
            if !past_context {
                tenses.reverse();
            }
            for t in tenses {
                for f in inflected(ctx, &r.lemma, r.pos, t) {
                    let f = same_case(w.text, &f);
                    if !suggestions.contains(&f) {
                        suggestions.push(f);
                    }
                }
            }
        }
        let Some(first) = suggestions.first().cloned() else {
            return;
        };
        out.push(
            RawIssue::new(w.start, w.end, SUBJUNCTIVE)
                .arg(w.text)
                .arg(first)
                .arg(trigger)
                .with_suggestions(suggestions),
        );
        return;
    }
    let indicative: Vec<&Reading> = form
        .readings
        .iter()
        .filter(|r| r.mode_tense() == Some("ind.pres") && r.person() == Some(person))
        .collect();
    // Aucune forme du subjonctif de même graphie : l'erreur est certaine.
    let same = form
        .readings
        .iter()
        .any(|r| r.mode_tense() == Some("sub.pres") && r.person() == Some(person));
    if indicative.is_empty() || same {
        return;
    }
    // « Il faut que je le fais » : le verbe n'est pas à l'infinitif ni participe.
    // Lexique ne note pas le subjonctif présent de la 1re et de la 2e personne du
    // pluriel quand il s'écrit comme l'imparfait (« veniez », « chantions »).
    let wanted = [format!("sub.pres.{person}"), format!("ind.impf.{person}")];
    let mut suggestions: Vec<String> = Vec::new();
    for r in indicative {
        for traits in wanted
            .iter()
            .take(if matches!(person, "1p" | "2p") { 2 } else { 1 })
        {
            for f in ctx
                .lexicon
                .inflect(&r.lemma, |x| x.pos == r.pos && x.traits == *traits)
            {
                let f = same_case(w.text, f);
                if !suggestions.contains(&f) {
                    suggestions.push(f);
                }
            }
            if !suggestions.is_empty() {
                break;
            }
        }
    }
    // « que vous oubliiez » : correct, mais personne ne l'écrit ; on s'abstient.
    suggestions.retain(|f| !f.contains("ii"));
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    out.push(
        RawIssue::new(w.start, w.end, SUBJUNCTIVE)
            .arg(w.text)
            .arg(first)
            .arg(trigger)
            .with_suggestions(suggestions),
    );
}

/// Verbes qui introduisent un discours rapporté.
const REPORTING_VERBS: &[&str] = &[
    "dire",
    "penser",
    "croire",
    "savoir",
    "espérer",
    "annoncer",
    "promettre",
    "affirmer",
    "déclarer",
    "répondre",
    "expliquer",
    "supposer",
    "imaginer",
    "prétendre",
    "assurer",
    "jurer",
    "prévoir",
    "écrire",
    "avouer",
    "ajouter",
    "certifier",
    "garantir",
    "déduire",
    "estimer",
    "juger",
    "soutenir",
    "soupçonner",
    "confirmer",
];

/// Verbe introducteur au passé et sujet de la complétive, quand le verbe `i`
/// (un futur) est le verbe d'un discours rapporté au passé.
fn concordance_context(ctx: &Context, i: usize) -> Option<(FoundSubject, usize)> {
    let subject = simple_subject(ctx, i)?;
    let que = ctx.prev(subject.start)?;
    if !matches!(lower(ctx, que), "que" | "qu’" | "qu'") {
        return None;
    }
    // Le verbe introducteur, à travers adverbes, négation et pronoms.
    let mut v = que;
    loop {
        v = ctx.prev(v)?;
        let l = lower(ctx, v);
        if !(ctx.data.is(l, "adverbe")
            || ctx.data.is(l, "negation")
            || is_class(ctx, v, "clitique")
            || matches!(l, "ne" | "n’" | "pas" | "jamais" | "toujours" | "déjà"))
        {
            break;
        }
    }
    let vf = ctx.form(v)?;
    let past = |r: &Reading| matches!(r.mode_tense(), Some("ind.impf" | "ind.ps"));
    // « dit » est aussi un présent et un participe (« Il est dit que », « Rien ne dit
    // que ») : seules les formes sans ambiguïté comptent.
    let unambiguous = vf.readings.iter().all(|r| r.is_finite_verb() && past(r));
    let direct = unambiguous && vf.has(|r| REPORTING_VERBS.contains(&r.lemma.as_str()));
    // « avait dit que » : le participe, précédé d'un auxiliaire à l'imparfait.
    let compound = vf
        .has(|r| r.is_past_participle() && REPORTING_VERBS.contains(&r.lemma.as_str()))
        && ctx.prev(v).is_some_and(|a| {
            ctx.form(a)
                .is_some_and(|f| f.has(|r| r.pos == Pos::Aux && r.is_finite_verb() && past(r)))
        });
    (direct || compound).then_some((subject, v))
}

fn concordance_applies(ctx: &Context, i: usize) -> bool {
    concordance_context(ctx, i).is_some()
}

/// « Elle disait qu’elle viendra » : après un verbe introducteur à l'imparfait, au
/// passé simple ou au plus-que-parfait, le futur du discours direct devient un
/// conditionnel (« viendrait »).
fn concordance(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let Some(form) = ctx.form(i) else { return };
    // Pré-filtre : un futur de l'indicatif, et seulement lui.
    let finite: Vec<&Reading> = form
        .readings
        .iter()
        .filter(|r| r.is_finite_verb())
        .collect();
    if finite.is_empty() || !finite.iter().all(|r| r.mode_tense() == Some("ind.fut")) {
        return;
    }
    let Some((subject, v)) = concordance_context(ctx, i) else {
        return;
    };
    let person = subject.person.as_str();
    let mut suggestions: Vec<String> = Vec::new();
    for r in &finite {
        for f in inflected(ctx, &r.lemma, r.pos, &format!("cond.pres.{person}")) {
            let f = same_case(ctx.words[i].text, &f);
            if !suggestions.contains(&f) {
                suggestions.push(f);
            }
        }
    }
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    let w = &ctx.words[i];
    out.push(
        RawIssue::new(w.start, w.end, CONCORDANCE)
            .arg(w.text)
            .arg(first)
            .arg(ctx.words[v].text)
            .with_suggestions(suggestions),
    );
}

/// « Je vous rappel que… » : après un pronom sujet (et ses pronoms compléments),
/// un mot qui n'est qu'un nom ne peut pas être le verbe ; on propose les formes
/// conjuguées voisines à la personne du sujet (« rappelle »).
fn noun_for_verb(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    let Some(form) = ctx.form(i) else { return };
    if w.compound_part
        || w.ignored
        || is_capitalized(w.text)
        || w.text.contains('-')
        || !form.only(|r| r.pos == Pos::Nom)
        || ctx.next(i).is_none()
    {
        return;
    }
    let Some(subject) = simple_subject(ctx, i) else {
        return;
    };
    if !subject.pronoun {
        return;
    }
    // « qu’elle heure il est » : homophone déjà signalé (« quelle »).
    let start = &ctx.words[subject.start];
    if out
        .iter()
        .any(|o| o.rule.starts_with("HOMO_") && o.end >= start.start && o.start <= start.end)
    {
        return;
    }
    // Le présent d'abord (« rappelle » avant « rappela »), puis l'ordre de proximité.
    let fits = |f: &str, present: bool| {
        ctx.lexicon.get(&f.to_lowercase()).is_some_and(|x| {
            x.has(|r| {
                r.is_finite_verb()
                    && r.person() == Some(subject.person.as_str())
                    && (!present || r.mode_tense() == Some("ind.pres"))
            })
        })
    };
    let all = crate::suggest::suggest(ctx.lexicon, &ctx.data.elisions, &w.norm, 10);
    let near: Vec<String> = all
        .iter()
        .filter(|f| fits(f, true))
        .chain(all.iter().filter(|f| fits(f, false) && !fits(f, true)))
        .map(|f| same_case(w.text, f))
        .collect();
    let Some(first) = near.first().cloned() else {
        return;
    };
    out.push(
        RawIssue::new(w.start, w.end, SUBJECT_VERB)
            .arg(w.text)
            .arg(first)
            .arg(subject_text(ctx, &subject, i))
            .with_suggestions(near),
    );
}

/// Formes de `lemma` au trait `wanted` : celles du lexique, sinon celles que le
/// modèle de conjugaison du verbe donne (« tu rentras » est absent de Lexique).
pub(super) fn inflected(ctx: &Context, lemma: &str, pos: Pos, wanted: &str) -> Vec<String> {
    let found: Vec<String> = ctx
        .lexicon
        .inflect(lemma, |x| x.pos == pos && x.traits == wanted)
        .into_iter()
        .map(String::from)
        .collect();
    if found.is_empty() && pos == Pos::Ver {
        return ctx.data.conjugator.forms_for(lemma, wanted);
    }
    found
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
    // « Je croyais que tu viendrai » : la concordance des temps s'en charge.
    if form
        .readings
        .iter()
        .filter(|r| r.is_finite_verb())
        .all(|r| r.mode_tense() == Some("ind.fut"))
        && concordance_applies(ctx, i)
    {
        return;
    }
    // « les charges relevées contre lui » : préposition, pas le verbe contrer.
    if form.has_pos(Pos::Pre) {
        return;
    }
    // « Le chat et le chien dorme », « Marie et Paul est » : sujet coordonné.
    if let Some(sub) = and_subject(ctx, i) {
        and_subject_verb(ctx, i, &sub, out);
        return;
    }
    // « … et les clients semble satisfaits » : un sujet pluriel, même coordonné,
    // demande un verbe au pluriel.
    let Some(mut subject) = simple_subject(ctx, i) else {
        return;
    };
    // « sa chétive aventure » : nom après un adjectif, pas un verbe.
    if form.has_pos(Pos::Nom)
        && ctx
            .prev(i)
            .and_then(|p| ctx.form(p))
            .is_some_and(|f| f.only(|r| r.pos == Pos::Adj))
    {
        return;
    }
    // « qui » : l'antécédent est trop souvent incertain pour l'accord du verbe.
    let first = subject.chain.first().copied().unwrap_or(i);
    if ctx.prev(first).is_some_and(|q| lower(ctx, q) == "qui") {
        return;
    }
    // Seulement après une proposition (« ont augmenté de 12 % et les clients
    // semble »), pas après un nom (« sa voiture et ses bijoux pouvait »).
    let plural_subject = !subject.pronoun
        && subject.gn.number == Some('p')
        && ctx
            .prev(subject.start)
            .filter(|&p| matches!(lower(ctx, p), "et" | "ou" | "ni"))
            .and_then(|p| p.checked_sub(1))
            .is_some_and(|b| {
                ctx.words[b].has_digit
                    || ctx.text[ctx.words[b].end..].trim_start().starts_with('%')
                    || ctx
                        .form(b)
                        .is_some_and(|f| !f.has_pos(Pos::Nom) && !f.has_pos(Pos::Adj))
            });
    // « des ménages » : après un déterminant (autre que « le », « la », « les »,
    // qui peuvent être pronoms), le mot est un nom, pas un verbe.
    if ctx.prev(i).is_some_and(|p| {
        ctx.data.determiners.contains_key(lower(ctx, p)) && !is_class(ctx, p, "clitique")
    }) {
        return;
    }
    if !plural_subject && subject_of(ctx, i).is_none() {
        return;
    }
    // « La plupart des gens est » : « la plupart des » demande le pluriel.
    let plupart = !subject.pronoun
        && (subject.start..i)
            .find(|&k| lower(ctx, k) == "plupart")
            .is_some_and(|k| {
                k == subject.start + 1
                    && ctx
                        .next(k)
                        .is_some_and(|n| matches!(lower(ctx, n), "des" | "de" | "d’" | "d'"))
            });
    if plupart {
        subject.person = "3p".into();
        subject.gn.number = Some('p');
    }
    // Après un groupe nominal, un mot qui peut être adjectif est ambigu.
    if !subject.pronoun && form.has(adjectival) {
        return;
    }
    let vw = &ctx.words[i];
    let chain: Vec<&str> = subject.chain.iter().map(|&c| lower(ctx, c)).collect();
    // « les types A », « organisées en Écosse », « chevaux Barbe » : nom propre.
    // « en livres », « en bois » : « en » préposition devant un nom. « les
    // portes » : déterminant devant un nom. « une crise puis un conflit » : adverbe.
    let noun_after_clitic = form.has_pos(Pos::Nom)
        && chain
            .iter()
            .any(|c| matches!(*c, "en" | "le" | "la" | "l’" | "les" | "leur"));
    if (is_capitalized(vw.text) && !vw.sentence_start)
        || noun_after_clitic
        || (!subject.pronoun && form.has(|r| matches!(r.pos, Pos::Con | Pos::Adv)))
    {
        return;
    }
    if !subject.pronoun {
        let head = subject.start;
        let collective = collective_subject(ctx, &subject, i);
        // « la traduction comme la notice sont », « sa fiabilité ainsi que sa
        // taille ont » : sujets coordonnés. « 9 % des polytechniciens s’oriente » :
        // le sujet est la quantité.
        let before = head.checked_sub(1);
        let coordinated = coordinated_subject(ctx, &subject)
            || (!ctx.words[head].sentence_start
                && before.is_some_and(|b| {
                    ctx.words[b].has_digit
                        || ctx.text[ctx.words[b].end..ctx.words[head].start].contains('%')
                }));
        if (collective && !plupart) || (coordinated && !plural_subject) {
            return;
        }
    }
    // « que nous montre le film », « il nous voit, nous aborde » : hors du début
    // de phrase, « nous » et « vous » devant un verbe qui ne s'accorde pas sont
    // plutôt compléments.
    // « nous somme allés » → « sommes » : forme d'être ou d'avoir à une lettre
    // près, à la personne du sujet ; faute de frappe probable.
    let word = ctx.words[i].lower.as_str();
    let near_auxiliary: Vec<String> = ["être", "avoir"]
        .iter()
        .flat_map(|aux| {
            ctx.lexicon.inflect(aux, |x| {
                x.pos == Pos::Aux && x.person() == Some(subject.person.as_str())
            })
        })
        .filter(|f| {
            let (a, b) = if f.len() > word.len() {
                (*f, word)
            } else {
                (word, *f)
            };
            a.len() == b.len() + 1 && a.starts_with(b)
        })
        .map(|f| same_case(ctx.words[i].text, f))
        .collect();
    let start = &ctx.words[subject.start];
    if matches!(start.lower.as_str(), "nous" | "vous")
        && !start.sentence_start
        && near_auxiliary.is_empty()
    {
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
    let mut suggestions: Vec<String> = near_auxiliary;
    // « Je vous est dit » : un pronom complément d'une autre personne que le
    // sujet, puis le participe d'un verbe conjugué avec avoir ; c'est l'auxiliaire
    // avoir qu'il faut (« je vous ai dit »), pas « suis ».
    if wants_avoir(ctx, &subject, i) {
        for r in &finite {
            let wanted = format!("{}.{}", r.mode_tense().unwrap_or(""), subject.person);
            for f in ctx
                .lexicon
                .inflect("avoir", |x| x.pos == Pos::Aux && x.traits == wanted)
            {
                let f = same_case(ctx.words[i].text, f);
                if !suggestions.contains(&f) {
                    suggestions.push(f);
                }
            }
        }
    }
    // « qu’il vas » après « il faut » : le subjonctif d'abord (« aille »).
    if ctx
        .prev(subject.start)
        .is_some_and(|q| subjunctive_context(ctx, q).is_some())
    {
        let mut moods: Vec<String> = Vec::new();
        for r in &finite {
            let wanted = format!("sub.pres.{}", subject.person);
            for f in inflected(ctx, &r.lemma, r.pos, &wanted) {
                let f = same_case(ctx.words[i].text, &f);
                if !moods.contains(&f) {
                    moods.push(f);
                }
            }
        }
        moods.retain(|f| !f.contains("ii"));
        moods.extend(std::mem::take(&mut suggestions));
        suggestions = moods;
    }
    for r in readings {
        let wanted = format!("{}.{}", r.mode_tense().unwrap_or(""), subject.person);
        for f in inflected(ctx, &r.lemma, r.pos, &wanted) {
            let f = same_case(ctx.words[i].text, &f);
            if !suggestions.contains(&f) {
                suggestions.push(f);
            }
        }
    }
    if suggestions.is_empty() {
        return;
    }
    // La forme écrite est celle que le modèle donne : Lexique ne note pas toujours
    // toutes les personnes (« je pus », « que je contredise »).
    if suggestions
        .iter()
        .any(|f| f.to_lowercase() == ctx.words[i].lower)
    {
        return;
    }
    suggestions.truncate(ctx.options.max_suggestions.max(1));
    let w = &ctx.words[i];
    // « Il ont fini » : on peut accorder le verbe ou mettre le pronom au pluriel.
    if subject.pronoun
        && subject.chain.is_empty()
        && ctx.next(subject.start) == Some(i)
        && matches!(lower(ctx, subject.start), "il" | "elle")
        && matches!(subject.person.as_str(), "3s")
        && ctx.form(i).is_some_and(|f| {
            !f.has(|r| r.person() == Some("3s")) && f.has(|r| r.person() == Some("3p"))
        })
    {
        let pron = &ctx.words[subject.start];
        let swapped = match pron.lower.as_str() {
            "il" => "ils",
            _ => "elles",
        };
        let swapped = same_case(pron.text, swapped);
        // Un « s » oublié ou ajouté au pronom est la faute la plus courante.
        let both = vec![
            format!("{swapped} {}", w.text),
            format!("{} {}", pron.text, suggestions[0]),
        ];
        let found = &ctx.text[pron.start..w.end];
        out.push(
            RawIssue::new(pron.start, w.end, SUBJECT_VERB)
                .arg(found)
                .arg(both[0].clone())
                .arg(subject_text(ctx, &subject, i))
                .with_suggestions(both),
        );
        return;
    }
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
    let (mut etre, mut avoir) = (aux("être"), aux("avoir"));
    if !etre && !avoir {
        return;
    }
    // « apprendre a utiliser » : « a » est déjà signalé comme préposition.
    if out.iter().any(|o| {
        o.rule == "HOMO_A_PREPOSITION" && o.start == ctx.words[i].start && o.end == ctx.words[i].end
    }) {
        return;
    }
    // « la ligne A relie » : lettre ou nom en capitale, pas l'auxiliaire.
    if ctx.words[i].text == "A" && !ctx.words[i].sentence_start {
        return;
    }
    // « et les tests ont étés réalisés » : « été » ne prend jamais de s.
    if avoir {
        if let Some(t) = ctx.next(i).filter(|&t| ctx.words[t].lower == "étés") {
            let w = &ctx.words[t];
            out.push(
                RawIssue::new(w.start, w.end, PARTICIPLE_AVOIR)
                    .arg(w.text)
                    .arg(same_case(w.text, "été"))
                    .suggest(same_case(w.text, "été")),
            );
            return;
        }
    }
    // « Odette avait présenter », « la porte a tourner » : avoir suivi d'un
    // infinitif en -er est fautif même sans sujet reconnu (nom propre…).
    // « machine a laver » : « a » pour « à » après un nom, signalé ailleurs.
    // « n’aurait pas suffi a recomposer », « facile a dire » : de même après un
    // participe ou un adjectif. « A continuer ainsi » : « À » sans accent, tour
    // des textes anciens.
    let after_noun = ctx
        .prev(i)
        .and_then(|p| ctx.form(p))
        .is_some_and(|f| f.only(|r| r.pos == Pos::Nom) || f.has(adjectival));
    if avoir && !after_noun && ctx.words[i].text != "A" && subject_of(ctx, i).is_none() {
        if let Some(t) = ctx.next(i).filter(|&t| !is_capitalized(ctx.words[t].text)) {
            if let Some(ft) = ctx.form(t) {
                let er = ft.has(|r| r.is_infinitive() && r.lemma.ends_with("er"));
                if er && !ft.has(adjectival) && !ft.has_pos(Pos::Nom) {
                    let ms = Gn {
                        gender: Some('m'),
                        number: Some('s'),
                    };
                    let suggestions = participles(ctx, ft, Reading::is_infinitive, ms);
                    if let Some(first) = suggestions.first().cloned() {
                        let w = &ctx.words[t];
                        out.push(
                            RawIssue::new(w.start, w.end, INFINITIVE)
                                .arg(w.text)
                                .arg(first)
                                .with_suggestions(suggestions),
                        );
                    }
                }
            }
        }
        return;
    }
    let Some(subject) = and_subject(ctx, i).or_else(|| subject_of(ctx, i)) else {
        return;
    };
    // « la pluie dont nous avait menacés le capucin » : « nous » ne peut pas
    // être le sujet de « avait », il est complément ; le sujet suit.
    if subject.pronoun
        && !form.has(|r| r.pos == Pos::Aux && r.person() == Some(subject.person.as_str()))
    {
        return;
    }

    // Mot qui suit l'auxiliaire, après d'éventuels adverbes (« n’est pas encore parti »)
    // ou « tous », « toutes » (« ils étaient tous très contents »).
    let mut t = ctx.next(i);
    for _ in 0..3 {
        match t {
            // « notamment », « souvent » : adverbes en -ment ou de la liste.
            Some(j)
                if is_class(ctx, j, "adverbe")
                    || (ctx.words[j].lower.ends_with("ment")
                        && ctx.form(j).is_some_and(|f| f.has_pos(Pos::Adv))) =>
            {
                t = ctx.next(j)
            }
            _ => break,
        }
    }
    let Some(mut t) = t else { return };
    // « la réunion a été reportée » : être à un temps composé, le mot suivant
    // s'accorde avec le sujet.
    if avoir && lower(ctx, t) == "été" {
        let mut next = ctx.next(t);
        for _ in 0..3 {
            match next {
                Some(j) if is_class(ctx, j, "adverbe") => next = ctx.next(j),
                _ => break,
            }
        }
        let Some(n) = next else { return };
        // « elle avait été chercher » : « été » au sens d'« allé ».
        if ctx.form(n).is_some_and(|f| f.has(Reading::is_infinitive)) {
            return;
        }
        (t, etre, avoir) = (n, true, false);
    }
    let Some(ft) = ctx.form(t) else { return };
    // « Il a tombé » → « est tombé » : verbes qui se conjuguent seulement avec être.
    if avoir && ctx.next(i) == Some(t) && etre_only_participle(ft) {
        let target = if subject.gn == Gn::default() {
            Gn {
                gender: Some('m'),
                number: Some('s'),
            }
        } else {
            subject.gn
        };
        let traits = form
            .readings
            .iter()
            .find(|r| r.pos == Pos::Aux && r.lemma == "avoir" && r.is_finite_verb())
            .map(|r| r.traits.clone());
        let aux_form = traits.and_then(|tr| {
            ctx.lexicon
                .inflect("être", |x| x.pos == Pos::Aux && x.traits == tr)
                .first()
                .map(|f| f.to_string())
        });
        let participle = participles(ctx, ft, Reading::is_past_participle, target)
            .first()
            .cloned();
        if let (Some(a), Some(pp)) = (aux_form, participle) {
            let (aw, tw) = (&ctx.words[i], &ctx.words[t]);
            let fixed = format!("{} {pp}", same_case(aw.text, &a));
            out.push(
                RawIssue::new(aw.start, tw.end, AUXILIARY_ETRE)
                    .arg(&ctx.text[aw.start..tw.end])
                    .arg(fixed.clone())
                    .suggest(fixed),
            );
            return;
        }
    }
    // « elle était sur le pas », « comme s’il était entre guillemets » : préposition.
    if ft.has_pos(Pos::Pre) {
        return;
    }
    let w = &ctx.words[t];
    let has_object = subject.chain.iter().any(|&c| is_class(ctx, c, "objet"));

    // « il a manger », « elle est aller » → participe passé. « Mes collègues,
    // que j’ai toujours respecter » : accordé avec l'antécédent de « que ».
    if ft.has(Reading::is_infinitive) && !ft.has(adjectival) {
        let target = if avoir {
            let before = if subject.chain.iter().all(|&c| {
                matches!(
                    lower(ctx, c),
                    "me" | "m’"
                        | "m'"
                        | "te"
                        | "t’"
                        | "t'"
                        | "lui"
                        | "leur"
                        | "y"
                        | "ne"
                        | "n’"
                        | "n'"
                )
            }) {
                que_antecedent(ctx, &subject).map(|(_, g)| g)
            } else {
                None
            };
            before.unwrap_or(Gn {
                gender: Some('m'),
                number: Some('s'),
            })
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

    // « Les objectifs que nous nous étions fixé » : le pronom réfléchi est
    // indirect, « que » est le complément direct placé avant.
    let reflexive_pair = subject.pronoun
        && subject.chain.len() == 1
        && matches!(
            (lower(ctx, subject.start), lower(ctx, subject.chain[0])),
            ("je", "me" | "m’" | "m'")
                | ("tu", "te" | "t’" | "t'")
                | ("nous", "nous")
                | ("vous", "vous")
                | ("il" | "elle" | "on" | "ils" | "elles", "se" | "s’" | "s'")
        );
    // Seulement pour les verbes dont le pronom réfléchi est indirect : avec
    // « qu’elle s’était habituée », « se » est le complément direct et le
    // participe s'accorde avec le sujet.
    let indirect =
        ft.has(|r| r.is_past_participle() && ctx.data.is(&r.lemma, "pronominal_indirect"));
    if etre && reflexive_participle(ctx, &subject, t, ft, out) {
        return;
    }
    if etre && reflexive_pair && indirect && preceding_object(ctx, &subject, t, ft, out) {
        return;
    }
    if etre && (!has_object || reflexive_agrees(ctx, &subject, t, ft)) {
        // Attribut ou participe : accord avec le sujet (« elles sont parties »).
        // « elle est tout contente », « elle est tout le temps » : « tout »,
        // « fort », « bien »… peuvent être adverbes, donc invariables. Mais en fin
        // de proposition, un mot qui peut être adjectif l'est (« les délais sont
        // trop long. », « long » étant rarement adverbe).
        let clause_end = ft.has(|r| r.pos == Pos::Adj)
            && ctx
                .next(t)
                .is_none_or(|n| matches!(lower(ctx, n), "et" | "mais" | "ou" | "que" | "qu’"));
        if subject.gn == Gn::default()
            || (ft.has_pos(Pos::Adv) && !clause_end)
            || ft.has_pos(Pos::Det)
            || (collective_subject(ctx, &subject, i) && lower(ctx, subject.start + 1) != "plupart")
            || coordinated_subject(ctx, &subject)
            || subject.chain.iter().any(|&c| lower(ctx, c) == "en")
            || distributed_attribute(ctx, &subject, t, ft)
            || nuanced_colour(ctx, t, ft)
        {
            return;
        }
        let adj: Vec<&Reading> = ft.readings.iter().filter(|r| adjectival(r)).collect();
        let fits = adj
            .iter()
            .any(|r| reading_gn(r).is_some_and(|g| g.compatible(subject.gn)));
        // « elle est partit » : forme conjuguée à la place du participe.
        let finite_instead =
            adj.is_empty() && !ft.has_pos(Pos::Nom) && ft.has(Reading::is_finite_verb);
        if fits {
            coordinated_attribute(ctx, &subject, i, t, out);
            return;
        }
        if adj.is_empty() && !finite_instead {
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

    // « les informations que vous m’avez transmises » : un pronom complément
    // indirect ou une négation n'empêchent pas l'accord avec « que » ; « le »,
    // « la », « les », « en » changent le complément direct.
    let indirect_only = subject.chain.iter().all(|&c| {
        matches!(
            lower(ctx, c),
            "me" | "m’"
                | "m'"
                | "te"
                | "t’"
                | "t'"
                | "lui"
                | "nous"
                | "vous"
                | "leur"
                | "y"
                | "ne"
                | "n’"
                | "n'"
        )
    });
    if avoir && indirect_only && preceding_object(ctx, &subject, t, ft, out) {
        return;
    }
    if avoir && plural_object_pronoun(ctx, &subject, t, ft, out) {
        return;
    }

    // « Il a prit », « elle a apprit » : forme conjuguée à la place du participe.
    // (« n’a semble-t-il pas » : inversion, la partie composée n'est pas le verbe.)
    if avoir
        && !ctx.words[t].compound_part
        && !ft.has(Reading::is_past_participle)
        && ft.has(|r| {
            r.is_finite_verb() && matches!(r.mode_tense(), Some("ind.ps" | "ind.pres" | "sub.impf"))
        })
        && !ft.has(|r| {
            matches!(
                r.pos,
                Pos::Nom | Pos::Adj | Pos::Adv | Pos::Pre | Pos::Con | Pos::Pro | Pos::Det
            )
        })
    {
        let ms = Gn {
            gender: Some('m'),
            number: Some('s'),
        };
        let suggestions = participles(ctx, ft, Reading::is_finite_verb, ms);
        if let Some(first) = suggestions.first().cloned() {
            out.push(
                RawIssue::new(w.start, w.end, INFINITIVE)
                    .arg(w.text)
                    .arg(first)
                    .with_suggestions(suggestions),
            );
            return;
        }
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

// ---------------------------------------------------------------------------
// Verbes modaux, « tout », verbes pronominaux, sujets coordonnés

/// Verbe modal qui précède le mot `i` (pronoms compléments et négation sautés) :
/// « il faut », « tu peux m’… », « ils vont ».
fn modal_before(ctx: &Context, i: usize) -> Option<usize> {
    let mut p = ctx.prev(i);
    // Seulement des pronoms qui ne sont pas aussi des articles, et la négation.
    while let Some(k) = p.filter(|&k| {
        matches!(
            lower(ctx, k),
            "me" | "m’"
                | "te"
                | "t’"
                | "se"
                | "s’"
                | "nous"
                | "vous"
                | "lui"
                | "y"
                | "ne"
                | "n’"
                | "pas"
                | "jamais"
                | "déjà"
                | "toujours"
                | "encore"
                | "bien"
                | "mal"
        )
    }) {
        p = ctx.prev(k);
    }
    p.filter(|&k| {
        // Le modal est la tête de sa proposition : pas « que nous désirons »,
        // « qui peut » (le verbe suivant appartient alors à la principale).
        !(k.saturating_sub(4)..k)
            .any(|q| matches!(lower(ctx, q), "que" | "qu’" | "qu'" | "qui" | "dont" | "où"))
            && ctx.form(k).is_some_and(|f| {
                f.has(|r| {
                    r.is_finite_verb()
                        && matches!(
                            r.lemma.as_str(),
                            "falloir"
                                | "pouvoir"
                                | "devoir"
                                | "vouloir"
                                | "aller"
                                | "savoir"
                                | "oser"
                                | "espérer"
                                | "compter"
                                | "préférer"
                                | "désirer"
                                | "daigner"
                                | "aimer"
                                | "adorer"
                                | "détester"
                                | "souhaiter"
                        )
                }) && !f.has(|r| matches!(r.pos, Pos::Adv | Pos::Con))
                    // « compte », « sens » : noms ; seuls devoir, pouvoir, aller, falloir et
                    // vouloir gardent leur sens de modal malgré un homographe nom.
                    && (!f.has_pos(Pos::Nom)
                        || f.has(|r| {
                            r.is_finite_verb()
                                && matches!(
                                    r.lemma.as_str(),
                                    "devoir" | "pouvoir" | "aller" | "falloir" | "vouloir"
                                )
                        }))
            })
            && !ctx
                .prev(k)
                .is_some_and(|d| ctx.data.determiners.contains_key(lower(ctx, d)))
    })
}

/// « Je vais finis » → « finir » : forme conjuguée après un verbe modal.
fn modal_finite(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    // « pouvoir lui dire » : « lui » est un pronom, pas le verbe luire.
    if w.compound_part
        || w.ignored
        || w.has_digit
        || is_capitalized(w.text)
        || is_class(ctx, i, "clitique")
        || ctx.data.subjects.contains_key(w.lower.as_str())
    {
        return;
    }
    let Some(form) = ctx.form(i) else { return };
    if form.has(Reading::is_infinitive)
        || !form.has(Reading::is_finite_verb)
        || form.has(|r| {
            matches!(
                r.pos,
                Pos::Adv | Pos::Pre | Pos::Det | Pos::Pro | Pos::Con | Pos::Nom | Pos::NomPropre
            )
        })
    {
        return;
    }
    let Some(m) = modal_before(ctx, i) else {
        return;
    };
    // « une chose que je sais fausse » : après savoir ou vouloir, un adjectif est
    // un attribut ; après aller, pouvoir, devoir, falloir, c'est un verbe mal formé.
    if form.has_pos(Pos::Adj)
        && !ctx.form(m).is_some_and(|f| {
            f.has(|r| {
                r.is_finite_verb()
                    && matches!(r.lemma.as_str(), "aller" | "pouvoir" | "devoir" | "falloir")
            })
        })
    {
        return;
    }
    // Le verbe est bien à lui seul un verbe, pas une suite « modal, sujet verbe ».
    let Some(lemma) = form
        .readings
        .iter()
        .find(|r| r.is_finite_verb())
        .map(|r| r.lemma.clone())
    else {
        return;
    };
    if !ctx
        .lexicon
        .get(&lemma)
        .is_some_and(|f| f.has(Reading::is_infinitive))
    {
        return;
    }
    let fixed = same_case(w.text, &lemma);
    out.push(
        RawIssue::new(w.start, w.end, PARTICIPLE_FOR_INFINITIVE)
            .arg(w.text)
            .arg(fixed.clone())
            .arg(ctx.words[m].text)
            .suggest(fixed),
    );
}

/// « Toute les personnes », « Tous les femmes » : « tout » s'accorde avec le nom.
fn tout_agreement(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    if !matches!(w.lower.as_str(), "tout" | "toute" | "tous" | "toutes") || w.compound_part {
        return;
    }
    // « avant tout la partie », « après tout le monde » : locution adverbiale.
    if ctx
        .prev(i)
        .is_some_and(|p| matches!(lower(ctx, p), "avant" | "après" | "malgré" | "sur" | "en"))
    {
        return;
    }
    let Some(d) = ctx.next(i) else { return };
    // Seulement devant un déterminant défini ou démonstratif ou possessif.
    if !matches!(
        lower(ctx, d),
        "le" | "la"
            | "l’"
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
    ) {
        return;
    }
    let Some(np) = find_noun_phrase(ctx, d) else {
        return;
    };
    let Some(gn) = phrase_gn(ctx, &np) else {
        return;
    };
    let Some(number) = gn.number else { return };
    // « ils portent tous le nom », « de tous sa servante » : « tous » pluriel avec
    // un nom singulier est un pronom ou un quantificateur flottant.
    if number == 's' && matches!(w.lower.as_str(), "tous" | "toutes") {
        return;
    }
    let ok = matches!(
        (number, gn.gender, w.lower.as_str()),
        ('s', Some('f'), "toute")
            | ('s', Some('m'), "tout")
            | ('p', Some('f'), "toutes")
            | ('p', Some('m'), "tous")
            | ('s', None, "tout" | "toute")
            | ('p', None, "tous" | "toutes")
    );
    if ok {
        return;
    }
    let fixed = match (number, gn.gender) {
        ('s', Some('f')) => "toute",
        ('s', _) => "tout",
        ('p', Some('f')) => "toutes",
        _ => "tous",
    };
    let fixed = same_case(w.text, fixed);
    out.push(
        RawIssue::new(w.start, w.end, TOUT)
            .arg(w.text)
            .arg(fixed.clone())
            .suggest(fixed),
    );
}

/// Participes des verbes qui se conjuguent seulement avec être.
fn etre_only_participle(ft: &Form) -> bool {
    ft.has(|r| {
        r.is_past_participle()
            && matches!(
                r.lemma.as_str(),
                "aller"
                    | "venir"
                    | "revenir"
                    | "devenir"
                    | "parvenir"
                    | "partir"
                    | "arriver"
                    | "tomber"
                    | "rester"
                    | "naître"
                    | "mourir"
                    | "décéder"
                    | "advenir"
                    | "survenir"
            )
    })
}

/// « Elle s’est lavée les mains » → « lavé » ; « Elles se sont parlées » →
/// « parlé » : le participe reste invariable. Vrai si le cas a été reconnu.
fn reflexive_participle(
    ctx: &Context,
    subject: &FoundSubject,
    t: usize,
    ft: &Form,
    out: &mut Vec<RawIssue>,
) -> bool {
    let person = subject.person.as_str();
    let reflexive = subject
        .chain
        .iter()
        .any(|&c| match (lower(ctx, c), person) {
            ("se" | "s’", p) => p.starts_with('3'),
            ("me" | "m’", "1s") | ("te" | "t’", "2s") | ("nous", "1p") | ("vous", "2p") => true,
            _ => false,
        });
    if !reflexive {
        return false;
    }
    // « les lettres que nous nous sommes écrites » : « que » est le complément
    // direct placé avant, le participe s'accorde (voir `preceding_object`).
    if ctx
        .prev(subject.start)
        .is_some_and(|q| matches!(lower(ctx, q), "que" | "qu’" | "qu'"))
    {
        return false;
    }
    let invariable_verb =
        ft.has(|r| r.is_past_participle() && ctx.data.is(&r.lemma, "pronominal_invariable"));
    let cod_follows = ft
        .has(|r| r.is_past_participle() && ctx.data.is(&r.lemma, "pronominal_indirect"))
        && ctx.next(t).is_some_and(|n| {
            ctx.data.determiners.contains_key(lower(ctx, n))
                && !matches!(lower(ctx, n), "de" | "du" | "des" | "d’")
        });
    if !(invariable_verb || cod_follows) {
        return false;
    }
    let ms = Gn {
        gender: Some('m'),
        number: Some('s'),
    };
    let has_ms = ft.has(|r| r.is_past_participle() && r.gn().is_some_and(|g| g.compatible(ms)));
    if has_ms {
        return true;
    }
    let suggestions: Vec<String> = agree_forms(ctx, ft, Reading::is_past_participle, ms)
        .iter()
        .map(|s| s.to_string())
        .collect();
    let Some(first) = suggestions.first().cloned() else {
        return true;
    };
    let w = &ctx.words[t];
    out.push(
        RawIssue::new(w.start, w.end, PRONOMINAL_PARTICIPLE)
            .arg(w.text)
            .arg(first)
            .with_suggestions(suggestions),
    );
    true
}

/// Opérande d'une coordination : prénom (ou nom propre connu), « moi », « toi »,
/// ou groupe nominal ; renvoie le premier mot et le genre.
fn operand(ctx: &Context, last: usize) -> Option<(usize, Option<char>)> {
    let w = &ctx.words[last];
    if is_capitalized(w.text) && !w.compound_part {
        if let Some(f) = ctx.form(last).filter(|f| f.has_pos(Pos::NomPropre)) {
            let gender = f
                .readings
                .iter()
                .find(|r| r.pos == Pos::NomPropre)
                .and_then(|r| r.gn())
                .and_then(|g| g.gender);
            return Some((last, gender));
        }
    }
    if matches!(w.lower.as_str(), "moi" | "toi") {
        return Some((last, None));
    }
    // « Madame Leroy », « Docteur Martin », « Leroy » : nom propre inconnu du
    // lexique, éventuellement précédé d'un titre ou d'un prénom.
    if is_capitalized(w.text) && !w.sentence_start && !w.compound_part && ctx.form(last).is_none() {
        let title = |t: &str| {
            matches!(
                t,
                "madame"
                    | "monsieur"
                    | "mme"
                    | "mlle"
                    | "m"
                    | "docteur"
                    | "dr"
                    | "maître"
                    | "professeur"
                    | "pr"
            )
        };
        let mut start = last;
        let mut gender = None;
        while let Some(p) = ctx.prev(start).filter(|&p| {
            title(ctx.words[p].lower.as_str())
                || (is_capitalized(ctx.words[p].text)
                    && ctx.form(p).is_some_and(|f| f.has_pos(Pos::NomPropre)))
        }) {
            gender = gender.or(match ctx.words[p].lower.as_str() {
                "madame" | "mme" | "mlle" => Some('f'),
                "monsieur" | "m" | "docteur" | "dr" | "professeur" | "pr" => Some('m'),
                _ => ctx
                    .form(p)
                    .and_then(|f| f.readings.iter().find(|r| r.pos == Pos::NomPropre))
                    .and_then(|r| r.gn())
                    .and_then(|g| g.gender),
            });
            start = p;
        }
        return Some((start, gender));
    }
    (last.saturating_sub(4)..last)
        .rev()
        .filter_map(|d| find_noun_phrase(ctx, d))
        .find(|np| np.post.unwrap_or(np.noun) == last)
        .map(|np| (np.det, phrase_gn(ctx, &np).and_then(|g| g.gender)))
}

/// « Le chat et le chien dorme », « Marie et Paul sont », « Mon frère et moi
/// sommes » : sujet fait de deux éléments coordonnés par « et », devant le verbe `verb`.
fn and_subject(ctx: &Context, verb: usize) -> Option<FoundSubject> {
    if ctx.words[verb].compound_part {
        return None;
    }
    // Pré-filtre : un « et » dans les huit mots qui précèdent, sans changer de phrase.
    if !(verb.saturating_sub(8)..verb)
        .rev()
        .take_while(|&k| !ctx.words[k + 1].sentence_start)
        .any(|k| lower(ctx, k) == "et")
    {
        return None;
    }
    let mut k = verb;
    let mut chain = Vec::new();
    while let Some(p) = ctx.prev(k).filter(|&p| {
        matches!(
            lower(ctx, p),
            "se" | "s’"
                | "ne"
                | "n’"
                | "y"
                | "en"
                | "me"
                | "m’"
                | "te"
                | "t’"
                | "lui"
                | "leur"
                | "le"
                | "la"
                | "l’"
                | "les"
        )
    }) {
        chain.push(p);
        k = p;
    }
    chain.reverse();
    let last2 = ctx.prev(k)?;
    // « Il mange et ils boit » : un pronom sujet ouvre une proposition, il ne
    // s'ajoute pas au sujet précédent.
    if ctx.data.subjects.contains_key(lower(ctx, last2))
        && !matches!(lower(ctx, last2), "moi" | "toi")
    {
        return None;
    }
    let (s2, g2) = operand(ctx, last2)?;
    let et = ctx.prev(s2).filter(|&e| lower(ctx, e) == "et")?;
    let last1 = ctx.prev(et)?;
    let (s1, g1) = operand(ctx, last1)?;
    // Début de phrase ou après une conjonction ; pas après une virgule (« l’histoire,
    // les cultures et les religions »), ni après « ne … que » (« n’y ont exercé
    // qu’une souveraineté relative et Alexandre a »).
    let after_conjunction = ctx.prev(s1).is_some_and(|p| is_class(ctx, p, "charniere"));
    if !(ctx.words[s1].sentence_start || after_conjunction) {
        return None;
    }
    if ctx
        .prev(s1)
        .is_some_and(|p| matches!(lower(ctx, p), "qu’" | "qu'"))
        && (s1.saturating_sub(7)..s1).any(|k| matches!(lower(ctx, k), "ne" | "n’" | "n'"))
    {
        return None;
    }
    // « elle lui donnait sa voiture et son hôtel et ses bijoux » : énumération.
    if ctx
        .prev(s1)
        .is_some_and(|p| matches!(lower(ctx, p), "et" | "ou" | "ni"))
    {
        return None;
    }
    // « chaque homme et chaque femme est », « tout homme et toute femme » : singulier.
    if matches!(
        lower(ctx, s1),
        "chaque" | "tout" | "toute" | "aucun" | "aucune" | "nul" | "nulle" | "l’un" | "l'un"
    ) || (s1 > 0 && lower(ctx, s1 - 1) == "l’")
    {
        return None;
    }
    let person = match lower(ctx, last2) {
        "moi" => "1p",
        "toi" => "2p",
        _ => "3p",
    };
    let gender = match (g1, g2) {
        (Some('m'), _) | (_, Some('m')) => Some('m'),
        (Some('f'), Some('f')) => Some('f'),
        _ => None,
    };
    Some(FoundSubject {
        person: person.into(),
        gn: Gn {
            gender,
            number: Some('p'),
        },
        start: s1,
        chain,
        pronoun: false,
    })
}

/// Le verbe d'un sujet coordonné doit être au pluriel.
fn and_subject_verb(ctx: &Context, i: usize, sub: &FoundSubject, out: &mut Vec<RawIssue>) {
    let Some(form) = ctx.form(i) else { return };
    let person = sub.person.as_str();
    let finite: Vec<&Reading> = form
        .readings
        .iter()
        .filter(|r| r.person().is_some())
        .collect();
    if finite.is_empty()
        || finite.iter().any(|r| r.person() == Some(person))
        || form.has(|r| matches!(r.pos, Pos::Adj | Pos::Adv | Pos::Con | Pos::Pre))
    {
        return;
    }
    let mut suggestions: Vec<String> = Vec::new();
    for r in &finite {
        let wanted = format!("{}.{person}", r.mode_tense().unwrap_or(""));
        for f in inflected(ctx, &r.lemma, r.pos, &wanted) {
            let f = same_case(ctx.words[i].text, &f);
            if !suggestions.contains(&f) {
                suggestions.push(f);
            }
        }
    }
    suggestions.truncate(ctx.options.max_suggestions.max(1));
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    let w = &ctx.words[i];
    out.push(
        RawIssue::new(w.start, w.end, SUBJECT_VERB)
            .arg(w.text)
            .arg(first)
            .arg(subject_text(ctx, sub, i))
            .with_suggestions(suggestions),
    );
}

/// « Elle est très belle et intelligent » : le second adjectif de l'attribut
/// s'accorde aussi avec le sujet.
fn coordinated_attribute(
    ctx: &Context,
    subject: &FoundSubject,
    verb: usize,
    t: usize,
    out: &mut Vec<RawIssue>,
) {
    let Some(c) = ctx
        .next(t)
        .filter(|&c| matches!(lower(ctx, c), "et" | "ou"))
    else {
        return;
    };
    let mut k = ctx.next(c);
    while let Some(j) = k.filter(|&j| {
        matches!(
            lower(ctx, j),
            "très"
                | "trop"
                | "plus"
                | "moins"
                | "si"
                | "aussi"
                | "bien"
                | "tellement"
                | "assez"
                | "fort"
        )
    }) {
        k = ctx.next(j);
    }
    let Some(u) = k else { return };
    let Some(fu) = ctx.form(u) else { return };
    if !fu.has(adjectival)
        || fu.has(|r| matches!(r.pos, Pos::Adv | Pos::Det | Pos::Pre | Pos::Con | Pos::Pro))
        || follows_noun(ctx, u)
        || ctx.words[u].compound_part
        || is_capitalized(ctx.words[u].text)
    {
        return;
    }
    let fits = fu
        .readings
        .iter()
        .filter(|r| adjectival(r))
        .any(|r| adjective_gn(ctx, r).is_some_and(|g| g.compatible(subject.gn)));
    if fits {
        return;
    }
    let suggestions: Vec<String> = agree_forms(ctx, fu, adjectival, subject.gn)
        .iter()
        .map(|s| same_case(ctx.words[u].text, s))
        .collect();
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    let w = &ctx.words[u];
    out.push(
        RawIssue::new(w.start, w.end, PARTICIPLE_ETRE)
            .arg(w.text)
            .arg(first)
            .arg(subject_text(ctx, subject, verb))
            .with_suggestions(suggestions),
    );
}

/// « des livres intéressants et amusant » : le second adjectif coordonné
/// s'accorde avec le même nom.
fn coordinated_adjective(ctx: &Context, np: &NounPhrase, target: Gn, out: &mut Vec<RawIssue>) {
    let Some(post) = np.post else { return };
    // « les époques gauloise et républicaine » : adjectif singulier, répartition
    // sur deux ; « courant / tension » : adjectifs séparés par une barre.
    if !ctx.words[post].lower.ends_with(['s', 'x', 'z']) && target.number == Some('p') {
        return;
    }
    let first_ok = ctx.form(post).is_some_and(|f| {
        f.readings
            .iter()
            .filter(|r| adjectival(r))
            .any(|r| adjective_gn(ctx, r).is_some_and(|g| g.compatible(target)))
    });
    if !first_ok {
        return;
    }
    let Some(c) = ctx
        .next(post)
        .filter(|&c| matches!(lower(ctx, c), "et" | "ou"))
    else {
        return;
    };
    let Some(u) = ctx.next(c) else { return };
    let Some(fu) = ctx.form(u) else { return };
    // « dépouillé de ses insignes impériaux et emprisonné » : un adjectif et un
    // participe ne se coordonnent pas sur le même nom ; le participe se rapporte
    // à un sujet éloigné.
    let is_participle = |k: usize| {
        ctx.form(k)
            .is_some_and(|f| f.has(Reading::is_past_participle))
    };
    if is_participle(u) != is_participle(post) {
        return;
    }
    // « valeur intellectuelle et appropriées à la recherche » : un adjectif qui a
    // son propre complément peut se rapporter à un nom plus éloigné.
    if ctx.next(u).is_some_and(|n| {
        !matches!(lower(ctx, n), "et" | "ou" | "mais" | "car" | "donc")
            && !ctx.form(n).is_some_and(|f| {
                f.has(Reading::is_finite_verb)
                    && !f.has(|r| matches!(r.pos, Pos::Nom | Pos::Adj | Pos::Adv | Pos::Pre))
            })
    }) {
        return;
    }
    if !fu.has(adjectival)
        || fu.has(|r| matches!(r.pos, Pos::Adv | Pos::Det | Pos::Pre | Pos::Con | Pos::Pro))
        || follows_noun(ctx, u)
        || ctx.words[u].compound_part
        || is_capitalized(ctx.words[u].text)
    {
        return;
    }
    let fits = fu
        .readings
        .iter()
        .filter(|r| adjectival(r))
        .any(|r| adjective_gn(ctx, r).is_some_and(|g| g.compatible(target)));
    if fits {
        return;
    }
    let suggestions: Vec<String> = agree_forms(ctx, fu, adjectival, target)
        .iter()
        .map(|s| same_case(ctx.words[u].text, s))
        .collect();
    let Some(first) = suggestions.first().cloned() else {
        return;
    };
    let w = &ctx.words[u];
    out.push(
        RawIssue::new(w.start, w.end, ADJECTIVE)
            .arg(w.text)
            .arg(first)
            .arg(ctx.words[np.noun].text)
            .with_suggestions(suggestions),
    );
}
