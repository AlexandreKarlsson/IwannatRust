//! Build-time generator of the interface translations: TOML files in, typed Rust functions out.
//!
//! A crate keeps its texts in `i18n/<lang>/<module>.toml` (`en` is the reference language) and a
//! `_lang.toml` per language with its own name (`name = "日本語"`). Its `build.rs` calls [`build`],
//! which writes `$OUT_DIR/i18n.rs`: a `Lang` enum and a module `t` with one function per text.
//!
//! ```toml
//! # i18n/en/settings.toml           # i18n/ja/settings.toml
//! title = "Settings"                 title = "設定"
//! saved = "Saved {name}"             saved = "{name} を保存しました"
//! parts = { one = "{n} part", other = "{n} parts" }
//!                                    parts = "{n} パート"
//! ```
//!
//! becomes `t::settings::title() -> &'static str`, `t::settings::saved(name) -> String` and
//! `t::settings::parts(n: usize) -> String`. Every mistake is a build error, not a blank button:
//! a text missing in one language or only in one, a `{placeholder}` that differs between
//! languages, a plural form the language needs and lacks, a malformed brace. A call with the wrong
//! arguments does not compile either.
//!
//! Text syntax: `{name}` is an argument, `{{` and `}}` are literal braces. A table whose keys are
//! all plural categories (`zero one two few many other`, `other` required) is a plural, which
//! always takes `n: usize` first; any other table groups keys (`[menu] open = ".."` → `menu_open`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

/// Where the generated functions get the language from.
pub enum LangArg {
    /// Each function takes `lang: Lang` first (a library that serves several languages at once).
    Param,
    /// Each function reads this expression (an app with one current interface language).
    Global(&'static str),
}

/// The reference language: every key exists there first, with its arguments.
const BASE: &str = "en";
const CATEGORIES: &[&str] = &["zero", "one", "two", "few", "many", "other"];

/// Generate `$OUT_DIR/i18n.rs` from `<manifest dir>/<dir>` in a `build.rs`; any problem stops the
/// build with every error listed.
pub fn build(dir: &str, lang: LangArg) {
    let root = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("run from build.rs")).join(dir);
    println!("cargo:rerun-if-changed={}", root.display());
    for d in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        println!("cargo:rerun-if-changed={}", d.path().display());
        for f in std::fs::read_dir(d.path()).into_iter().flatten().flatten() {
            println!("cargo:rerun-if-changed={}", f.path().display());
        }
    }
    match generate(&root, &lang) {
        Ok(code) => {
            let out = Path::new(&std::env::var("OUT_DIR").expect("run from build.rs")).join("i18n.rs");
            std::fs::write(&out, code).expect("write i18n.rs");
        }
        Err(errs) => {
            for e in &errs {
                println!("cargo:warning=i18n: {e}");
            }
            panic!("{} translation error(s) in {}:\n  {}", errs.len(), root.display(), errs.join("\n  "));
        }
    }
}

/// A piece of a text: words as they are, or an argument.
#[derive(Debug, Clone, PartialEq)]
enum Seg {
    Lit(String),
    Arg(String),
}

/// One text in one language.
#[derive(Debug, Clone)]
enum Form {
    Simple(Vec<Seg>),
    /// by plural category, in [`CATEGORIES`] order
    Plural(Vec<(String, Vec<Seg>)>),
}

/// A language: its code, its own name, its texts by (module, key).
struct LangData {
    code: String,
    name: String,
    texts: BTreeMap<(String, String), Form>,
}

/// The plural categories a language uses, and the Rust condition that picks each one but the
/// last (`other`). Unknown languages are an error: add their rule here.
fn plural_rule(code: &str) -> Option<Vec<(&'static str, &'static str)>> {
    Some(match code {
        "en" | "de" | "nl" | "sv" | "da" | "nb" | "it" | "es" | "fi" | "el" => vec![("one", "n == 1"), ("other", "")],
        "fr" | "pt" => vec![("one", "n <= 1"), ("other", "")],
        "ja" | "zh" | "ko" | "th" | "vi" | "id" => vec![("other", "")],
        _ => return None,
    })
}

const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let",
    "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
    "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do",
    "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen", "lang",
];

/// A lowercase Rust identifier that is not a keyword.
fn ident_ok(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_ascii_lowercase() || f == '_')
        && s.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        && !KEYWORDS.contains(&s)
}

/// Split a text into words and `{arg}`s; `{{` / `}}` are literal braces.
fn parse_text(text: &str) -> Result<Vec<Seg>, String> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                lit.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                lit.push('}');
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match it.next() {
                        Some('}') => break,
                        Some(ch) => name.push(ch),
                        None => return Err(format!("unclosed `{{` in {text:?} (a literal brace is `{{{{`)")),
                    }
                }
                if !ident_ok(&name) {
                    return Err(format!("bad argument `{{{name}}}` in {text:?}: lowercase name, not a Rust keyword"));
                }
                if !lit.is_empty() {
                    out.push(Seg::Lit(std::mem::take(&mut lit)));
                }
                out.push(Seg::Arg(name));
            }
            '}' => return Err(format!("lone `}}` in {text:?} (a literal brace is `}}}}`)")),
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        out.push(Seg::Lit(lit));
    }
    Ok(out)
}

/// The arguments of a text, in order of first appearance.
fn args_of(segs: &[Seg], into: &mut Vec<String>) {
    for s in segs {
        if let Seg::Arg(a) = s {
            if !into.contains(a) {
                into.push(a.clone());
            }
        }
    }
}

impl Form {
    fn args(&self) -> Vec<String> {
        let mut v = Vec::new();
        match self {
            Form::Simple(s) => args_of(s, &mut v),
            Form::Plural(fs) => fs.iter().for_each(|(_, s)| args_of(s, &mut v)),
        }
        v
    }
}

/// Is this table a plural (only category keys, `other` among them)?
fn is_plural(t: &toml::Table) -> bool {
    t.contains_key("other") && t.keys().all(|k| CATEGORIES.contains(&k.as_str()))
}

/// Flatten one TOML file into `key → form`, nested tables joined with `_`.
fn read_table(prefix: &str, t: &toml::Table, file: &str, out: &mut BTreeMap<String, Form>, errs: &mut Vec<String>) {
    for (k, v) in t {
        let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}_{k}") };
        // a group's key may be a keyword (`[title] loop`): only the joined name is a function
        let leaf = !matches!(v, toml::Value::Table(t) if !is_plural(t));
        if k.is_empty() || !k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') || (leaf && !ident_ok(&key)) {
            errs.push(format!("{file}: key `{key}` must be a lowercase Rust identifier, not a keyword"));
            continue;
        }
        match v {
            toml::Value::String(s) => match parse_text(s) {
                Ok(segs) => {
                    out.insert(key, Form::Simple(segs));
                }
                Err(e) => errs.push(format!("{file}: {key}: {e}")),
            },
            toml::Value::Table(sub) if is_plural(sub) => {
                let mut forms = Vec::new();
                for cat in CATEGORIES {
                    match sub.get(*cat) {
                        Some(toml::Value::String(s)) => match parse_text(s) {
                            Ok(segs) => forms.push((cat.to_string(), segs)),
                            Err(e) => errs.push(format!("{file}: {key}.{cat}: {e}")),
                        },
                        Some(_) => errs.push(format!("{file}: {key}.{cat}: must be a string")),
                        None => {}
                    }
                }
                out.insert(key, Form::Plural(forms));
            }
            toml::Value::Table(sub) => read_table(&key, sub, file, out, errs),
            _ => errs.push(format!("{file}: {key}: must be a string or a table")),
        }
    }
}

/// Read every language directory under `root`.
fn read_langs(root: &Path, errs: &mut Vec<String>) -> Vec<LangData> {
    let mut langs = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else {
        errs.push(format!("no translation directory at {}", root.display()));
        return langs;
    };
    let mut dirs: Vec<_> = rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.path()).collect();
    dirs.sort();
    for d in dirs {
        let code = d.file_name().unwrap().to_string_lossy().to_string();
        if code.len() < 2 || code.len() > 3 || !code.chars().all(|c| c.is_ascii_lowercase()) {
            errs.push(format!("{code}/: a language directory is a 2-3 letter lowercase ISO code"));
            continue;
        }
        if plural_rule(&code).is_none() {
            errs.push(format!("{code}/: no plural rule known for `{code}`: add one in iwr-i18n-gen `plural_rule`"));
            continue;
        }
        let mut name = String::new();
        let mut texts = BTreeMap::new();
        let mut files: Vec<_> = std::fs::read_dir(&d).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        files.sort();
        for f in files {
            if f.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let stem = f.file_stem().unwrap().to_string_lossy().to_string();
            let label = format!("{code}/{stem}.toml");
            let src = match std::fs::read_to_string(&f) {
                Ok(s) => s,
                Err(e) => {
                    errs.push(format!("{label}: {e}"));
                    continue;
                }
            };
            let table: toml::Table = match src.parse() {
                Ok(t) => t,
                Err(e) => {
                    errs.push(format!("{label}: {e}"));
                    continue;
                }
            };
            if stem == "_lang" {
                match table.get("name") {
                    Some(toml::Value::String(s)) if !s.is_empty() => name = s.clone(),
                    _ => errs.push(format!("{label}: needs `name = \"<the language's own name>\"`")),
                }
                continue;
            }
            if !ident_ok(&stem) {
                errs.push(format!("{label}: file name must be a lowercase Rust identifier (it becomes a module)"));
                continue;
            }
            let mut flat = BTreeMap::new();
            read_table("", &table, &label, &mut flat, errs);
            for (k, v) in flat {
                texts.insert((stem.clone(), k), v);
            }
        }
        if name.is_empty() && !errs.iter().any(|e| e.starts_with(&format!("{code}/_lang"))) {
            errs.push(format!("{code}/_lang.toml: missing (it holds `name = \"<the language's own name>\"`)"));
        }
        langs.push(LangData { code, name, texts });
    }
    // the reference language first
    langs.sort_by_key(|l| l.code != BASE);
    langs
}

/// Check every language against the reference: same keys, same kind, same arguments, the plural
/// forms its rule needs.
fn check(langs: &[LangData], errs: &mut Vec<String>) {
    let Some(base) = langs.first().filter(|l| l.code == BASE) else {
        errs.push(format!("the reference language `{BASE}/` is missing"));
        return;
    };
    for l in langs {
        let rule: Vec<&str> = plural_rule(&l.code).unwrap().iter().map(|(c, _)| *c).collect();
        for ((m, k), form) in &l.texts {
            let at = format!("{}/{m}.toml: {k}", l.code);
            let Some(bform) = base.texts.get(&(m.clone(), k.clone())) else {
                errs.push(format!("{at}: not in {BASE}/ (remove it, or add it there first)"));
                continue;
            };
            if let Form::Plural(fs) = form {
                let have: Vec<&str> = fs.iter().map(|(c, _)| c.as_str()).collect();
                for c in &rule {
                    if !have.contains(c) {
                        errs.push(format!("{at}: plural form `{c}` missing (`{}` uses {})", l.code, rule.join(", ")));
                    }
                }
                for c in &have {
                    if !rule.contains(c) {
                        errs.push(format!("{at}: plural form `{c}` is not used by `{}` (it uses {})", l.code, rule.join(", ")));
                    }
                }
            }
            match (bform, form) {
                (Form::Simple(_), Form::Plural(_)) => errs.push(format!("{at}: plural here but not in {BASE}/")),
                (Form::Plural(_), Form::Simple(_)) if rule != ["other"] => {
                    errs.push(format!("{at}: must be a plural table ({})", rule.join(", ")))
                }
                _ => {}
            }
            let (mut a, mut b) = (form.args(), bform.args());
            if matches!(bform, Form::Plural(_)) {
                a.retain(|x| x != "n");
                b.retain(|x| x != "n");
            }
            let (sa, sb): (BTreeSet<_>, BTreeSet<_>) = (a.iter().collect(), b.iter().collect());
            if sa != sb {
                errs.push(format!("{at}: arguments {:?} differ from {BASE}/ {:?}", sa, sb));
            }
            let empty = |f: &Form| match f {
                Form::Simple(s) => s.is_empty(),
                Form::Plural(fs) => fs.iter().any(|(_, s)| s.is_empty()),
            };
            if empty(form) && !empty(bform) {
                errs.push(format!("{at}: empty translation"));
            }
        }
        for (m, k) in base.texts.keys() {
            if !l.texts.contains_key(&(m.clone(), k.clone())) {
                errs.push(format!("{}/{m}.toml: `{k}` missing (it is in {BASE}/)", l.code));
            }
        }
    }
}

/// `Lit` / `Arg` pieces as a `format!` string literal (or a plain one without arguments).
fn rust_fmt(segs: &[Seg]) -> String {
    let mut s = String::new();
    for seg in segs {
        match seg {
            Seg::Lit(l) => s.push_str(&l.replace('{', "{{").replace('}', "}}")),
            Seg::Arg(a) => {
                let _ = write!(s, "{{{a}}}");
            }
        }
    }
    format!("{s:?}")
}

fn plain(segs: &[Seg]) -> String {
    let s: String = segs.iter().map(|x| match x {
        Seg::Lit(l) => l.clone(),
        Seg::Arg(a) => format!("{{{a}}}"),
    }).collect();
    s
}

fn variant(code: &str) -> String {
    let mut c = code.chars();
    c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
}

/// Turn checked languages into Rust source.
fn emit(langs: &[LangData], lang_arg: &LangArg) -> String {
    let mut o = String::new();
    o.push_str("// @generated by iwr-i18n-gen from the i18n/ TOML files. Do not edit.\n\n");
    o.push_str("/// A language of the interface.\n#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]\npub enum Lang {\n");
    for (i, l) in langs.iter().enumerate() {
        let _ = writeln!(o, "    /// {}\n    {}{},", l.name, if i == 0 { "#[default]\n    " } else { "" }, variant(&l.code));
    }
    o.push_str("}\n\n#[allow(dead_code)]\nimpl Lang {\n    /// Every language, the reference one first.\n    pub const ALL: &'static [Lang] = &[");
    o.push_str(&langs.iter().map(|l| format!("Lang::{}", variant(&l.code))).collect::<Vec<_>>().join(", "));
    o.push_str("];\n\n    /// ISO code (`en`, `ja`).\n    pub fn code(self) -> &'static str {\n        match self {\n");
    for l in langs {
        let _ = writeln!(o, "            Lang::{} => {:?},", variant(&l.code), l.code);
    }
    o.push_str("        }\n    }\n\n    /// The language's own name (`English`, `日本語`).\n    pub fn name(self) -> &'static str {\n        match self {\n");
    for l in langs {
        let _ = writeln!(o, "            Lang::{} => {:?},", variant(&l.code), l.name);
    }
    o.push_str("        }\n    }\n\n    /// From a code or a browser locale (`ja`, `ja-JP`, `JA_jp`); `None` when not translated.\n    pub fn from_code(code: &str) -> Option<Lang> {\n        let c = code.trim().split(['-', '_']).next().unwrap_or(\"\").to_ascii_lowercase();\n        match c.as_str() {\n");
    for l in langs {
        let _ = writeln!(o, "            {:?} => Some(Lang::{}),", l.code, variant(&l.code));
    }
    o.push_str("            _ => None,\n        }\n    }\n}\n\n");

    let (param, sel) = match lang_arg {
        LangArg::Param => ("lang: Lang, ".to_string(), "lang".to_string()),
        LangArg::Global(e) => (String::new(), e.to_string()),
    };
    o.push_str("/// Every text of the interface, one module per file and one function per key.\n#[allow(clippy::all, unused_variables, dead_code)]\npub mod t {\n    use super::Lang;\n");
    let base = &langs[0];
    let mut modules: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (m, k) in base.texts.keys() {
        modules.entry(m).or_default().push(k);
    }
    for (m, keys) in modules {
        let _ = writeln!(o, "\n    pub mod {m} {{\n        use super::Lang;");
        for k in keys {
            let id = (m.to_string(), k.to_string());
            let bform = &base.texts[&id];
            let doc = match bform {
                Form::Simple(s) => plain(s),
                Form::Plural(fs) => fs.iter().map(|(c, s)| format!("{c}: {}", plain(s))).collect::<Vec<_>>().join(" | "),
            };
            let _ = writeln!(o, "        #[doc = {doc:?}]");
            match bform {
                Form::Simple(_) if bform.args().is_empty() => {
                    let _ = writeln!(o, "        pub fn {k}({}) -> &'static str {{\n            match {sel} {{", param.trim_end_matches(", "));
                    for l in langs {
                        let s = match &l.texts[&id] {
                            Form::Simple(s) => plain(s),
                            Form::Plural(_) => unreachable!(),
                        };
                        let _ = writeln!(o, "                Lang::{} => {:?},", variant(&l.code), s);
                    }
                    o.push_str("            }\n        }\n");
                }
                Form::Simple(_) => {
                    let args: Vec<String> = bform.args().iter().map(|a| format!("{a}: impl ::core::fmt::Display")).collect();
                    let _ = writeln!(o, "        pub fn {k}({param}{}) -> String {{\n            match {sel} {{", args.join(", "));
                    for l in langs {
                        let Form::Simple(s) = &l.texts[&id] else { unreachable!() };
                        let _ = writeln!(o, "                Lang::{} => format!({}),", variant(&l.code), rust_fmt(s));
                    }
                    o.push_str("            }\n        }\n");
                }
                Form::Plural(_) => {
                    let mut args = vec!["n: usize".to_string()];
                    args.extend(bform.args().iter().filter(|a| *a != "n").map(|a| format!("{a}: impl ::core::fmt::Display")));
                    let _ = writeln!(o, "        pub fn {k}({param}{}) -> String {{\n            match {sel} {{", args.join(", "));
                    for l in langs {
                        let rule = plural_rule(&l.code).unwrap();
                        let forms: Vec<(String, Vec<Seg>)> = match &l.texts[&id] {
                            Form::Plural(fs) => fs.clone(),
                            Form::Simple(s) => vec![("other".into(), s.clone())],
                        };
                        let get = |c: &str| forms.iter().find(|(x, _)| x == c).map(|(_, s)| rust_fmt(s)).unwrap_or_default();
                        // `if n == 1 { one } else { other }`, built from the last category back
                        let mut body = String::new();
                        for (c, cond) in rule.iter().rev() {
                            body = if cond.is_empty() { format!("format!({})", get(c)) } else { format!("if {cond} {{ format!({}) }} else {{ {body} }}", get(c)) };
                        }
                        let n_used = forms.iter().any(|(_, s)| s.contains(&Seg::Arg("n".into())));
                        let _ = writeln!(o, "                Lang::{} => {{ {}{body} }}", variant(&l.code), if n_used { "" } else { "let _ = n; " });
                    }
                    o.push_str("            }\n        }\n");
                }
            }
        }
        o.push_str("    }\n");
    }
    o.push_str("}\n");
    o
}

/// Read, check and generate; every error at once.
pub fn generate(root: &Path, lang: &LangArg) -> Result<String, Vec<String>> {
    let mut errs = Vec::new();
    let langs = read_langs(root, &mut errs);
    if errs.is_empty() {
        check(&langs, &mut errs);
    }
    if errs.is_empty() {
        Ok(emit(&langs, lang))
    } else {
        Err(errs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("iwr-i18n-{}-{}", std::process::id(), files.len() * 31 + files.iter().map(|f| f.1.len()).sum::<usize>()));
        let _ = std::fs::remove_dir_all(&d);
        for (p, c) in files {
            let f = d.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, c).unwrap();
        }
        d
    }

    #[test]
    fn parses_args_and_braces() {
        assert_eq!(parse_text("a {x} {{b}}").unwrap(), vec![Seg::Lit("a ".into()), Seg::Arg("x".into()), Seg::Lit(" {b}".into())]);
        assert!(parse_text("a {x").is_err());
        assert!(parse_text("a }").is_err());
        assert!(parse_text("{Bad}").is_err());
        assert!(parse_text("{type}").is_err());
    }

    #[test]
    fn generates_typed_functions() {
        let d = tree(&[
            ("en/_lang.toml", "name = \"English\""),
            ("ja/_lang.toml", "name = \"日本語\""),
            ("en/menu.toml", "title = \"Settings\"\nsaved = \"Saved {name}\"\nparts = { one = \"{n} part\", other = \"{n} parts\" }\n[sub]\nopen = \"Open\""),
            ("ja/menu.toml", "title = \"設定\"\nsaved = \"{name} を保存\"\nparts = \"{n} パート\"\n[sub]\nopen = \"開く\""),
        ]);
        let code = generate(&d, &LangArg::Global("crate::lang()")).unwrap();
        assert!(code.contains("pub fn title() -> &'static str"));
        assert!(code.contains("pub fn saved(name: impl ::core::fmt::Display) -> String"));
        assert!(code.contains("pub fn parts(n: usize) -> String"));
        assert!(code.contains("pub fn sub_open()"));
        assert!(code.contains("if n == 1 { format!(\"{n} part\") } else { format!(\"{n} parts\") }"), "{code}");
        let code = generate(&d, &LangArg::Param).unwrap();
        assert!(code.contains("pub fn title(lang: Lang) -> &'static str"));
    }

    #[test]
    fn reports_every_mismatch() {
        let d = tree(&[
            ("en/_lang.toml", "name = \"English\""),
            ("ja/_lang.toml", "name = \"日本語\""),
            ("en/m.toml", "a = \"A {x}\"\nb = \"B\"\nc = { one = \"c\", other = \"cs\" }"),
            ("ja/m.toml", "a = \"A {y}\"\nextra = \"E\"\nc = { one = \"c\", other = \"cs\" }"),
        ]);
        let errs = generate(&d, &LangArg::Param).unwrap_err();
        let all = errs.join("\n");
        assert!(all.contains("ja/m.toml: a: arguments"), "{all}");
        assert!(all.contains("`b` missing"), "{all}");
        assert!(all.contains("extra: not in en/"), "{all}");
        assert!(all.contains("plural form `one` is not used by `ja`"), "{all}");
    }

    #[test]
    fn english_plural_needs_one() {
        let d = tree(&[
            ("en/_lang.toml", "name = \"English\""),
            ("en/m.toml", "c = { other = \"cs\" }"),
        ]);
        let errs = generate(&d, &LangArg::Param).unwrap_err();
        assert!(errs.join("\n").contains("plural form `one` missing"));
    }
}
