//! Découpage du texte en mots analysés : séparation des élisions (`l’`, `qu’`…)
//! et des composés absents du lexique (`dit-il`), repérage des débuts de phrase.

use std::collections::HashSet;

use crate::data::Data;
use crate::lexicon::Lexicon;
use crate::text::{in_spans, is_apostrophe, normalize_apostrophes, raw_words};

#[derive(Debug, Clone)]
pub struct Word<'a> {
    pub start: usize,
    pub end: usize,
    /// Texte tel qu'il apparaît.
    pub text: &'a str,
    /// Texte avec apostrophes typographiques.
    pub norm: String,
    /// `norm` en minuscules.
    pub lower: String,
    /// Élément d'un composé à trait d'union absent du lexique (`dit` dans `dit-il`).
    pub compound_part: bool,
    pub has_digit: bool,
    /// Dans une adresse web ou un courriel.
    pub ignored: bool,
    /// Premier mot d'une phrase.
    pub sentence_start: bool,
    /// Seuls des blancs (ou rien, après une élision) le séparent du mot précédent.
    pub joined_to_prev: bool,
}

pub fn analyze<'a>(
    text: &'a str,
    lexicon: &Lexicon,
    data: &Data,
    user_words: &HashSet<String>,
    ignored: &[(usize, usize)],
) -> Vec<Word<'a>> {
    let mut pieces: Vec<(usize, usize, bool)> = Vec::new();
    for raw in raw_words(text) {
        split(
            text,
            raw.start,
            raw.end,
            lexicon,
            data,
            user_words,
            &mut pieces,
        );
    }

    let mut words: Vec<Word<'a>> = Vec::with_capacity(pieces.len());
    for (i, &(start, end, compound_part)) in pieces.iter().enumerate() {
        let slice = &text[start..end];
        let norm = normalize_apostrophes(slice);
        let lower = norm.to_lowercase();
        let gap = if i == 0 {
            ""
        } else {
            &text[pieces[i - 1].1..start]
        };
        let joined_to_prev = i > 0 && gap.chars().all(char::is_whitespace);
        let sentence_start = !compound_part && is_sentence_start(text, start, words.last(), data);
        words.push(Word {
            start,
            end,
            text: slice,
            norm,
            lower,
            compound_part,
            has_digit: slice.chars().any(|c| c.is_ascii_digit()),
            ignored: in_spans(ignored, start),
            sentence_start,
            joined_to_prev,
        });
    }
    words
}

fn known(word: &str, lexicon: &Lexicon, user_words: &HashSet<String>) -> bool {
    user_words.contains(word) || lexicon.find(word).is_some()
}

fn split(
    text: &str,
    start: usize,
    end: usize,
    lexicon: &Lexicon,
    data: &Data,
    user_words: &HashSet<String>,
    out: &mut Vec<(usize, usize, bool)>,
) {
    let slice = &text[start..end];
    let norm = normalize_apostrophes(slice);
    if slice.chars().any(|c| c.is_ascii_digit()) || known(&norm, lexicon, user_words) {
        out.push((start, end, false));
        return;
    }
    // Élision : « l’arbre » → « l’ » + « arbre ».
    if let Some((pos, c)) = slice.char_indices().find(|&(_, c)| is_apostrophe(c)) {
        let cut = pos + c.len_utf8();
        let prefix = normalize_apostrophes(&slice[..cut]).to_lowercase();
        if data.elisions.contains(&prefix) {
            out.push((start, start + cut, false));
            split(text, start + cut, end, lexicon, data, user_words, out);
            return;
        }
    }
    // Composé absent du lexique : « dit-il » → « dit » + « il ».
    if slice.contains('-') {
        let mut offset = start;
        for part in slice.split('-') {
            if !part.is_empty() {
                out.push((offset, offset + part.len(), true));
            }
            offset += part.len() + 1;
        }
        return;
    }
    out.push((start, end, false));
}

/// Un mot commence une phrase s'il n'est précédé (hors blancs, guillemets,
/// tirets et parenthèses ouvrantes) que du début du texte, d'un saut de ligne
/// ou d'une ponctuation finale.
fn is_sentence_start(text: &str, start: usize, prev: Option<&Word>, data: &Data) -> bool {
    for c in text[..start].chars().rev() {
        match c {
            '\n' | '\r' => return true,
            '!' | '?' | '…' => return true,
            '.' => {
                // « M. Dupont », « p. ex. » : le point d'une abréviation ne termine pas la phrase.
                return match prev {
                    Some(w) => !data.is_abbreviation(&w.norm),
                    None => true,
                };
            }
            c if c.is_whitespace() => continue,
            '«' | '"' | '“' | '(' | '[' | '—' | '–' | '-' => continue,
            _ => return false,
        }
    }
    true
}
