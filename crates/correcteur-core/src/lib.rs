//! Moteur d'analyse du français.
//!
//! Point d'entrée : [`Engine::check`], qui reçoit un texte et des [`Options`]
//! et renvoie une liste de [`Issue`]. Tout le traitement est local : le moteur
//! n'a aucune dépendance réseau et ne lit que les fichiers de données qu'on lui
//! fournit (voir [`EngineData`]).

mod analysis;
pub mod conjugation;
pub mod corpus;
mod data;
pub mod interop;
mod lexicon;
mod rules;
mod suggest;
mod text;
pub mod thesaurus;

use std::collections::HashSet;

pub use conjugation::Conjugation;
pub use data::{DataError, EngineData, RuleInfo, OPTIONAL_FILES, RULE_FILES};
pub use lexicon::{Gn, Lexicon, Pos, Reading};
pub use rules::style::Readability;
pub use thesaurus::SynonymGroup;

/// Famille d'une erreur, utilisée pour le filtrage et pour les mesures de qualité.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// Mot absent du lexique.
    Orthographe,
    /// Confusion entre homophones grammaticaux (a/à, ou/où, ce/se, et/est…).
    Homophone,
    /// Accords, négation, prépositions (à venir).
    Grammaire,
    /// Espaces, ponctuation, apostrophes, tirets, majuscules.
    Typographie,
    /// Conseils de rédaction : répétitions, phrases longues. Désactivé par défaut.
    Style,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Orthographe,
        Category::Homophone,
        Category::Grammaire,
        Category::Typographie,
        Category::Style,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Orthographe => "orthographe",
            Category::Homophone => "homophone",
            Category::Grammaire => "grammaire",
            Category::Typographie => "typographie",
            Category::Style => "style",
        }
    }

    pub fn parse(s: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// Un problème détecté dans le texte.
///
/// Les suggestions remplacent le texte compris entre `start` et `end`.
#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    /// Début, en octets UTF-8.
    pub start: usize,
    /// Fin (exclue), en octets UTF-8.
    pub end: usize,
    /// Début, en unités UTF-16 (positions utilisées par Office.js et UNO).
    pub start_utf16: usize,
    /// Fin (exclue), en unités UTF-16.
    pub end_utf16: usize,
    /// Identifiant stable de la règle (voir `data/regles/regles.tsv`).
    pub rule: String,
    pub category: Category,
    /// Message court affiché à côté de l'erreur.
    pub message: String,
    /// Explication de la règle, pour qui veut comprendre.
    pub explanation: String,
    /// Remplacements proposés, du plus probable au moins probable.
    pub suggestions: Vec<String>,
}

/// Réglages d'une analyse.
#[derive(Debug, Clone)]
pub struct Options {
    /// Catégories à ne pas signaler.
    pub disabled_categories: Vec<Category>,
    /// Règles à ne pas signaler, par identifiant.
    pub disabled_rules: Vec<String>,
    /// Proposer l'espace fine insécable (U+202F) avant `;`, `!` et `?`.
    /// Sinon, l'espace insécable (U+00A0) est proposée partout.
    /// Les deux sont toujours acceptées dans le texte.
    pub narrow_nbsp: bool,
    /// Signaler les apostrophes droites (') et proposer l'apostrophe typographique (’).
    pub typographic_apostrophe: bool,
    /// Signaler les guillemets droits ou anglais et proposer les guillemets français.
    pub french_quotes: bool,
    /// Signaler les points de suspension écrits avec trois points.
    pub ellipsis_character: bool,
    /// Vérifier les mots à majuscule initiale en milieu de phrase
    /// (souvent des noms propres absents du lexique).
    pub check_proper_nouns: bool,
    /// Mots ajoutés par l'utilisateur, acceptés tels quels.
    pub user_words: Vec<String>,
    /// Nombre maximal de suggestions par erreur d'orthographe.
    pub max_suggestions: usize,
    /// Phrase signalée comme longue au-delà de ce nombre de mots.
    pub long_sentence_words: usize,
    /// Distance maximale, en mots, entre deux emplois d'un même mot signalés comme répétition.
    pub repetition_window: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            disabled_categories: vec![Category::Style],
            disabled_rules: Vec::new(),
            narrow_nbsp: true,
            typographic_apostrophe: true,
            french_quotes: true,
            ellipsis_character: true,
            check_proper_nouns: false,
            user_words: Vec::new(),
            max_suggestions: 5,
            long_sentence_words: 35,
            repetition_window: 40,
        }
    }
}

/// Le moteur, construit une fois à partir des données puis réutilisé.
pub struct Engine {
    data: data::Data,
    lexicon: Lexicon,
}

impl Engine {
    /// Construit le moteur à partir de données déjà lues (cas du WebAssembly).
    pub fn new(data: EngineData) -> Result<Engine, DataError> {
        let lexicon = Lexicon::from_tsv_files(&data.lexicons)?;
        let data = data::Data::parse(&data)?;
        Ok(Engine { data, lexicon })
    }

    /// Construit le moteur à partir d'un dossier `data/`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_dir(path: impl AsRef<std::path::Path>) -> Result<Engine, DataError> {
        Engine::new(EngineData::read_dir(path.as_ref())?)
    }

    pub fn lexicon(&self) -> &Lexicon {
        &self.lexicon
    }

    /// Métadonnées de toutes les règles connues.
    pub fn rules(&self) -> impl Iterator<Item = &RuleInfo> {
        self.data.rules.values()
    }

    pub fn rule(&self, id: &str) -> Option<&RuleInfo> {
        self.data.rules.get(id)
    }

    /// Tableau de conjugaison d'un verbe, donné à l'infinitif (« se laver » pour
    /// un pronominal) ou à une forme conjuguée (« mangeons »).
    pub fn conjugate(&self, verb: &str) -> Option<Conjugation> {
        let elides = |w: &str| {
            let lower = w.to_lowercase();
            match lower.chars().next().map(text::base_letter) {
                Some('a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'œ') => true,
                Some('h') => {
                    !self.data.h_aspire.contains(&lower)
                        && !self
                            .data
                            .h_aspire
                            .iter()
                            .any(|h| lower.starts_with(h.as_str()))
                }
                _ => false,
            }
        };
        let c = &self.data.conjugator;
        if let Some(found) = c.conjugate(verb, &elides) {
            if !found.guessed {
                return Some(found);
            }
        }
        // Forme conjuguée : retrouver l'infinitif par le lexique.
        let norm = text::normalize_apostrophes(verb.trim()).to_lowercase();
        let infinitive = self.lexicon.find(&norm).and_then(|f| {
            f.readings
                .iter()
                .find(|r| r.pos.is_verb() && c.model_of(&r.lemma).is_some())
                .map(|r| r.lemma.clone())
        });
        match infinitive {
            Some(inf) if inf != norm => c.conjugate(&inf, &elides),
            _ => c.conjugate(verb, &elides),
        }
    }

    /// Synonymes d'un mot, par sens. Une forme fléchie (« mangeons »,
    /// « belles ») est ramenée à son lemme. Renvoie le mot consulté et les groupes.
    pub fn synonyms(&self, word: &str) -> Option<(String, Vec<SynonymGroup>)> {
        self.lookup(word, |w| self.data.thesaurus.get(w))
    }

    /// Antonymes d'un mot (Wiktionnaire), par catégorie ; forme ramenée au lemme.
    pub fn antonyms(&self, word: &str) -> Option<(String, Vec<SynonymGroup>)> {
        self.lookup(word, |w| self.data.thesaurus.antonyms(w))
    }

    /// Cherche `word`, puis ses lemmes, avec `get`.
    fn lookup<'s>(
        &'s self,
        word: &str,
        get: impl Fn(&str) -> Option<&'s [SynonymGroup]>,
    ) -> Option<(String, Vec<SynonymGroup>)> {
        let norm = text::normalize_apostrophes(word.trim()).to_lowercase();
        if let Some(g) = get(&norm) {
            return Some((norm, g.to_vec()));
        }
        let form = self.lexicon.find(&norm)?;
        form.readings
            .iter()
            .find_map(|r| get(&r.lemma).map(|g| (r.lemma.clone(), g.to_vec())))
    }

    /// Indice de lisibilité et décomptes (mots, phrases, syllabes).
    pub fn readability(&self, text: &str) -> Readability {
        let options = Options::default();
        let none = HashSet::new();
        let ctx = rules::Context::new(text, &options, &self.lexicon, &self.data, &none);
        rules::style::readability(&ctx)
    }

    /// Analyse un texte et renvoie les problèmes triés par position.
    pub fn check(&self, text: &str, options: &Options) -> Vec<Issue> {
        let user_words: HashSet<String> = options
            .user_words
            .iter()
            .map(|w| text::normalize_apostrophes(w))
            .collect();
        let ctx = rules::Context::new(text, options, &self.lexicon, &self.data, &user_words);
        let mut raw = Vec::new();
        rules::run_all(&ctx, &mut raw);

        raw.sort_by_key(|r| (r.start, r.end));
        raw.dedup_by(|b, a| a.start == b.start && a.end == b.end && a.rule == b.rule);

        let utf16 = text::Utf16Map::new(text);
        raw.into_iter()
            .filter_map(|r| {
                let info = self.data.rules.get(r.rule)?;
                if options.disabled_categories.contains(&info.category)
                    || options.disabled_rules.iter().any(|d| d == r.rule)
                {
                    return None;
                }
                Some(Issue {
                    start: r.start,
                    end: r.end,
                    start_utf16: utf16.convert(r.start),
                    end_utf16: utf16.convert(r.end),
                    rule: info.id.clone(),
                    category: info.category,
                    message: fill_template(&info.message, &r.args),
                    explanation: fill_template(&info.explanation, &r.args),
                    suggestions: r.suggestions,
                })
            })
            .collect()
    }
}

/// Remplace `{0}`, `{1}`… par les arguments fournis par la règle.
fn fill_template(template: &str, args: &[String]) -> String {
    let mut out = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), arg);
    }
    out
}

/// Raccourci : analyse avec le moteur et les options données.
pub fn check(engine: &Engine, text: &str, options: &Options) -> Vec<Issue> {
    engine.check(text, options)
}
