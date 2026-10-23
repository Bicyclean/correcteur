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
        auxiliary(ctx, i, out);
        negation(ctx, i, out);
        si_conditional(ctx, i, out);
        participle_for_infinitive(ctx, i, out);
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
        if pronoun
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
    // « la SA écrit » : sigle, pas déterminant.
    if w.ignored
        || w.compound_part
        || (w.text.chars().filter(|c| c.is_alphabetic()).count() > 1 && is_all_upper(w.text))
    {
        return None;
    }
    let determiner = ctx.data.determiners.get(&w.lower)?;
    let mut j = ctx.next(det)?;
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
        let next_is_plain_noun = fnext.has_pos(Pos::Nom)
            && !fnext.has(|r| r.pos == Pos::Adj || r.is_finite_verb())
            && !elided(ctx, n)
            && !is_class(ctx, n, "clitique");
        // « les neuf heures » : nombre, pas adjectif.
        // « le Pure race espagnole » : majuscule, nom propre ou titre.
        let capital = is_capitalized(ctx.words[j].text) && !ctx.words[j].sentence_start;
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
        !ctx.form(p).is_some_and(|f| {
            f.has_pos(Pos::Pre)
                || (f.has(|r| r.is_finite_verb() || r.is_infinitive()) && !f.has_pos(Pos::Nom))
        })
    });
    if object_pronoun
        && noun_form
            .has(|r| r.pos.is_verb() && (r.is_infinitive() || (before_verb && r.is_finite_verb())))
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
                if ok || f.has(|r| r.pos.is_verb() && r.traits == "ppre") {
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
                if Some(adj) != np.post
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
        .is_some_and(|p| matches!(lower(ctx, p), "et" | "ou" | "ni"));
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
fn participle_for_infinitive(ctx: &Context, i: usize, out: &mut Vec<RawIssue>) {
    let w = &ctx.words[i];
    // « d’Honoré de Balzac », « Sablé-sur-Sarthe » : nom propre.
    if is_capitalized(w.text) {
        return;
    }
    let Some(stem) = w.lower.strip_suffix('é') else {
        return;
    };
    let Some(form) = ctx.form(i) else { return };
    // « le passé », « un fermé » : nom possible, on s'abstient.
    if !form.has(|r| r.is_past_participle() && r.lemma.ends_with("er")) || form.has_pos(Pos::Nom) {
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
    let Some(p) = ctx.prev(i) else { return };
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
                            | "savoir"
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
    if !introduces || !complement {
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
    // « les charges relevées contre lui » : préposition, pas le verbe contrer.
    if form.has_pos(Pos::Pre) {
        return;
    }
    // « … et les clients semble satisfaits » : un sujet pluriel, même coordonné,
    // demande un verbe au pluriel.
    let Some(subject) = simple_subject(ctx, i) else {
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
            .any(|c| matches!(*c, "en" | "le" | "la" | "l’" | "les"));
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
        if collective || (coordinated && !plural_subject) {
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
    let (mut etre, mut avoir) = (aux("être"), aux("avoir"));
    if !etre && !avoir {
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
    let Some(subject) = subject_of(ctx, i) else {
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
                .is_none_or(|n| matches!(lower(ctx, n), "et" | "mais" | "ou"));
        if subject.gn == Gn::default()
            || (ft.has_pos(Pos::Adv) && !clause_end)
            || ft.has_pos(Pos::Det)
            || collective_subject(ctx, &subject, i)
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
