//! The tutorial of the UI is a codecast like any other (`crates/iwr-ui/assets/tutorial/`), recorded
//! once with a natural voice. Its spoken text never names the project, so the recording fits every
//! project; only its directive lines do, through placeholders that [`fill`] replaces with refs of
//! the project that is open.
//!
//! | Placeholder | Becomes |
//! |-------------|---------|
//! | `{dir}` | a folder with Rust files, as a folder ref (`src/storage/`) |
//! | `{file}` | a Rust file, outside that folder when there is one |
//! | `{fn}` | the entry point, else a function with a body |
//! | `{fn_line}` | the first line of that function (`src/main.rs:12`) |
//! | `{fn_b1}` | its first block (`crate::main/b1`) |
//!
//! A line whose placeholder the project cannot fill (no folder, no function) is left out, so a
//! cue keeps its sentence and only shows less.

use crate::model::Project;
use crate::script;

/// The refs the placeholders stand for in `p`, by name.
fn refs(p: &Project) -> Vec<(&'static str, String)> {
    let rust: Vec<&crate::model::SourceFile> = p.files.iter().filter(|f| f.rust).collect();
    let mut dirs: Vec<(&str, usize)> = Vec::new();
    for f in &rust {
        for (i, _) in f.path.match_indices('/') {
            let d = &f.path[..i];
            match dirs.iter_mut().find(|(x, _)| *x == d) {
                Some((_, n)) => *n += 1,
                None => dirs.push((d, 1)),
            }
        }
    }
    // the deepest folder holding two Rust files or more: `src/storage` says more than `src`
    let dir = dirs.iter().filter(|(d, _)| script::resolve_dir(p, d).as_deref() == Some(*d)).max_by_key(|(d, n)| (*n >= 2, d.matches('/').count(), *n)).map(|(d, _)| *d);
    let file = rust.iter().find(|f| dir.is_none_or(|d| !f.path.starts_with(&format!("{}/", d)))).or(rust.first()).map(|f| f.path.clone());
    let fun = crate::views::default_root(p)
        .filter(|id| p.item(*id).fn_info().is_some_and(|f| f.blocks.is_some()))
        .or_else(|| p.items.iter().find(|i| i.is_callable() && i.fn_info().is_some_and(|f| !f.is_test && f.blocks.is_some())).map(|i| i.id));
    let mut out = Vec::new();
    if let Some(d) = dir {
        out.push(("dir", format!("{}/", d)));
    }
    if let Some(f) = file {
        out.push(("file", f));
    }
    if let Some(id) = fun {
        let it = p.item(id);
        out.push(("fn", it.path.clone()));
        out.push(("fn_line", format!("{}:{}", p.file(it.span.file).path, it.span.line_start)));
        let b1 = format!("{}/b1", it.path);
        if script::resolve(p, &b1).is_some() {
            out.push(("fn_b1", b1));
        }
    }
    out
}

/// Replace the placeholders of a tutorial part with refs of `p`; drop the lines it cannot fill.
pub fn fill(p: &Project, text: &str) -> String {
    let refs = refs(p);
    let mut out = String::new();
    'lines: for line in text.lines() {
        let mut l = line.to_string();
        while let Some(a) = l.find('{') {
            let Some(b) = l[a..].find('}') else { break };
            let name = &l[a + 1..a + b];
            if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') || name.is_empty() {
                break;
            }
            match refs.iter().find(|(n, _)| *n == name) {
                Some((_, v)) => l.replace_range(a..a + b + 1, v),
                None => continue 'lines,
            }
        }
        out.push_str(&l);
        out.push('\n');
    }
    out
}
