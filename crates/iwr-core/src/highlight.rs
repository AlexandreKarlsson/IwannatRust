//! Syntax highlighting for the source panes: small lexers (Rust, shell, Terraform/HCL,
//! TOML/YAML-like config) that cut a file into coloured tokens, line by line.
//! Syntactic only, tolerant of broken code.

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
    /// `#[...]` and `#![...]`; a `--flag` in shell.
    Attr,
    /// Variable of a script: `$x`, `${x}`, `var.x`, `${...}` in a string.
    Var,
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
            Tok::Var => "t-var",
        }
    }
}

/// One line of highlighted source: consecutive `(kind, text)` runs.
pub type Line = Vec<(Tok, String)>;

/// Languages the source panes colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Shell,
    /// Terraform and other HCL files.
    Hcl,
    /// TOML, YAML, INI, `.env`: `#` comments, keys, strings.
    Conf,
    Plain,
}

impl Lang {
    /// Guess from the file name, or from a `#!` line for scripts without an extension.
    pub fn detect(path: &str, src: &str) -> Lang {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        match ext.as_str() {
            "rs" => Lang::Rust,
            "sh" | "bash" | "zsh" | "ksh" | "bats" | "bashrc" | "zshrc" | "profile" | "envrc" => Lang::Shell,
            "tf" | "tfvars" | "hcl" | "nomad" => Lang::Hcl,
            "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" | "env" => Lang::Conf,
            _ => {
                // `#!/bin/bash`, `#!/usr/bin/env -S bash -e`
                let mut words = src.lines().next().and_then(|l| l.strip_prefix("#!")).unwrap_or("").split_whitespace();
                let mut prog = words.next().map(|w| w.rsplit('/').next().unwrap_or(w));
                if prog == Some("env") {
                    prog = words.find(|w| !w.starts_with('-'));
                }
                if prog.is_some_and(|p| matches!(p, "sh" | "bash" | "zsh" | "ksh" | "dash")) { Lang::Shell } else { Lang::Plain }
            }
        }
    }
}

/// Highlight a file in the language its path (or `#!` line) tells. One entry per line of `src.lines()`.
pub fn file(path: &str, src: &str) -> Vec<Line> {
    match Lang::detect(path, src) {
        Lang::Rust => rust(src),
        Lang::Shell => shell(src),
        Lang::Hcl => hcl(src),
        Lang::Conf => conf(src),
        Lang::Plain => src.lines().map(|l| vec![(Tok::Plain, l.to_string())]).collect(),
    }
}

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
    out.finish(src)
}

const SH_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac", "in", "function", "select", "return", "local", "export", "readonly", "declare", "typeset", "exit", "break", "continue", "shift", "source", "eval", "exec", "trap", "set", "unset",
];

/// Highlight a shell script (sh, bash, zsh): commands, variables, strings, here-documents.
pub fn shell(src: &str) -> Vec<Line> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Out { lines: vec![Vec::new()] };
    let mut i = 0;
    // the next word is a command name
    let mut cmd = true;
    // here-document delimiters whose bodies start after the current line
    let mut heredocs: Vec<String> = Vec::new();
    while i < c.len() {
        let ch = c[i];
        let start = i;
        let tok = if ch == '\n' {
            i += 1;
            out.push(Tok::Plain, &c[start..i]);
            cmd = true;
            for d in std::mem::take(&mut heredocs) {
                let s = i;
                i = heredoc_end(&c, i, &d);
                out.push(Tok::Str, &c[s..i]);
            }
            continue;
        } else if ch == '\\' {
            i = (i + 2).min(c.len());
            Tok::Plain
        } else if ch == '#' && (i == 0 || c[i - 1].is_whitespace() || matches!(c[i - 1], ';' | '(' | '|' | '&')) {
            i = line_end(&c, i);
            Tok::Comment
        } else if ch == '\'' {
            i = quote_end(&c, i, '\'', false);
            Tok::Str
        } else if ch == '"' {
            i = interp_string(&c, i, &mut out, '$');
            cmd = false;
            continue;
        } else if ch == '$' {
            i = var_end(&c, i);
            if i == start + 1 {
                Tok::Plain
            } else {
                cmd = c[i - 1] == '(';
                Tok::Var
            }
        } else if ch == '<' && c.get(i + 1) == Some(&'<') && c.get(i + 2) != Some(&'<') {
            match heredoc_start(&c, i + 2) {
                Some((end, delim)) => {
                    heredocs.push(delim);
                    i = end;
                    Tok::Str
                }
                None => {
                    i += 2;
                    Tok::Plain
                }
            }
        } else if is_sh_word(ch) {
            while i < c.len() && is_sh_word(c[i]) {
                i += 1;
            }
            let w: String = c[start..i].iter().collect();
            // NAME=value
            if let Some(eq) = w.find('=').filter(|&k| k > 0 && w[..k].chars().all(is_ident) && !w.starts_with(|x: char| x.is_ascii_digit())) {
                let eq = w[..eq].chars().count();
                out.push(Tok::Var, &c[start..start + eq]);
                out.push(Tok::Plain, &c[start + eq..i]);
                continue;
            }
            if SH_KEYWORDS.contains(&w.as_str()) {
                cmd = matches!(w.as_str(), "if" | "then" | "else" | "elif" | "do" | "while" | "until" | "exec" | "eval");
                Tok::Keyword
            } else if w.starts_with('-') {
                Tok::Attr
            } else if cmd {
                cmd = false;
                Tok::Fn
            } else if w.chars().all(|x| x.is_ascii_digit()) {
                Tok::Num
            } else {
                Tok::Plain
            }
        } else {
            i += 1;
            if matches!(ch, ';' | '|' | '&' | '(' | '{' | '`' | '!') {
                cmd = true;
            } else if !ch.is_whitespace() {
                cmd = false;
            }
            Tok::Plain
        };
        out.push(tok, &c[start..i]);
    }
    out.finish(src)
}

fn is_sh_word(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '+' | ':' | '@' | '%' | ',' | '~' | '=')
}

/// End of `$name`, `$1`, `$?`, `${...}`, or of the `$(` / `$((` that opens a substitution.
fn var_end(c: &[char], i: usize) -> usize {
    match c.get(i + 1) {
        Some('{') => {
            let mut depth = 0;
            let mut j = i + 1;
            while j < c.len() {
                match c[j] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return j + 1;
                        }
                    }
                    '\n' => return j,
                    _ => {}
                }
                j += 1;
            }
            c.len()
        }
        Some('(') => i + if c.get(i + 2) == Some(&'(') { 3 } else { 2 },
        Some(&x) if is_ident_start(x) => {
            let mut j = i + 1;
            while j < c.len() && is_ident(c[j]) {
                j += 1;
            }
            j
        }
        Some(x) if x.is_ascii_digit() || matches!(x, '@' | '*' | '#' | '?' | '$' | '!' | '-') => i + 2,
        _ => i + 1,
    }
}

/// After `<<`: the optional `-`, the (maybe quoted) delimiter word. Returns its end and the word.
fn heredoc_start(c: &[char], mut j: usize) -> Option<(usize, String)> {
    if c.get(j) == Some(&'-') {
        j += 1;
    }
    while c.get(j) == Some(&' ') {
        j += 1;
    }
    let quote = c.get(j).copied().filter(|q| matches!(q, '\'' | '"'));
    if quote.is_some() {
        j += 1;
    }
    let ws = j;
    while j < c.len() && is_ident(c[j]) {
        j += 1;
    }
    if j == ws {
        return None;
    }
    let word = c[ws..j].iter().collect();
    if quote.is_some() && c.get(j).copied() == quote {
        j += 1;
    }
    Some((j, word))
}

/// End of a here-document body starting at `i`: the end of its delimiter line
/// (before the newline), or the end of the file.
fn heredoc_end(c: &[char], mut i: usize, delim: &str) -> usize {
    while i < c.len() {
        let e = line_end(c, i);
        let line: String = c[i..e].iter().collect();
        if line.trim_start_matches('\t').trim_end_matches('\r') == delim {
            return e;
        }
        i = e + 1;
    }
    c.len()
}

fn line_end(c: &[char], mut i: usize) -> usize {
    while i < c.len() && c[i] != '\n' {
        i += 1;
    }
    i
}

/// End of a string opened by `q` at `i` (past the closing quote, or at the end of the file).
fn quote_end(c: &[char], mut i: usize, q: char, escapes: bool) -> usize {
    i += 1;
    while i < c.len() && c[i] != q {
        i += if escapes && c[i] == '\\' { 2 } else { 1 };
    }
    (i + 1).min(c.len())
}

/// Push a `"` string whose `$x` / `${...}` (shell, `sigil = '$'`) or `${...}` / `%{...}`
/// (HCL, `sigil = '%'`) pieces are variables. Returns the end of the string.
fn interp_string(c: &[char], i: usize, out: &mut Out, sigil: char) -> usize {
    let mut s = i;
    let mut j = i + 1;
    while j < c.len() && c[j] != '"' {
        if c[j] == '\\' {
            j += 2;
            continue;
        }
        let opens = (c[j] == '$' || c[j] == sigil) && (sigil == '$' || c.get(j + 1) == Some(&'{'));
        if opens {
            let e = if sigil == '$' { var_end(c, j) } else { brace_end(c, j + 1) };
            if e > j + 1 {
                out.push(Tok::Str, &c[s..j]);
                out.push(Tok::Var, &c[j..e]);
                j = e;
                s = j;
                continue;
            }
        }
        j += 1;
    }
    let j = (j + 1).min(c.len());
    out.push(Tok::Str, &c[s..j]);
    j
}

/// Past the `}` matching the `{` at `i` (or the end of the file).
fn brace_end(c: &[char], i: usize) -> usize {
    let mut depth = 0;
    for (j, &x) in c.iter().enumerate().skip(i) {
        match x {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
    }
    c.len()
}

/// Only blanks between the start of the line and `i`.
fn line_start(c: &[char], i: usize) -> bool {
    c[..i].iter().rev().take_while(|&&x| x != '\n').all(|x| x.is_whitespace())
}

/// Index of the first character at or after `i` that is not a space or tab.
fn skip_blanks(c: &[char], mut i: usize) -> usize {
    while i < c.len() && matches!(c[i], ' ' | '\t') {
        i += 1;
    }
    i
}

const HCL_KEYWORDS: &[&str] = &["for", "in", "if", "else", "endif", "endfor"];
/// Roots of references: `var.x`, `local.x`, `module.x.y`…
const HCL_ROOTS: &[&str] = &["var", "local", "module", "data", "each", "count", "path", "terraform", "self"];

/// Highlight Terraform / HCL: block types, attribute names, references, interpolations.
pub fn hcl(src: &str) -> Vec<Line> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Out { lines: vec![Vec::new()] };
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let start = i;
        let tok = if ch == '#' || (ch == '/' && c.get(i + 1) == Some(&'/')) {
            i = line_end(&c, i);
            Tok::Comment
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i < c.len() && !(c[i] == '*' && c.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(c.len());
            Tok::Comment
        } else if ch == '"' {
            i = interp_string(&c, i, &mut out, '%');
            continue;
        } else if ch == '<' && c.get(i + 1) == Some(&'<') {
            match heredoc_start(&c, i + 2) {
                Some((end, delim)) => {
                    let body = (line_end(&c, end) + 1).min(c.len());
                    i = heredoc_end(&c, body, &delim);
                    Tok::Str
                }
                None => {
                    i += 2;
                    Tok::Plain
                }
            }
        } else if ch.is_ascii_digit() {
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '.') {
                i += 1;
            }
            Tok::Num
        } else if is_ident_start(ch) {
            while i < c.len() && (is_ident(c[i]) || c[i] == '-') {
                i += 1;
            }
            let w: String = c[start..i].iter().collect();
            let k = skip_blanks(&c, i);
            let (next, after) = (c.get(k).copied(), c.get(k + 1).copied());
            if line_start(&c, start) && matches!(next, Some('"') | Some('{')) {
                Tok::Keyword
            } else if (next == Some('=') && after != Some('=')) || (next == Some(':') && line_start(&c, start)) {
                Tok::Fn
            } else if next == Some('(') {
                Tok::Macro
            } else if matches!(w.as_str(), "true" | "false" | "null") {
                Tok::Builtin
            } else if HCL_KEYWORDS.contains(&w.as_str()) {
                Tok::Keyword
            } else if HCL_ROOTS.contains(&w.as_str()) && c.get(i) == Some(&'.') {
                Tok::Var
            } else {
                Tok::Plain
            }
        } else {
            i += 1;
            Tok::Plain
        };
        out.push(tok, &c[start..i]);
    }
    out.finish(src)
}

/// Highlight a TOML / YAML / INI / `.env` file: comments, `[sections]`, keys, strings, numbers.
pub fn conf(src: &str) -> Vec<Line> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Out { lines: vec![Vec::new()] };
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let start = i;
        let first = line_start(&c, i);
        let tok = if ch == '#' && (i == 0 || c[i - 1].is_whitespace()) || (ch == ';' && first) {
            i = line_end(&c, i);
            Tok::Comment
        } else if ch == '[' && first {
            i = line_end(&c, i);
            Tok::Type
        } else if ch == '"' || ch == '\'' {
            i = quote_end(&c, i, ch, ch == '"');
            Tok::Str
        } else if ch.is_ascii_digit() || (ch == '-' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            i += 1;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || matches!(c[i], '.' | '_' | ':' | '-')) {
                i += 1;
            }
            Tok::Num
        } else if is_ident_start(ch) {
            while i < c.len() && (is_ident(c[i]) || matches!(c[i], '-' | '.')) {
                i += 1;
            }
            let w: String = c[start..i].iter().collect();
            // a key starts its line (or follows a YAML `- `) and is followed by `=` or `: `
            let keyed = first || (start >= 2 && c[start - 2] == '-' && c[start - 1] == ' ' && line_start(&c, start - 2));
            let k = skip_blanks(&c, i);
            let sep = c.get(k).copied();
            let colon = sep == Some(':') && c.get(k + 1).is_none_or(|x| x.is_whitespace());
            if keyed && (sep == Some('=') || colon) {
                Tok::Fn
            } else if matches!(w.as_str(), "true" | "false" | "yes" | "no" | "on" | "off" | "null") {
                Tok::Builtin
            } else {
                Tok::Plain
            }
        } else {
            i += 1;
            Tok::Plain
        };
        out.push(tok, &c[start..i]);
    }
    out.finish(src)
}

struct Out {
    lines: Vec<Line>,
}

impl Out {
    /// The lines, one per line of `src.lines()`.
    fn finish(self, src: &str) -> Vec<Line> {
        let mut lines = self.lines;
        // `str::lines` drops the empty piece after a final newline
        if src.ends_with('\n') {
            lines.pop();
        }
        lines
    }

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

    fn kinds_of(lines: Vec<Line>) -> Vec<(Tok, String)> {
        lines.into_iter().flatten().filter(|(t, _)| *t != Tok::Plain).collect()
    }

    #[test]
    fn detect() {
        assert_eq!(Lang::detect("src/main.rs", ""), Lang::Rust);
        assert_eq!(Lang::detect("infra/main.tf", ""), Lang::Hcl);
        assert_eq!(Lang::detect("deploy.sh", ""), Lang::Shell);
        assert_eq!(Lang::detect("scripts/iwr", "#!/usr/bin/env bash\n"), Lang::Shell);
        assert_eq!(Lang::detect("Cargo.toml", ""), Lang::Conf);
        assert_eq!(Lang::detect("README", "hello"), Lang::Plain);
    }

    #[test]
    fn shell_tokens() {
        let src = "#!/bin/bash\n# build\nNAME=x\nif [ -f \"$f\" ]; then\n  cargo build --release \"${NAME}-1\" | tee log\nfi\ncat <<'EOF'\n$not a var\nEOF\necho done\n";
        let lines = shell(src);
        assert_eq!(lines.len(), src.lines().count());
        let k = kinds_of(lines);
        let has = |t: Tok, s: &str| k.iter().any(|(a, b)| *a == t && b == s);
        assert!(has(Tok::Comment, "# build"));
        assert!(has(Tok::Var, "NAME"));
        assert!(has(Tok::Keyword, "if"));
        assert!(has(Tok::Attr, "-f"));
        assert!(has(Tok::Var, "$f"));
        assert!(has(Tok::Fn, "cargo"));
        assert!(has(Tok::Attr, "--release"));
        assert!(has(Tok::Var, "${NAME}"));
        assert!(has(Tok::Fn, "tee"));
        assert!(has(Tok::Str, "$not a var"));
        assert!(has(Tok::Fn, "echo"));
        assert!(!has(Tok::Fn, "build"));
    }

    #[test]
    fn hcl_tokens() {
        let src = "# vpc\nresource \"aws_vpc\" \"main\" {\n  cidr_block = var.cidr\n  tags = merge(local.tags, { Name = \"${var.name}-vpc\" })\n  enabled = true\n}\n";
        let lines = hcl(src);
        assert_eq!(lines.len(), src.lines().count());
        let k = kinds_of(lines);
        let has = |t: Tok, s: &str| k.iter().any(|(a, b)| *a == t && b == s);
        assert!(has(Tok::Comment, "# vpc"));
        assert!(has(Tok::Keyword, "resource"));
        assert!(has(Tok::Str, "\"aws_vpc\""));
        assert!(has(Tok::Fn, "cidr_block"));
        assert!(has(Tok::Var, "var"));
        assert!(has(Tok::Macro, "merge"));
        assert!(has(Tok::Var, "${var.name}"));
        assert!(has(Tok::Builtin, "true"));
    }

    #[test]
    fn conf_tokens() {
        let k = kinds_of(conf("[package]\nname = \"iwr\" # ok\nversion: 2\n- key: yes\n"));
        let has = |t: Tok, s: &str| k.iter().any(|(a, b)| *a == t && b == s);
        assert!(has(Tok::Type, "[package]"));
        assert!(has(Tok::Fn, "name"));
        assert!(has(Tok::Str, "\"iwr\""));
        assert!(has(Tok::Comment, "# ok"));
        assert!(has(Tok::Fn, "version"));
        assert!(has(Tok::Num, "2"));
        assert!(has(Tok::Fn, "key"));
        assert!(has(Tok::Builtin, "yes"));
    }

    #[test]
    fn raw_strings_and_ranges() {
        let k = kinds("let s = r#\"a \"quoted\" b\"#; for i in 0..10 {}");
        assert!(k.contains(&(Tok::Str, "r#\"a \"quoted\" b\"#".to_string())));
        assert!(k.contains(&(Tok::Num, "0".to_string())));
        assert!(k.contains(&(Tok::Num, "10".to_string())));
    }
}
