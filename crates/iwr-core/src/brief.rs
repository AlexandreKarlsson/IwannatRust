//! Compact text brief of a project for an LLM (or a person) to write a codecast script.

use crate::model::*;

#[derive(Debug, Clone, Default)]
pub struct BriefOptions {
    /// Only this function (by path) and the signatures of what it calls.
    pub only: Option<String>,
    /// Include function bodies as block lists.
    pub bodies: bool,
    /// Include modules / types listing.
    pub overview: bool,
}

fn short_doc(it: &Item) -> String {
    it.doc.as_ref().and_then(|d| d.lines().next()).map(|l| format!("  \"{}\"", crate::shorten(l, 90))).unwrap_or_default()
}

fn block_lines(p: &Project, b: &Block, depth: usize, out: &mut String) {
    for c in &b.children {
        let kind = match c.kind {
            BlockKind::Propagate => "?".to_string(),
            k => format!("{:?}", k).to_lowercase(),
        };
        let mut line = format!("  b{} {}{:<7}{}", c.id, "  ".repeat(depth), kind, crate::shorten(&c.label, 70));
        if !c.defines.is_empty() {
            line.push_str(&format!("  ＋{}", c.defines.join(" ")));
        }
        let uses: Vec<&String> = c.uses.iter().filter(|u| !c.defines.contains(u)).collect();
        if !uses.is_empty() {
            line.push_str(&format!("  {}", uses.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")));
        }
        for call in &c.calls {
            line.push_str(&format!("  → {}", p.item(*call).path));
        }
        out.push_str(&line);
        out.push('\n');
        block_lines(p, c, depth + 1, out);
    }
}

fn fn_brief(p: &Project, it: &Item, bodies: bool, out: &mut String) {
    let f = it.fn_info().unwrap();
    let file = &p.files[it.span.file].path;
    out.push_str(&format!("fn {}  {}:{}-{}{}\n", it.path, file, it.span.line_start, it.span.line_end, short_doc(it)));
    let mut facts = Vec::new();
    if f.returns_result {
        facts.push("Result".to_string());
    }
    if f.returns_option {
        facts.push("Option".to_string());
    }
    if f.is_recursive {
        facts.push("recursive".to_string());
    }
    if f.loops > 0 {
        facts.push(format!("{} loop", f.loops));
    }
    if f.branches > 0 {
        facts.push(format!("{} branch", f.branches));
    }
    if f.question_marks > 0 {
        facts.push(format!("{}×?", f.question_marks));
    }
    out.push_str(&format!("  {}{}\n", it.signature, if facts.is_empty() { String::new() } else { format!("   {}", facts.join(" · ")) }));
    if bodies {
        if let Some(b) = &f.blocks {
            block_lines(p, b, 0, out);
        }
    }
}

pub fn build(p: &Project, opts: &BriefOptions) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}  ({} rust files, {} modules, {} functions)\n", p.name, p.files.iter().filter(|f| f.rust).count(), p.modules.len(), p.functions().count()));
    out.push_str("refs: <item path> | <item path>/b<n> (block n) | <file>:<line>[-<line>][:<c1>-<c2>]\nviews: code calls flow arch branches structure types errors\n\n");
    if let Some(only) = &opts.only {
        let Some(id) = crate::script::resolve(p, only).and_then(|r| r.item) else {
            out.push_str(&format!("unknown function {}\n", only));
            return out;
        };
        let it = p.item(id);
        fn_brief(p, it, true, &mut out);
        let mut callees: Vec<ItemId> = p.callees(id).iter().filter_map(|c| c.to).collect();
        callees.sort();
        callees.dedup();
        if !callees.is_empty() {
            out.push_str("\ncalls:\n");
            for c in callees {
                let ci = p.item(c);
                if ci.is_callable() {
                    out.push_str(&format!("  {}  {}{}\n", ci.path, ci.signature, short_doc(ci)));
                } else {
                    out.push_str(&format!("  {}  {}\n", ci.path, ci.kind.label()));
                }
            }
        }
        let mut callers: Vec<ItemId> = p.callers(id).iter().map(|c| c.from).collect();
        callers.sort();
        callers.dedup();
        if !callers.is_empty() {
            out.push_str(&format!("called by: {}\n", callers.iter().map(|c| p.item(*c).path.clone()).collect::<Vec<_>>().join(", ")));
        }
        return out;
    }
    if opts.overview {
        for m in &p.modules {
            if m.items.is_empty() && m.children.is_empty() {
                continue;
            }
            let file = m.file.map(|f| p.files[f].path.clone()).unwrap_or_default();
            let doc = m.doc.as_ref().and_then(|d| d.lines().next()).map(|l| format!("  \"{}\"", crate::shorten(l, 80))).unwrap_or_default();
            out.push_str(&format!("mod {:<28} {}{}\n", m.path, file, doc));
        }
        out.push('\n');
        for it in &p.items {
            match &it.extra {
                ItemExtra::Struct(s) => {
                    let f: Vec<String> = s.fields.iter().map(|f| format!("{}: {}", f.name, crate::shorten(&f.ty, 24))).collect();
                    out.push_str(&format!("struct {}  {{{}}}{}\n", it.path, f.join(", "), short_doc(it)));
                }
                ItemExtra::Enum(e) => {
                    let v: Vec<String> = e.variants.iter().map(|v| if v.fields.is_empty() { v.name.clone() } else { format!("{}({})", v.name, v.fields.iter().map(|f| crate::shorten(&f.ty, 20)).collect::<Vec<_>>().join(", ")) }).collect();
                    out.push_str(&format!("enum {}  {}{}\n", it.path, v.join(" | "), short_doc(it)));
                }
                ItemExtra::Trait(t) => {
                    let m: Vec<String> = t.methods.iter().map(|m| format!("{}()", p.item(*m).name)).collect();
                    let impls: Vec<String> = p.type_rels.iter().filter(|r| r.to == it.id && r.kind == TypeRelKind::Implements).map(|r| p.item(r.from).name.clone()).collect();
                    out.push_str(&format!("trait {}  {}{}{}\n", it.path, m.join(" "), if impls.is_empty() { String::new() } else { format!("   impl by {}", impls.join(", ")) }, short_doc(it)));
                }
                _ => {}
            }
        }
        out.push('\n');
    }
    for it in p.functions() {
        if it.fn_info().map(|f| f.is_test).unwrap_or(false) {
            continue;
        }
        fn_brief(p, it, opts.bodies, &mut out);
    }
    out
}
