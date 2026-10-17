//! `correcteur` : analyse un texte, évalue le moteur sur le corpus, mesure les temps.
//!
//! ```text
//! correcteur analyser [--donnees DOSSIER] [--style] [--json] [FICHIER]   (entrée standard par défaut)
//! correcteur evaluer  [--donnees DOSSIER] [--corpus DOSSIER] [--sortie FICHIER.md]
//! correcteur mesurer  [--donnees DOSSIER] [--mots N] [--tours N]
//! correcteur importer-lexique383 Lexique383.tsv [--sortie FICHIER.tsv]
//! correcteur attribuer-conjugaisons [--donnees DOSSIER]
//! correcteur ecarter-lectures [--donnees DOSSIER]
//! correcteur conjuguer VERBE [--donnees DOSSIER]
//! correcteur synonymes MOT [--donnees DOSSIER]
//! bzcat frwiktionary-…-pages-articles.xml.bz2 | correcteur importer-antonymes [--sortie FICHIER.tsv]
//! ```

mod antonymes;
mod conjugaison;
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
    style: bool,
    /// `analyser --json` : problèmes au format JSON du moteur (positions en octets).
    json: bool,
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
        style: false,
        json: false,
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
            "--style" => a.style = true,
            "--json" => a.json = true,
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
    if command == "importer-antonymes" {
        return import_antonyms(&a);
    }
    let engine = Engine::from_dir(&a.data).map_err(|e| e.to_string())?;
    match command.as_str() {
        "analyser" => analyze(&engine, &a),
        "evaluer" => eval(&engine, &a),
        "mesurer" => bench(&engine, &a),
        "conjuguer" => conjugate(&engine, &a),
        "synonymes" => synonyms(&engine, &a),
        "attribuer-conjugaisons" => assign_conjugations(&engine, &a),
        "ecarter-lectures" => discard_readings(&engine, &a),
        other => Err(format!("commande inconnue « {other} »")),
    }
}

fn analyze(engine: &Engine, a: &Args) -> Result<ExitCode, String> {
    let text = if let Some(path) = a.positional.first() {
        std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?
    } else {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        s
    };
    let mut options = Options::default();
    if a.style {
        options.disabled_categories.clear();
        let r = engine.readability(&text);
        println!(
            "lisibilité : {:.0} ({}) — {} mots, {} phrases, {:.1} mots par phrase",
            r.score,
            r.level,
            r.words,
            r.sentences,
            r.words as f64 / r.sentences.max(1) as f64
        );
    }
    let issues = engine.check(&text, &options);
    if a.json {
        println!("{}", correcteur_core::interop::issues_to_json(&issues));
        return Ok(if issues.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        });
    }
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

fn conjugate(engine: &Engine, a: &Args) -> Result<ExitCode, String> {
    let verb = a.positional.join(" ");
    let Some(c) = engine.conjugate(&verb) else {
        return Err(format!("« {verb} » : verbe inconnu"));
    };
    println!(
        "{} — modèle « {} », auxiliaire {}{}",
        c.infinitive,
        c.model,
        c.auxiliary,
        if c.guessed {
            " (modèle déduit de la terminaison)"
        } else {
            ""
        }
    );
    for mode in &c.modes {
        println!("\n{}", mode.name.to_uppercase());
        for t in &mode.tenses {
            println!("  {} : {}", t.name, t.lines.join(" · "));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn assign_conjugations(engine: &Engine, a: &Args) -> Result<ExitCode, String> {
    let models_path = a.data.join("conjugaison/modeles.txt");
    let models = std::fs::read_to_string(&models_path)
        .map_err(|e| format!("{} : {e}", models_path.display()))?;
    let models = correcteur_core::conjugation::Models::parse(&models).map_err(|e| e.to_string())?;
    let assignments = conjugaison::assign(engine.lexicon(), &models);
    let (text, summary) = conjugaison::render(&assignments);
    let out = a
        .output
        .clone()
        .unwrap_or_else(|| a.data.join("conjugaison/verbes.tsv"));
    std::fs::write(&out, text).map_err(|e| format!("{} : {e}", out.display()))?;
    println!("{summary}\nécrit dans {}", out.display());
    Ok(ExitCode::SUCCESS)
}

/// Retire du lexique dérivé de Lexique 3.83 les lectures verbales que le
/// modèle de conjugaison du verbe contredit (« lui » présent de *luire*, « n’ »
/// présent de *avoir*…) : formes conjuguées et infinitifs, quand la forme
/// garde au moins une autre lecture. Les
/// lignes retirées sont consignées dans `lexique/lectures-ecartees.txt` (pas en
/// `.tsv` : tous les `.tsv` du dossier sont chargés comme lexiques).
fn discard_readings(engine: &Engine, a: &Args) -> Result<ExitCode, String> {
    let models_path = a.data.join("conjugaison/modeles.txt");
    let models = std::fs::read_to_string(&models_path)
        .map_err(|e| format!("{} : {e}", models_path.display()))?;
    let models = correcteur_core::conjugation::Models::parse(&models).map_err(|e| e.to_string())?;
    let lexicon = engine.lexicon();

    // Forme → lectures (lemme, traits) à retirer.
    let mut by_form: std::collections::BTreeMap<String, Vec<(String, String)>> = Default::default();
    for asg in conjugaison::assign(lexicon, &models) {
        // Formes conjuguées et infinitifs seulement : un participe contredit est
        // souvent une variante correcte que le modèle ne produit pas (« inclus »).
        let finite = |t: &str| {
            t == "inf"
                || t.rsplit('.')
                    .next()
                    .is_some_and(|p| p.starts_with(['1', '2', '3']))
        };
        for (form, traits) in asg.contradicted.into_iter().filter(|(_, t)| finite(t)) {
            by_form
                .entry(form)
                .or_default()
                .push((asg.verb.clone(), traits));
        }
    }
    let mut discard: std::collections::HashSet<(String, String, String)> = Default::default();
    let mut kept_alone = 0;
    for (form, drops) in &by_form {
        let Some(f) = lexicon.get(form) else { continue };
        let remaining = f
            .readings
            .iter()
            .filter(|r| {
                !(r.pos.is_verb() && drops.iter().any(|(l, t)| *l == r.lemma && *t == r.traits))
            })
            .count();
        // Seule lecture de la forme : la garder, sinon le mot deviendrait inconnu.
        if remaining == 0 {
            kept_alone += 1;
            continue;
        }
        for (lemma, traits) in drops {
            discard.insert((form.clone(), lemma.clone(), traits.clone()));
        }
    }

    let path = a.data.join("lexique/10-lexique383.tsv");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{} : {e}", path.display()))?;
    let (mut kept, mut removed) = (String::new(), Vec::new());
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        let hit = cols.len() >= 4
            && matches!(cols[2], "VER" | "AUX")
            && discard.contains(&(
                cols[0].to_string(),
                cols[1].to_string(),
                cols[3].to_string(),
            ));
        if hit {
            removed.push(line);
        } else {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    std::fs::write(&path, kept).map_err(|e| format!("{} : {e}", path.display()))?;

    let log_path = a.data.join("lexique/lectures-ecartees.txt");
    let mut log = std::fs::read_to_string(&log_path).unwrap_or_else(|_| {
        "# Lectures verbales de Lexique 3.83 (CC BY-SA 4.0) retirées de 10-lexique383.tsv par\n\
         # `correcteur ecarter-lectures` : le modèle de conjugaison du verbe les contredit.\n\
         # Mêmes colonnes que 10-lexique383.tsv. Ce fichier dérivé est distribué sous la même licence.\n"
            .to_string()
    });
    for line in &removed {
        log.push_str(line);
        log.push('\n');
    }
    std::fs::write(&log_path, log).map_err(|e| format!("{} : {e}", log_path.display()))?;
    println!(
        "{} lectures retirées (consignées dans {}) ; {} formes gardées car sans autre lecture",
        removed.len(),
        log_path.display(),
        kept_alone
    );
    Ok(ExitCode::SUCCESS)
}

fn synonyms(engine: &Engine, a: &Args) -> Result<ExitCode, String> {
    let word = a.positional.join(" ");
    let Some((entry, groups)) = engine.synonyms(&word) else {
        return Err(format!("« {word} » : aucun synonyme connu"));
    };
    println!("{entry}");
    for g in groups {
        println!("  ({}) {}", g.category, g.words.join(", "));
    }
    Ok(ExitCode::SUCCESS)
}

/// Lit l'export XML du Wiktionnaire sur l'entrée standard (ou dans le fichier donné).
fn import_antonyms(a: &Args) -> Result<ExitCode, String> {
    let (antonyms, report) = match a.positional.first() {
        Some(path) => {
            let f = std::fs::File::open(path).map_err(|e| format!("{path} : {e}"))?;
            antonymes::extract(std::io::BufReader::new(f))
        }
        None => antonymes::extract(std::io::stdin().lock()),
    }
    .map_err(|e| e.to_string())?;
    let out = a
        .output
        .clone()
        .unwrap_or_else(|| a.data.join("thesaurus/antonymes.tsv"));
    std::fs::write(&out, antonymes::render(&antonyms))
        .map_err(|e| format!("{} : {e}", out.display()))?;
    println!(
        "{} pages lues ; {} entrées, {} antonymes écrits dans {}",
        report.pages,
        report.entries,
        report.antonyms,
        out.display()
    );
    Ok(ExitCode::SUCCESS)
}
