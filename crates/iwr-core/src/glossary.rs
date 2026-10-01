//! Glossaries: questions that appear by themselves when a cue mentions a term.
//!
//! Two sources, kept apart:
//! - the **built-in Rust glossary** ([`BUILTIN`], from `crates/iwr-core/glossary.md`): generic terms
//!   (crate, impl, trait, `?`…), shipped with every player; a script opts out with `glossary: off`;
//! - a **project glossary**: `glossary.md` next to a `codecast/` directory, same format, for the
//!   project's own vocabulary. Its entries win over built-in ones with the same term.
//!
//! Format (markdown):
//!
//! ```text
//! # Any title
//! ## crate                     one entry per `##` heading: the term
//! aliases: crates, cargo crate optional, comma separated
//! ask: What is a crate?        optional; default "What is a <term>?"
//! The answer, markdown.        the rest of the section; may hold `@` `!` `=` lines (project glossary)
//! ```
//!
//! Questions written in the script with `?` always come first; a glossary question is offered once
//! per session (the player remembers what was asked).

use crate::script::{plain, Question};
use serde::{Deserialize, Serialize};

/// The built-in Rust glossary, as text.
pub const BUILTIN: &str = include_str!("../glossary.md");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Entry {
    pub term: String,
    pub aliases: Vec<String>,
    pub ask: String,
    pub answer: String,
    pub show: Option<String>,
    pub hl: Option<Vec<String>>,
    pub code: Option<String>,
}

impl Entry {
    pub fn question(&self) -> Question {
        Question { ask: self.ask.clone(), answer: self.answer.clone(), show: self.show.clone(), hl: self.hl.clone(), code: self.code.clone(), term: Some(self.term.clone()) }
    }
}

/// Parse a glossary text. Never fails; sections without an answer are dropped.
pub fn parse(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let mut cur: Option<Entry> = None;
    for raw in text.lines() {
        let t = raw.trim();
        if let Some(term) = t.strip_prefix("## ") {
            if let Some(e) = cur.take() {
                out.push(e);
            }
            let term = term.trim().trim_matches('`').to_string();
            cur = Some(Entry { ask: format!("What is a {}?", term), term, ..Default::default() });
            continue;
        }
        let Some(e) = cur.as_mut() else { continue };
        if let Some(a) = t.strip_prefix("aliases:") {
            e.aliases = a.split(',').map(|x| x.trim().trim_matches('`').to_string()).filter(|x| !x.is_empty()).collect();
        } else if let Some(a) = t.strip_prefix("ask:") {
            e.ask = a.trim().to_string();
        } else if let Some(v) = t.strip_prefix("@ ") {
            e.show = Some(v.trim().to_string());
        } else if let Some(h) = t.strip_prefix("! ") {
            e.hl = Some(h.split_whitespace().map(|r| r.to_string()).collect());
        } else if let Some(c) = t.strip_prefix("= ") {
            e.code = Some(c.trim().to_string());
        } else if t.starts_with("//") || (t.starts_with('#') && !t.starts_with("##")) {
            // comments and the title
        } else {
            e.answer.push_str(raw.trim_end());
            e.answer.push('\n');
        }
    }
    if let Some(e) = cur.take() {
        out.push(e);
    }
    for e in &mut out {
        e.answer = e.answer.trim().to_string();
    }
    out.retain(|e| !e.answer.is_empty());
    out
}

/// The built-in glossary, parsed.
pub fn builtin() -> Vec<Entry> {
    parse(BUILTIN)
}

/// Merge glossaries: later lists win over earlier ones for the same term (case-insensitive).
pub fn merge(lists: &[Vec<Entry>]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for l in lists {
        for e in l {
            out.retain(|o| !o.term.eq_ignore_ascii_case(&e.term));
            out.push(e.clone());
        }
    }
    out
}

/// Entries whose term (or an alias) is said in `text`, in order of first mention. Whole words; a
/// trailing `s` is tolerated. A term that starts with a capital (`Box`, `Option`, `Result`) is a
/// type name and matches case-sensitively, so "every box is a module" does not offer `Box`;
/// everything else is case-insensitive. Terms without a letter or digit (`?`, `&`) match through
/// their aliases only.
pub fn mentioned<'a>(entries: &'a [Entry], text: &str) -> Vec<&'a Entry> {
    let exact = plain(text);
    let spoken = exact.to_lowercase();
    let mut hits: Vec<(usize, &Entry)> = Vec::new();
    for e in entries {
        let names = std::iter::once((e.term.as_str(), e.term.chars().next().map(|c| c.is_uppercase()).unwrap_or(false))).chain(e.aliases.iter().map(|a| (a.as_str(), false)));
        let pos = names.filter_map(|(n, cased)| if cased { find_word(&exact, n) } else { find_word(&spoken, &n.to_lowercase()) }).min();
        if let Some(p) = pos {
            hits.push((p, e));
        }
    }
    hits.sort_by_key(|(p, _)| *p);
    hits.into_iter().map(|(_, e)| e).collect()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Position of `word` in `text` as a whole word (optionally followed by `s`), if any.
fn find_word(text: &str, word: &str) -> Option<usize> {
    if word.is_empty() || !word.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let start = from + i;
        let end = start + word.len();
        let before_ok = start == 0 || !text[..start].chars().next_back().map(is_word).unwrap_or(false);
        let mut after = text[end..].chars();
        let after_ok = match after.next() {
            None => true,
            Some('s') => !after.next().map(is_word).unwrap_or(false),
            Some(c) => !is_word(c),
        };
        if before_ok && after_ok {
            return Some(start);
        }
        from = end;
    }
    None
}

/// The questions to offer for a cue: the ones written in the script, then glossary matches of
/// the cue text that were not asked yet (by term), at most `max` in all.
pub fn questions_for(cue: &crate::script::Cue, glossary: &[Entry], asked: &[String], max: usize) -> Vec<Question> {
    let mut out: Vec<Question> = cue.questions.clone();
    for e in mentioned(glossary, &cue.say) {
        if out.len() >= max {
            break;
        }
        let dup = asked.iter().any(|a| a.eq_ignore_ascii_case(&e.term)) || out.iter().any(|q| q.ask.eq_ignore_ascii_case(&e.ask) || q.term.as_deref().map(|t| t.eq_ignore_ascii_case(&e.term)).unwrap_or(false));
        if !dup {
            out.push(e.question());
        }
    }
    out.truncate(max.max(cue.questions.len()));
    out
}
