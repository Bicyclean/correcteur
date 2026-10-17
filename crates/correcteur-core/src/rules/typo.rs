//! Typographie : espaces autour de la ponctuation, guillemets, apostrophes,
//! tirets, majuscule en début de phrase, points de suspension.
//!
//! Les règles travaillent sur les caractères. Les deux espaces insécables
//! (U+00A0 et U+202F) sont acceptées partout où une espace insécable est
//! attendue ; l'option `narrow_nbsp` ne règle que la suggestion.

use super::{Context, RawIssue};
use crate::text::{capitalize, in_spans, is_hspace, is_nbsp, NBSP, NNBSP};

pub const SPACE_BEFORE_MISSING: &str = "TYPO_ESPACE_AVANT_MANQUANTE";
pub const SPACE_BEFORE_BREAKING: &str = "TYPO_ESPACE_AVANT_SECABLE";
pub const SPACE_AFTER_MISSING: &str = "TYPO_ESPACE_APRES_MANQUANTE";
pub const QUOTE_SPACE: &str = "TYPO_ESPACE_GUILLEMETS";
pub const STRAIGHT_QUOTES: &str = "TYPO_GUILLEMETS_FRANCAIS";
pub const STRAIGHT_APOSTROPHE: &str = "TYPO_APOSTROPHE_DROITE";
pub const APOSTROPHE_SPACE: &str = "TYPO_ESPACE_APRES_APOSTROPHE";
pub const DASH_INCISE: &str = "TYPO_TIRET_INCISE";
pub const DOUBLE_HYPHEN: &str = "TYPO_DOUBLE_TRAIT";
pub const MULTIPLE_SPACES: &str = "TYPO_ESPACES_MULTIPLES";
pub const SPACE_BEFORE_COMMA: &str = "TYPO_ESPACE_AVANT_VIRGULE";
pub const SPACE_INSIDE_PARENS: &str = "TYPO_ESPACE_PARENTHESES";
pub const CAPITAL_START: &str = "TYPO_MAJUSCULE_PHRASE";
pub const ELLIPSIS: &str = "TYPO_POINTS_SUSPENSION";

/// Ponctuation double, précédée d'une espace insécable en français.
const DOUBLE_PUNCT: &str = ";:!?";

struct Chars<'a> {
    text: &'a str,
    v: Vec<(usize, char)>,
}

impl Chars<'_> {
    fn get(&self, k: usize) -> Option<char> {
        self.v.get(k).map(|&(_, c)| c)
    }
    fn before(&self, k: usize) -> Option<char> {
        k.checked_sub(1).and_then(|k| self.get(k))
    }
    fn pos(&self, k: usize) -> usize {
        self.v.get(k).map_or(self.text.len(), |&(i, _)| i)
    }
    /// Début (indice de caractère) de la suite de blancs horizontaux qui finit en `k - 1`.
    fn hspace_run_before(&self, k: usize) -> usize {
        let mut s = k;
        while s > 0 && self.get(s - 1).is_some_and(is_hspace) {
            s -= 1;
        }
        s
    }
    /// Fin (exclue) de la suite de blancs horizontaux qui commence en `k`.
    fn hspace_run_after(&self, k: usize) -> usize {
        let mut e = k;
        while self.get(e).is_some_and(is_hspace) {
            e += 1;
        }
        e
    }
}

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    let cs = Chars {
        text: ctx.text,
        v: ctx.text.char_indices().collect(),
    };
    let ignored = |k: usize| in_spans(&ctx.ignored, cs.pos(k));

    for k in 0..cs.v.len() {
        if ignored(k) {
            continue;
        }
        let c = cs.v[k].1;
        match c {
            ';' | ':' | '!' | '?' => double_punct(ctx, &cs, k, c, out),
            ',' | '.' => {
                comma_period(ctx, &cs, k, c, out);
            }
            '«' | '»' => guillemet(&cs, k, c, out),
            '\'' | '’' => apostrophe(ctx, &cs, k, c, out),
            '-' => hyphen(&cs, k, out),
            '(' | ')' => parens(&cs, k, c, out),
            ' ' => multiple_spaces(&cs, k, out),
            _ => {}
        }
    }
    if ctx.options.french_quotes {
        english_quotes(&cs, &ignored, out);
    }
    capitals(ctx, out);
}

fn nbsp_for(ctx: &Context, c: char) -> char {
    if ctx.options.narrow_nbsp && c != ':' {
        NNBSP
    } else {
        NBSP
    }
}

fn double_punct(ctx: &Context, cs: &Chars, k: usize, c: char, out: &mut Vec<RawIssue>) {
    let next = cs.get(k + 1);
    // Heure ou rapport (10:30, 1:2) ; frimousses ;-) :) :D.
    if c == ':'
        && cs.before(k).is_some_and(|p| p.is_ascii_digit())
        && next.is_some_and(|n| n.is_ascii_digit())
    {
        return;
    }
    if matches!(c, ':' | ';')
        && next.is_some_and(|n| matches!(n, ')' | '(' | '-' | 'D' | 'P' | 'p'))
        && !cs.get(k + 2).is_some_and(char::is_alphanumeric)
    {
        return;
    }
    let Some(prev) = cs.before(k) else { return };
    let missing_after = next.is_some_and(char::is_alphabetic);
    let sp = nbsp_for(ctx, c);

    if is_nbsp(prev) || DOUBLE_PUNCT.contains(prev) || matches!(prev, '(' | '[' | '\n' | '\r') {
        // Espace correcte ou suite « ?! » : seule l'espace après est en cause.
    } else if prev == ' ' || prev == '\t' {
        let s = cs.hspace_run_before(k);
        if s > 0 && !matches!(cs.get(s - 1), Some('\n' | '\r')) {
            out.push(
                RawIssue::new(cs.pos(s), cs.pos(k), SPACE_BEFORE_BREAKING)
                    .arg(c)
                    .suggest(sp),
            );
        }
    } else {
        let after = if missing_after { " " } else { "" };
        out.push(
            RawIssue::new(cs.pos(k), cs.pos(k + 1), SPACE_BEFORE_MISSING)
                .arg(c)
                .suggest(format!("{sp}{c}{after}")),
        );
        return;
    }
    if missing_after {
        out.push(
            RawIssue::new(cs.pos(k), cs.pos(k + 1), SPACE_AFTER_MISSING)
                .arg(c)
                .suggest(format!("{c} ")),
        );
    }
}

fn comma_period(ctx: &Context, cs: &Chars, k: usize, c: char, out: &mut Vec<RawIssue>) {
    let next = cs.get(k + 1);
    let prev = cs.before(k);
    // Points de suspension écrits « ... ».
    if c == '.' && next == Some('.') && cs.get(k + 2) == Some('.') && prev != Some('.') {
        if cs.get(k + 3) != Some('.') && ctx.options.ellipsis_character {
            out.push(RawIssue::new(cs.pos(k), cs.pos(k + 3), ELLIPSIS).suggest("…"));
        }
        return;
    }
    if c == '.' && (next == Some('.') || prev == Some('.')) {
        return;
    }
    // Espace avant la virgule ou le point.
    if prev.is_some_and(|p| p == ' ' || p == '\t') && !next.is_some_and(|n| n.is_ascii_digit()) {
        let s = cs.hspace_run_before(k);
        if s > 0 && !cs.get(s - 1).is_some_and(char::is_whitespace) {
            out.push(
                RawIssue::new(cs.pos(s), cs.pos(k), SPACE_BEFORE_COMMA)
                    .arg(c)
                    .suggest(""),
            );
        }
    }
    // Espace manquante après : « oui,non », « fin.Début ».
    let Some(n) = next else { return };
    let missing = match c {
        ',' => n.is_alphabetic() && prev.is_some_and(char::is_alphabetic),
        _ => n.is_uppercase() && previous_word_is_long_lowercase(cs, k),
    };
    if missing {
        out.push(
            RawIssue::new(cs.pos(k), cs.pos(k + 1), SPACE_AFTER_MISSING)
                .arg(c)
                .suggest(format!("{c} ")),
        );
    }
}

/// Le mot qui précède `k` a-t-il au moins deux lettres dont une minuscule ?
/// Évite de signaler les sigles (« S.N.C.F. ») et les initiales.
fn previous_word_is_long_lowercase(cs: &Chars, k: usize) -> bool {
    let mut letters = 0;
    let mut lower = false;
    let mut j = k;
    while j > 0 {
        match cs.get(j - 1) {
            Some(c) if c.is_alphabetic() => {
                letters += 1;
                lower |= c.is_lowercase();
                j -= 1;
            }
            _ => break,
        }
    }
    letters >= 2 && lower
}

fn guillemet(cs: &Chars, k: usize, c: char, out: &mut Vec<RawIssue>) {
    if c == '«' {
        match cs.get(k + 1) {
            Some(n) if is_nbsp(n) => {}
            Some(' ' | '\t') => {
                let e = cs.hspace_run_after(k + 1);
                if cs.get(e).is_some_and(|c| !c.is_whitespace()) {
                    out.push(RawIssue::new(cs.pos(k + 1), cs.pos(e), QUOTE_SPACE).suggest(NBSP));
                }
            }
            Some(n) if !n.is_whitespace() => {
                out.push(
                    RawIssue::new(cs.pos(k), cs.pos(k + 1), QUOTE_SPACE)
                        .suggest(format!("«{NBSP}")),
                );
            }
            _ => {}
        }
    } else {
        match cs.before(k) {
            Some(p) if is_nbsp(p) => {}
            Some(' ' | '\t') => {
                let s = cs.hspace_run_before(k);
                if s > 0 && !cs.get(s - 1).is_some_and(char::is_whitespace) {
                    out.push(RawIssue::new(cs.pos(s), cs.pos(k), QUOTE_SPACE).suggest(NBSP));
                }
            }
            Some(p) if !p.is_whitespace() => {
                out.push(
                    RawIssue::new(cs.pos(k), cs.pos(k + 1), QUOTE_SPACE)
                        .suggest(format!("{NBSP}»")),
                );
            }
            _ => {}
        }
        // « »mot » : espace manquante après le guillemet fermant.
        if cs.get(k + 1).is_some_and(char::is_alphanumeric) {
            out.push(
                RawIssue::new(cs.pos(k), cs.pos(k + 1), SPACE_AFTER_MISSING)
                    .arg(c)
                    .suggest("» "),
            );
        }
    }
}

fn apostrophe(ctx: &Context, cs: &Chars, k: usize, c: char, out: &mut Vec<RawIssue>) {
    let prev_letter = cs.before(k).is_some_and(char::is_alphabetic);
    if !prev_letter {
        return;
    }
    let next = cs.get(k + 1);
    if next.is_some_and(char::is_alphabetic) {
        if c == '\'' && ctx.options.typographic_apostrophe {
            out.push(RawIssue::new(cs.pos(k), cs.pos(k + 1), STRAIGHT_APOSTROPHE).suggest("’"));
        }
        return;
    }
    // « l’ homme » : espace après une élision.
    if next.is_some_and(|n| n == ' ' || n == '\t') {
        let e = cs.hspace_run_after(k + 1);
        if !cs.get(e).is_some_and(char::is_alphabetic) {
            return;
        }
        let mut s = k;
        while s > 0 && cs.get(s - 1).is_some_and(char::is_alphabetic) {
            s -= 1;
        }
        let elided = format!("{}’", &cs.text[cs.pos(s)..cs.pos(k)]).to_lowercase();
        if ctx.data.elisions.contains(&elided) {
            out.push(RawIssue::new(cs.pos(k), cs.pos(e), APOSTROPHE_SPACE).suggest("’"));
        }
    }
}

fn hyphen(cs: &Chars, k: usize, out: &mut Vec<RawIssue>) {
    let prev = cs.before(k);
    let next = cs.get(k + 1);
    // « -- » isolé.
    if next == Some('-') && prev != Some('-') && cs.get(k + 2) != Some('-') {
        out.push(RawIssue::new(cs.pos(k), cs.pos(k + 2), DOUBLE_HYPHEN).suggest("–"));
        return;
    }
    // Trait d'union entouré d'espaces, hors début de ligne (puces de liste).
    if prev.is_some_and(is_hspace) && next.is_some_and(is_hspace) {
        let s = cs.hspace_run_before(k);
        if s > 0 && !matches!(cs.get(s - 1), Some('\n' | '\r')) {
            out.push(RawIssue::new(cs.pos(k), cs.pos(k + 1), DASH_INCISE).suggest("–"));
        }
    }
}

fn parens(cs: &Chars, k: usize, c: char, out: &mut Vec<RawIssue>) {
    if c == '(' {
        if cs.get(k + 1).is_some_and(|n| n == ' ' || n == '\t') {
            let e = cs.hspace_run_after(k + 1);
            if cs.get(e).is_some_and(|c| !c.is_whitespace()) {
                out.push(
                    RawIssue::new(cs.pos(k + 1), cs.pos(e), SPACE_INSIDE_PARENS)
                        .arg(c)
                        .suggest(""),
                );
            }
        }
    } else if cs.before(k).is_some_and(|p| p == ' ' || p == '\t') {
        let s = cs.hspace_run_before(k);
        if s > 0 && !cs.get(s - 1).is_some_and(char::is_whitespace) {
            out.push(
                RawIssue::new(cs.pos(s), cs.pos(k), SPACE_INSIDE_PARENS)
                    .arg(c)
                    .suggest(""),
            );
        }
    }
}

fn multiple_spaces(cs: &Chars, k: usize, out: &mut Vec<RawIssue>) {
    // Ne traiter chaque suite qu'une fois, depuis son premier caractère.
    if cs.before(k).is_some_and(is_hspace) {
        return;
    }
    let e = cs.hspace_run_after(k);
    if e - k < 2 || k == 0 {
        return;
    }
    let prev = cs.before(k).unwrap_or('\n');
    let next = cs.get(e);
    if prev.is_whitespace() || prev == '«' || next.is_none_or(char::is_whitespace) {
        return;
    }
    let next = next.unwrap_or(' ');
    if DOUBLE_PUNCT.contains(next) || matches!(next, '»' | ',' | '.' | ')') {
        return; // traité par les règles de ponctuation
    }
    let run: Vec<char> = (k..e).filter_map(|j| cs.get(j)).collect();
    if run.contains(&'\t') {
        return; // tabulations : alignement voulu
    }
    let keep = if run.contains(&NNBSP) {
        NNBSP
    } else if run.contains(&NBSP) {
        NBSP
    } else {
        ' '
    };
    out.push(RawIssue::new(cs.pos(k), cs.pos(e), MULTIPLE_SPACES).suggest(keep));
}

/// Guillemets droits (appariés) et guillemets anglais hors citation imbriquée.
fn english_quotes(cs: &Chars, ignored: &dyn Fn(usize) -> bool, out: &mut Vec<RawIssue>) {
    let mut straight = Vec::new();
    let mut depth = 0i32;
    for k in 0..cs.v.len() {
        if ignored(k) {
            continue;
        }
        match cs.v[k].1 {
            '«' => depth += 1,
            '»' => depth -= 1,
            '"' if !cs.before(k).is_some_and(|p| p.is_ascii_digit()) => straight.push(k),
            '“' if depth <= 0 => push_quote(cs, k, true, out),
            '”' if depth <= 0 => push_quote(cs, k, false, out),
            _ => {}
        }
    }
    if straight.len() % 2 == 0 {
        for (n, &k) in straight.iter().enumerate() {
            push_quote(cs, k, n % 2 == 0, out);
        }
    }
}

fn push_quote(cs: &Chars, k: usize, opening: bool, out: &mut Vec<RawIssue>) {
    let (start, end, suggestion) = if opening {
        let e = cs.hspace_run_after(k + 1);
        (cs.pos(k), cs.pos(e), format!("«{NBSP}"))
    } else {
        let s = cs.hspace_run_before(k);
        (cs.pos(s), cs.pos(k + 1), format!("{NBSP}»"))
    };
    out.push(RawIssue::new(start, end, STRAIGHT_QUOTES).suggest(suggestion));
}

/// Minuscule en début de texte ou après un point final.
fn capitals(ctx: &Context, out: &mut Vec<RawIssue>) {
    for (i, w) in ctx.words.iter().enumerate() {
        if w.compound_part || w.ignored || w.has_digit {
            continue;
        }
        let first = w.text.chars().next().unwrap_or(' ');
        if !first.is_lowercase() || w.text.chars().skip(1).any(char::is_uppercase) {
            continue; // déjà en majuscule, ou graphie voulue (« iPhone »)
        }
        let before = ctx.text[..w.start].trim_end_matches(|c: char| {
            c.is_whitespace() || matches!(c, '«' | '"' | '“' | '(' | '—' | '–')
        });
        let starts_sentence = if before.is_empty() {
            true
        } else if let Some(rest) = before.strip_suffix('.') {
            let prev_word = i.checked_sub(1).map(|p| &ctx.words[p]);
            // « librarything.fr » : point collé au mot suivant, nom de domaine.
            let domain = before.len() == w.start;
            !domain
                && !rest.ends_with('.')
                && !rest.ends_with(|c: char| c.is_ascii_digit())
                && prev_word.is_some_and(|p| {
                    p.end == rest.len()
                        && p.text.chars().count() > 1
                        && !ctx.data.is_abbreviation(&p.norm)
                })
        } else {
            false
        };
        if starts_sentence {
            out.push(
                RawIssue::new(w.start, w.end, CAPITAL_START)
                    .arg(w.text)
                    .suggest(capitalize(w.text)),
            );
        }
    }
}
