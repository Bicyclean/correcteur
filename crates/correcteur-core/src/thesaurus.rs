//! Synonymes, d'après un thésaurus au format myThes (`data/thesaurus/thes_fr.dat`) :
//! une ligne d'encodage, puis pour chaque mot une ligne `mot|n` suivie de `n`
//! lignes `(Catégorie)|synonyme|synonyme…`.

use std::collections::HashMap;

/// Synonymes d'un mot pour un de ses sens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynonymGroup {
    /// Catégorie donnée par le thésaurus (« Nom », « Verbe »…), sans parenthèses.
    pub category: String,
    pub words: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Thesaurus {
    entries: HashMap<String, Vec<SynonymGroup>>,
}

impl Thesaurus {
    /// Lecture tolérante : une entrée mal formée est ignorée, pas le fichier entier.
    pub fn parse(content: &str) -> Thesaurus {
        let mut entries: HashMap<String, Vec<SynonymGroup>> = HashMap::new();
        let mut lines = content.lines().skip(1); // ligne d'encodage
        while let Some(head) = lines.next() {
            let Some((word, count)) = head.rsplit_once('|') else {
                continue;
            };
            let Ok(count) = count.trim().parse::<usize>() else {
                continue;
            };
            let groups = entries.entry(word.trim().to_lowercase()).or_default();
            for line in lines.by_ref().take(count) {
                let mut parts = line.split('|');
                let category = parts
                    .next()
                    .unwrap_or("")
                    .trim_matches(['(', ')', ' '])
                    .to_string();
                let words: Vec<String> = parts
                    .map(str::trim)
                    .filter(|w| !w.is_empty())
                    .map(str::to_string)
                    .collect();
                if !words.is_empty() {
                    groups.push(SynonymGroup { category, words });
                }
            }
        }
        Thesaurus { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, word: &str) -> Option<&[SynonymGroup]> {
        self.entries.get(&word.to_lowercase()).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lecture() {
        let t = Thesaurus::parse("UTF-8\nabaissé|1\n(Adjectif)|avili|rabaissé\nmalformé\nbeau|2\n(Adjectif)|joli|charmant\n(Nom)|beauté\n");
        assert_eq!(t.len(), 2);
        assert_eq!(
            t.get("Beau").unwrap()[1],
            SynonymGroup {
                category: "Nom".into(),
                words: vec!["beauté".into()]
            }
        );
        assert!(t.get("laid").is_none());
    }
}
