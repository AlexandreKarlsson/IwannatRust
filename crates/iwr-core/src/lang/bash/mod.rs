//! Shell scripts (sh, bash), read with tree-sitter: one module per script, items for its
//! functions and top-level variables, and one for the script itself (named after the file)
//! whose body is the commands outside any function.
//!
//! A command is a call when it names a function of the project or another of its scripts
//! (`./build.sh`, `bash scripts/build.sh`); every other command is a plain statement. `#`
//! comments are the docs (above a function, at the top of the file) and the notes of the flow.

pub mod blocks;
pub(crate) mod body;
pub mod cfg;
mod items;

use crate::lang::ts::*;
use crate::lang::{Analyzed, Source};
use crate::model::*;
use crate::parse::Ctx;
use crate::resolve::Index;
use tree_sitter::Node;

/// A parser for shell scripts.
pub fn parser() -> tree_sitter::Parser {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_bash::LANGUAGE.into()).expect("tree-sitter-bash grammar");
    p
}

/// Module path of a script: its folders then its name without extension
/// (`scripts/deploy.sh` → `crate::scripts::deploy`).
pub fn module_path(rel_path: &str) -> String {
    let mut segs = vec!["crate".to_string()];
    let parts: Vec<&str> = rel_path.split('/').filter(|p| !p.is_empty()).collect();
    for (i, p) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        segs.push(if last { p.rsplit_once('.').map(|(s, _)| s).filter(|s| !s.is_empty()).unwrap_or(p).to_string() } else { p.to_string() });
    }
    segs.join("::")
}

/// The file name of a path.
pub(crate) fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The text of a node on one line.
pub(crate) fn line(n: Node, src: &str) -> String {
    one_line(raw(n, src))
}

/// The text of a `# comment` line, when `l` is one (not a `#!` line).
fn hash_comment(l: &str) -> Option<&str> {
    let t = l.trim();
    (t.starts_with('#') && !t.starts_with("#!")).then(|| t.trim_start_matches('#').trim())
}

/// A short single-line `#` comment directly above `line` (1-based): the title of the statement there.
pub(crate) fn note_above(lines: &[&str], line: usize) -> Option<String> {
    if line < 2 {
        return None;
    }
    let text = hash_comment(lines.get(line - 2)?)?;
    // part of a longer comment block: that is an explanation, not a title
    if line >= 3 && lines.get(line - 3).and_then(|l| hash_comment(l)).is_some() {
        return None;
    }
    (!text.is_empty() && text.chars().count() <= crate::views::NOTE_MAX).then(|| text.to_string())
}

/// The `#` comment block directly above `line` (1-based): the doc of the function there.
pub(crate) fn doc_above(lines: &[&str], line: usize) -> Option<String> {
    let mut i = line.checked_sub(1)?;
    let mut out = Vec::new();
    while i > 0 {
        match lines.get(i - 1).and_then(|l| hash_comment(l)) {
            Some(t) => out.push(t),
            None => break,
        }
        i -= 1;
    }
    out.reverse();
    let text = out.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// What a script is for: the first paragraph of the comment at its top, under the `#!` line.
pub(crate) fn file_doc(lines: &[&str]) -> Option<String> {
    let mut out = Vec::new();
    for l in lines.iter().skip_while(|l| l.starts_with("#!") || l.trim().is_empty()) {
        match hash_comment(l) {
            Some(t) if !t.is_empty() => out.push(t),
            _ => break,
        }
    }
    let text = out.join("\n");
    let lower = text.to_lowercase();
    (!text.is_empty() && !lower.contains("copyright") && !lower.contains("license") && !lower.starts_with("shellcheck")).then_some(text)
}

/// The name a command runs, as written (`./build.sh`, `echo`).
pub(crate) fn command_name(n: Node, src: &str) -> String {
    field(n, "name").map(|x| raw(x, src).to_string()).unwrap_or_default()
}

/// The arguments of a command, quotes taken off.
pub(crate) fn arguments(n: Node, src: &str) -> Vec<String> {
    let mut cur = n.walk();
    let v: Vec<String> = n.children_by_field_name("argument", &mut cur).map(|a| raw(a, src).trim_matches(['"', '\'']).to_string()).collect();
    v
}

/// `&&` or `||` between the two sides of a list.
pub(crate) fn list_operator(n: Node) -> &'static str {
    let mut cur = n.walk();
    let or = n.children(&mut cur).any(|c| c.kind() == "||");
    if or { "||" } else { "&&" }
}

/// The statements a function or a script runs: a function's `{ }` body, or the script's
/// commands outside its functions.
pub(crate) fn statements(n: Node) -> Vec<Node> {
    match n.kind() {
        "function_definition" => field(n, "body").map(named).unwrap_or_default(),
        _ => named(n).into_iter().filter(|c| c.kind() != "function_definition").collect(),
    }
}

/// First pass for the shell scripts of a project: parse each, create its module, collect its items.
pub(crate) fn collect(ctx: &mut Ctx, files: Vec<Source>) {
    let mut parser = parser();
    let had_main = ctx.project.main.is_some();
    let mut scripts: Vec<ItemId> = Vec::new();
    for Source { file, path, content } in files {
        ctx.parsing(file);
        let Some(tree) = parser.parse(&content, None) else {
            ctx.project.diagnostics.push(format!("{}: could not parse", path));
            continue;
        };
        if let Some(d) = syntax_error(tree.root_node(), &path) {
            ctx.project.diagnostics.push(d);
        }
        let mpath = module_path(&path);
        let mid = ctx.module_for(&mpath);
        if path.split('/').any(|c| c == "tests" || c == "test") {
            ctx.project.modules[mid].is_test = true;
        }
        let lines: Vec<&str> = content.lines().collect();
        if ctx.project.modules[mid].file.is_none() {
            ctx.project.modules[mid].file = Some(file);
            ctx.project.modules[mid].doc = file_doc(&lines);
        }
        scripts.extend(items::collect_file(ctx, &tree, file, &content, mid, &mpath));
    }
    // where a project of scripts starts: the first script that no other script names
    if !had_main {
        let named_elsewhere = |id: ItemId| {
            let it = &ctx.project.items[id];
            ctx.project.files.iter().any(|f| f.lang == Some(SourceLang::Bash) && f.id != it.span.file && f.content.contains(it.name.as_str()))
        };
        ctx.project.main = scripts.iter().copied().find(|s| !named_elsewhere(*s)).or(scripts.first().copied());
    }
}

/// Second pass for one shell function, or for a script's own commands.
pub(crate) fn analyze(p: &Project, ix: &Index, item: ItemId, file: FileId, node: Node) -> Analyzed {
    let fn_span = p.items[item].span;
    let src = &p.files[file].content;
    let (calls, stats) = body::analyze(p, ix, item, file, node, src);
    let cfg = cfg::build(node, file, &calls, fn_span, src);
    let blocks = blocks::build(node, file, &calls, fn_span, src, &p.items[item].signature);
    Analyzed { calls, cfg, blocks, stats }
}
