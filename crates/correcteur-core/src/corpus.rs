//! Corpus annoté et mesure de la qualité.
//!
//! Une ligne de corpus : `étiquette<TAB>phrase<TAB>source`. L'étiquette est un
//! identifiant de règle (corpus de règles) ou une catégorie (corpus
//! d'évaluation). Dans la phrase, chaque erreur attendue est notée
//! `⟦texte fautif→correction⟧` ; la correction est facultative
//! (`⟦texte fautif⟧`). Une ligne sans annotation est une phrase correcte.
//! `{nbsp}` et `{fine}` désignent les espaces insécables U+00A0 et U+202F.

use std::collections::BTreeMap;

use crate::{Category, Engine, Issue, Options};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub start: usize,
    pub end: usize,
    /// Correction attendue en tête des suggestions, si précisée.
    pub correction: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CorpusLine {
    pub file: String,
    pub line: usize,
    pub label: String,
    /// Phrase sans les marques d'annotation.
    pub text: String,
    pub annotations: Vec<Annotation>,
    pub source: String,
}

fn unescape(s: &str) -> String {
    s.replace("{nbsp}", "\u{00A0}")
        .replace("{fine}", "\u{202F}")
}

pub fn parse(file: &str, content: &str) -> Result<Vec<CorpusLine>, String> {
    let mut lines = Vec::new();
    for (n, raw) in crate::data::data_lines(content) {
        let at = |m: &str| format!("{file}:{n} : {m}");
        let cols: Vec<&str> = raw.split('\t').collect();
        if cols.len() != 3 {
            return Err(at(&format!(
                "3 colonnes attendues, {} trouvées",
                cols.len()
            )));
        }
        if cols[2].trim().is_empty() {
            return Err(at("source manquante"));
        }
        let mut text = String::new();
        let mut annotations = Vec::new();
        let mut rest = cols[1];
        while let Some(open) = rest.find('⟦') {
            text.push_str(&unescape(&rest[..open]));
            let after = &rest[open + '⟦'.len_utf8()..];
            let close = after.find('⟧').ok_or_else(|| at("⟦ sans ⟧"))?;
            let inner = &after[..close];
            let (wrong, correction) = match inner.split_once('→') {
                Some((w, c)) => (w, Some(unescape(c))),
                None => (inner, None),
            };
            let start = text.len();
            text.push_str(&unescape(wrong));
            annotations.push(Annotation {
                start,
                end: text.len(),
                correction,
            });
            rest = &after[close + '⟧'.len_utf8()..];
        }
        text.push_str(&unescape(rest));
        lines.push(CorpusLine {
            file: file.to_string(),
            line: n,
            label: cols[0].to_string(),
            text,
            annotations,
            source: cols[2].to_string(),
        });
    }
    Ok(lines)
}

/// Écart entre ce qu'attend une ligne du corpus de règles et ce que trouve le moteur.
pub fn check_rule_line(
    engine: &Engine,
    line: &CorpusLine,
    options: &Options,
) -> Result<(), String> {
    let issues = engine.check(&line.text, options);
    let describe = |i: &Issue| {
        format!(
            "{} « {} » → {:?}",
            i.rule,
            &line.text[i.start..i.end],
            i.suggestions.first()
        )
    };
    let mut problems = Vec::new();
    for a in &line.annotations {
        let found = issues
            .iter()
            .find(|i| i.start == a.start && i.end == a.end && i.rule == line.label);
        match (found, &a.correction) {
            (None, _) => problems.push(format!(
                "non détecté : « {} » ({})",
                &line.text[a.start..a.end],
                line.label
            )),
            (Some(i), Some(c)) if i.suggestions.first() != Some(c) => problems.push(format!(
                "suggestion {:?} au lieu de {:?}",
                i.suggestions.first(),
                c
            )),
            _ => {}
        }
    }
    for i in &issues {
        let expected = line
            .annotations
            .iter()
            .any(|a| i.start == a.start && i.end == a.end && i.rule == line.label);
        if !expected {
            problems.push(format!("signalement inattendu : {}", describe(i)));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{}:{} « {} »\n    {}",
            line.file,
            line.line,
            line.text,
            problems.join("\n    ")
        ))
    }
}

/// Comptes par catégorie.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub true_positives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    /// Vrais positifs dont la première suggestion est la correction attendue.
    pub good_first_suggestion: usize,
    /// Vrais positifs pour lesquels une correction était annotée.
    pub with_correction: usize,
}

impl Counts {
    pub fn precision(&self) -> Option<f64> {
        let d = self.true_positives + self.false_positives;
        (d > 0).then(|| self.true_positives as f64 / d as f64)
    }

    pub fn recall(&self) -> Option<f64> {
        let d = self.true_positives + self.false_negatives;
        (d > 0).then(|| self.true_positives as f64 / d as f64)
    }
}

/// Catégorie désignée par une étiquette (nom de catégorie ou identifiant de règle).
pub fn label_category(engine: &Engine, label: &str) -> Option<Category> {
    Category::parse(label).or_else(|| engine.rule(label).map(|r| r.category))
}

/// Précision et rappel par catégorie sur un ensemble de lignes.
pub fn evaluate(
    engine: &Engine,
    lines: &[CorpusLine],
    options: &Options,
) -> Result<BTreeMap<Category, Counts>, String> {
    let mut counts: BTreeMap<Category, Counts> = Category::ALL
        .iter()
        .map(|&c| (c, Counts::default()))
        .collect();
    for line in lines {
        let category = label_category(engine, &line.label).ok_or_else(|| {
            format!(
                "{}:{} : étiquette inconnue « {} »",
                line.file, line.line, line.label
            )
        })?;
        let issues = engine.check(&line.text, options);
        for a in &line.annotations {
            let c = counts.get_mut(&category).expect("toutes les catégories");
            match issues
                .iter()
                .find(|i| i.category == category && i.start == a.start && i.end == a.end)
            {
                Some(i) => {
                    c.true_positives += 1;
                    if let Some(corr) = &a.correction {
                        c.with_correction += 1;
                        if i.suggestions.first() == Some(corr) {
                            c.good_first_suggestion += 1;
                        }
                    }
                }
                None => c.false_negatives += 1,
            }
        }
        for i in &issues {
            let expected = category == i.category
                && line
                    .annotations
                    .iter()
                    .any(|a| a.start == i.start && a.end == i.end);
            if !expected {
                counts
                    .get_mut(&i.category)
                    .expect("toutes les catégories")
                    .false_positives += 1;
            }
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotations() {
        let lines = parse("t", "R\tLe ⟦chein→chien⟧ dort{fine}⟦ →{nbsp}⟧!\tx\n").unwrap();
        let l = &lines[0];
        assert_eq!(l.text, "Le chein dort\u{202F} !");
        assert_eq!(l.annotations.len(), 2);
        assert_eq!(
            &l.text[l.annotations[0].start..l.annotations[0].end],
            "chein"
        );
        assert_eq!(l.annotations[1].correction.as_deref(), Some("\u{00A0}"));
    }
}
