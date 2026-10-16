//! Lexique des formes fléchies.
//!
//! Format TSV, six colonnes : `forme`, `lemme`, `catégorie`, `traits`,
//! `fréquence` (occurrences par million de mots), `source`. Une forme
//! homographe occupe plusieurs lignes (une par lecture).

use std::collections::HashMap;

use crate::data::{data_lines, DataError};
use crate::text::{base_letter, normalize_apostrophes};

/// Catégorie grammaticale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pos {
    Nom,
    NomPropre,
    Adj,
    Ver,
    /// Auxiliaires être et avoir.
    Aux,
    Adv,
    Pre,
    Con,
    Det,
    Pro,
    Int,
    Abr,
}

impl Pos {
    pub fn parse(s: &str) -> Option<Pos> {
        Some(match s {
            "NOM" => Pos::Nom,
            "NPR" => Pos::NomPropre,
            "ADJ" => Pos::Adj,
            "VER" => Pos::Ver,
            "AUX" => Pos::Aux,
            "ADV" => Pos::Adv,
            "PRE" => Pos::Pre,
            "CON" => Pos::Con,
            "DET" => Pos::Det,
            "PRO" => Pos::Pro,
            "INT" => Pos::Int,
            "ABR" => Pos::Abr,
            _ => return None,
        })
    }

    pub fn is_verb(self) -> bool {
        matches!(self, Pos::Ver | Pos::Aux)
    }
}

/// Une lecture possible d'une forme.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub lemma: String,
    pub pos: Pos,
    /// Traits morphologiques, par exemple `m.s`, `inf`, `ppas.f.p`, `ind.pres.3s`.
    pub traits: String,
}

impl Reading {
    pub fn is_infinitive(&self) -> bool {
        self.pos.is_verb() && self.traits == "inf"
    }

    pub fn is_past_participle(&self) -> bool {
        self.pos.is_verb() && self.traits.starts_with("ppas")
    }

    /// Verbe conjugué (ni infinitif ni participe).
    pub fn is_finite_verb(&self) -> bool {
        self.pos.is_verb()
            && !self.traits.starts_with("inf")
            && !self.traits.starts_with("ppas")
            && !self.traits.starts_with("ppre")
    }
}

#[derive(Debug, Clone)]
pub struct Form {
    pub text: String,
    /// Somme des fréquences de toutes les lectures.
    pub freq: f32,
    pub readings: Vec<Reading>,
}

impl Form {
    pub fn has(&self, pred: impl Fn(&Reading) -> bool) -> bool {
        self.readings.iter().any(pred)
    }

    pub fn only(&self, pred: impl Fn(&Reading) -> bool) -> bool {
        !self.readings.is_empty() && self.readings.iter().all(pred)
    }

    pub fn has_pos(&self, pos: Pos) -> bool {
        self.has(|r| r.pos == pos)
    }
}

#[derive(Debug, Default)]
pub struct Lexicon {
    forms: Vec<Form>,
    by_text: HashMap<String, u32>,
    /// Formes contenant une majuscule, indexées en minuscules.
    by_lower: HashMap<String, Vec<u32>>,
    /// Formes indexées par squelette (voir [`skeleton`]).
    by_skeleton: HashMap<String, Vec<u32>>,
}

impl Lexicon {
    pub fn from_tsv_files(files: &[(String, String)]) -> Result<Lexicon, DataError> {
        let mut lex = Lexicon::default();
        for (name, content) in files {
            lex.add_tsv(name, content)?;
        }
        Ok(lex)
    }

    fn add_tsv(&mut self, name: &str, content: &str) -> Result<(), DataError> {
        for (n, line) in data_lines(content) {
            let err = |message: String| DataError {
                file: name.to_string(),
                line: n,
                message,
            };
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 6 {
                return Err(err(format!(
                    "6 colonnes attendues, {} trouvées",
                    cols.len()
                )));
            }
            let pos = Pos::parse(cols[2])
                .ok_or_else(|| err(format!("catégorie « {} » inconnue", cols[2])))?;
            let freq: f32 = cols[4]
                .parse()
                .map_err(|_| err(format!("fréquence « {} » invalide", cols[4])))?;
            if cols[5].trim().is_empty() {
                return Err(err("source manquante".into()));
            }
            let text = normalize_apostrophes(cols[0]);
            let reading = Reading {
                lemma: normalize_apostrophes(cols[1]),
                pos,
                traits: cols[3].to_string(),
            };
            self.insert(text, reading, freq);
        }
        Ok(())
    }

    fn insert(&mut self, text: String, reading: Reading, freq: f32) {
        if let Some(&id) = self.by_text.get(&text) {
            let form = &mut self.forms[id as usize];
            form.freq += freq;
            if !form.readings.contains(&reading) {
                form.readings.push(reading);
            }
            return;
        }
        let id = self.forms.len() as u32;
        let lower = text.to_lowercase();
        if lower != text {
            self.by_lower.entry(lower).or_default().push(id);
        }
        self.by_skeleton
            .entry(skeleton(&text))
            .or_default()
            .push(id);
        self.by_text.insert(text.clone(), id);
        self.forms.push(Form {
            text,
            freq,
            readings: vec![reading],
        });
    }

    pub fn len(&self) -> usize {
        self.forms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.forms.is_empty()
    }

    pub fn form(&self, id: u32) -> &Form {
        &self.forms[id as usize]
    }

    /// Recherche exacte (casse et accents compris).
    pub fn get(&self, text: &str) -> Option<&Form> {
        self.by_text.get(text).map(|&id| self.form(id))
    }

    /// Recherche tolérante à la majuscule de début de phrase, aux mots en
    /// capitales et à « oe » saisi à la place de la ligature « œ ».
    pub fn find(&self, text: &str) -> Option<&Form> {
        self.find_case(text).or_else(|| {
            let lig = text
                .replace("oe", "œ")
                .replace("Oe", "Œ")
                .replace("OE", "Œ");
            (lig != text).then(|| self.find_case(&lig)).flatten()
        })
    }

    fn find_case(&self, text: &str) -> Option<&Form> {
        if let Some(f) = self.get(text) {
            return Some(f);
        }
        let mut chars = text.chars();
        let first = chars.next()?;
        if first.is_uppercase() {
            let decap: String = first.to_lowercase().chain(chars).collect();
            if let Some(f) = self.get(&decap) {
                return Some(f);
            }
            let lower = text.to_lowercase();
            if lower != decap {
                if let Some(f) = self.get(&lower) {
                    return Some(f);
                }
            }
        }
        None
    }

    /// Identifiants des formes qui s'écrivent `text` en minuscules.
    pub(crate) fn ids_for_lower(&self, lower: &str, out: &mut Vec<u32>) {
        if let Some(&id) = self.by_text.get(lower) {
            out.push(id);
        }
        if let Some(ids) = self.by_lower.get(lower) {
            out.extend_from_slice(ids);
        }
    }

    pub(crate) fn ids_for_skeleton(&self, skel: &str) -> &[u32] {
        self.by_skeleton.get(skel).map_or(&[], Vec::as_slice)
    }
}

/// Squelette d'un mot : minuscules, sans accents, lettres doublées réduites.
/// Deux mots qui ne diffèrent que par ces points ont le même squelette.
pub fn skeleton(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut last = None;
    for c in word.chars().flat_map(char::to_lowercase) {
        let pieces: &[char] = match c {
            'œ' => &['o', 'e'],
            'æ' => &['a', 'e'],
            _ => &[],
        };
        let mut push = |b: char| {
            if last != Some(b) {
                out.push(b);
                last = Some(b);
            }
        };
        if pieces.is_empty() {
            push(base_letter(c));
        } else {
            pieces.iter().for_each(|&p| push(p));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex() -> Lexicon {
        let tsv = "maison\tmaison\tNOM\tf.s\t300\ttest\n\
                   Paris\tParis\tNPR\t\t200\ttest\n\
                   été\tété\tNOM\tm.s\t100\ttest\n\
                   été\têtre\tAUX\tppas\t500\ttest\n";
        Lexicon::from_tsv_files(&[("t".into(), tsv.into())]).unwrap()
    }

    #[test]
    fn recherche() {
        let l = lex();
        assert!(l.find("Maison").is_some());
        assert!(l.find("MAISON").is_some());
        assert!(l.find("paris").is_none());
        let ete = l.get("été").unwrap();
        assert_eq!(ete.readings.len(), 2);
        assert_eq!(ete.freq, 600.0);
    }

    #[test]
    fn squelettes() {
        assert_eq!(skeleton("Éléphant"), "elephant");
        assert_eq!(skeleton("appeller"), "apeler");
        assert_eq!(skeleton("cœur"), "coeur");
    }
}
