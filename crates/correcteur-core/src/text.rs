//! Outils de bas niveau sur le texte : découpage en mots bruts, zones à ignorer,
//! conversion de positions.

pub const NBSP: char = '\u{00A0}';
pub const NNBSP: char = '\u{202F}';
pub const APOSTROPHE: char = '\u{2019}';

/// Remplace l'apostrophe droite par l'apostrophe typographique.
pub fn normalize_apostrophes(s: &str) -> String {
    s.replace('\'', "\u{2019}")
}

pub fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == APOSTROPHE
}

/// Espace insécable (normale ou fine).
pub fn is_nbsp(c: char) -> bool {
    c == NBSP || c == NNBSP
}

/// Blanc horizontal (pas de saut de ligne).
pub fn is_hspace(c: char) -> bool {
    c == ' ' || c == '\t' || is_nbsp(c) || c == '\u{2009}'
}

/// Lettre de base, sans accent ni cédille. Les ligatures sont laissées telles quelles.
pub fn base_letter(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' | 'í' => 'i',
        'ô' | 'ö' | 'ó' => 'o',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ÿ' => 'y',
        'ç' => 'c',
        'À' | 'Â' | 'Ä' | 'Á' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'Î' | 'Ï' | 'Í' => 'I',
        'Ô' | 'Ö' | 'Ó' => 'O',
        'Ù' | 'Û' | 'Ü' | 'Ú' => 'U',
        'Ÿ' => 'Y',
        'Ç' => 'C',
        _ => c,
    }
}

/// Met la première lettre en majuscule.
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn is_capitalized(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}

pub fn is_all_upper(s: &str) -> bool {
    let mut letters = s.chars().filter(|c| c.is_alphabetic()).peekable();
    letters.peek().is_some() && letters.all(char::is_uppercase)
}

/// Mot brut : suite de lettres et de chiffres, avec apostrophes et traits
/// d'union internes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawWord {
    pub start: usize,
    pub end: usize,
}

pub fn raw_words(text: &str) -> Vec<RawWord> {
    let mut words = Vec::new();
    let mut iter = text.char_indices().peekable();
    while let Some((start, c)) = iter.next() {
        if !c.is_alphanumeric() {
            continue;
        }
        let mut end = start + c.len_utf8();
        while let Some(&(i, c)) = iter.peek() {
            if c.is_alphanumeric() {
                iter.next();
                end = i + c.len_utf8();
            } else if is_apostrophe(c) || c == '-' {
                // Ne garder le séparateur que s'il est suivi d'une lettre ou d'un chiffre.
                let next = text[i + c.len_utf8()..].chars().next();
                if next.is_some_and(char::is_alphanumeric) {
                    iter.next();
                    end = i + c.len_utf8();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        words.push(RawWord { start, end });
    }
    words
}

/// Zones à ne pas analyser : adresses web et courriels.
pub fn ignored_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut chunk_start = None;
    let mut push = |s: usize, e: usize| {
        let chunk = &text[s..e];
        let looks_like_email = chunk
            .find('@')
            .is_some_and(|at| at > 0 && chunk[at..].contains('.'));
        if chunk.contains("://") || chunk.starts_with("www.") || looks_like_email {
            // Ne pas inclure la ponctuation finale (« voir www.exemple.org. »).
            let trimmed = chunk.trim_end_matches(|c: char| ".,;:!?»)\"".contains(c));
            spans.push((s, s + trimmed.len()));
        }
    };
    for (i, c) in text.char_indices() {
        if c.is_whitespace() || c == '«' || c == '(' {
            if let Some(s) = chunk_start.take() {
                push(s, i);
            }
        } else if chunk_start.is_none() {
            chunk_start = Some(i);
        }
    }
    if let Some(s) = chunk_start {
        push(s, text.len());
    }
    spans
}

pub fn in_spans(spans: &[(usize, usize)], pos: usize) -> bool {
    spans.iter().any(|&(s, e)| pos >= s && pos < e)
}

/// Conversion de positions UTF-8 en positions UTF-16.
pub struct Utf16Map {
    /// (position UTF-8, position UTF-16) de chaque caractère, plus la fin du texte.
    points: Vec<(usize, usize)>,
}

impl Utf16Map {
    pub fn new(text: &str) -> Utf16Map {
        let mut points = Vec::with_capacity(text.len() + 1);
        let mut u16pos = 0;
        for (i, c) in text.char_indices() {
            points.push((i, u16pos));
            u16pos += c.len_utf16();
        }
        points.push((text.len(), u16pos));
        Utf16Map { points }
    }

    pub fn convert(&self, byte: usize) -> usize {
        match self.points.binary_search_by_key(&byte, |p| p.0) {
            Ok(i) => self.points[i].1,
            Err(i) => self.points[i.saturating_sub(1)].1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<&str> {
        raw_words(text)
            .iter()
            .map(|w| &text[w.start..w.end])
            .collect()
    }

    #[test]
    fn decoupe_mots() {
        assert_eq!(
            words("L’enfant, peut-être ; aujourd'hui - non."),
            ["L’enfant", "peut-être", "aujourd'hui", "non"]
        );
        assert_eq!(words("l' homme"), ["l", "homme"]);
    }

    #[test]
    fn utf16() {
        let text = "a😀é";
        let map = Utf16Map::new(text);
        assert_eq!(map.convert(0), 0);
        assert_eq!(map.convert(1), 1);
        assert_eq!(map.convert(5), 3);
        assert_eq!(map.convert(text.len()), 4);
    }

    #[test]
    fn zones_ignorees() {
        let text = "Voir https://exemple.org/a?b=c. Écrire à moi@exemple.fr !";
        let spans = ignored_spans(text);
        assert_eq!(spans.len(), 2);
        assert_eq!(&text[spans[0].0..spans[0].1], "https://exemple.org/a?b=c");
        assert_eq!(&text[spans[1].0..spans[1].1], "moi@exemple.fr");
    }
}
