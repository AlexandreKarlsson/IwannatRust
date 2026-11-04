//! Guard against interface text written in English in the web UI instead of
//! `crates/iwr-ui/i18n/`: a string literal that reads like prose fails the test. Code, CSS
//! classes, JavaScript (`document::eval`, raw strings) and comments are skipped; a literal that
//! is deliberately not translated says why on its line: `// i18n: <reason>`.

use std::path::Path;

/// Does a literal read like an English sentence or label ("Show the source", "load failed")?
fn prose(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    let letters = |w: &str| w.chars().all(|c| c.is_ascii_alphabetic() || ",.:?!'".contains(c)) && w.chars().any(|c| c.is_ascii_alphabetic());
    let wordy = words.iter().filter(|w| letters(w)).count();
    // CSS class lists are lowercase words: prose starts with a capital, or punctuates a few words
    let capital = words.first().is_some_and(|w| w.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && w.len() > 1);
    // (a CSS selector list has `.class` words)
    let punctuated = wordy >= 3 && s.contains([',', '.', '?', '!', '…']) && !s.starts_with(['&', '.']) && !words.iter().any(|w| w.starts_with('.'));
    wordy >= 2 && (capital || punctuated) && !s.contains(['{', '}', ';', '=', '(', '<', '/', '#'])
}

#[test]
fn ui_has_no_english_literals() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../iwr-ui/src");
    let mut bad = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&src).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rs")).collect();
    files.sort();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let mut raw = false;
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            // raw strings hold JavaScript and CSS
            if raw {
                raw = !line.contains("\"#");
                continue;
            }
            if let Some(rest) = line.split_once("r#\"").map(|(_, r)| r) {
                raw = !rest.contains("\"#");
                continue;
            }
            if t.starts_with("//") || line.contains("// i18n:") || line.contains("eval(") || line.contains("eval!(") {
                continue;
            }
            let code = line.split(" // ").next().unwrap_or(line);
            for lit in code.split('"').skip(1).step_by(2) {
                if prose(lit) {
                    bad.push(format!("{}:{}: \"{}\"", f.file_name().unwrap().to_string_lossy(), i + 1, lit));
                }
            }
        }
    }
    assert!(bad.is_empty(), "English text in the UI: move it to crates/iwr-ui/i18n/ (docs/i18n.md), or say why with `// i18n: <reason>`\n  {}", bad.join("\n  "));
}

#[test]
fn prose_detection() {
    assert!(prose("Show the source"));
    assert!(prose("load failed, try again"));
    assert!(!prose("ibtn view active"));
    assert!(!prose("&, button, input, select, .bhead .label.note"));
    assert!(!prose("gen-mini go big"));
    assert!(!prose("chip act"));
    assert!(!prose("xMidYMid meet"));
    assert!(!prose("{n} parts"));
}
