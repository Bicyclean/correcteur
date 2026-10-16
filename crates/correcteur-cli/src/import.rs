//! Conversion de Lexique 3.83 (lexique.org, CC BY-SA 4.0) au format du projet.
//!
//! Colonnes lues : `ortho`, `lemme`, `cgram`, `genre`, `nombre`, `freqfilms2`,
//! `freqlivres`, `infover`. Une ligne verbale qui porte plusieurs analyses
//! (`ind:pre:3s;par:pas;`) donne une lecture par analyse ; la fréquence est
//! portée par la première seulement, pour que leur somme reste celle de la forme.

use std::collections::BTreeSet;

pub const SOURCE: &str = "LEXIQUE383";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub lines: usize,
    pub readings: usize,
    /// Lignes écartées : catégorie absente ou inutile (liaison), forme à espaces.
    pub skipped: usize,
}

/// Convertit le contenu de `Lexique383.tsv` en lignes au format `data/lexique`.
///
/// `ligature_stems` : radicaux pour lesquels « oe » s'écrit « œ » (Lexique n'emploie pas la ligature).
pub fn convert_lexique383(
    content: &str,
    ligature_stems: &[&str],
) -> Result<(Vec<String>, Report), String> {
    let mut lines = content.lines();
    let header: Vec<&str> = lines.next().ok_or("fichier vide")?.split('\t').collect();
    let col = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .ok_or_else(|| format!("colonne « {name} » absente"))
    };
    let (c_ortho, c_lemme, c_cgram, c_genre, c_nombre, c_ffilms, c_flivres, c_infover) = (
        col("ortho")?,
        col("lemme")?,
        col("cgram")?,
        col("genre")?,
        col("nombre")?,
        col("freqfilms2")?,
        col("freqlivres")?,
        col("infover")?,
    );

    let mut report = Report::default();
    let mut out = BTreeSet::new();
    for (n, line) in lines.enumerate() {
        report.lines += 1;
        let cols: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        let get = |i: usize| cols.get(i).copied().unwrap_or("");
        let form = ligatures(&get(c_ortho).replace('\'', "’"), ligature_stems);
        let lemma = ligatures(&get(c_lemme).replace('\'', "’"), ligature_stems);
        let Some(pos) = map_pos(get(c_cgram)) else {
            report.skipped += 1;
            continue;
        };
        if form.is_empty() || form.contains(' ') {
            report.skipped += 1;
            continue;
        }
        let parse_freq = |i: usize| -> Result<f64, String> {
            let v = get(i);
            if v.is_empty() {
                return Ok(0.0);
            }
            v.parse()
                .map_err(|_| format!("ligne {} : fréquence « {v} » invalide", n + 2))
        };
        let freq = (parse_freq(c_ffilms)? + parse_freq(c_flivres)?) / 2.0;
        let gn = gender_number(get(c_genre), get(c_nombre));

        let traits: Vec<String> = if pos == "VER" || pos == "AUX" {
            let mut seen = Vec::new();
            for analysis in get(c_infover).split(';').filter(|a| !a.is_empty()) {
                let t = verb_traits(analysis, &gn)
                    .ok_or_else(|| format!("ligne {} : analyse « {analysis} » inconnue", n + 2))?;
                if !seen.contains(&t) {
                    seen.push(t);
                }
            }
            if seen.is_empty() {
                seen.push(String::new());
            }
            seen
        } else if matches!(pos, "NOM" | "ADJ" | "DET") {
            vec![gn.clone()]
        } else {
            vec![String::new()]
        };

        for (k, t) in traits.iter().enumerate() {
            let f = if k == 0 { freq } else { 0.0 };
            out.insert(format!(
                "{form}\t{lemma}\t{pos}\t{t}\t{}\t{SOURCE}",
                fmt_freq(f)
            ));
            report.readings += 1;
        }
    }
    Ok((out.into_iter().collect(), report))
}

fn ligatures(word: &str, stems: &[&str]) -> String {
    if stems.iter().any(|s| word.contains(s)) {
        word.replace("oe", "œ")
    } else {
        word.to_string()
    }
}

fn fmt_freq(f: f64) -> String {
    let s = format!("{f:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Catégories de Lexique → catégories du projet. `None` : ligne écartée.
fn map_pos(cgram: &str) -> Option<&'static str> {
    Some(match cgram {
        "NOM" => "NOM",
        "VER" => "VER",
        "AUX" => "AUX",
        "ADJ" => "ADJ",
        "ADV" => "ADV",
        "PRE" => "PRE",
        "CON" => "CON",
        "ONO" => "INT",
        c if c.starts_with("PRO") => "PRO",
        c if c.starts_with("ART") => "DET",
        // Possessifs, démonstratifs, interrogatifs, indéfinis, numéraux.
        c if c.starts_with("ADJ:") => "DET",
        _ => return None,
    })
}

/// Genre et nombre au format `m.s` ; « e » quand Lexique ne précise pas.
fn gender_number(genre: &str, nombre: &str) -> String {
    let g = if genre.is_empty() { "e" } else { genre };
    let n = if nombre.is_empty() { "e" } else { nombre };
    format!("{g}.{n}")
}

/// `ind:pre:3s` → `ind.pres.3s` ; `par:pas` → `ppas.m.s` ; `inf` → `inf`.
fn verb_traits(analysis: &str, gn: &str) -> Option<String> {
    let parts: Vec<&str> = analysis.split(':').collect();
    let mode = *parts.first()?;
    let tense = parts.get(1).copied().unwrap_or("");
    let person = parts.get(2).copied().unwrap_or("");
    Some(match (mode, tense) {
        ("inf", _) => "inf".into(),
        ("par", "pas") => {
            if gn == "e.e" {
                "ppas".into()
            } else {
                format!("ppas.{gn}")
            }
        }
        ("par", "pre") => "ppre".into(),
        _ => {
            let mode = match mode {
                "ind" | "sub" | "imp" => mode,
                "cnd" => "cond",
                _ => return None,
            };
            let tense = match tense {
                "pre" => "pres",
                "imp" => "impf",
                "pas" => "ps",
                "fut" => "fut",
                _ => return None,
            };
            format!("{mode}.{tense}.{person}")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "ortho\tphon\tlemme\tcgram\tgenre\tnombre\tfreqlemfilms2\tfreqlemlivres\tfreqfilms2\tfreqlivres\tinfover\n\
        fait\tfE\tfaire\tVER\tm\ts\t0\t0\t2751.99\t1459.27\tind:pre:3s;par:pas;par:pas;\n\
        fait\tfE\tfait\tNOM\tm\ts\t0\t0\t384.71\t325.27\t\n\
        soeur\tsoeR\tsoeur\tNOM\tf\ts\t0\t0\t1\t1\t\n\
        moelle\tmwal\tmoelle\tNOM\tf\ts\t0\t0\t1\t1\t\n\
        l'\tl\tl'\tART:def\tm\ts\t0\t0\t8129.41\t12746.77\t\n\
        l'\tl\tl'\tLIA\t\t\t0\t0\t0\t412.57\t\n\
        a priori\ta\ta priori\tADV\t\t\t0\t0\t1\t1\t\n\
        lave\tlav\tlaver\tVER\t\t\t0\t0\t16.98\t7.64\timp:pre:2s;ind:pre:1s;cnd:pre:3s;\n";

    #[test]
    fn conversion() {
        let (lines, report) = convert_lexique383(SAMPLE, &["soeur"]).unwrap();
        assert_eq!(report.lines, 8);
        assert!(lines.contains(&"sœur\tsœur\tNOM\tf.s\t1\tLEXIQUE383".to_string()));
        assert!(lines.contains(&"moelle\tmoelle\tNOM\tf.s\t1\tLEXIQUE383".to_string()));
        assert_eq!(report.skipped, 2);
        assert!(lines.contains(&"fait\tfaire\tVER\tind.pres.3s\t2105.63\tLEXIQUE383".to_string()));
        assert!(lines.contains(&"fait\tfaire\tVER\tppas.m.s\t0\tLEXIQUE383".to_string()));
        assert!(lines.contains(&"fait\tfait\tNOM\tm.s\t354.99\tLEXIQUE383".to_string()));
        assert!(lines.contains(&"l’\tl’\tDET\tm.s\t10438.09\tLEXIQUE383".to_string()));
        assert!(lines.contains(&"lave\tlaver\tVER\tcond.pres.3s\t0\tLEXIQUE383".to_string()));
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.starts_with("fait\tfaire"))
                .count(),
            2
        );
    }

    #[test]
    fn analyse_inconnue() {
        let bad = "ortho\tlemme\tcgram\tgenre\tnombre\tfreqfilms2\tfreqlivres\tinfover\n\
                   x\tx\tVER\t\t\t0\t0\tzzz:pre:1s;\n";
        assert!(convert_lexique383(bad, &[]).is_err());
    }
}
