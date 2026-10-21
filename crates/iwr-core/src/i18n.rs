//! Codecasts in other languages, translated when played: no translated files to keep.
//!
//! A script is written in English. To play it in Japanese, the player collects every piece of text
//! a viewer reads or hears ([`texts`]: cues, questions and answers, part names, plan lines, side
//! notes, glossary questions), has them translated (offline model behind `iwr serve`, or a map
//! baked by `iwr export --lang`), and puts them back ([`apply`]). Directives, refs, diagrams and
//! glossary terms stay as they are, so a translated script plays exactly like the English one.
//!
//! Code must survive translation: before a piece is sent, code-looking words and the project's
//! item names are put in backticks ([`protect`]), which translation models leave alone. Markdown is
//! cut into lines (and table cells); list markers, heading marks and code fences are kept.

use crate::glossary::Entry;
use crate::model::{ItemKind, Project};
use crate::script::Script;
use std::collections::HashSet;

/// A language a codecast can be played in.
pub struct Lang {
    /// the code the translation models use (`ja`)
    pub code: &'static str,
    /// its own name, for the language menu
    pub name: &'static str,
    /// BCP 47 tag for browser speech (`ja-JP`)
    pub speech: &'static str,
}

/// English (the source) first, then the targets offered in the player.
pub const LANGS: &[Lang] = &[
    Lang { code: "en", name: "English", speech: "en-US" },
    Lang { code: "ja", name: "日本語", speech: "ja-JP" },
    Lang { code: "fr", name: "Français", speech: "fr-FR" },
    Lang { code: "de", name: "Deutsch", speech: "de-DE" },
    Lang { code: "es", name: "Español", speech: "es-ES" },
    Lang { code: "it", name: "Italiano", speech: "it-IT" },
    Lang { code: "pt", name: "Português", speech: "pt-BR" },
    Lang { code: "sv", name: "Svenska", speech: "sv-SE" },
    Lang { code: "zh", name: "中文", speech: "zh-CN" },
    Lang { code: "ko", name: "한국어", speech: "ko-KR" },
];

pub fn lang(code: &str) -> Option<&'static Lang> {
    LANGS.iter().find(|l| l.code == code)
}

/// Names that must not be translated: the project's items that cannot be a plain word (`Project`,
/// `parse_file`, `v2`). A lowercase name like `run` or `files` is also an English word the prose
/// uses: it is protected only when the script puts it in backticks.
pub fn names(p: &Project) -> HashSet<String> {
    p.items.iter().filter(|i| !matches!(i.kind, ItemKind::Impl) && i.name.len() > 2 && i.name.contains(|c: char| c.is_uppercase() || c == '_' || c.is_ascii_digit())).map(|i| i.name.clone()).collect()
}

/// Every piece of text to translate, protected, each once, in reading order.
pub fn texts(s: &Script, glossary: &[Entry], names: &HashSet<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut s = s.clone();
    let mut g = glossary.to_vec();
    walk(&mut s, &mut g, &mut |chunk| {
        let p = protect(chunk, names);
        if seen.insert(p.clone()) {
            out.push(p);
        }
        chunk.to_string()
    });
    out
}

/// Put translations back: `tr` gets a protected piece (as [`texts`] gave it) and returns its
/// translation; a piece it does not know stays in English. Backticks that [`protect`] added go
/// away again, so code is written as the source wrote it.
pub fn apply(s: &mut Script, glossary: &mut [Entry], names: &HashSet<String>, tr: &dyn Fn(&str) -> Option<String>) {
    walk(s, glossary, &mut |chunk| {
        let Some(mut t) = tr(&protect(chunk, names)) else { return chunk.to_string() };
        for w in added(chunk, names) {
            t = t.replace(&format!("`{w}`"), &w);
        }
        t
    });
}

/// The words [`protect`] puts in backticks in `text`.
fn added(text: &str, names: &HashSet<String>) -> Vec<String> {
    let p = protect(text, names);
    let before: HashSet<&str> = text.split('`').skip(1).step_by(2).collect();
    p.split('`').skip(1).step_by(2).filter(|w| !before.contains(w)).map(str::to_string).collect()
}

/// Play the recordings made in `lang` (`audio.ja:` lines): each part that has one gets it as its
/// recording, with its cue times; the others (and the whole-script recording, in English) none, so
/// TTS speaks them.
pub fn use_voices(s: &mut Script, lang: &str, voice: &str) {
    if lang != "en" {
        s.audio = None;
    }
    for part in &mut s.parts {
        let mine = part.voices.iter().filter(|v| v.lang == lang && !v.audio.is_empty());
        let v = mine.clone().find(|v| v.voice == voice).or_else(|| {
            // that voice did not record this part: the part's own English recording, else another voice
            if lang == "en" && part.audio.is_some() { None } else { mine.clone().next() }
        });
        match v {
            Some(v) => {
                part.audio = Some(v.audio.clone());
                for (i, c) in part.cues.iter_mut().enumerate() {
                    c.t = v.times.get(i).copied();
                }
            }
            None if lang == "en" => {}
            None => {
                part.audio = None;
                for c in &mut part.cues {
                    c.t = None;
                }
            }
        }
    }
}

/// The voices a script has recordings by in `lang` (`""` for one whose line does not say), with
/// the number of parts each recorded, most parts first.
pub fn recorded_voices(s: &Script, lang: &str) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for v in s.parts.iter().flat_map(|p| &p.voices).filter(|v| v.lang == lang && !v.audio.is_empty()) {
        match out.iter_mut().find(|(n, _)| *n == v.voice) {
            Some(e) => e.1 += 1,
            None => out.push((v.voice.clone(), 1)),
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

/// Every translatable text of a script and a glossary, through `f` (markdown cut into pieces).
fn walk(s: &mut Script, glossary: &mut [Entry], f: &mut dyn FnMut(&str) -> String) {
    let mut md = |t: &mut String| *t = map_markdown(t, f);
    md(&mut s.title);
    if let Some(t) = &mut s.summary {
        md(t);
    }
    for part in &mut s.parts {
        md(&mut part.name);
        if let Some(t) = &mut part.summary {
            md(t);
        }
        for note in &mut part.notes {
            if let Some(t) = &mut note.title {
                md(t);
            }
            md(&mut note.text);
        }
        for cue in &mut part.cues {
            md(&mut cue.say);
            for q in &mut cue.questions {
                md(&mut q.ask);
                md(&mut q.answer);
            }
        }
    }
    // the term and its aliases stay: they are matched against the English cue
    for e in glossary {
        md(&mut e.ask);
        md(&mut e.answer);
    }
}

/// Markdown, line by line: code fences and table rules as they are; heading marks, list markers,
/// quote marks and indentation kept around the text; table rows cell by cell.
pub fn map_markdown(text: &str, f: &mut dyn FnMut(&str) -> String) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut fence = false;
    for line in text.split('\n') {
        let t = line.trim();
        if t.starts_with("```") {
            fence = !fence;
            out.push(line.to_string());
            continue;
        }
        if fence || t.is_empty() || !t.chars().any(char::is_alphabetic) {
            out.push(line.to_string());
            continue;
        }
        if t.starts_with('|') {
            let cells: Vec<String> = t.trim_matches('|').split('|').map(|c| if c.trim().chars().any(char::is_alphabetic) { format!(" {} ", f(c.trim())) } else { c.to_string() }).collect();
            out.push(format!("{}|{}|", &line[..line.len() - line.trim_start().len()], cells.join("|")));
            continue;
        }
        let body_at = line.len() - line.trim_start().len() + marker_len(t);
        let (head, body) = line.split_at(body_at);
        out.push(format!("{}{}", head, f(body.trim_end())));
    }
    out.join("\n")
}

/// Length of the markdown marker at the start of a trimmed line, with its space: `## `, `- `,
/// `1. `, `> `.
fn marker_len(t: &str) -> usize {
    let hashes = t.len() - t.trim_start_matches('#').len();
    if hashes > 0 && t[hashes..].starts_with(' ') {
        return hashes + 1;
    }
    for m in ["- ", "* ", "• ", "> "] {
        if t.starts_with(m) {
            return m.len();
        }
    }
    let digits = t.len() - t.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 && t[digits..].starts_with(". ") {
        return digits + 2;
    }
    0
}

/// Put code-looking words and project names in backticks, outside the spans that have them already.
pub fn protect(text: &str, names: &HashSet<String>) -> String {
    let mut out = String::new();
    for (i, piece) in text.split('`').enumerate() {
        if i > 0 {
            out.push('`');
        }
        if i % 2 == 1 {
            out.push_str(piece);
            continue;
        }
        let mut first = true;
        for w in piece.split(' ') {
            if !first {
                out.push(' ');
            }
            first = false;
            // peel opening and closing punctuation around the word
            let core = w.trim_start_matches(['(', '"', '“']);
            let lead = &w[..w.len() - core.len()];
            let mut core = core;
            while let Some(last) = core.chars().next_back() {
                if !matches!(last, '.' | ',' | ';' | ':' | '"' | '”' | ')' | '?' | '!') || (last == ')' && core.contains('(')) {
                    break;
                }
                core = &core[..core.len() - last.len_utf8()];
            }
            let trail = &w[lead.len() + core.len()..];
            let code = !core.is_empty() && (names.contains(core) || crate::speech::looks_like_code(core) && core.chars().all(|c| c.is_ascii_graphic()));
            if code {
                out.push_str(&format!("{lead}`{core}`{trail}"));
            } else {
                out.push_str(w);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> HashSet<String> {
        ["collect", "Project"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn code_and_names_get_backticks() {
        let n = names();
        assert_eq!(protect("collect returns a Project, see parse_file.", &n), "`collect` returns a `Project`, see `parse_file`.");
        // already quoted, and plain words
        assert_eq!(protect("the `Project` and the project", &n), "the `Project` and the project");
        assert_eq!(protect("call crate::run(x) now", &n), "call `crate::run(x)` now");
    }

    #[test]
    fn markdown_keeps_its_marks() {
        let mut seen = vec![];
        let out = map_markdown("## Title\n- one item\n12. two\n\n```rust\nlet x = 1;\n```\n| a | b |\n|---|---|\n| cell | 2 |", &mut |t| {
            seen.push(t.to_string());
            t.to_uppercase()
        });
        assert_eq!(seen, ["Title", "one item", "two", "a", "b", "cell"]);
        assert_eq!(out, "## TITLE\n- ONE ITEM\n12. TWO\n\n```rust\nlet x = 1;\n```\n| A | B |\n|---|---|\n| CELL | 2 |");
    }

    #[test]
    fn apply_translates_what_texts_listed() {
        let mut s = crate::script::parse("# Demo\n## Start\n! crate::run\ncollect is pass one.\n? Why?\n  Because.\n");
        let t = texts(&s, &[], &names());
        assert_eq!(t, ["Demo", "Start", "`collect` is pass one.", "Why?", "Because."]);
        apply(&mut s, &mut [], &names(), &|p| if p == "`collect` is pass one." { Some("`collect` は第一段階です。".into()) } else { None });
        let cue = &s.parts[0].cues[0];
        // the backticks were only there to get through translation
        assert_eq!(cue.say, "collect は第一段階です。");
        assert_eq!(cue.hl.as_deref(), Some(&["crate::run".to_string()][..]));
        // unknown pieces stay in English, unprotected
        assert_eq!(cue.questions[0].answer, "Because.");
    }

    #[test]
    fn recordings_follow_the_language() {
        let mut s = crate::script::parse("# T\naudio: all.mp3\n## A\naudio.ja: 01-a.ja.ogg\ntimes.ja: 0 2.5\none\n[4] two\n## B\nthree\n");
        assert_eq!(s.parts[0].voices[0].times, [0.0, 2.5]);
        // the text form keeps them
        assert_eq!(crate::script::parse(&crate::script::to_text(&s)).parts[0].voices, s.parts[0].voices);
        use_voices(&mut s, "ja", "");
        assert_eq!((s.audio.as_deref(), s.parts[0].audio.as_deref(), s.parts[1].audio.as_deref()), (None, Some("01-a.ja.ogg"), None));
        assert_eq!(s.parts[0].cues.iter().map(|c| c.t).collect::<Vec<_>>(), [Some(0.0), Some(2.5)]);
    }

    #[test]
    fn the_listener_picks_a_voice() {
        let text = "# T\n## A\naudio: 01-a.ogg\ntimes: 0 1\naudio.en.am_adam: 01-a.am_adam.ogg\ntimes.en.am_adam: 0 3\naudio.en.af_heart: 01-a.af_heart.ogg\ntimes.en.af_heart: 0 2\none\ntwo\n## B\naudio.en.af_heart: 02-b.af_heart.ogg\ntimes.en.af_heart: 0\nthree\n";
        let s = crate::script::parse(text);
        assert_eq!(crate::script::parse(&crate::script::to_text(&s)).parts[0].voices, s.parts[0].voices);
        assert_eq!(recorded_voices(&s, "en"), [("af_heart".to_string(), 2), ("am_adam".to_string(), 1)]);
        let mut adam = s.clone();
        use_voices(&mut adam, "en", "am_adam");
        assert_eq!(adam.parts[0].audio.as_deref(), Some("01-a.am_adam.ogg"));
        assert_eq!(adam.parts[0].cues[1].t, Some(3.0));
        // Adam did not record part B: another voice plays it
        assert_eq!(adam.parts[1].audio.as_deref(), Some("02-b.af_heart.ogg"));
        // no voice asked: the part's own recording
        let mut own = s.clone();
        use_voices(&mut own, "en", "");
        assert_eq!(own.parts[0].audio.as_deref(), Some("01-a.ogg"));
    }

    #[test]
    fn japanese_is_spoken_with_code_as_words() {
        let pron = crate::speech::builtin_pronounce();
        assert_eq!(crate::speech::spoken("`module_for` はツリー内のモジュールを見つけます。", &pron), "module for はツリー内のモジュールを見つけます。");
    }
}
