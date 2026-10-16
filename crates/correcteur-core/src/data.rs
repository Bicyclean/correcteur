//! Chargement des fichiers de données (règles, listes, lexiques).
//!
//! Tous les fichiers sont du texte UTF-8 : TSV pour les tables, une entrée par
//! ligne pour les listes. Les lignes vides et celles qui commencent par `#`
//! sont ignorées. Le format est décrit dans `docs/format-donnees.md`.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::lexicon::Gn;
use crate::text::normalize_apostrophes;
use crate::Category;

/// Fichiers de `data/regles/` lus par le moteur. Tous sont obligatoires.
pub const RULE_FILES: &[&str] = &[
    "regles/regles.tsv",
    "regles/locutions.tsv",
    "regles/elisions.txt",
    "regles/abreviations.txt",
    "regles/verbes_a.txt",
    "regles/determinants.tsv",
    "regles/pronoms_sujets.tsv",
    "regles/mots_grammaticaux.tsv",
    "regles/h_aspire.txt",
    "regles/noms_deux_genres.txt",
];

/// Contenu brut des fichiers de données, tel que lu sur disque ou reçu par le
/// module WebAssembly.
#[derive(Debug, Clone, Default)]
pub struct EngineData {
    /// Un ou plusieurs lexiques TSV (`data/lexique/*.tsv`), triés par nom.
    pub lexicons: Vec<(String, String)>,
    /// Fichiers de règles, par chemin relatif (voir [`RULE_FILES`]).
    pub rule_files: HashMap<String, String>,
}

impl EngineData {
    /// Range un fichier selon son chemin relatif au dossier `data/`
    /// (`lexique/….tsv`, `regles/regles.tsv`…). Sert quand les fichiers ne
    /// sont pas lus sur disque, par exemple dans le module WebAssembly.
    /// Les fichiers que le moteur n'utilise pas sont ignorés.
    pub fn add_file(&mut self, path: &str, content: String) -> Result<(), DataError> {
        if path.starts_with("lexique/") && path.ends_with(".tsv") {
            self.lexicons.push((path.to_string(), content));
            self.lexicons.sort_by(|a, b| a.0.cmp(&b.0));
        } else if RULE_FILES.contains(&path) {
            self.rule_files.insert(path.to_string(), content);
        } else if !path.starts_with("regles/") && path.ends_with(".tsv") {
            return Err(DataError {
                file: path.to_string(),
                line: 0,
                message: "fichier de données inattendu".into(),
            });
        }
        Ok(())
    }

    fn file(&self, name: &str) -> Result<&str, DataError> {
        self.rule_files
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| DataError {
                file: name.to_string(),
                line: 0,
                message: "fichier manquant".into(),
            })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_dir(dir: &std::path::Path) -> Result<EngineData, DataError> {
        let read = |path: &std::path::Path| -> Result<String, DataError> {
            std::fs::read_to_string(path).map_err(|e| DataError {
                file: path.display().to_string(),
                line: 0,
                message: e.to_string(),
            })
        };
        let lex_dir = dir.join("lexique");
        let mut lex_paths: Vec<_> = std::fs::read_dir(&lex_dir)
            .map_err(|e| DataError {
                file: lex_dir.display().to_string(),
                line: 0,
                message: e.to_string(),
            })?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "tsv"))
            .collect();
        lex_paths.sort();
        let mut data = EngineData::default();
        for p in lex_paths {
            data.lexicons.push((p.display().to_string(), read(&p)?));
        }
        for name in RULE_FILES {
            data.rule_files
                .insert(name.to_string(), read(&dir.join(name))?);
        }
        Ok(data)
    }
}

/// Erreur de lecture ou de format d'un fichier de données.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataError {
    pub file: String,
    /// Numéro de ligne (à partir de 1), ou 0 si l'erreur concerne le fichier entier.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "{}:{} : {}", self.file, self.line, self.message)
        } else {
            write!(f, "{} : {}", self.file, self.message)
        }
    }
}

impl std::error::Error for DataError {}

/// Métadonnées d'une règle : catégorie, textes affichés, provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleInfo {
    pub id: String,
    pub category: Category,
    pub message: String,
    pub explanation: String,
    pub source: String,
}

/// Élément d'une locution : mot exact, ou toute forme d'un lemme (`=pallier`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocToken {
    Word(String),
    Lemma(String),
}

/// Locution fautive reconnue mot à mot, avec sa correction.
#[derive(Debug, Clone)]
pub(crate) struct Locution {
    /// Mots attendus, en minuscules, apostrophes typographiques.
    pub words: Vec<LocToken>,
    /// Correction ; `$1`, `$2`… reprennent le texte du 1er, 2e… mot trouvé.
    pub replacement: String,
    pub rule: &'static str,
}

#[derive(Debug, Clone)]
pub(crate) struct Determiner {
    pub gn: Gn,
    /// Forme de l'autre genre (le → la).
    pub other_gender: Option<String>,
    /// Forme de l'autre nombre (le → les).
    pub other_number: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Subject {
    /// `1s`, `3p`…
    pub person: String,
    /// Genre et nombre imposés au participe ou à l'adjectif attribut.
    pub gn: Gn,
}

/// Données prêtes à l'emploi.
pub(crate) struct Data {
    pub rules: HashMap<String, RuleInfo>,
    pub locutions: Vec<Locution>,
    pub elisions: HashSet<String>,
    pub abbreviations: HashSet<String>,
    pub verbs_with_a: HashSet<String>,
    pub determiners: HashMap<String, Determiner>,
    pub subjects: HashMap<String, Subject>,
    /// Classe → mots (voir `mots_grammaticaux.tsv`).
    pub word_classes: HashMap<String, HashSet<String>>,
    pub h_aspire: HashSet<String>,
    /// Noms employés aux deux genres (genre non vérifié).
    pub two_gender_nouns: HashSet<String>,
}

impl Data {
    pub fn is(&self, word: &str, class: &str) -> bool {
        self.word_classes
            .get(class)
            .is_some_and(|set| set.contains(word))
    }
}

/// Lignes utiles d'un fichier : (numéro, contenu).
pub(crate) fn data_lines(content: &str) -> impl Iterator<Item = (usize, &str)> {
    content
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r')))
        .filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#'))
}

fn word_list(content: &str) -> HashSet<String> {
    data_lines(content)
        .map(|(_, l)| normalize_apostrophes(l.trim()))
        .collect()
}

impl Data {
    pub fn parse(src: &EngineData) -> Result<Data, DataError> {
        let mut rules = HashMap::new();
        for (n, line) in data_lines(src.file("regles/regles.tsv")?) {
            let err = |message: String| DataError {
                file: "regles/regles.tsv".into(),
                line: n,
                message,
            };
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 5 {
                return Err(err(format!(
                    "5 colonnes attendues, {} trouvées",
                    cols.len()
                )));
            }
            let category = Category::parse(cols[1])
                .ok_or_else(|| err(format!("catégorie inconnue « {} »", cols[1])))?;
            let info = RuleInfo {
                id: cols[0].to_string(),
                category,
                message: cols[2].to_string(),
                explanation: cols[3].to_string(),
                source: cols[4].to_string(),
            };
            if rules.insert(info.id.clone(), info).is_some() {
                return Err(err(format!("règle « {} » définie deux fois", cols[0])));
            }
        }
        for id in crate::rules::CODE_RULES {
            if !rules.contains_key(*id) {
                return Err(DataError {
                    file: "regles/regles.tsv".into(),
                    line: 0,
                    message: format!(
                        "la règle « {id} » est implémentée mais n'a pas de description"
                    ),
                });
            }
        }

        let mut locutions = Vec::new();
        for (n, line) in data_lines(src.file("regles/locutions.tsv")?) {
            let err = |message: String| DataError {
                file: "regles/locutions.tsv".into(),
                line: n,
                message,
            };
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 4 {
                return Err(err(format!(
                    "4 colonnes attendues, {} trouvées",
                    cols.len()
                )));
            }
            // L'identifiant doit être une règle connue du code, pour obtenir une
            // chaîne 'static et garantir qu'elle est documentée.
            let rule = crate::rules::CODE_RULES
                .iter()
                .find(|id| **id == cols[2])
                .copied()
                .ok_or_else(|| err(format!("règle « {} » inconnue", cols[2])))?;
            let words: Vec<LocToken> = normalize_apostrophes(cols[0])
                .to_lowercase()
                .split(' ')
                .filter(|w| !w.is_empty())
                .map(|w| match w.strip_prefix('=') {
                    Some(lemma) => LocToken::Lemma(lemma.to_string()),
                    None => LocToken::Word(w.to_string()),
                })
                .collect();
            if words.is_empty() {
                return Err(err("locution vide".into()));
            }
            locutions.push(Locution {
                words,
                replacement: normalize_apostrophes(cols[1]),
                rule,
            });
        }

        let table =
            |name: &'static str, ncols: usize| -> Result<Vec<(usize, Vec<String>)>, DataError> {
                let mut rows = Vec::new();
                for (n, line) in data_lines(src.file(name)?) {
                    let cols: Vec<String> = line.split('\t').map(normalize_apostrophes).collect();
                    if cols.len() != ncols {
                        return Err(DataError {
                            file: name.into(),
                            line: n,
                            message: format!("{ncols} colonnes attendues, {} trouvées", cols.len()),
                        });
                    }
                    rows.push((n, cols));
                }
                Ok(rows)
            };
        let gn = |name: &str, n: usize, s: &str| {
            Gn::parse(s).ok_or_else(|| DataError {
                file: name.into(),
                line: n,
                message: format!("genre et nombre « {s} » invalides"),
            })
        };
        let other = |s: &str| (s != "-").then(|| s.to_string());

        let mut determiners = HashMap::new();
        for (n, c) in table("regles/determinants.tsv", 5)? {
            let d = Determiner {
                gn: gn("regles/determinants.tsv", n, &c[1])?,
                other_gender: other(&c[2]),
                other_number: other(&c[3]),
            };
            determiners.insert(c[0].clone(), d);
        }
        let mut subjects = HashMap::new();
        for (n, c) in table("regles/pronoms_sujets.tsv", 5)? {
            let s = Subject {
                person: c[1].clone(),
                gn: gn(
                    "regles/pronoms_sujets.tsv",
                    n,
                    &format!("{}.{}", c[2], c[3]),
                )?,
            };
            subjects.insert(c[0].clone(), s);
        }
        let mut word_classes: HashMap<String, HashSet<String>> = HashMap::new();
        for (_, c) in table("regles/mots_grammaticaux.tsv", 3)? {
            word_classes
                .entry(c[1].clone())
                .or_default()
                .insert(c[0].clone());
        }

        Ok(Data {
            rules,
            locutions,
            elisions: word_list(src.file("regles/elisions.txt")?),
            abbreviations: word_list(src.file("regles/abreviations.txt")?),
            verbs_with_a: word_list(src.file("regles/verbes_a.txt")?),
            determiners,
            subjects,
            word_classes,
            h_aspire: word_list(src.file("regles/h_aspire.txt")?),
            two_gender_nouns: word_list(src.file("regles/noms_deux_genres.txt")?),
        })
    }
}
