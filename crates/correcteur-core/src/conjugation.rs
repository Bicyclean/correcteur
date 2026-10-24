//! Conjugaison : tableau complet d'un verbe à partir de modèles
//! (`data/conjugaison/modeles.txt`, format décrit en tête du fichier).
//!
//! Un modèle ne donne que le présent, le passé simple, le radical du futur et
//! le participe passé ; les autres temps simples s'en déduisent par les règles
//! régulières du français, sauf exception écrite dans le modèle. Les temps
//! composés associent l'auxiliaire et le participe passé.

use std::collections::{HashMap, HashSet};

use crate::data::{data_lines, DataError};
use crate::lexicon::skeleton;
use crate::text::normalize_apostrophes;

/// Une case : une ou plusieurs formes admises, ou rien (verbe défectif).
pub type Cell = Option<Vec<String>>;

#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    /// Terminaison de l'infinitif retirée pour obtenir le radical.
    pub ending: String,
    /// Lignes du fichier : clé (`pres`, `ps`, `fut`, `ppas`, `impf`…) → cases.
    lines: HashMap<String, Vec<String>>,
    /// Seule la 3e personne du singulier existe (verbes impersonnels).
    pub impersonal: bool,
}

/// Formes simples d'un verbe.
#[derive(Debug, Clone, Default)]
pub struct SimpleForms {
    pub infinitive: String,
    /// Temps → cases (6 personnes, 3 pour l'impératif).
    pub tenses: HashMap<&'static str, Vec<Cell>>,
    pub present_participle: Cell,
    /// Masculin singulier, masculin pluriel, féminin singulier, féminin pluriel ;
    /// une seule forme si le participe est invariable.
    pub past_participle: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Models {
    pub models: Vec<Model>,
}

impl Models {
    pub fn parse(content: &str) -> Result<Models, DataError> {
        let mut models: Vec<Model> = Vec::new();
        for (n, line) in data_lines(content) {
            let err = |m: String| DataError {
                file: "conjugaison/modeles.txt".into(),
                line: n,
                message: m,
            };
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["modele", name, ending] => models.push(Model {
                    name: name.to_string(),
                    ending: ending.to_string(),
                    lines: HashMap::new(),
                    impersonal: false,
                }),
                ["defectif", "3s"] => {
                    models
                        .last_mut()
                        .ok_or_else(|| err("« defectif » hors d'un modèle".into()))?
                        .impersonal = true;
                }
                [key, cells @ ..] => {
                    let model = models
                        .last_mut()
                        .ok_or_else(|| err("ligne hors d'un modèle".into()))?;
                    let ok = match *key {
                        "pres" | "ps" | "impf" | "subj" | "subi" | "cond" => cells.len() == 6,
                        "imp" => cells.len() == 3,
                        "fut" | "ppre" => cells.len() == 1,
                        // une forme, une forme suivie de « inv », ou les quatre formes
                        "ppas" => matches!(cells, [_] | [_, "inv"] | [_, _, _, _]),
                        _ => return Err(err(format!("ligne « {key} » inconnue"))),
                    };
                    if !ok {
                        return Err(err(format!("« {key} » : {} cases", cells.len())));
                    }
                    model.lines.insert(
                        key.to_string(),
                        cells.iter().map(std::string::ToString::to_string).collect(),
                    );
                }
                [] => {}
            }
        }
        for m in &models {
            for key in ["pres", "ps", "fut", "ppas"] {
                if !m.lines.contains_key(key) {
                    return Err(DataError {
                        file: "conjugaison/modeles.txt".into(),
                        line: 0,
                        message: format!("modèle « {} » : ligne « {key} » manquante", m.name),
                    });
                }
            }
        }
        Ok(Models { models })
    }

    pub fn get(&self, name: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.name == name)
    }

    /// Modèles applicables à un infinitif (terminaison compatible), de la
    /// terminaison la plus longue à la plus courte.
    pub fn candidates(&self, infinitive: &str) -> Vec<&Model> {
        let mut v: Vec<&Model> = self
            .models
            .iter()
            .filter(|m| infinitive.ends_with(&m.ending))
            .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.ending.chars().count()));
        v
    }
}

/// Ajoute une terminaison en respectant l'orthographe : « ç » et « ge » ne
/// gardent leur cédille ou leur « e » que devant a, o, u (« plaçons », « placions »).
fn join(stem: &str, ending: &str) -> String {
    if ending.starts_with(['i', 'e', 'é', 'è']) {
        if let Some(s) = stem.strip_suffix('ç') {
            return format!("{s}c{ending}");
        }
        if let Some(s) = stem.strip_suffix("ge").filter(|_| ending.starts_with('i')) {
            return format!("{s}g{ending}");
        }
    }
    format!("{stem}{ending}")
}

/// Voyelle finale avec accent circonflexe (« chanta » → « chantâ », « vin » → « vîn »).
fn circumflex(s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    if let Some(i) = chars.iter().rposition(|c| "aiuâîû".contains(*c)) {
        chars[i] = match chars[i] {
            'a' => 'â',
            'i' => 'î',
            'u' => 'û',
            c => c,
        };
    }
    chars.into_iter().collect()
}

const IMPF: [&str; 6] = ["ais", "ais", "ait", "ions", "iez", "aient"];
const FUT: [&str; 6] = ["ai", "as", "a", "ons", "ez", "ont"];

impl Model {
    /// Cases d'une ligne du modèle, préfixées par le radical.
    fn cells(&self, key: &str, base: &str) -> Option<Vec<Cell>> {
        self.lines.get(key).map(|cells| {
            cells
                .iter()
                .map(|c| (c != "-").then(|| c.split('/').map(|v| format!("{base}{v}")).collect()))
                .collect()
        })
    }

    /// Participe passé de `infinitive` selon ce modèle : masculin singulier,
    /// masculin pluriel, féminin singulier, féminin pluriel (une seule forme s'il
    /// est invariable).
    pub fn past_participles(&self, infinitive: &str) -> Option<Vec<String>> {
        let base = infinitive.strip_suffix(&self.ending)?;
        let pp = &self.lines["ppas"];
        Some(match pp.as_slice() {
            [p, inv] if inv == "inv" => vec![format!("{base}{p}")],
            [p] => {
                let m = format!("{base}{p}");
                let mp = if m.ends_with(['s', 'x']) {
                    m.clone()
                } else {
                    format!("{m}s")
                };
                vec![m.clone(), mp, format!("{m}e"), format!("{m}es")]
            }
            four => four.iter().map(|p| format!("{base}{p}")).collect(),
        })
    }

    /// Formes simples du verbe `infinitive` selon ce modèle.
    pub fn conjugate(&self, infinitive: &str) -> Option<SimpleForms> {
        let base = infinitive.strip_suffix(&self.ending)?;
        let first = |cells: &[Cell], i: usize| -> Option<String> {
            cells
                .get(i)
                .and_then(|c| c.as_ref())
                .and_then(|v| v.first().cloned())
        };
        let pres = self.cells("pres", base)?;
        let ps = self.cells("ps", base)?;
        let fut_stems: Vec<String> = self.lines["fut"][0]
            .split('/')
            .map(|v| format!("{base}{v}"))
            .collect();

        // Radical de « nous » au présent (« chantons » → « chant »).
        let nous_stem = first(&pres, 3).and_then(|f| f.strip_suffix("ons").map(str::to_string));
        let derive = |stem: &Option<String>, endings: &[&str]| -> Vec<Cell> {
            endings
                .iter()
                .map(|e| stem.as_ref().map(|s| vec![join(s, e)]))
                .collect()
        };

        let impf = self
            .cells("impf", base)
            .unwrap_or_else(|| derive(&nous_stem, &IMPF));
        let fut: Vec<Cell> = FUT
            .iter()
            .map(|e| Some(fut_stems.iter().map(|s| format!("{s}{e}")).collect()))
            .collect();
        let cond = self.cells("cond", base).unwrap_or_else(|| {
            IMPF.iter()
                .map(|e| Some(fut_stems.iter().map(|s| format!("{s}{e}")).collect()))
                .collect()
        });
        let subj = self.cells("subj", base).unwrap_or_else(|| {
            let ils = first(&pres, 5).and_then(|f| f.strip_suffix("ent").map(str::to_string));
            let mut v = derive(&ils, &["e", "es", "e", "", "", "ent"]);
            v[3] = impf[3].clone();
            v[4] = impf[4].clone();
            v
        });
        let subi = self.cells("subi", base).unwrap_or_else(|| {
            let stem = first(&ps, 1).and_then(|f| f.strip_suffix('s').map(str::to_string));
            match stem {
                Some(s) => vec![
                    Some(vec![format!("{s}sse")]),
                    Some(vec![format!("{s}sses")]),
                    Some(vec![format!("{}t", circumflex(&s))]),
                    Some(vec![format!("{s}ssions")]),
                    Some(vec![format!("{s}ssiez")]),
                    Some(vec![format!("{s}ssent")]),
                ],
                None => vec![None; 6],
            }
        });
        let imp = self.cells("imp", base).unwrap_or_else(|| {
            let tu = pres[1].as_ref().map(|v| {
                v.iter()
                    .map(|f| f.strip_suffix("es").map_or(f.clone(), |s| format!("{s}e")))
                    .collect()
            });
            vec![tu, pres[3].clone(), pres[4].clone()]
        });
        let ppre = match self.lines.get("ppre") {
            Some(c) => Some(vec![c[0].clone()]),
            None => nous_stem.as_ref().map(|s| vec![join(s, "ant")]),
        };
        let past_participle = self.past_participles(infinitive)?;

        let mut tenses: HashMap<&'static str, Vec<Cell>> = HashMap::new();
        for (key, cells) in [
            ("ind.pres", pres),
            ("ind.impf", impf),
            ("ind.ps", ps),
            ("ind.fut", fut),
            ("cond.pres", cond),
            ("sub.pres", subj),
            ("sub.impf", subi),
            ("imp.pres", imp),
        ] {
            let cells = if self.impersonal && key != "imp.pres" {
                cells
                    .into_iter()
                    .enumerate()
                    .map(|(i, c)| if i == 2 { c } else { None })
                    .collect()
            } else {
                cells
            };
            tenses.insert(key, cells);
        }
        Some(SimpleForms {
            infinitive: infinitive.to_string(),
            tenses,
            present_participle: ppre,
            past_participle,
        })
    }
}

/// Toutes les formes simples avec leurs traits au format du lexique
/// (`ind.pres.3s`, `ppas.f.p`…), pour comparer avec les formes attestées.
pub fn traits_forms(forms: &SimpleForms) -> Vec<(String, String)> {
    let persons = ["1s", "2s", "3s", "1p", "2p", "3p"];
    let mut out = vec![("inf".to_string(), forms.infinitive.clone())];
    for (key, cells) in &forms.tenses {
        let ps: &[&str] = if *key == "imp.pres" {
            &["2s", "1p", "2p"]
        } else {
            &persons
        };
        for (cell, p) in cells.iter().zip(ps) {
            for f in cell.iter().flatten() {
                out.push((format!("{key}.{p}"), f.clone()));
            }
        }
    }
    for f in forms.present_participle.iter().flatten() {
        out.push(("ppre".into(), f.clone()));
    }
    let gn = ["m.s", "m.p", "f.s", "f.p"];
    for (f, g) in forms.past_participle.iter().zip(gn) {
        out.push((format!("ppas.{g}"), f.clone()));
    }
    out
}

// ---------------------------------------------------------------------------
// Tableau affiché

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tense {
    pub name: &'static str,
    /// Lignes prêtes à afficher (« j’aime », « que tu aies aimé »).
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mode {
    pub name: &'static str,
    pub tenses: Vec<Tense>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conjugation {
    pub infinitive: String,
    pub model: String,
    /// « avoir » ou « être ».
    pub auxiliary: &'static str,
    pub pronominal: bool,
    /// Verbe absent de la liste : modèle choisi d'après la terminaison.
    pub guessed: bool,
    pub modes: Vec<Mode>,
}

/// Données de conjugaison chargées.
#[derive(Debug, Default)]
pub struct Conjugator {
    models: Models,
    /// Infinitif → nom du modèle.
    verbs: HashMap<String, String>,
    etre: HashSet<String>,
    /// Verbes absents du lexique (Wiktionnaire) : toutes leurs formes, infinitif
    /// et participes compris, sont reconnues par `is_form`.
    extra: HashSet<String>,
    /// (squelette de l'infinitif, infinitif), trié : verbes par début de mot.
    by_skeleton: Vec<(String, String)>,
}

impl Conjugator {
    pub fn new(models: &str, verbs: &str, etre: &str) -> Result<Conjugator, DataError> {
        let models = Models::parse(models)?;
        let mut map = HashMap::new();
        for (n, line) in data_lines(verbs) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 2 || models.get(cols[1]).is_none() {
                return Err(DataError {
                    file: "conjugaison/verbes.tsv".into(),
                    line: n,
                    message: format!("modèle inconnu : {line}"),
                });
            }
            map.insert(normalize_apostrophes(cols[0]), cols[1].to_string());
        }
        let etre = data_lines(etre)
            .map(|(_, l)| l.trim().to_string())
            .collect();
        let mut by_skeleton: Vec<(String, String)> =
            map.keys().map(|v| (skeleton(v), v.clone())).collect();
        by_skeleton.sort();
        Ok(Conjugator {
            models,
            verbs: map,
            etre,
            extra: HashSet::new(),
            by_skeleton,
        })
    }

    /// Ajoute des verbes absents du lexique (`verbe`, `modèle`, source). Leurs
    /// formes ne sont pas ajoutées au lexique (plus d'un million) : `is_form`
    /// les reconnaît à la demande.
    pub fn with_extra(mut self, verbs: &str) -> Result<Conjugator, DataError> {
        // « macher », « repondre », « reparer » : variantes sans accent d'un verbe
        // courant (« mâcher », « répondre »), leurs formes masqueraient des fautes.
        let known: HashSet<String> = self.verbs.keys().map(|v| skeleton(v)).collect();
        for (n, line) in data_lines(verbs) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 2 || self.models.get(cols[1]).is_none() {
                return Err(DataError {
                    file: "conjugaison/verbes-wiktionnaire.tsv".into(),
                    line: n,
                    message: format!("modèle inconnu : {line}"),
                });
            }
            let verb = normalize_apostrophes(cols[0]);
            if self.verbs.contains_key(&verb) || known.contains(&skeleton(&verb)) {
                continue;
            }
            self.by_skeleton.push((skeleton(&verb), verb.clone()));
            self.verbs.insert(verb.clone(), cols[1].to_string());
            self.extra.insert(verb);
        }
        self.by_skeleton.sort();
        Ok(self)
    }

    /// Vrai si `word` (en minuscules) est une forme conjuguée d'un verbe de
    /// Lexique, même absente du lexique (« rétablît », « embarquerons »).
    /// Participes et infinitifs exclus : le lexique les a, et un participe de
    /// verbe rare masquerait une faute courante (« musé » de *muser* pour
    /// « musée »).
    pub fn is_form(&self, word: &str) -> bool {
        self.find_form(word, false).is_some()
    }

    /// Si `word` est une forme, infinitif et participes compris, d'un verbe
    /// ajouté du Wiktionnaire (« modéliser », « modélisé »), absent du lexique :
    /// toutes les formes de ce verbe.
    pub fn extra_forms_of(&self, word: &str) -> Option<Vec<String>> {
        self.find_form(word, true)
    }

    /// Les verbes essayés sont ceux dont l'infinitif commence comme le mot,
    /// accents et lettres doublées mis à part (« cède » → « céder », « jette » →
    /// « jeter ») ; `extra` choisit entre les verbes de Lexique et ceux du
    /// Wiktionnaire.
    fn find_form(&self, word: &str, extra: bool) -> Option<Vec<String>> {
        const MAX_TRIED: usize = 150;
        let skel = skeleton(word);
        let chars: Vec<char> = skel.chars().collect();
        let mut tried: HashSet<&str> = HashSet::new();
        for k in (3..=chars.len()).rev() {
            let prefix: String = chars[..k].iter().collect();
            let from = self
                .by_skeleton
                .partition_point(|(s, _)| s.as_str() < prefix.as_str());
            for (s, verb) in &self.by_skeleton[from..] {
                if !s.starts_with(&prefix) || tried.len() >= MAX_TRIED {
                    break;
                }
                if self.extra.contains(verb.as_str()) != extra || !tried.insert(verb) {
                    continue;
                }
                let forms = self
                    .verbs
                    .get(verb)
                    .and_then(|m| self.models.get(m))
                    .and_then(|m| m.conjugate(verb))
                    .map(|f| traits_forms(&f))
                    .unwrap_or_default();
                let found = forms
                    .iter()
                    .any(|(t, x)| x == word && (extra || (!t.starts_with("pp") && t != "inf")));
                if found {
                    return Some(forms.into_iter().map(|(_, x)| x).collect());
                }
            }
            if tried.len() >= MAX_TRIED {
                break;
            }
        }
        None
    }

    /// Formes de `infinitive` pour le trait `wanted` (`ind.ps.2s`), d'après son
    /// modèle ; utile quand le lexique n'atteste pas la forme.
    pub fn forms_for(&self, infinitive: &str, wanted: &str) -> Vec<String> {
        self.verbs
            .get(infinitive)
            .and_then(|m| self.models.get(m))
            .and_then(|m| m.conjugate(infinitive))
            .map(|f| {
                traits_forms(&f)
                    .into_iter()
                    .filter(|(t, _)| t == wanted)
                    .map(|(_, x)| x)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Participes passés (m.s, m.p, f.s, f.p) de `infinitive`, d'après son modèle.
    pub fn past_participles(&self, infinitive: &str) -> Option<Vec<String>> {
        self.verbs
            .get(infinitive)
            .and_then(|m| self.models.get(m))
            .and_then(|m| m.past_participles(infinitive))
    }

    /// Infinitifs dont le squelette commence par celui de `prefix`.
    pub fn verbs_with_prefix(&self, prefix: &str) -> Vec<&str> {
        let prefix = skeleton(prefix);
        let from = self
            .by_skeleton
            .partition_point(|(s, _)| s.as_str() < prefix.as_str());
        self.by_skeleton[from..]
            .iter()
            .take_while(|(s, _)| s.starts_with(&prefix))
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// Verbes de la liste, avec leur modèle.
    pub fn verbs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.verbs.iter().map(|(v, m)| (v.as_str(), m.as_str()))
    }

    pub fn models(&self) -> &Models {
        &self.models
    }

    /// Vrai si le verbe se conjugue avec être (« venir », « partir »).
    pub fn takes_etre(&self, infinitive: &str) -> bool {
        self.etre.contains(infinitive)
    }

    pub fn model_of(&self, infinitive: &str) -> Option<&str> {
        self.verbs.get(infinitive).map(String::as_str)
    }

    /// Tableau complet de `infinitive` (« se » ou « s’ » devant : verbe pronominal).
    /// `elides` dit si un mot commence par une voyelle ou un h muet.
    pub fn conjugate(
        &self,
        infinitive: &str,
        elides: &dyn Fn(&str) -> bool,
    ) -> Option<Conjugation> {
        let input = normalize_apostrophes(infinitive.trim()).to_lowercase();
        let (pronominal, verb) = match input
            .strip_prefix("se ")
            .or_else(|| input.strip_prefix("s’"))
        {
            Some(v) => (true, v.trim().to_string()),
            None => (false, input.clone()),
        };
        // « s’en aller », « s’en souvenir » : le pronom « en » suit le réfléchi.
        let (en, verb) = match verb.strip_prefix("en ").filter(|_| pronominal) {
            Some(v) => (true, v.trim().to_string()),
            None => (false, verb),
        };
        let (model, guessed) = match self.verbs.get(&verb).and_then(|m| self.models.get(m)) {
            Some(m) => (m, false),
            // Verbe inconnu en -er : premier groupe, le seul encore productif.
            None if verb.ends_with("er") && verb.chars().count() > 2 => {
                (self.models.get("chanter")?, true)
            }
            None => return None,
        };
        let forms = model.conjugate(&verb)?;
        // h aspiré : connu pour l'infinitif (« haïr »), il vaut pour toutes ses formes (« je hais »).
        let aspirated = verb.starts_with('h') && !elides(&verb);
        let elides = |w: &str| !(aspirated && w.starts_with('h')) && elides(w);
        let elides: &dyn Fn(&str) -> bool = &elides;
        let auxiliary = if pronominal || self.etre.contains(&verb) {
            "être"
        } else {
            "avoir"
        };
        let aux = self.models.get(auxiliary)?.conjugate(auxiliary)?;
        let shown = if pronominal {
            let verb = if en {
                format!("en {verb}")
            } else {
                verb.clone()
            };
            format!("{}{verb}", if elides(&verb) { "s’" } else { "se " })
        } else {
            verb.clone()
        };
        Some(Conjugation {
            infinitive: shown,
            model: model.name.clone(),
            auxiliary,
            pronominal,
            guessed,
            modes: table(
                &forms,
                &aux,
                auxiliary == "être",
                pronominal,
                en,
                model.impersonal,
                elides,
            ),
        })
    }
}

const SUBJECTS: [&str; 6] = ["je", "tu", "il", "nous", "vous", "ils"];
const REFLEXIVE: [&str; 6] = ["me", "te", "se", "nous", "vous", "se"];

fn table(
    f: &SimpleForms,
    aux: &SimpleForms,
    etre: bool,
    pronominal: bool,
    en: bool,
    impersonal: bool,
    elides: &dyn Fn(&str) -> bool,
) -> Vec<Mode> {
    let pp = &f.past_participle[0];
    let invariable = f.past_participle.len() == 1;
    // Participe dans les temps composés : accord indiqué avec être (« allé(e)s »).
    let participle = |person: usize| -> String {
        if !etre || invariable {
            return pp.clone();
        }
        match (person >= 3, pp.ends_with(['s', 'x'])) {
            (false, _) => format!("{pp}(e)"),
            (true, false) => format!("{pp}(e)s"),
            (true, true) => format!("{pp}(es)"),
        }
    };
    let elide = |word: &str, next: &str| -> String {
        let short = match word {
            "je" => "j’",
            "me" => "m’",
            "te" => "t’",
            "se" => "s’",
            "que" => "qu’",
            _ => return format!("{word} "),
        };
        if elides(next) {
            short.to_string()
        } else {
            format!("{word} ")
        }
    };
    let alts = |cell: &Cell| -> Option<String> { cell.as_ref().map(|v| v.join(" / ")) };

    let line = |person: usize, verb: &str, subjunctive: bool| -> String {
        let subject = SUBJECTS[person];
        let mut rest = verb.to_string();
        if pronominal {
            if en {
                rest = format!("en {rest}");
            }
            rest = format!("{}{rest}", elide(REFLEXIVE[person], &rest));
        }
        let mut s = format!("{}{rest}", elide(subject, &rest));
        if subjunctive {
            s = format!("{}{s}", elide("que", &s));
        }
        s
    };

    let simple = |key: &str, subjunctive: bool| -> Vec<String> {
        f.tenses[key]
            .iter()
            .enumerate()
            .filter_map(|(p, c)| alts(c).map(|v| line(p, &v, subjunctive)))
            .collect()
    };
    let compound = |aux_key: &str, subjunctive: bool| -> Vec<String> {
        (0..6)
            .filter(|&p| !impersonal || p == 2)
            .filter_map(|p| {
                let a = alts(&aux.tenses[aux_key][p])?;
                Some(line(p, &format!("{a} {}", participle(p)), subjunctive))
            })
            .collect()
    };
    let imperative: Vec<String> = f.tenses["imp.pres"]
        .iter()
        .zip(if en {
            ["t’en", "nous-en", "vous-en"]
        } else {
            ["toi", "nous", "vous"]
        })
        .filter_map(|(c, pr)| alts(c).map(|v| if pronominal { format!("{v}-{pr}") } else { v }))
        .collect();
    let imperative_past: Vec<String> = if pronominal || impersonal {
        Vec::new()
    } else {
        aux.tenses["imp.pres"]
            .iter()
            .enumerate()
            .filter_map(|(i, c)| alts(c).map(|a| format!("{a} {}", participle([1, 3, 4][i]))))
            .collect()
    };
    let aux_inf = &aux.infinitive;
    let aux_ppre = alts(&aux.present_participle).unwrap_or_default();
    let refl = |s: String| {
        if pronominal {
            let s = if en { format!("en {s}") } else { s };
            format!("{}{s}", elide("se", &s))
        } else {
            s
        }
    };
    let ppre = alts(&f.present_participle).unwrap_or_default();
    let mut pp_forms = f.past_participle.clone();
    pp_forms.dedup();

    let t = |name: &'static str, lines: Vec<String>| Tense { name, lines };
    let mut modes = vec![
        Mode {
            name: "Indicatif",
            tenses: vec![
                t("Présent", simple("ind.pres", false)),
                t("Imparfait", simple("ind.impf", false)),
                t("Passé simple", simple("ind.ps", false)),
                t("Futur simple", simple("ind.fut", false)),
                t("Passé composé", compound("ind.pres", false)),
                t("Plus-que-parfait", compound("ind.impf", false)),
                t("Passé antérieur", compound("ind.ps", false)),
                t("Futur antérieur", compound("ind.fut", false)),
            ],
        },
        Mode {
            name: "Conditionnel",
            tenses: vec![
                t("Présent", simple("cond.pres", false)),
                t("Passé", compound("cond.pres", false)),
            ],
        },
        Mode {
            name: "Subjonctif",
            tenses: vec![
                t("Présent", simple("sub.pres", true)),
                t("Imparfait", simple("sub.impf", true)),
                t("Passé", compound("sub.pres", true)),
                t("Plus-que-parfait", compound("sub.impf", true)),
            ],
        },
    ];
    if !impersonal && !imperative.is_empty() {
        let mut tenses = vec![t("Présent", imperative)];
        if !imperative_past.is_empty() {
            tenses.push(t("Passé", imperative_past));
        }
        modes.push(Mode {
            name: "Impératif",
            tenses,
        });
    }
    modes.push(Mode {
        name: "Participe",
        tenses: vec![
            t("Présent", vec![refl(ppre)]),
            t("Passé", vec![pp_forms.join(", ")]),
            t(
                "Passé composé",
                vec![refl(format!("{aux_ppre} {}", participle(0)))],
            ),
        ],
    });
    modes.push(Mode {
        name: "Infinitif",
        tenses: vec![
            t("Présent", vec![refl(f.infinitive.clone())]),
            t("Passé", vec![refl(format!("{aux_inf} {}", participle(0)))]),
        ],
    });
    modes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conj() -> Conjugator {
        let models = include_str!("../../../data/conjugaison/modeles.txt");
        let etre = include_str!("../../../data/conjugaison/auxiliaire_etre.txt");
        Conjugator::new(models, "aimer\tchanter\nvenir\tvenir\nplacer\tplacer\nmanger\tmanger\nfalloir\tfalloir\nfinir\tfinir\n", etre).unwrap()
    }

    fn vowel(w: &str) -> bool {
        w.starts_with(|c: char| "aeiouyéèêâîôûh".contains(c))
    }

    fn tense<'a>(c: &'a Conjugation, mode: &str, name: &str) -> &'a [String] {
        &c.modes
            .iter()
            .find(|m| m.name == mode)
            .unwrap()
            .tenses
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .lines
    }

    #[test]
    fn premier_groupe() {
        let c = conj().conjugate("aimer", &vowel).unwrap();
        assert_eq!(tense(&c, "Indicatif", "Présent")[0], "j’aime");
        assert_eq!(
            tense(&c, "Indicatif", "Passé composé")[3],
            "nous avons aimé"
        );
        assert_eq!(tense(&c, "Subjonctif", "Imparfait")[2], "qu’il aimât");
        assert_eq!(
            tense(&c, "Impératif", "Présent"),
            ["aime", "aimons", "aimez"]
        );
        assert_eq!(
            tense(&c, "Participe", "Passé")[0],
            "aimé, aimés, aimée, aimées"
        );
    }

    #[test]
    fn orthographe_des_radicaux() {
        let c = conj().conjugate("placer", &vowel).unwrap();
        assert_eq!(
            tense(&c, "Indicatif", "Imparfait"),
            [
                "je plaçais",
                "tu plaçais",
                "il plaçait",
                "nous placions",
                "vous placiez",
                "ils plaçaient"
            ]
        );
        let m = conj().conjugate("manger", &vowel).unwrap();
        assert_eq!(tense(&m, "Subjonctif", "Présent")[3], "que nous mangions");
        assert_eq!(tense(&m, "Participe", "Présent"), ["mangeant"]);
    }

    #[test]
    fn irregulier_avec_etre() {
        let c = conj().conjugate("venir", &vowel).unwrap();
        assert_eq!(c.auxiliary, "être");
        assert_eq!(tense(&c, "Indicatif", "Présent")[5], "ils viennent");
        assert_eq!(tense(&c, "Indicatif", "Passé simple")[3], "nous vînmes");
        assert_eq!(tense(&c, "Subjonctif", "Imparfait")[2], "qu’il vînt");
        assert_eq!(
            tense(&c, "Indicatif", "Passé composé")[0],
            "je suis venu(e)"
        );
        assert_eq!(
            tense(&c, "Indicatif", "Passé composé")[5],
            "ils sont venu(e)s"
        );
    }

    #[test]
    fn pronominal_et_impersonnel() {
        let c = conj().conjugate("s'aimer", &vowel).unwrap();
        assert!(c.pronominal);
        assert_eq!(c.infinitive, "s’aimer");
        assert_eq!(tense(&c, "Indicatif", "Présent")[0], "je m’aime");
        assert_eq!(
            tense(&c, "Indicatif", "Passé composé")[2],
            "il s’est aimé(e)"
        );
        assert_eq!(tense(&c, "Impératif", "Présent")[0], "aime-toi");
        let f = conj().conjugate("falloir", &vowel).unwrap();
        assert_eq!(tense(&f, "Indicatif", "Présent"), ["il faut"]);
        let h = conj().conjugate("finir", &|w: &str| vowel(w) && !w.starts_with('h'));
        assert!(h.is_some());
        assert_eq!(tense(&f, "Indicatif", "Futur simple"), ["il faudra"]);
        assert!(f.modes.iter().all(|m| m.name != "Impératif"));
    }

    #[test]
    fn verbe_inconnu_en_er() {
        let c = conj().conjugate("googler", &vowel).unwrap();
        assert!(c.guessed);
        assert_eq!(tense(&c, "Indicatif", "Futur simple")[0], "je googlerai");
        assert!(conj().conjugate("xyzir", &vowel).is_none());
    }
}
