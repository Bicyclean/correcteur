//! `correcteur` : analyse un texte, évalue le moteur sur le corpus, mesure les temps.
//!
//! ```text
//! correcteur analyser [--donnees DOSSIER] [FICHIER]     (entrée standard par défaut)
//! correcteur evaluer  [--donnees DOSSIER] [--corpus DOSSIER] [--sortie FICHIER.md]
//! correcteur mesurer  [--donnees DOSSIER] [--mots N] [--tours N]
//! correcteur importer-lexique383 Lexique383.tsv [--sortie FICHIER.tsv]
//! ```

mod import;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use correcteur_core::corpus::{evaluate, parse, CorpusLine};
use correcteur_core::{Category, Engine, Options};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("erreur : {e}");
            ExitCode::from(2)
        }
    }
}

struct Args {
    positional: Vec<String>,
    data: PathBuf,
    corpus: PathBuf,
    output: Option<PathBuf>,
    words: usize,
    rounds: usize,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        positional: Vec::new(),
        data: std::env::var_os("CORRECTEUR_DONNEES")
            .map_or_else(|| PathBuf::from("data"), PathBuf::from),
        corpus: PathBuf::from("corpus/fr/evaluation"),
        output: None,
        words: 500,
        rounds: 50,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("valeur manquante après {arg}"))
        };
        match arg.as_str() {
            "--donnees" => a.data = value()?.into(),
            "--corpus" => a.corpus = value()?.into(),
            "--sortie" => a.output = Some(value()?.into()),
            "--mots" => a.words = value()?.parse().map_err(|_| "--mots : nombre attendu")?,
            "--tours" => a.rounds = value()?.parse().map_err(|_| "--tours : nombre attendu")?,
            _ => a.positional.push(arg.clone()),
        }
    }
    Ok(a)
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some(command) = args.first() else {
        return Err("commande attendue : analyser, evaluer, mesurer ou importer-lexique383".into());
    };
    let a = parse_args(&args[1..])?;
    if command == "importer-lexique383" {
        return import_lexique383(&a);
    }
    let engine = Engine::from_dir(&a.data).map_err(|e| e.to_string())?;
    match command.as_str() {
        "analyser" => analyze(&engine, &a),
        "evaluer" => eval(&engine, &a),
        "mesurer" => bench(&engine, &a),
        other => Err(format!("commande inconnue « {other} »")),
    }
}

fn analyze(engine: &Engine, a: &Args) -> Result<ExitCode, String> {
    let text = match a.positional.first() {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?,
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| e.to_string())?;
            s
        }
    };
    let issues = engine.check(&text, &Options::default());
    for i in &issues {
        let (line, col) = line_col(&text, i.start);
        // Les espaces insécables sont rendues visibles.
        let show = |s: &str| s.replace(['\u{a0}', '\u{202f}'], "⍽");
        let found = show(&text[i.start..i.end]);
        let sugg: Vec<String> = i
            .suggestions
            .iter()
            .map(|s| format!("« {} »", show(s)))
            .collect();
        println!(
            "{line}:{col} [{}] {} « {found} » {}",
            i.category.as_str(),
            i.message,
            if sugg.is_empty() {
                String::new()
            } else {
                format!("→ {}", sugg.join(", "))
            }
        );
    }
    Ok(if issues.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn line_col(text: &str, pos: usize) -> (usize, usize) {
    let before = &text[..pos];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (line, col)
}

fn read_corpus(dir: &Path) -> Result<Vec<CorpusLine>, String> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{} : {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "tsv"))
        .collect();
    paths.sort();
    let mut lines = Vec::new();
    for p in paths {
        let content = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
        lines.extend(parse(&p.display().to_string(), &content)?);
    }
    Ok(lines)
}

fn eval(engine: &Engine, a: &Args) -> Result<ExitCode, String> {
    let lines = read_corpus(&a.corpus)?;
    let counts = evaluate(engine, &lines, &Options::default())?;
    let pct = |v: Option<f64>| v.map_or("—".to_string(), |v| format!("{:.1} %", v * 100.0));
    let mut md = String::new();
    md.push_str(&format!(
        "Corpus : `{}` ({} phrases), lexique : {} formes.\n\n",
        a.corpus.display(),
        lines.len(),
        engine.lexicon().len()
    ));
    md.push_str("| Catégorie | VP | FP | FN | Précision | Rappel | 1re suggestion juste |\n");
    md.push_str("|---|---:|---:|---:|---:|---:|---:|\n");
    for c in Category::ALL {
        let n = counts[&c];
        let first = if n.with_correction > 0 {
            format!("{}/{}", n.good_first_suggestion, n.with_correction)
        } else {
            "—".into()
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            c.as_str(),
            n.true_positives,
            n.false_positives,
            n.false_negatives,
            pct(n.precision()),
            pct(n.recall()),
            first
        ));
    }
    print!("{md}");
    if let Some(out) = &a.output {
        std::fs::write(out, &md).map_err(|e| format!("{} : {e}", out.display()))?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Texte de démonstration répété jusqu'à `words` mots, avec quelques fautes.
fn sample_text(words: usize) -> String {
    const BASE: &str =
        "Le chat dort dans la maison pendant que les enfants jouent dans le jardin. \
        Il à mangé du pain , puis il ce lave les mains. Quelle belle journée! \
        Nous partons maintenat à la gare de Lyon avec mon frère et ma sœur. ";
    let mut out = String::new();
    while out.split_whitespace().count() < words {
        out.push_str(BASE);
    }
    out.split_whitespace()
        .take(words)
        .collect::<Vec<_>>()
        .join(" ")
}

fn bench(engine: &Engine, a: &Args) -> Result<ExitCode, String> {
    let text = sample_text(a.words);
    let options = Options::default();
    let mut issues = 0;
    let mut times = Vec::with_capacity(a.rounds);
    for _ in 0..a.rounds.max(1) {
        let t = Instant::now();
        issues = engine.check(&text, &options).len();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    let median = times[times.len() / 2];
    let max = times[times.len() - 1];
    println!(
        "{} mots, {} signalements, {} tours : médiane {:.2} ms, max {:.2} ms",
        a.words,
        issues,
        times.len(),
        median,
        max
    );
    Ok(if median < 100.0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

const LEXIQUE383_HEADER: &str = "\
# Dérivé de Lexique 3.83 (http://www.lexique.org), par Boris New, Christophe Pallier et coll.
# Licence : CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/).
# Ce fichier est distribué sous la même licence. Ne pas modifier à la main :
# régénérer avec `correcteur importer-lexique383 Lexique383.tsv`.
# Modifications : colonnes réduites, catégories et traits convertis, apostrophes
# typographiques, ligature œ (data/regles/ligatures.txt), fréquence = moyenne
# de freqfilms2 et freqlivres.
";

fn import_lexique383(a: &Args) -> Result<ExitCode, String> {
    let input = a
        .positional
        .first()
        .ok_or("chemin de Lexique383.tsv attendu")?;
    let content = std::fs::read_to_string(input).map_err(|e| format!("{input} : {e}"))?;
    let lig_path = a.data.join("regles/ligatures.txt");
    let ligatures =
        std::fs::read_to_string(&lig_path).map_err(|e| format!("{} : {e}", lig_path.display()))?;
    let stems: Vec<&str> = ligatures
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let (lines, report) = import::convert_lexique383(&content, &stems)?;
    let output = a
        .output
        .clone()
        .unwrap_or_else(|| a.data.join("lexique/10-lexique383.tsv"));
    let mut text = String::from(LEXIQUE383_HEADER);
    for l in &lines {
        text.push_str(l);
        text.push('\n');
    }
    std::fs::write(&output, text).map_err(|e| format!("{} : {e}", output.display()))?;
    println!(
        "{} lignes lues, {} écartées, {} lectures et {} mots de locutions écrits dans {}",
        report.lines,
        report.skipped,
        report.readings,
        report.from_locutions,
        output.display()
    );
    Ok(ExitCode::SUCCESS)
}
