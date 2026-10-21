//! Syntax highlighting for the source panes: a small Rust lexer that cuts a file
//! into coloured tokens, line by line. Syntactic only, tolerant of broken code.

/// What a piece of source text is, for colouring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Keyword,
    /// `true`, `false`, `self`, `Self`, `None`, `Some`, `Ok`, `Err`.
    Builtin,
    /// Capitalised name: a type, trait or variant.
    Type,
    /// Name followed by `(`, or named by `fn`.
    Fn,
    /// `name!`.
    Macro,
    Str,
    Num,
    Lifetime,
    Comment,
    /// `///`, `//!`, `/** */`.
    Doc,
    /// `#[...]` and `#![...]`.
    Attr,
}

impl Tok {
    /// CSS class of the token (`""` for plain text).
    pub fn class(self) -> &'static str {
        match self {
            Tok::Plain => "",
            Tok::Keyword => "t-kw",
            Tok::Builtin => "t-bi",
            Tok::Type => "t-ty",
            Tok::Fn => "t-fn",
            Tok::Macro => "t-mac",
            Tok::Str => "t-str",
            Tok::Num => "t-num",
            Tok::Lifetime => "t-lt",
            Tok::Comment => "t-com",
            Tok::Doc => "t-doc",
            Tok::Attr => "t-attr",
        }
    }
}

/// One line of highlighted source: consecutive `(kind, text)` runs.
pub type Line = Vec<(Tok, String)>;

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static", "struct", "super", "trait", "type", "union", "unsafe", "use", "where", "while", "yield",
];
const BUILTINS: &[&str] = &["true", "false", "self", "Self", "None", "Some", "Ok", "Err"];

/// Highlight a Rust file. Returns one entry per line of `src.lines()`.
pub fn rust(src: &str) -> Vec<Line> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Out { lines: vec![Vec::new()] };
    let mut i = 0;
    let mut prev_word = String::new();
    while i < c.len() {
        let ch = c[i];
        let start = i;
        let tok = if ch == '/' && c.get(i + 1) == Some(&'/') {
            let doc = matches!(c.get(i + 2), Some('!')) || (c.get(i + 2) == Some(&'/') && c.get(i + 3) != Some(&'/'));
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            if doc { Tok::Doc } else { Tok::Comment }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            let doc = matches!(c.get(i + 2), Some('!')) || (c.get(i + 2) == Some(&'*') && c.get(i + 3) != Some(&'/'));
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            if doc { Tok::Doc } else { Tok::Comment }
        } else if let Some(n) = string_start(&c, i) {
            i = string_end(&c, i, n);
            Tok::Str
        } else if ch == '\'' {
            // char literal or lifetime
            if c.get(i + 1) == Some(&'\\') {
                i += 2;
                while i < c.len() && c[i] != '\'' && c[i] != '\n' {
                    i += 1;
                }
                i = (i + 1).min(c.len());
                Tok::Str
            } else if c.get(i + 2) == Some(&'\'') {
                i += 3;
                Tok::Str
            } else {
                i += 1;
                while i < c.len() && is_ident(c[i]) {
                    i += 1;
                }
                if i == start + 1 { Tok::Plain } else { Tok::Lifetime }
            }
        } else if ch == '#' && (c.get(i + 1) == Some(&'[') || (c.get(i + 1) == Some(&'!') && c.get(i + 2) == Some(&'['))) {
            let mut depth = 0;
            while i < c.len() {
                match c[i] {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            Tok::Attr
        } else if ch.is_ascii_digit() {
            while i < c.len() && (is_ident(c[i]) || (c[i] == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit()))) {
                i += 1;
            }
            Tok::Num
        } else if is_ident_start(ch) {
            while i < c.len() && is_ident(c[i]) {
                i += 1;
            }
            let w: String = c[start..i].iter().collect();
            let tok = if c.get(i) == Some(&'!') && c.get(i + 1) != Some(&'=') && prev_word != "macro_rules" {
                i += 1;
                Tok::Macro
            } else if BUILTINS.contains(&w.as_str()) {
                Tok::Builtin
            } else if KEYWORDS.contains(&w.as_str()) {
                Tok::Keyword
            } else if prev_word == "fn" || next_non_space(&c, i) == Some('(') || turbofish(&c, i) {
                if w.starts_with(|x: char| x.is_uppercase()) { Tok::Type } else { Tok::Fn }
            } else if w.starts_with(|x: char| x.is_uppercase()) {
                Tok::Type
            } else {
                Tok::Plain
            };
            prev_word = w;
            out.push(tok, &c[start..i]);
            continue;
        } else {
            i += 1;
            Tok::Plain
        };
        if !c[start..i].iter().all(|x| x.is_whitespace()) {
            prev_word.clear();
        }
        out.push(tok, &c[start..i]);
    }
    let mut lines = out.lines;
    // `str::lines` drops the empty piece after a final newline
    if src.ends_with('\n') {
        lines.pop();
    }
    lines
}

struct Out {
    lines: Vec<Line>,
}

impl Out {
    /// Append text, starting a new line at each `\n` (a `\r` before it is dropped, like `str::lines`).
    fn push(&mut self, tok: Tok, text: &[char]) {
        let s: String = text.iter().collect();
        for (k, part) in s.split('\n').enumerate() {
            if k > 0 {
                if let Some((_, t)) = self.lines.last_mut().and_then(|l| l.last_mut()) {
                    if t.ends_with('\r') {
                        t.pop();
                    }
                }
                self.lines.push(Vec::new());
            }
            if part.is_empty() {
                continue;
            }
            let line = self.lines.last_mut().unwrap();
            match line.last_mut() {
                Some((t, s)) if *t == tok => s.push_str(part),
                _ => line.push((tok, part.to_string())),
            }
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn next_non_space(c: &[char], mut i: usize) -> Option<char> {
    while i < c.len() && c[i] == ' ' {
        i += 1;
    }
    c.get(i).copied()
}

/// `name::<T>(` — a call with explicit generics.
fn turbofish(c: &[char], i: usize) -> bool {
    c.get(i) == Some(&':') && c.get(i + 1) == Some(&':') && c.get(i + 2) == Some(&'<')
}

/// If a string literal starts at `i`, the length of its prefix up to and including the
/// opening quote, plus the number of `#`s (raw strings), packed as `prefix_len * 256 + hashes`.
fn string_start(c: &[char], i: usize) -> Option<usize> {
    let mut j = i;
    // prefixes: b, c, r, br, cr
    if matches!(c.get(j), Some('b') | Some('c')) {
        j += 1;
    }
    let raw = c.get(j) == Some(&'r');
    if raw {
        j += 1;
    }
    // must not be the tail of an identifier (`attr"…"` is not a string prefix)
    if j > i && i > 0 && is_ident(c[i - 1]) {
        return None;
    }
    let mut hashes = 0;
    if raw {
        while c.get(j) == Some(&'#') {
            hashes += 1;
            j += 1;
        }
    }
    if c.get(j) == Some(&'"') {
        Some((j + 1 - i) * 256 + if raw { hashes + 1 } else { 0 })
    } else if j == i + 1 && c[i] == 'b' && c.get(j) == Some(&'\'') {
        // byte literal b'x'
        Some(1)
    } else {
        None
    }
}

fn string_end(c: &[char], i: usize, packed: usize) -> usize {
    if packed == 1 {
        // b'x' / b'\n'
        let mut j = i + 2;
        if c.get(j) == Some(&'\\') {
            j += 2;
        } else {
            j += 1;
        }
        while j < c.len() && c[j] != '\'' && c[j] != '\n' {
            j += 1;
        }
        return (j + 1).min(c.len());
    }
    let mut j = i + packed / 256;
    let raw = packed % 256;
    if raw > 0 {
        let hashes = raw - 1;
        while j < c.len() {
            if c[j] == '"' && (0..hashes).all(|k| c.get(j + 1 + k) == Some(&'#')) {
                return j + 1 + hashes;
            }
            j += 1;
        }
        return c.len();
    }
    while j < c.len() {
        match c[j] {
            '\\' => j += 2,
            '"' => return j + 1,
            _ => j += 1,
        }
    }
    c.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(Tok, String)> {
        rust(src).into_iter().flatten().filter(|(t, _)| *t != Tok::Plain).collect()
    }

    #[test]
    fn one_entry_per_line() {
        let src = "fn a() {\n    /* x\n y */\n    let s = \"a\nb\";\n}\n";
        assert_eq!(rust(src).len(), src.lines().count());
        let src = "a\r\nb";
        let l = rust(src);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0], vec![(Tok::Plain, "a".to_string())]);
    }

    #[test]
    fn tokens() {
        let k = kinds("/// doc\n#[derive(Debug)]\npub fn go<'a>(x: &'a str) -> Option<u8> { println!(\"{x}\"); let c = 'c'; Some(b'1' + 2) }");
        let has = |t: Tok, s: &str| k.iter().any(|(a, b)| *a == t && b == s);
        assert!(has(Tok::Doc, "/// doc"));
        assert!(has(Tok::Attr, "#[derive(Debug)]"));
        assert!(has(Tok::Keyword, "pub"));
        assert!(has(Tok::Fn, "go"));
        assert!(has(Tok::Lifetime, "'a"));
        assert!(has(Tok::Type, "Option"));
        assert!(has(Tok::Macro, "println!"));
        assert!(has(Tok::Str, "\"{x}\""));
        assert!(has(Tok::Str, "'c'"));
        assert!(has(Tok::Str, "b'1'"));
        assert!(has(Tok::Builtin, "Some"));
        assert!(has(Tok::Num, "2"));
    }

    #[test]
    fn raw_strings_and_ranges() {
        let k = kinds("let s = r#\"a \"quoted\" b\"#; for i in 0..10 {}");
        assert!(k.contains(&(Tok::Str, "r#\"a \"quoted\" b\"#".to_string())));
        assert!(k.contains(&(Tok::Num, "0".to_string())));
        assert!(k.contains(&(Tok::Num, "10".to_string())));
    }
}
