//! Chargement des fichiers de données (règles, listes, lexiques).
//!
//! Tous les fichiers sont du texte UTF-8 : TSV pour les tables, une entrée par
//! ligne pour les listes. Les lignes vides et celles qui commencent par `#`
//! sont ignorées. Le format est décrit dans `docs/format-donnees.md`.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::text::normalize_apostrophes;
use crate::Category;

/// Contenu brut des fichiers de données, tel que lu sur disque ou reçu par le
/// module WebAssembly.
#[derive(Debug, Clone, Default)]
pub struct EngineData {
    /// Un ou plusieurs lexiques TSV (`data/lexique/*.tsv`).
    pub lexicons: Vec<(String, String)>,
    /// `data/regles/regles.tsv`.
    pub rules: String,
    /// `data/regles/locutions.tsv`.
    pub locutions: String,
    /// `data/regles/elisions.txt`.
    pub elisions: String,
    /// `data/regles/abreviations.txt`.
    pub abbreviations: String,
    /// `data/regles/verbes_a.txt`.
    pub verbs_with_a: String,
}

impl EngineData {
    /// Range un fichier selon son chemin relatif au dossier `data/`
    /// (`lexique/….tsv`, `regles/regles.tsv`…). Sert quand les fichiers ne
    /// sont pas lus sur disque, par exemple dans le module WebAssembly.
    pub fn add_file(&mut self, path: &str, content: String) -> Result<(), DataError> {
        let slot = match path {
            "regles/regles.tsv" => &mut self.rules,
            "regles/locutions.tsv" => &mut self.locutions,
            "regles/elisions.txt" => &mut self.elisions,
            "regles/abreviations.txt" => &mut self.abbreviations,
            "regles/verbes_a.txt" => &mut self.verbs_with_a,
            p if p.starts_with("lexique/") && p.ends_with(".tsv") => {
                self.lexicons.push((p.to_string(), content));
                self.lexicons.sort_by(|a, b| a.0.cmp(&b.0));
                return Ok(());
            }
            // Fichiers utiles seulement aux outils (import, documentation).
            p if p.starts_with("regles/") || !p.ends_with(".tsv") => return Ok(()),
            _ => {
                return Err(DataError {
                    file: path.to_string(),
                    line: 0,
                    message: "fichier de données inattendu".into(),
                })
            }
        };
        *slot = content;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_dir(dir: &std::path::Path) -> Result<EngineData, DataError> {
        let read = |rel: &str| -> Result<String, DataError> {
            let path = dir.join(rel);
            std::fs::read_to_string(&path).map_err(|e| DataError {
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
        let mut lexicons = Vec::new();
        for p in lex_paths {
            let name = p.display().to_string();
            let content = std::fs::read_to_string(&p).map_err(|e| DataError {
                file: name.clone(),
                line: 0,
                message: e.to_string(),
            })?;
            lexicons.push((name, content));
        }
        Ok(EngineData {
            lexicons,
            rules: read("regles/regles.tsv")?,
            locutions: read("regles/locutions.tsv")?,
            elisions: read("regles/elisions.txt")?,
            abbreviations: read("regles/abreviations.txt")?,
            verbs_with_a: read("regles/verbes_a.txt")?,
        })
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

/// Locution fautive reconnue mot à mot, avec sa correction.
#[derive(Debug, Clone)]
pub(crate) struct Locution {
    /// Mots attendus, en minuscules, apostrophes typographiques.
    pub words: Vec<String>,
    pub replacement: String,
    pub rule: &'static str,
}

/// Données prêtes à l'emploi.
pub(crate) struct Data {
    pub rules: HashMap<String, RuleInfo>,
    pub locutions: Vec<Locution>,
    pub elisions: HashSet<String>,
    pub abbreviations: HashSet<String>,
    pub verbs_with_a: HashSet<String>,
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
        for (n, line) in data_lines(&src.rules) {
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
        for (n, line) in data_lines(&src.locutions) {
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
            let words: Vec<String> = normalize_apostrophes(cols[0])
                .to_lowercase()
                .split(' ')
                .filter(|w| !w.is_empty())
                .map(str::to_string)
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

        Ok(Data {
            rules,
            locutions,
            elisions: word_list(&src.elisions),
            abbreviations: word_list(&src.abbreviations),
            verbs_with_a: word_list(&src.verbs_with_a),
        })
    }
}
