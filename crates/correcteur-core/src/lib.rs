//! Moteur d'analyse du français.
//!
//! Point d'entrée : [`Engine::check`], qui reçoit un texte et des [`Options`]
//! et renvoie une liste de [`Issue`]. Tout le traitement est local : le moteur
//! n'a aucune dépendance réseau et ne lit que les fichiers de données qu'on lui
//! fournit (voir [`EngineData`]).

mod analysis;
pub mod corpus;
mod data;
pub mod interop;
mod lexicon;
mod rules;
mod suggest;
mod text;

use std::collections::HashSet;

pub use data::{DataError, EngineData, RuleInfo, RULE_FILES};
pub use lexicon::{Gn, Lexicon, Pos, Reading};

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
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Orthographe,
        Category::Homophone,
        Category::Grammaire,
        Category::Typographie,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Orthographe => "orthographe",
            Category::Homophone => "homophone",
            Category::Grammaire => "grammaire",
            Category::Typographie => "typographie",
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
}

impl Default for Options {
    fn default() -> Self {
        Options {
            disabled_categories: Vec::new(),
            disabled_rules: Vec::new(),
            narrow_nbsp: true,
            typographic_apostrophe: true,
            french_quotes: true,
            ellipsis_character: true,
            check_proper_nouns: false,
            user_words: Vec::new(),
            max_suggestions: 5,
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
