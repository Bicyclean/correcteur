//! Style : répétitions rapprochées, phrases longues. Catégorie désactivée par
//! défaut (`Options::disabled_categories`), car il s'agit de conseils.

use std::collections::HashMap;

use super::{Context, RawIssue};
use crate::analysis::Word;
use crate::lexicon::{Pos, Reading};
use crate::text::{capitalize, is_capitalized};

pub const REPETITION: &str = "STYLE_REPETITION";
pub const LONG_SENTENCE: &str = "STYLE_PHRASE_LONGUE";

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    if ctx
        .options
        .disabled_categories
        .contains(&crate::Category::Style)
    {
        return; // évite le travail (synonymes) quand la catégorie est masquée
    }
    repetitions(ctx, out);
    long_sentences(ctx, out);
}

/// Le mot compte-t-il comme un mot à part entière ? Les éléments d'un
/// composé (« peut-être ») et les élisions (« l’ ») ne comptent pas.
pub fn counts_as_word(ctx: &Context, i: usize) -> bool {
    let w = &ctx.words[i];
    if ctx.data.elisions.contains(&w.lower) {
        return false;
    }
    !(w.compound_part && i > 0 && ctx.text[ctx.words[i - 1].end..w.start] == *"-")
}

/// Lecture d'un mot plein (nom, adjectif, verbe autre qu'auxiliaire).
fn content_reading<'a>(ctx: &'a Context, i: usize) -> Option<&'a Reading> {
    let w: &Word = &ctx.words[i];
    if w.ignored || w.has_digit || (is_capitalized(w.text) && !w.sentence_start) {
        return None;
    }
    let form = ctx.form(i)?;
    // Mot grammatical ou adverbe possible (« pas », « bien », « tout ») : on s'abstient.
    if form.has(|r| {
        matches!(
            r.pos,
            Pos::Det | Pos::Pro | Pos::Pre | Pos::Con | Pos::Aux | Pos::Adv
        )
    }) {
        return None;
    }
    form.readings
        .iter()
        .filter(|r| matches!(r.pos, Pos::Nom | Pos::Adj | Pos::Ver))
        .find(|r| r.lemma.chars().count() >= 4 && !ctx.data.is(&r.lemma, "style_ignore"))
}

fn repetitions(ctx: &Context, out: &mut Vec<RawIssue>) {
    let mut last: HashMap<&str, usize> = HashMap::new();
    let mut position = 0;
    for i in 0..ctx.words.len() {
        if !counts_as_word(ctx, i) {
            continue;
        }
        position += 1;
        let Some(r) = content_reading(ctx, i) else {
            continue;
        };
        if let Some(&before) = last.get(r.lemma.as_str()) {
            let distance = position - before;
            if distance <= ctx.options.repetition_window {
                let w = &ctx.words[i];
                let issue = RawIssue::new(w.start, w.end, REPETITION)
                    .arg(w.text)
                    .arg((distance - 1).to_string())
                    .with_suggestions(synonyms(ctx, r, w.text));
                out.push(issue);
            }
        }
        last.insert(r.lemma.as_str(), position);
    }
}

/// Synonymes du lemme, accordés comme le mot remplacé quand le lexique le permet.
fn synonyms(ctx: &Context, reading: &Reading, like: &str) -> Vec<String> {
    let Some(groups) = ctx.data.thesaurus.get(&reading.lemma) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for word in groups.iter().flat_map(|g| g.words.iter()) {
        if word.contains(' ') || *word == reading.lemma {
            continue;
        }
        // Même catégorie et mêmes traits (« maisons » → « demeures »).
        let inflected = ctx
            .lexicon
            .inflect(word, |r| r.pos == reading.pos && r.traits == reading.traits)
            .first()
            .map(std::string::ToString::to_string);
        let usable = inflected.or_else(|| {
            ctx.lexicon
                .get(word)
                .is_some_and(|f| f.has(|r| r.pos == reading.pos && r.traits == reading.traits))
                .then(|| word.clone())
        });
        if let Some(s) = usable {
            let s = if is_capitalized(like) {
                capitalize(&s)
            } else {
                s
            };
            if !out.contains(&s) {
                out.push(s);
            }
        }
        if out.len() >= ctx.options.max_suggestions {
            break;
        }
    }
    out
}

fn long_sentences(ctx: &Context, out: &mut Vec<RawIssue>) {
    let mut start: Option<usize> = None;
    let mut count = 0;
    let flush = |start: Option<usize>, end: usize, count: usize, out: &mut Vec<RawIssue>| {
        if let Some(s) = start {
            if count > ctx.options.long_sentence_words {
                out.push(
                    RawIssue::new(ctx.words[s].start, ctx.words[end].end, LONG_SENTENCE)
                        .arg(count.to_string())
                        .arg(ctx.options.long_sentence_words.to_string()),
                );
            }
        }
    };
    for i in 0..ctx.words.len() {
        if ctx.words[i].sentence_start && i > 0 {
            flush(start, i - 1, count, out);
            start = None;
            count = 0;
        }
        if start.is_none() {
            start = Some(i);
        }
        if counts_as_word(ctx, i) {
            count += 1;
        }
    }
    if !ctx.words.is_empty() {
        flush(start, ctx.words.len() - 1, count, out);
    }
}

/// Mesures de lisibilité d'un texte.
#[derive(Debug, Clone, PartialEq)]
pub struct Readability {
    pub words: usize,
    pub sentences: usize,
    pub syllables: usize,
    /// Indice de Kandel et Moles (adaptation au français de la formule de
    /// Flesch) : 207 − 1,015 × mots par phrase − 73,6 × syllabes par mot.
    /// Plus il est élevé, plus le texte est facile ; borné à 0..100, il n'est qu'indicatif.
    pub score: f64,
    pub level: &'static str,
}

/// Nombre de syllabes écrites d'un mot, par une règle simple : groupes de
/// voyelles, sans le « e » muet final (« ta-ble », « por-tes »).
pub fn syllables(word: &str) -> usize {
    let lower = word.to_lowercase();
    let vowel = |c: char| "aeiouyàâäéèêëîïôöùûüœæ".contains(c);
    let chars: Vec<char> = lower.chars().collect();
    let mut groups = 0;
    let mut previous = false;
    for &c in &chars {
        let v = vowel(c);
        if v && !previous {
            groups += 1;
        }
        previous = v;
    }
    let mute_end = lower.ends_with('e') || lower.ends_with("es");
    if groups > 1 && mute_end && !lower.ends_with("ée") && !lower.ends_with("ées") {
        groups -= 1;
    }
    groups.max(1)
}

pub fn readability(ctx: &Context) -> Readability {
    let mut words = 0;
    let mut sentences = 0;
    let mut syl = 0;
    for (i, w) in ctx.words.iter().enumerate() {
        if w.sentence_start {
            sentences += 1;
        }
        if counts_as_word(ctx, i) && !w.has_digit {
            words += 1;
            syl += syllables(w.text);
        }
    }
    let sentences = sentences.max(usize::from(words > 0));
    let score = if words == 0 {
        0.0
    } else {
        // La formule sort de l'échelle pour des textes très simples ou très
        // complexes : on la borne à 0..100 pour l'affichage.
        (207.0 - 1.015 * (words as f64 / sentences as f64) - 73.6 * (syl as f64 / words as f64))
            .clamp(0.0, 100.0)
    };
    let level = match score {
        s if s >= 90.0 => "très facile",
        s if s >= 80.0 => "facile",
        s if s >= 70.0 => "plutôt facile",
        s if s >= 60.0 => "standard",
        s if s >= 50.0 => "plutôt difficile",
        s if s >= 30.0 => "difficile",
        _ => "très difficile",
    };
    Readability {
        words,
        sentences,
        syllables: syl,
        score,
        level,
    }
}

#[cfg(test)]
mod tests {
    use super::syllables;

    #[test]
    fn syllabes() {
        assert_eq!(syllables("table"), 1);
        assert_eq!(syllables("portes"), 1);
        assert_eq!(syllables("chocolat"), 3);
        assert_eq!(syllables("beauté"), 2);
        assert_eq!(syllables("année"), 2);
        assert_eq!(syllables("a"), 1);
    }
}
