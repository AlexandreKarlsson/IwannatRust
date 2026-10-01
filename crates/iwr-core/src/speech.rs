//! How code is spoken. Browser TTS reads `crate::parser::parse_all` as "crate colon colon parser
//! colon colon parse underscore all"; this module rewrites code tokens into words ("parse all")
//! while the caption keeps the code.
//!
//! [`segments`] splits a cue into plain text and code segments (backtick spans, plus bare tokens
//! that look like code: `parse_all`, `Task::weight`, `&tasks`, `Option<Task>`, `t.clone()`…),
//! each code segment carrying its spoken form from [`say_code`]. [`spoken`] joins them for TTS.
//! `pronounce` pairs (`pronounce: Dioxus = dee ox us` in a script header or glossary) win over
//! the rules.

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Text(String),
    /// a code token as written, and how to say it
    Code { code: String, say: String },
}

/// `written = spoken` pairs.
pub type Pronounce = [(String, String)];

/// Built-in pronunciations.
pub fn builtin_pronounce() -> Vec<(String, String)> {
    [("IwannatRust", "I wanna Rust"), ("iwr", "I W R"), ("TTS", "text to speech"), ("rs", "R S"), ("md", "markdown"), ("toml", "tommel"), ("wasm", "wazm"), ("usize", "u size"), ("isize", "i size"), ("str", "stir"), ("dyn", "dine"), ("impl", "impul"), ("enum", "ee num"), ("println", "print line"), ("eprintln", "e print line"), ("cfg", "config"), ("Vec", "veck"), ("HashMap", "hash map"), ("ok_or", "ok or"), ("ok_or_else", "ok or else"), ("map_err", "map err"), ("Deserialize", "deserialize")]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

/// Parse `pronounce: written = spoken` lines out of any text (script header, glossary).
pub fn parse_pronounce(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("pronounce:"))
        .filter_map(|rest| rest.split_once('=').map(|(a, b)| (a.trim().to_string(), b.trim().to_string())))
        .filter(|(a, b)| !a.is_empty() && !b.is_empty())
        .collect()
}

/// Does a bare word (no backticks) look like code? Paths, snake_case, calls, generics, references,
/// attributes, known macros, CamelCase (two humps), primitive types, lifetimes, a lone `?`.
pub fn looks_like_code(tok: &str) -> bool {
    let t = tok;
    if t.is_empty() {
        return false;
    }
    if t == "?" || t.starts_with("#[") || t.starts_with('&') && t.len() > 1 || t.starts_with('|') && t.len() > 1 || t.contains("::") || t.contains("->") {
        return true;
    }
    if t.contains("()") || (t.contains('(') && t.ends_with(')')) {
        return true;
    }
    if t.contains('<') && t.contains('>') {
        return true;
    }
    if t.len() > 2 && t.starts_with('\'') && t[1..].chars().all(|c| c.is_ascii_lowercase()) {
        return true;
    }
    if let Some(name) = t.strip_suffix('!') {
        return MACROS.contains(&name);
    }
    // snake_case: an underscore between word characters
    let b = t.as_bytes();
    if (1..b.len().saturating_sub(1)).any(|i| b[i] == b'_' && b[i - 1].is_ascii_alphanumeric() && b[i + 1].is_ascii_alphanumeric()) {
        return true;
    }
    if is_primitive(t) {
        return true;
    }
    // CamelCase: an uppercase letter after a lowercase one (MemoryStorage, HashMap), not "Rust"
    let mut prev_lower = false;
    for c in t.chars() {
        if c.is_uppercase() && prev_lower {
            return true;
        }
        prev_lower = c.is_lowercase();
    }
    // method chain on an identifier: t.clone, task.name (not a sentence end, not a number)
    if let Some((a, c)) = t.split_once('.') {
        if !a.is_empty() && !c.is_empty() && a.chars().all(|ch| ch.is_alphanumeric() || ch == '_') && c.chars().all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '.') && !a.chars().all(|ch| ch.is_ascii_digit()) && c.chars().any(|ch| ch.is_alphabetic()) {
            return true;
        }
    }
    false
}

const MACROS: [&str; 14] = ["println", "eprintln", "print", "eprint", "format", "vec", "panic", "assert", "assert_eq", "write", "writeln", "dbg", "todo", "matches"];

fn is_primitive(t: &str) -> bool {
    matches!(t, "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64")
}

/// Keywords TTS mangles, and primitive types.
fn word(id: &str, pron: &Pronounce) -> String {
    if let Some((_, say)) = pron.iter().find(|(w, _)| w == id) {
        return say.clone();
    }
    match id {
        "fn" => "function".into(),
        "mod" => "module".into(),
        "mut" => "mute".into(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" | "f32" | "f64" => format!("{} {}", &id[..1], &id[1..]),
        "usize" => "u size".into(),
        "isize" => "i size".into(),
        _ => split_identifier(id),
    }
}

/// `parse_all` → "parse all", `MemoryStorage` → "Memory Storage", `HTMLParser` → "HTML Parser",
/// `INPUT` → "INPUT".
pub fn split_identifier(id: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = id.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            continue;
        }
        if i > 0 && c.is_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false);
            if (prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next_lower)) && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// The spoken form of one code token or expression.
pub fn say_code(code: &str, pron: &Pronounce) -> String {
    let code = code.trim();
    if let Some((_, say)) = pron.iter().find(|(w, _)| w == code) {
        return say.clone();
    }
    let mut p = Parser { s: code.chars().collect(), i: 0, pron, out: Vec::new() };
    p.expr(0);
    let mut s = p.out.join(" ");
    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
    s.replace(" ,", ",").trim().trim_matches(',').trim().to_string()
}

struct Parser<'a> {
    s: Vec<char>,
    i: usize,
    pron: &'a Pronounce,
    out: Vec<String>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn peek2(&self) -> Option<char> {
        self.s.get(self.i + 1).copied()
    }
    fn starts(&self, pat: &str) -> bool {
        pat.chars().enumerate().all(|(k, c)| self.s.get(self.i + k) == Some(&c))
    }
    fn push(&mut self, w: impl Into<String>) {
        let w: String = w.into();
        if !w.is_empty() {
            self.out.push(w);
        }
    }

    /// Parse until a closing bracket of the given depth-0 kind (or the end). `in_args` turns
    /// commas into "and".
    fn expr(&mut self, depth: usize) {
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\n' | ';' | '{' | '}' => self.i += 1,
                ')' | '>' | ']' | '|' if depth > 0 => return,
                '#' if self.peek2() == Some('[') => {
                    self.i += 2;
                    self.push("the attribute");
                    self.expr(depth + 1);
                    self.i += 1; // ]
                }
                '"' => {
                    self.i += 1;
                    let mut lit = String::new();
                    while let Some(ch) = self.peek() {
                        self.i += 1;
                        if ch == '"' {
                            break;
                        }
                        if ch == '\\' {
                            self.i += 1;
                            lit.push(' ');
                            continue;
                        }
                        lit.push(ch);
                    }
                    let lit = lit.replace(['{', '}'], " ");
                    let lit = lit.split_whitespace().collect::<Vec<_>>().join(" ");
                    if lit.is_empty() {
                        self.push("an empty string");
                    } else {
                        self.push(format!("the string {}", lit));
                    }
                }
                '\'' if self.peek2().map(|n| n.is_ascii_alphabetic()).unwrap_or(false) => {
                    self.i += 1;
                    let id = self.ident();
                    self.push(format!("lifetime {}", id));
                }
                '&' => {
                    self.i += 1;
                    if self.starts("&") {
                        self.i += 1;
                        self.push("and");
                    } else if self.starts("mut ") {
                        self.i += 4;
                        self.push("a mutable reference to");
                    } else if self.starts("self") && !self.s.get(self.i + 4).map(|c| c.is_alphanumeric()).unwrap_or(false) {
                        self.i += 4;
                        self.push("self by reference");
                    } else if self.starts("str") && !self.s.get(self.i + 3).map(|c| c.is_alphanumeric()).unwrap_or(false) {
                        self.i += 3;
                        self.push("string slice");
                    } else if self.starts("[") {
                        self.i += 1;
                        self.push("a slice of");
                        self.expr(depth + 1);
                        self.i += 1;
                    } else {
                        self.push("a reference to");
                    }
                }
                '|' => {
                    // closure: |a, b| body
                    self.i += 1;
                    if self.starts("|") {
                        self.i += 1;
                        self.push("or");
                        continue;
                    }
                    let mut params: Vec<String> = Vec::new();
                    while let Some(ch) = self.peek() {
                        self.i += 1;
                        if ch == '|' {
                            break;
                        }
                        if ch == ',' {
                            params.push("and".into());
                            continue;
                        }
                        if ch == ' ' {
                            continue;
                        }
                        let mut id = String::from(ch);
                        while let Some(n) = self.peek().filter(|n| n.is_alphanumeric() || *n == '_') {
                            id.push(n);
                            self.i += 1;
                        }
                        params.push(if id == "_" { "nothing".into() } else { word(&id, self.pron) });
                    }
                    let plist = params.join(" ");
                    self.push(if plist.is_empty() { "a closure with no arguments," .to_string() } else { format!("the closure taking {},", plist) });
                }
                '-' if self.peek2() == Some('>') => {
                    self.i += 2;
                    self.push("returns");
                }
                '=' if self.peek2() == Some('=') => {
                    self.i += 2;
                    self.push("equals");
                }
                '=' if self.peek2() == Some('>') => {
                    self.i += 2;
                    self.push("gives");
                }
                '!' if self.peek2() == Some('=') => {
                    self.i += 2;
                    self.push("is not");
                }
                '+' | '-' | '*' | '/' | '%' if self.peek2() == Some('=') => {
                    self.i += 2;
                    self.push(match c { '+' => "plus equals", '-' => "minus equals", '*' => "times equals", '/' => "divided equals", _ => "modulo equals" });
                }
                '=' => {
                    self.i += 1;
                    self.push("equals");
                }
                '<' => {
                    self.i += 1;
                    if self.peek() == Some('=') {
                        self.i += 1;
                        self.push("is at most");
                    } else {
                        self.push("is less than");
                    }
                }
                '>' => {
                    self.i += 1;
                    if self.peek() == Some('=') {
                        self.i += 1;
                        self.push("is at least");
                    } else {
                        self.push("is greater than");
                    }
                }
                '+' => { self.i += 1; self.push("plus"); }
                '-' => { self.i += 1; self.push("minus"); }
                '*' => { self.i += 1; self.push("times"); }
                '/' => { self.i += 1; self.push("over"); }
                '%' => { self.i += 1; self.push("modulo"); }
                '!' => { self.i += 1; self.push("not"); }
                '?' => { self.i += 1; self.push(", question mark"); }
                ',' => { self.i += 1; self.push(if depth > 0 { "and" } else { "," }); }
                ':' if self.peek2() == Some(':') => { self.i += 2; }
                ':' => { self.i += 1; self.push("of type"); }
                '.' if self.peek2() == Some('.') => {
                    self.i += 2;
                    if self.peek() == Some('=') {
                        self.i += 1;
                        self.push("up to and including");
                    } else {
                        self.push("up to");
                    }
                }
                '.' => { self.i += 1; self.push("dot"); }
                '(' => {
                    self.i += 1;
                    let start = self.out.len();
                    self.expr(depth + 1);
                    self.i += 1; // )
                    if self.out.len() > start {
                        self.out.insert(start, "of".into());
                    }
                }
                '[' => {
                    self.i += 1;
                    self.push("at");
                    self.expr(depth + 1);
                    self.i += 1;
                }
                c if c.is_ascii_digit() => {
                    let mut n = String::new();
                    while let Some(ch) = self.peek().filter(|ch| ch.is_ascii_digit() || *ch == '_' || (*ch == '.' && self.peek2().map(|d| d.is_ascii_digit()).unwrap_or(false))) {
                        if ch != '_' {
                            n.push(ch);
                        }
                        self.i += 1;
                    }
                    let suffix = self.ident();
                    self.push(n);
                    if !suffix.is_empty() {
                        self.push(word(&suffix, self.pron));
                    }
                }
                c if c.is_alphanumeric() || c == '_' => self.path(depth),
                _ => self.i += 1,
            }
        }
    }

    fn ident(&mut self) -> String {
        let mut id = String::new();
        while let Some(ch) = self.peek().filter(|ch| ch.is_alphanumeric() || *ch == '_') {
            id.push(ch);
            self.i += 1;
        }
        id
    }

    /// `a::b::C::d<T>(args)!`: drop `crate`/`self`/`super` and leading lowercase modules when a
    /// later segment follows; speak generics as "of"; macros lose their `!` and arguments.
    fn path(&mut self, depth: usize) {
        let mut segs: Vec<String> = vec![self.ident()];
        while self.starts("::") {
            self.i += 2;
            if self.peek() == Some('<') {
                break;
            }
            let id = self.ident();
            if id.is_empty() {
                break;
            }
            segs.push(id);
        }
        segs.retain(|s| !matches!(s.as_str(), "crate" | "self" | "super" | "std" | "core"));
        if segs.is_empty() {
            self.push("crate");
            return;
        }
        // module::module::item → item ; Type::method → "Type method"
        while segs.len() > 1 && segs[0].chars().next().map(|c| c.is_lowercase()).unwrap_or(false) && !is_primitive(&segs[0]) {
            segs.remove(0);
        }
        let is_macro = self.peek() == Some('!') && self.peek2() != Some('=');
        let words: Vec<String> = segs.iter().map(|s| word(s, self.pron)).collect();
        self.push(words.join(" "));
        if is_macro {
            self.i += 1;
            if self.peek() == Some('(') || self.peek() == Some('[') {
                // skip the macro arguments entirely
                let open = self.peek().unwrap();
                let close = if open == '(' { ')' } else { ']' };
                let mut d = 0;
                while let Some(ch) = self.peek() {
                    self.i += 1;
                    if ch == open {
                        d += 1;
                    } else if ch == close {
                        d -= 1;
                        if d == 0 {
                            break;
                        }
                    }
                }
            }
            return;
        }
        if self.peek() == Some('<') {
            self.i += 1;
            self.push("of");
            self.expr(depth + 1);
            self.i += 1; // >
        }
    }
}

/// Split markdown-ish cue text into text and code segments. Fenced code blocks are dropped (they
/// are shown, not spoken), `**` and list bullets removed, backtick spans become code segments, and
/// bare words that [`looks_like_code`] become code segments too.
pub fn segments(text: &str, pron: &Pronounce) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut in_code = false;
    let mut first = true;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code || t.is_empty() {
            continue;
        }
        let t = t.trim_start_matches("• ").trim_start_matches("- ").replace("**", "");
        if !first {
            push_text(&mut out, " ");
        }
        first = false;
        let mut rest = t.as_str();
        while !rest.is_empty() {
            match rest.find('`') {
                Some(i) => {
                    segment_words(&mut out, &rest[..i], pron);
                    match rest[i + 1..].find('`') {
                        Some(j) => {
                            let code = &rest[i + 1..i + 1 + j];
                            if !code.trim().is_empty() {
                                out.push(Segment::Code { code: code.to_string(), say: say_code(code, pron) });
                            }
                            rest = &rest[i + 2 + j..];
                        }
                        None => {
                            segment_words(&mut out, &rest[i + 1..], pron);
                            rest = "";
                        }
                    }
                }
                None => {
                    segment_words(&mut out, rest, pron);
                    rest = "";
                }
            }
        }
    }
    out
}

fn push_text(out: &mut Vec<Segment>, s: &str) {
    if s.is_empty() {
        return;
    }
    if let Some(Segment::Text(t)) = out.last_mut() {
        t.push_str(s);
    } else {
        out.push(Segment::Text(s.to_string()));
    }
}

/// Plain text: every whitespace-separated word that looks like code becomes a code segment
/// (surrounding punctuation stays text).
fn segment_words(out: &mut Vec<Segment>, text: &str, pron: &Pronounce) {
    let mut rest = text;
    while !rest.is_empty() {
        let ws_end = rest.find(|c: char| !c.is_whitespace()).unwrap_or(rest.len());
        push_text(out, &rest[..ws_end]);
        rest = &rest[ws_end..];
        if rest.is_empty() {
            break;
        }
        let w_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let w = &rest[..w_end];
        rest = &rest[w_end..];
        // peel leading "(" and trailing sentence punctuation
        let lead = w.len() - w.trim_start_matches(['(', '"', '“']).len();
        let (lead_s, core) = w.split_at(lead);
        let mut core = core;
        while let Some(last) = core.chars().next_back() {
            if !matches!(last, '.' | ',' | ';' | ':' | '"' | '”' | ')' | '?' | '!') {
                break;
            }
            // keep `()` and a trailing `!` of a macro when the token is code as it stands
            if (last == ')' && core.contains('(') && looks_like_code(core)) || (last == '!' && looks_like_code(core)) {
                break;
            }
            core = &core[..core.len() - last.len_utf8()];
        }
        let trail = &w[lead + core.len()..];
        if looks_like_code(core) && pron.iter().all(|(a, _)| a != core) || pron.iter().any(|(a, _)| a == core) {
            push_text(out, lead_s);
            out.push(Segment::Code { code: core.to_string(), say: say_code(core, pron) });
            push_text(out, trail);
        } else {
            push_text(out, w);
        }
    }
}

/// The text to hand to TTS: segments joined, a short pause (comma) after a spoken expression when
/// the sentence goes on.
pub fn spoken(text: &str, pron: &Pronounce) -> String {
    let segs = segments(text, pron);
    let mut out = String::new();
    for (i, s) in segs.iter().enumerate() {
        match s {
            Segment::Text(t) => out.push_str(t),
            Segment::Code { code, say } => {
                out.push_str(say);
                // a breath after a spoken expression (not after a single name) when the sentence goes on
                let next = segs.get(i + 1).and_then(|n| if let Segment::Text(t) = n { Some(t.as_str()) } else { None }).unwrap_or("");
                let continues = next.starts_with(' ') && next.trim_start().chars().next().map(|c| c.is_lowercase()).unwrap_or(false);
                if continues && code.trim().contains(' ') && !say.ends_with(',') {
                    out.push(',');
                }
            }
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    out.replace(" ,", ",").replace(",,", ",").replace(",.", ".")
}

/// Code tokens whose spoken form still looks like code (nothing matched): for `iwr check`.
pub fn unspoken(text: &str, pron: &Pronounce) -> Vec<String> {
    segments(text, pron).into_iter().filter_map(|s| if let Segment::Code { code, say } = s { if say.contains("::") || say.contains('_') || say.is_empty() { Some(code) } else { None } } else { None }).collect()
}
