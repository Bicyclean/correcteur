//! Suggestions pour les mots inconnus.
//!
//! Candidats : formes de même squelette (erreurs d'accent ou de lettre doublée),
//! formes à une modification près du mot (insertion, suppression, substitution,
//! inversion), formes à une modification près du squelette, et coupure en deux
//! mots. Les candidats sont classés par une distance d'édition pondérée,
//! corrigée par la fréquence.

use std::collections::HashSet;

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

    let mut seen = HashSet::new();
    let mut scored: Vec<(f32, String)> = Vec::new();
    for id in ids {
        if !seen.insert(id) {
            continue;
        }
        let form = lexicon.form(id);
        let dist = weighted_distance(&lower, &form.text.to_lowercase());
        if dist > MAX_DISTANCE {
            continue;
        }
        let score = dist - FREQ_WEIGHT * (1.0 + form.freq).ln();
        scored.push((score, form.text.clone()));
    }

    // Deux mots collés : « ilya » → « il y a » n'est pas couvert, « deuxmots » l'est.
    let chars: Vec<(usize, char)> = lower.char_indices().collect();
    for &(i, _) in chars.iter().skip(1) {
        let (a, b) = lower.split_at(i);
        if elisions.contains(&format!("{a}’")) {
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
        }
        if elisions.contains(&format!("{a}’")) {
            continue; // « qu il » n'est jamais la bonne coupure
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

/// Toutes les chaînes à une modification de `word`.
fn edits1(word: &str, alphabet: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let letters: Vec<char> = alphabet.chars().collect();
    let mut out = Vec::with_capacity(n * (2 * letters.len() + 2) + letters.len());
    let build = |parts: &[&[char]]| -> String { parts.iter().flat_map(|p| p.iter()).collect() };
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

/// Distance d'édition (avec inversion de lettres voisines) dont les coûts
/// reflètent les fautes fréquentes : accent oublié ou erroné, lettre doublée
/// ou dédoublée.
pub fn weighted_distance(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0f32; m + 1]; n + 1];

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
    fn casse() {
        assert_eq!(restore_case("Ecole", "école"), "École");
        assert_eq!(restore_case("ECOLE", "école"), "ÉCOLE");
        assert_eq!(restore_case("pariss", "Paris"), "Paris");
    }
}
