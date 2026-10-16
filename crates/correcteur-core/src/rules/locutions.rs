//! Locutions fautives reconnues mot à mot (`data/regles/locutions.tsv`).

use super::{Context, RawIssue};
use crate::text::{capitalize, is_capitalized};

pub fn check(ctx: &Context, out: &mut Vec<RawIssue>) {
    let words = &ctx.words;
    for i in 0..words.len() {
        for loc in &ctx.data.locutions {
            let n = loc.words.len();
            if i + n > words.len() {
                continue;
            }
            let matches = loc.words.iter().enumerate().all(|(k, expected)| {
                let w = &words[i + k];
                w.lower == *expected && !w.ignored && (k == 0 || w.joined_to_prev)
            });
            if !matches {
                continue;
            }
            let (start, end) = (words[i].start, words[i + n - 1].end);
            if out
                .iter()
                .any(|o| o.rule.starts_with("HOMO_") && o.start < end && start < o.end)
            {
                continue; // déjà signalé par une règle plus précise
            }
            let found = &ctx.text[start..end];
            let replacement = if is_capitalized(found) {
                capitalize(&loc.replacement)
            } else {
                loc.replacement.clone()
            };
            out.push(
                RawIssue::new(start, end, loc.rule)
                    .arg(found)
                    .arg(replacement.clone())
                    .suggest(replacement),
            );
        }
    }
}
