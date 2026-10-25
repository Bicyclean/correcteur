//! Règles d'analyse. Chaque module ajoute des [`RawIssue`] ; le moteur y joint
//! ensuite les textes décrits dans `data/regles/regles.tsv`.

mod grammar;
mod homophones;
mod locutions;
mod spelling;
pub mod style;
mod syntax;
mod typo;
mod verbs;

use std::collections::HashSet;

use crate::analysis::{analyze, Word};
use crate::data::Data;
use crate::lexicon::{Form, Lexicon};
use crate::text::ignored_spans;
use crate::Options;

/// Identifiants des règles implémentées dans le code. Chacun doit être décrit
/// dans `data/regles/regles.tsv` (vérifié au chargement).
pub const CODE_RULES: &[&str] = &[
    spelling::UNKNOWN,
    homophones::A_ACCENT,
    homophones::A_SANS_ACCENT,
    homophones::OU_ACCENT,
    homophones::OU_SANS_ACCENT,
    homophones::CE_SE,
    homophones::SE_CE,
    homophones::C_S,
    homophones::ET_EST,
    homophones::ON_ONT,
    homophones::ONT_ON,
    homophones::SA_CA,
    homophones::CA_SA,
    homophones::SES_CES,
    homophones::CES_SES,
    homophones::LEURS_LEUR,
    homophones::QUELLE_QU_ELLE,
    homophones::QU_ELLE_QUELLE,
    homophones::PEU_PEUT,
    homophones::PEUT_PEU,
    homophones::QUAND_QUANT,
    homophones::QUANT_QUAND,
    homophones::CES_C_EST,
    homophones::SON_SONT,
    homophones::S_C,
    homophones::SONT_SON,
    homophones::QUELQUE_QUEL_QUE,
    grammar::SUBJECT_VERB,
    grammar::DETERMINER,
    grammar::ADJECTIVE,
    grammar::PARTICIPLE_ETRE,
    grammar::PARTICIPLE_AVOIR,
    grammar::PARTICIPLE_AVOIR_COD,
    grammar::SI_CONDITIONAL,
    grammar::PARTICIPLE_AVOIR_LES,
    grammar::PARTICIPLE_FOR_INFINITIVE,
    grammar::SUBJUNCTIVE,
    grammar::CONCORDANCE,
    grammar::SI_FUTURE,
    grammar::TOUT,
    grammar::PRONOMINAL_PARTICIPLE,
    grammar::AUXILIARY_ETRE,
    grammar::INFINITIVE,
    grammar::NEGATION,
    grammar::PREPOSITION,
    locutions::EXPRESSION,
    locutions::FORM,
    syntax::ELISION,
    syntax::CONTRACTION,
    syntax::HYPHEN,
    syntax::SENSE,
    syntax::PARONYM,
    syntax::DONT,
    syntax::NOUN_EXPECTED,
    verbs::IMPERATIVE,
    verbs::BARE_PARTICIPLE,
    verbs::BARE_INFINITIVE,
    verbs::PARTICIPLE_AGREEMENT,
    style::REPETITION,
    style::LONG_SENTENCE,
    typo::SPACE_BEFORE_MISSING,
    typo::SPACE_BEFORE_BREAKING,
    typo::SPACE_AFTER_MISSING,
    typo::QUOTE_SPACE,
    typo::STRAIGHT_QUOTES,
    typo::STRAIGHT_APOSTROPHE,
    typo::APOSTROPHE_SPACE,
    typo::DASH_INCISE,
    typo::DOUBLE_HYPHEN,
    typo::MULTIPLE_SPACES,
    typo::SPACE_BEFORE_COMMA,
    typo::SPACE_INSIDE_PARENS,
    typo::CAPITAL_START,
    typo::ELLIPSIS,
];

/// Problème avant mise en forme : position, règle, arguments du message.
#[derive(Debug, Clone)]
pub struct RawIssue {
    pub start: usize,
    pub end: usize,
    pub rule: &'static str,
    /// Valeurs des marqueurs `{0}`, `{1}`… du message.
    pub args: Vec<String>,
    pub suggestions: Vec<String>,
}

impl RawIssue {
    pub fn new(start: usize, end: usize, rule: &'static str) -> RawIssue {
        RawIssue {
            start,
            end,
            rule,
            args: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn arg(mut self, a: impl Into<String>) -> RawIssue {
        self.args.push(a.into());
        self
    }

    pub fn suggest(mut self, s: impl Into<String>) -> RawIssue {
        self.suggestions.push(s.into());
        self
    }

    pub fn with_suggestions(mut self, s: Vec<String>) -> RawIssue {
        self.suggestions = s;
        self
    }
}

/// Tout ce dont une règle a besoin.
pub struct Context<'a> {
    pub text: &'a str,
    pub options: &'a Options,
    pub lexicon: &'a Lexicon,
    pub data: &'a Data,
    pub user_words: &'a HashSet<String>,
    pub ignored: Vec<(usize, usize)>,
    pub words: Vec<Word<'a>>,
    /// Entrée du lexique de chaque mot, cherchée une seule fois : les règles
    /// interrogent souvent le même mot (voisins, sujet, groupe nominal).
    forms: Vec<Option<&'a Form>>,
}

impl<'a> Context<'a> {
    pub fn new(
        text: &'a str,
        options: &'a Options,
        lexicon: &'a Lexicon,
        data: &'a Data,
        user_words: &'a HashSet<String>,
    ) -> Context<'a> {
        let ignored = ignored_spans(text);
        let words = analyze(text, lexicon, data, user_words, &ignored);
        let forms = words
            .iter()
            .map(|w| {
                lexicon
                    .find(&w.norm)
                    .or_else(|| with_circumflex(lexicon, &w.lower))
            })
            .collect();
        Context {
            text,
            options,
            lexicon,
            data,
            user_words,
            ignored,
            words,
            forms,
        }
    }

    /// Indice du mot précédent, s'il n'en est séparé que par des blancs.
    pub fn prev(&self, i: usize) -> Option<usize> {
        (i > 0 && self.words[i].joined_to_prev).then(|| i - 1)
    }

    /// Indice du mot suivant, s'il n'en est séparé que par des blancs.
    pub fn next(&self, i: usize) -> Option<usize> {
        (i + 1 < self.words.len() && self.words[i + 1].joined_to_prev).then_some(i + 1)
    }

    /// Entrée du lexique pour le mot d'indice `i`.
    pub fn form(&self, i: usize) -> Option<&'a Form> {
        self.forms.get(i).copied().flatten()
    }
}

pub fn run_all(ctx: &Context, out: &mut Vec<RawIssue>) {
    typo::check(ctx, out);
    spelling::check(ctx, out);
    homophones::check(ctx, out);
    grammar::check(ctx, out);
    syntax::check(ctx, out);
    verbs::check(ctx, out);
    style::check(ctx, out);
    locutions::check(ctx, out);
}

/// Graphie rectifiée de 1990 sans circonflexe (« connait », « maitre ») : la forme
/// traditionnelle du lexique (« connaît », « maître »), pour que les règles
/// d'accord la voient.
fn with_circumflex<'a>(lexicon: &'a Lexicon, lower: &str) -> Option<&'a Form> {
    if lower.chars().count() < 4 || lower.contains(|c: char| !c.is_alphabetic()) {
        return None;
    }
    let chars: Vec<char> = lower.chars().collect();
    (0..chars.len())
        .filter(|&i| matches!(chars[i], 'i' | 'u'))
        .find_map(|i| {
            let mut v = chars.clone();
            v[i] = if v[i] == 'i' { 'î' } else { 'û' };
            lexicon.get(&v.into_iter().collect::<String>())
        })
}
