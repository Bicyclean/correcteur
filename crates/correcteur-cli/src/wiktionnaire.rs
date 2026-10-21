//! Lecture en flux d'un export XML du Wiktionnaire français (CC BY-SA 4.0),
//! page par page. Sert aux extractions (antonymes, verbes).

use std::io::BufRead;

pub const SOURCE: &str = "WIKTIONNAIRE";

pub fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&amp;", "&")
}

/// Catégorie d'un titre de section de mot : `{{S|adjectif|fr}}` → `adjectif`.
pub fn section_name(line: &str) -> Option<String> {
    let inner = line.trim().trim_matches('=').trim();
    let rest = inner.strip_prefix("{{S|")?;
    let name = rest.split(['|', '}']).next()?.trim();
    Some(name.to_string())
}

pub struct Page {
    pub title: String,
    pub namespace_main: bool,
    pub text: Vec<String>,
}

/// Appelle `f` pour chaque page de l'export ; renvoie le nombre de pages lues.
pub fn read_pages(input: impl BufRead, mut f: impl FnMut(&Page)) -> std::io::Result<usize> {
    let mut count = 0;
    let mut page: Option<Page> = None;
    let mut in_text = false;
    for line in input.lines() {
        let line = line?;
        let t = line.trim_start();
        if t.starts_with("<page>") {
            page = Some(Page {
                title: String::new(),
                namespace_main: false,
                text: Vec::new(),
            });
            continue;
        }
        let Some(p) = page.as_mut() else { continue };
        if let Some(rest) = t.strip_prefix("<title>") {
            p.title = unescape(rest.trim_end_matches("</title>"));
        } else if t.starts_with("<ns>") {
            p.namespace_main = t == "<ns>0</ns>";
        } else if t.starts_with("<text") {
            // <text bytes="…" xml:space="preserve">première ligne
            let content = t.split_once('>').map_or("", |(_, c)| c);
            if let Some(c) = content.strip_suffix("</text>") {
                p.text.push(unescape(c));
            } else if !t.ends_with("/>") {
                p.text.push(unescape(content));
                in_text = true;
            }
        } else if in_text {
            match line.strip_suffix("</text>") {
                Some(c) => {
                    p.text.push(unescape(c));
                    in_text = false;
                }
                None => p.text.push(unescape(&line)),
            }
        } else if t.starts_with("</page>") {
            if let Some(p) = page.take() {
                count += 1;
                f(&p);
            }
        }
    }
    Ok(count)
}
