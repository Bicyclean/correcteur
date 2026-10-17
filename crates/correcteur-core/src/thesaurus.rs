//! Synonymes et antonymes.
//!
//! Synonymes : thésaurus au format myThes (`data/thesaurus/thes_fr.dat`), une
//! ligne d'encodage puis, pour chaque mot, une ligne `mot|n` suivie de `n`
//! lignes `(Catégorie)|synonyme|synonyme…`. Antonymes : TSV `mot`,
//! `catégorie`, antonymes séparés par `|`, `source`
//! (`data/thesaurus/antonymes.tsv`).
//!
//! Les fichiers sont gardés tels quels en mémoire ; seul un index (mot → zones
//! du texte) est construit au chargement, et une entrée n'est lue qu'au moment
//! où on la consulte. Construire tous les groupes d'avance coûtait trois quarts
//! du temps de démarrage du moteur et 35 Mo, pour un outil consulté mot à mot.

use std::collections::HashMap;
use std::ops::Range;

/// Synonymes (ou antonymes) d'un mot pour un de ses sens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynonymGroup {
    /// Catégorie donnée par la source (« Nom », « Verbe »…), sans parenthèses.
    pub category: String,
    pub words: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Thesaurus {
    synonyms_text: String,
    /// Mot en minuscules → zones de `synonyms_text` contenant ses lignes de groupes.
    synonyms: HashMap<Box<str>, Vec<Range<u32>>>,
    antonyms_text: String,
    /// Mot en minuscules → lignes de `antonyms_text`.
    antonyms: HashMap<Box<str>, Vec<Range<u32>>>,
}

/// Lignes d'un texte avec leur zone (sans le saut de ligne).
fn lines_with_ranges(text: &str) -> impl Iterator<Item = (&str, Range<u32>)> {
    let mut offset = 0u32;
    text.split_inclusive('\n').map(move |raw| {
        let start = offset;
        offset += raw.len() as u32;
        let line = raw.trim_end_matches(['\n', '\r']);
        (line, start..start + line.len() as u32)
    })
}

impl Thesaurus {
    /// Indexe le thésaurus et les antonymes. Lecture tolérante : une entrée mal
    /// formée est ignorée, pas le fichier entier.
    pub fn new(synonyms_text: String, antonyms_text: String) -> Thesaurus {
        let mut synonyms: HashMap<Box<str>, Vec<Range<u32>>> = HashMap::new();
        {
            let mut lines = lines_with_ranges(&synonyms_text).skip(1); // ligne d'encodage
            while let Some((head, _)) = lines.next() {
                let Some((word, count)) = head.rsplit_once('|') else {
                    continue;
                };
                let Ok(count) = count.trim().parse::<usize>() else {
                    continue;
                };
                let block: Vec<Range<u32>> = lines.by_ref().take(count).map(|(_, r)| r).collect();
                if let (Some(first), Some(last)) = (block.first(), block.last()) {
                    synonyms
                        .entry(word.trim().to_lowercase().into_boxed_str())
                        .or_default()
                        .push(first.start..last.end);
                }
            }
        }

        let mut antonyms: HashMap<Box<str>, Vec<Range<u32>>> = HashMap::new();
        for (line, range) in lines_with_ranges(&antonyms_text) {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((word, _)) = line.split_once('\t') {
                antonyms
                    .entry(word.to_lowercase().into_boxed_str())
                    .or_default()
                    .push(range);
            }
        }

        Thesaurus {
            synonyms_text,
            synonyms,
            antonyms_text,
            antonyms,
        }
    }

    /// Nombre de mots ayant des synonymes.
    pub fn len(&self) -> usize {
        self.synonyms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.synonyms.is_empty()
    }

    /// Synonymes de `word`, par sens.
    pub fn get(&self, word: &str) -> Option<Vec<SynonymGroup>> {
        let ranges = self.synonyms.get(word.to_lowercase().as_str())?;
        let groups: Vec<SynonymGroup> = ranges
            .iter()
            .flat_map(|r| self.synonyms_text[r.start as usize..r.end as usize].lines())
            .filter_map(|line| {
                let mut parts = line.split('|');
                let category = parts.next()?.trim_matches(['(', ')', ' ']).to_string();
                let words: Vec<String> = parts
                    .map(str::trim)
                    .filter(|w| !w.is_empty())
                    .map(str::to_string)
                    .collect();
                (!words.is_empty()).then_some(SynonymGroup { category, words })
            })
            .collect();
        (!groups.is_empty()).then_some(groups)
    }

    /// Antonymes de `word`, par catégorie.
    pub fn antonyms(&self, word: &str) -> Option<Vec<SynonymGroup>> {
        let ranges = self.antonyms.get(word.to_lowercase().as_str())?;
        let groups: Vec<SynonymGroup> = ranges
            .iter()
            .filter_map(|r| {
                let cols: Vec<&str> = self.antonyms_text[r.start as usize..r.end as usize]
                    .split('\t')
                    .collect();
                let words: Vec<String> = cols
                    .get(2)?
                    .split('|')
                    .filter(|w| !w.is_empty())
                    .map(str::to_string)
                    .collect();
                (!words.is_empty()).then(|| SynonymGroup {
                    category: cols[1].to_string(),
                    words,
                })
            })
            .collect();
        (!groups.is_empty()).then_some(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lecture() {
        let t = Thesaurus::new(
            "UTF-8\nabaissé|1\n(Adjectif)|avili|rabaissé\nmalformé\nbeau|2\n(Adjectif)|joli|charmant\r\n(Nom)|beauté\n".into(),
            String::new(),
        );
        assert_eq!(t.len(), 2);
        let beau = t.get("Beau").unwrap();
        assert_eq!(beau.len(), 2);
        assert_eq!(
            beau[0].words,
            ["joli", "charmant"],
            "fin de ligne Windows retirée"
        );
        assert_eq!(
            beau[1],
            SynonymGroup {
                category: "Nom".into(),
                words: vec!["beauté".into()]
            }
        );
        assert!(t.get("laid").is_none());
    }

    #[test]
    fn antonymes() {
        let t = Thesaurus::new(
            String::new(),
            "# en-tête\ngrand\tAdjectif\tpetit|minuscule\tWIKTIONNAIRE\nmalformé\ngrand\tNom\tenfant\tWIKTIONNAIRE".into(),
        );
        let grand = t.antonyms("Grand").unwrap();
        assert_eq!(grand[0].words, ["petit", "minuscule"]);
        assert_eq!(
            grand[1].category, "Nom",
            "dernière ligne sans saut de ligne"
        );
        assert!(t.antonyms("malformé").is_none());
    }
}
