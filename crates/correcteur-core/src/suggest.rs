//! Suggestions pour les mots inconnus.
//!
//! Candidats : formes de même squelette (erreurs d'accent ou de lettre doublée),
//! formes à une modification près du mot (insertion, suppression, substitution,
//! inversion), formes à une modification près du squelette, formes obtenues
//! par une confusion phonétique courante (« fesait » → « faisait »), et coupure
//! en deux mots. Les candidats sont classés par une distance d'édition
//! pondérée, corrigée par la fréquence.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::lexicon::{skeleton, Lexicon};
use crate::text::{base_letter, capitalize, is_all_upper, is_capitalized};

const LETTERS: &str = "abcdefghijklmnopqrstuvwxyzàâäçéèêëîïôöùûüÿœæ-’";
const SKELETON_LETTERS: &str = "abcdefghijklmnopqrstuvwxyz-’";

/// Distance au-delà de laquelle un candidat n'est pas proposé.
const MAX_DISTANCE: f32 = 2.0;
/// Poids de la fréquence dans le classement.
const FREQ_WEIGHT: f32 = 0.08;
/// Pénalité d'une coupure en deux mots.
const SPLIT_PENALTY: f32 = 0.6;
/// Coût d'une apostrophe d'élision oubliée (« quil » → « qu’il »).
const MISSING_APOSTROPHE: f32 = 0.5;
/// Coût d'un nom propre proposé pour un mot écrit en minuscules : « parmis »
/// doit donner « parmi » avant « Paris », plus fréquent.
const CASE_CHANGE: f32 = 0.75;

/// Graphies qui se prononcent de la même façon, ou presque, et que l'on
/// confond souvent : remplacer l'une par l'autre coûte moins qu'une faute de
/// frappe. Chaque paire vaut dans les deux sens.
const CONFUSIONS: &[(&str, &str, f32)] = &[
    ("é", "ée", 0.25), // musé → musée
    ("e", "ai", 0.5),  // fesait → faisait
    ("è", "ai", 0.5),  // mèson → maison
    ("s", "ss", 0.5),  // pousser, poser
    ("c", "ç", 0.25),  // garcon → garçon
    ("ph", "f", 0.5),  // filosofie → philosophie
    ("an", "en", 0.5), // anfant → enfant
    ("am", "em", 0.5), // amporter → emporter
];

/// `elisions` : formes élidées connues (`qu’`, `l’`…), pour proposer une apostrophe oubliée.
pub fn suggest(
    lexicon: &Lexicon,
    elisions: &HashSet<String>,
    word: &str,
    max: usize,
) -> Vec<String> {
    if max == 0 {
        return Vec::new();
    }
    let lower = word.to_lowercase();
    let mut ids: Vec<u32> = Vec::new();

    ids.extend_from_slice(lexicon.ids_for_skeleton(&skeleton(&lower)));
    for e in edits1(&lower, LETTERS) {
        lexicon.ids_for_lower(&e, &mut ids);
    }
    for e in edits1(&skeleton(&lower), SKELETON_LETTERS) {
        ids.extend_from_slice(lexicon.ids_for_skeleton(&e));
    }
    for v in confusion_variants(&lower) {
        lexicon.ids_for_lower(&v, &mut ids);
        ids.extend_from_slice(lexicon.ids_for_skeleton(&skeleton(&v)));
    }

    let mut seen = HashSet::new();
    let mut scored: Vec<(f32, String)> = Vec::new();
    for id in ids {
        if !seen.insert(id) {
            continue;
        }
        let form = lexicon.form(id);
        let mut dist = weighted_distance(&lower, &form.text.to_lowercase());
        if dist > MAX_DISTANCE {
            continue;
        }
        if is_capitalized(&form.text) && !is_capitalized(word) {
            dist += CASE_CHANGE;
        }
        let score = dist - FREQ_WEIGHT * (1.0 + form.freq).ln();
        scored.push((score, form.text.clone()));
    }

    // Deux mots collés : « ilya » → « il y a » n'est pas couvert, « deuxmots » l'est.
    let chars: Vec<(usize, char)> = lower.char_indices().collect();
    for &(i, _) in chars.iter().skip(1) {
        let (a, b) = lower.split_at(i);
        if elisions.contains(&format!("{a}’")) {
            // « qu il » n'est jamais la bonne coupure : seule l'apostrophe oubliée compte.
            // L'élision ne se fait que devant une voyelle ou un h muet ; devant h,
            // le coût est plus élevé (« chein » ne doit pas donner « c’hein »).
            let cost = match b.chars().next() {
                Some(c) if "aâàeéèêëiîïoôuûùyœ".contains(c) => Some(MISSING_APOSTROPHE),
                Some('h') => Some(2.0 * MISSING_APOSTROPHE),
                _ => None,
            };
            if let (Some(cost), Some(fb)) = (cost, lexicon.get(b)) {
                let score = cost - FREQ_WEIGHT * (1.0 + fb.freq).ln();
                scored.push((score, format!("{a}’{b}")));
            }
            continue;
        }
        if let (Some(fa), Some(fb)) = (lexicon.get(a), lexicon.get(b)) {
            let short_ok = |s: &str| s.chars().count() > 1 || matches!(s, "a" | "à" | "y" | "ô");
            if short_ok(a) && short_ok(b) {
                let freq = fa.freq.min(fb.freq);
                let score = SPLIT_PENALTY + 1.0 - FREQ_WEIGHT * (1.0 + freq).ln();
                scored.push((score, format!("{a} {b}")));
            }
        }
    }

    // Pluriel régulier appliqué à un nom en -al, -ail, -au, -eu ou -ou :
    // « chevals » → « chevaux », « travails » → « travaux », « bateaus » →
    // « bateaux », « genous » → « genoux ». Proposé en premier.
    for (wrong, right) in [
        ("als", "aux"),
        ("ails", "aux"),
        ("aus", "aux"),
        ("eus", "eux"),
        ("ous", "oux"),
    ] {
        if let Some(stem) = lower.strip_suffix(wrong) {
            let plural = format!("{stem}{right}");
            if lexicon.get(&plural).is_some() {
                scored.push((f32::MIN, plural));
            }
        }
    }

    scored.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut out: Vec<String> = Vec::new();
    for (_, s) in scored {
        let s = restore_case(word, &s);
        if s != word && !out.contains(&s) {
            out.push(s);
        }
        if out.len() == max {
            break;
        }
    }
    out
}

/// Reporte la casse du mot d'origine sur la suggestion.
fn restore_case(original: &str, suggestion: &str) -> String {
    if is_capitalized(suggestion) {
        return suggestion.to_string();
    }
    if original.chars().count() > 1 && is_all_upper(original) {
        suggestion.to_uppercase()
    } else if is_capitalized(original) {
        capitalize(suggestion)
    } else {
        suggestion.to_string()
    }
}

/// Variantes de `word` où une ou deux graphies de [`CONFUSIONS`] sont
/// remplacées par l'autre (« filosofie » → « philosophie »).
fn confusion_variants(word: &str) -> Vec<String> {
    let once = |w: &str| {
        let mut out = Vec::new();
        for &(x, y, _) in CONFUSIONS {
            for (from, to) in [(x, y), (y, x)] {
                for (pos, _) in w.match_indices(from) {
                    out.push(format!("{}{to}{}", &w[..pos], &w[pos + from.len()..]));
                }
            }
        }
        out
    };
    let first = once(word);
    let mut out: Vec<String> = first.iter().flat_map(|v| once(v)).collect();
    out.extend(first);
    out.sort();
    out.dedup();
    out
}

/// Toutes les chaînes à une modification de `word`.
fn edits1(word: &str, alphabet: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let letters: Vec<char> = alphabet.chars().collect();
    let mut out = Vec::with_capacity(n * (2 * letters.len() + 2) + letters.len());
    // Capacité réservée d'emblée : ces chaînes sont créées par milliers par mot.
    let cap = word.len() + 4;
    let build = |parts: &[&[char]]| -> String {
        let mut s = String::with_capacity(cap);
        s.extend(parts.iter().flat_map(|p| p.iter()));
        s
    };
    for i in 0..=n {
        let (left, right) = chars.split_at(i);
        if i < n {
            out.push(build(&[left, &right[1..]]));
        }
        if i + 1 < n {
            out.push(build(&[left, &[right[1], right[0]], &right[2..]]));
        }
        for &l in &letters {
            if i < n && right[0] != l {
                out.push(build(&[left, &[l], &right[1..]]));
            }
            out.push(build(&[left, &[l], right]));
        }
    }
    out
}

/// [`CONFUSIONS`] dans les deux sens, en caractères, construites une seule fois
/// (la distance est calculée pour chaque candidat de chaque mot inconnu).
type ConfusionPair = (Vec<char>, Vec<char>, f32);

fn confusion_pairs() -> &'static [ConfusionPair] {
    static PAIRS: OnceLock<Vec<ConfusionPair>> = OnceLock::new();
    PAIRS.get_or_init(|| {
        CONFUSIONS
            .iter()
            .flat_map(|&(x, y, c)| {
                let (x, y): (Vec<char>, Vec<char>) = (x.chars().collect(), y.chars().collect());
                [(x.clone(), y.clone(), c), (y, x, c)]
            })
            .collect()
    })
}

/// Distance d'édition (avec inversion de lettres voisines) dont les coûts
/// reflètent les fautes fréquentes : accent oublié ou erroné, lettre doublée
/// ou dédoublée, confusion phonétique (voir [`CONFUSIONS`]).
pub fn weighted_distance(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0f32; m + 1]; n + 1];
    let confusions = confusion_pairs();

    // Coût d'insertion ou de suppression de `s[i]` : moindre si la lettre double sa voisine.
    let indel = |s: &[char], i: usize| -> f32 {
        let c = s[i];
        let doubled = (i > 0 && s[i - 1] == c) || (i + 1 < s.len() && s[i + 1] == c);
        if doubled {
            0.5
        } else {
            1.0
        }
    };
    let sub = |x: char, y: char| -> f32 {
        if x == y {
            0.0
        } else if base_letter(x) == base_letter(y) {
            0.25
        } else {
            1.0
        }
    };

    for i in 1..=n {
        d[i][0] = d[i - 1][0] + indel(&a, i - 1);
    }
    for j in 1..=m {
        d[0][j] = d[0][j - 1] + indel(&b, j - 1);
    }
    for i in 1..=n {
        for j in 1..=m {
            let mut v = (d[i - 1][j] + indel(&a, i - 1))
                .min(d[i][j - 1] + indel(&b, j - 1))
                .min(d[i - 1][j - 1] + sub(a[i - 1], b[j - 1]));
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[i - 2][j - 2] + 0.75);
            }
            for (x, y, c) in confusions {
                if a[..i].ends_with(x) && b[..j].ends_with(y) {
                    v = v.min(d[i - x.len()][j - y.len()] + c);
                }
            }
            d[i][j] = v;
        }
    }
    d[n][m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        assert_eq!(weighted_distance("ecole", "école"), 0.25);
        assert_eq!(weighted_distance("apeller", "appeler"), 1.0);
        assert_eq!(weighted_distance("maison", "maison"), 0.0);
        assert_eq!(weighted_distance("maiosn", "maison"), 0.75);
        assert!(weighted_distance("chat", "chien") > 2.0);
    }

    #[test]
    fn confusions_phonetiques() {
        assert_eq!(weighted_distance("musé", "musée"), 0.25);
        assert_eq!(weighted_distance("fesait", "faisait"), 0.5);
        assert_eq!(weighted_distance("filosofie", "philosophie"), 1.0);
        assert_eq!(weighted_distance("anfant", "enfant"), 0.5);
    }

    #[test]
    fn classement_phonetique() {
        let tsv = "muse\tmuse\tNOM\tf.s\t1.31\ttest\n\
                   musée\tmusée\tNOM\tm.s\t20.07\ttest\n\
                   faisait\tfaire\tVER\tind.impf.3s\t329.81\ttest\n\
                   fessait\tfesser\tVER\tind.impf.3s\t0.07\ttest\n\
                   pesait\tpeser\tVER\tind.impf.3s\t5.0\ttest\n";
        let lex = Lexicon::from_tsv_files(&[("t".into(), tsv.into())]).unwrap();
        let none = HashSet::new();
        assert_eq!(suggest(&lex, &none, "musé", 5)[0], "musée");
        assert_eq!(suggest(&lex, &none, "fesait", 5)[0], "faisait");
    }

    #[test]
    fn casse() {
        assert_eq!(restore_case("Ecole", "école"), "École");
        assert_eq!(restore_case("ECOLE", "école"), "ÉCOLE");
        assert_eq!(restore_case("pariss", "Paris"), "Paris");
    }
}
