//! Generated tours: a script line such as `architecture_tour` or `flow_tour:crate::run` becomes
//! cues built from the project when the script is played, so the tour follows the code.

use crate::guide::{self, label, told_doc, GuideOptions};
use crate::tr::{self, authored, led, t::tours as tt, Lang};
use crate::model::*;
use crate::script::{attach_note, resolve, step_cue, Cue, Note};

/// The tours, with their argument and what they show.
pub const TOURS: &[(&str, &str)] = &[
    ("architecture_tour", "[:module]  every module, with its `//!` doc"),
    ("types_tour", "[:module]  structs, enums and traits, with their `///` doc"),
    ("errors_tour", "[:module]  error types, where they come from, `?`, handling, panics"),
    ("entry_tour", "  main, then the public items"),
    ("flow_tour", ":fn  one function step by step, told from its `//` comments"),
    ("todo_tour", "[:module]  TODO / FIXME notes, `unsafe`, places that can panic"),
    ("files_tour", "[:module]  every file, with its `//!` doc and what it defines"),
];

/// A generated tour: its cues, and the side-panel notes they open (`| md:<title>`).
#[derive(Debug, Clone, Default)]
pub struct Tour {
    pub cues: Vec<Cue>,
    pub notes: Vec<Note>,
}

impl From<Vec<Cue>> for Tour {
    fn from(cues: Vec<Cue>) -> Self {
        Tour { cues, notes: vec![] }
    }
}

impl Tour {
    /// Add a cue; with a note, the side panel shows it from this cue on, else it closes.
    fn push(&mut self, mut c: Cue, note: Option<Note>) {
        attach_note(&mut self.notes, &self.cues, &mut c, note);
        self.cues.push(c);
    }

    /// One stop about a doc: the doc page's stops ([`guide::doc_page`]) when it has headings,
    /// tables or lists, the first saying `lead` before the page, else `lead` and `plain` alone.
    #[allow(clippy::too_many_arguments)]
    fn doc_stops(&mut self, lang: Lang, lead: &str, title: &str, doc: Option<&String>, plain: String, show: Option<String>, hl: Vec<String>) {
        match guide::doc_page(title, doc, lang) {
            Some(stops) => {
                for (k, st) in stops.into_iter().enumerate() {
                    let c = if k == 0 { cue(tr::sentences(lang, &[lead, &st.say]), show.clone(), hl.clone()) } else { Cue { say: st.say, ..Default::default() } };
                    self.push(c, Some(st.note));
                }
            }
            None => self.push(cue(tr::sentences(lang, &[lead, &plain]), show, hl), None),
        }
    }
}

/// Stops per tour; the rest is counted in the last cue.
const MAX_STOPS: usize = 20;

/// The cues of a tour line (`name_tour` or `name_tour:<ref>`), told in `lang`, or why it cannot be
/// built (for the author: in English).
pub fn generate(p: &Project, line: &str, lang: Lang) -> Result<Tour, String> {
    let (name, arg) = line.split_once(':').map_or((line, None), |(n, a)| (n, Some(a)));
    let scope = match (name, arg) {
        ("flow_tour" | "entry_tour", _) | (_, None) => None,
        (_, Some(a)) => Some(resolve(p, a).and_then(|r| r.module).ok_or_else(|| format!("`{}`: `{}` is not a module", line, a))?),
    };
    Ok(match name {
        "architecture_tour" => architecture(p, scope, lang),
        "types_tour" => types(p, scope, lang).into(),
        "errors_tour" => errors(p, scope, lang).into(),
        "entry_tour" => entry(p, lang).into(),
        "flow_tour" => flow(p, arg.ok_or("`flow_tour` needs a function: `flow_tour:crate::run`")?, lang)?.into(),
        "todo_tour" => todo(p, scope, lang).into(),
        "files_tour" => files(p, scope, lang),
        _ => return Err(format!("unknown tour `{}` (known: {})", name, TOURS.iter().map(|t| t.0).collect::<Vec<_>>().join(", "))),
    })
}

fn in_scope(p: &Project, m: ModuleId, scope: Option<ModuleId>) -> bool {
    let Some(s) = scope else { return true };
    let mut cur = Some(m);
    while let Some(c) = cur {
        if c == s {
            return true;
        }
        cur = p.module(c).parent;
    }
    false
}

/// `view` or `view:<module>`.
fn view(name: &str, p: &Project, scope: Option<ModuleId>) -> String {
    scope.map(|m| format!("{}:{}", name, p.module(m).path)).unwrap_or_else(|| name.to_string())
}

/// The module a scoped tour is limited to, as said (`net`).
fn scope_name(p: &Project, scope: Option<ModuleId>) -> Option<String> {
    scope.map(|m| p.module(m).path.trim_start_matches("crate::").to_string())
}

/// "First,", "Next,", "Then", … "And finally,".
fn lead(i: usize, n: usize, lang: Lang) -> &'static str {
    match i {
        0 => tt::lead_first(lang),
        _ if i + 1 == n => tt::lead_last(lang),
        _ => [tt::lead_next, tt::lead_then, tt::lead_there][i % 3](lang),
    }
}



fn cue(say: String, show: Option<String>, hl: Vec<String>) -> Cue {
    Cue { say, show, hl: Some(hl), ..Default::default() }
}

/// Keep `MAX_STOPS` cues after the intro; say how many were left out.
fn cap(mut cues: Vec<Cue>, total: usize, lang: Lang) -> Vec<Cue> {
    if total > MAX_STOPS {
        cues.truncate(MAX_STOPS + 1);
    }
    more(&mut cues, total, lang);
    cues
}

/// Say how many of `total` stops were left out after `MAX_STOPS`.
fn more(cues: &mut [Cue], total: usize, lang: Lang) {
    if let Some(last) = cues.last_mut().filter(|_| total > MAX_STOPS) {
        last.say = tr::sentences(lang, &[&last.say, &tt::more(lang, total - MAX_STOPS)]);
    }
}

fn is_test_item(p: &Project, it: &Item) -> bool {
    crate::views::is_test_module(p, it.module) || it.fn_info().map(|f| f.is_test).unwrap_or(false)
}

// ------------------------------------------------------------------ architecture

/// The architecture view, then one cue per module, glowing it and reading its `//!` doc.
fn architecture(p: &Project, scope: Option<ModuleId>, lang: Lang) -> Tour {
    let mods: Vec<&Module> = guide::tour_modules(p, scope);
    let g = crate::views::build(p, crate::views::Mode::Architecture, &crate::views::ViewOptions { module: scope, ..Default::default() });
    let root = match scope {
        Some(m) => p.module(m),
        None => &p.modules[0],
    };
    // the root's doc as a page: shown with the map, its sections read before the modules
    let mut page = guide::doc_page(&root.path.trim_start_matches("crate::").replace("crate", &p.name), root.doc.as_ref(), lang).unwrap_or_default().into_iter();
    let first = page.next();
    let goal = match (&first, scope) {
        (Some(st), _) => Some(st.say.clone()).filter(|s| !s.is_empty()),
        (None, Some(_)) => told_doc(root.doc.as_ref(), lang),
        (None, None) => root.doc.as_ref().and_then(|d| d.split("\n\n").next()).map(|d| authored(lang, &d.split_whitespace().collect::<Vec<_>>().join(" "))),
    };
    let visit = tt::arch_visit(lang, mods.len());
    let map = match scope_name(p, scope) {
        Some(sc) => tt::arch_map_in(lang, &p.name, sc),
        None => tt::arch_map(lang, &p.name),
    };
    let map = tr::sentences(lang, &[&map, goal.as_deref().unwrap_or("")]);
    let mut t = Tour::default();
    match first {
        Some(st) => {
            t.push(cue(map, Some("arch".into()), vec![]), Some(st.note));
            for st in page {
                t.push(Cue { say: st.say, ..Default::default() }, Some(st.note));
            }
            t.push(Cue { say: tr::sentences(lang, &[tt::arch_now_modules(lang), &visit]), ..Default::default() }, None);
        }
        None => t.push(cue(tr::sentences(lang, &[&map, &visit]), Some("arch".into()), vec![]), None),
    }
    let n = mods.len().min(MAX_STOPS);
    for (i, m) in mods.iter().take(MAX_STOPS).enumerate() {
        let inside = folded_count(&g, m);
        let folded = if inside > 0 { tt::arch_folded(lang, inside) } else { String::new() };
        let name = m.path.trim_start_matches("crate::");
        let lead = tr::sentences(lang, &[&guide::module_lead(lang, i, n, name), &folded]);
        t.doc_stops(lang, &lead, name, m.doc.as_ref(), guide::module_sentence(p, m, lang), None, vec![m.path.clone()]);
    }
    more(&mut t.cues, mods.len(), lang);
    t
}

/// How many modules the Architecture view folds into this box (its "+N inside" badge).
fn folded_count(g: &crate::views::Graph, m: &Module) -> usize {
    g.node(&format!("m{}", m.id)).and_then(|n| n.badges.iter().find_map(|b| b.strip_prefix('+')?.strip_suffix(" inside")?.parse().ok())).unwrap_or(0)
}

// ------------------------------------------------------------------ types

fn types(p: &Project, scope: Option<ModuleId>, lang: Lang) -> Vec<Cue> {
    let tys: Vec<&Item> = guide::tour_types(p).into_iter().filter(|i| in_scope(p, i.module, scope)).collect();
    let sc = scope_name(p, scope);
    if tys.is_empty() {
        let say = match &sc {
            Some(sc) => tt::types_none_in(lang, sc),
            None => tt::types_none(lang).to_string(),
        };
        return vec![cue(say, Some(view("types", p, scope)), vec![])];
    }
    let intro = match &sc {
        Some(sc) => tt::types_intro_in(lang, tys.len(), sc),
        None => tt::types_intro(lang, tys.len()),
    };
    let mut cues = vec![cue(intro, Some(view("types", p, scope)), vec![])];
    let n = tys.len().min(MAX_STOPS);
    for (i, it) in tys.iter().enumerate() {
        cues.push(cue(led(lang, lead(i, n, lang), &guide::type_sentence(p, it, lang)), None, vec![it.path.clone()]));
    }
    cap(cues, tys.len(), lang)
}

// ------------------------------------------------------------------ errors

/// The type item named by the last segment of a type text (`crate::E`, `E<T>` → `E`).
fn type_named<'a>(p: &'a Project, ty: &str) -> Option<&'a Item> {
    let last = ty.rsplit("::").next().unwrap_or(ty).split('<').next().unwrap_or(ty).trim();
    p.items.iter().find(|i| i.is_type() && i.name == last)
}

fn names(p: &Project, ids: &[ItemId], max: usize, lang: Lang) -> String {
    let mut v: Vec<String> = ids.iter().take(max).map(|i| format!("`{}`", label(p, *i))).collect();
    if ids.len() > max {
        v.push(crate::tr::t::guide::more_items(lang, ids.len() - max));
    }
    tr::join_and(lang, &v)
}

fn errors(p: &Project, scope: Option<ModuleId>, lang: Lang) -> Vec<Cue> {
    let fns: Vec<&Item> = p.functions().filter(|f| in_scope(p, f.module, scope) && !is_test_item(p, f)).collect();
    let fallible: Vec<&Item> = fns.iter().copied().filter(|f| f.fn_info().map(|i| i.returns_result).unwrap_or(false)).collect();
    let panicky: Vec<ItemId> = fns.iter().filter(|f| f.fn_info().map(|i| i.unwraps + i.expects + i.panics > 0).unwrap_or(false)).map(|f| f.id).collect();
    let show = Some(view("errors", p, scope));
    let sc = scope_name(p, scope);
    if fallible.is_empty() && panicky.is_empty() {
        let say = match &sc {
            Some(sc) => tt::errors_none_in(lang, sc),
            None => tt::errors_none(lang).to_string(),
        };
        return vec![cue(say, show, vec![])];
    }
    let fal = tt::errors_fns(lang, fallible.len());
    let pan = match panicky.len() {
        0 => tt::errors_no_panics(lang).to_string(),
        n => n.to_string(),
    };
    let intro = match &sc {
        Some(sc) => tt::errors_intro_in(lang, sc, fal, pan),
        None => tt::errors_intro(lang, fal, pan),
    };
    let mut cues = vec![cue(intro, show, vec![])];
    // each error type: what it is, who returns it, what converts into it
    let mut err_types: Vec<(ItemId, Vec<ItemId>)> = Vec::new();
    for f in &fallible {
        let Some(t) = f.fn_info().and_then(|i| i.result_err.as_deref()).and_then(|e| type_named(p, e)) else { continue };
        match err_types.iter_mut().find(|(id, _)| *id == t.id) {
            Some((_, from)) => from.push(f.id),
            None => err_types.push((t.id, vec![f.id])),
        }
    }
    for (t, producers) in &err_types {
        let it = p.item(*t);
        let froms: Vec<String> = p
            .items
            .iter()
            .filter_map(|i| match &i.extra {
                ItemExtra::Impl(ii) if ii.target_id == Some(*t) && ii.trait_name.as_deref().is_some_and(|n| n.starts_with("From<")) => {
                    Some(format!("`{}`", ii.trait_name.as_deref().unwrap().trim_start_matches("From<").trim_end_matches('>').rsplit("::").next().unwrap_or("")))
                }
                _ => None,
            })
            .collect();
        let from = if froms.is_empty() { String::new() } else { tt::errors_from(lang, froms.len(), tr::join_and(lang, &froms)) };
        let say = tr::sentences(lang, &[&guide::type_sentence(p, it, lang), &tt::errors_comes_back(lang, names(p, producers, 5, lang)), &from]);
        cues.push(cue(say, None, vec![it.path.clone()]));
    }
    // propagation with `?`
    let mut q: Vec<&Item> = fns.iter().copied().filter(|f| f.fn_info().map(|i| i.question_marks > 0).unwrap_or(false)).collect();
    q.sort_by_key(|f| std::cmp::Reverse(f.fn_info().unwrap().question_marks));
    if !q.is_empty() {
        let list: Vec<String> = q.iter().take(5).map(|f| tt::errors_times(lang, f.fn_info().unwrap().question_marks, label(p, f.id))).collect();
        let list = tr::join_and(lang, &list);
        cues.push(cue(
            if q.len() > 5 { tt::errors_question_more(lang, list, q.len() - 5) } else { tt::errors_question(lang, list) },
            None,
            q.iter().map(|f| f.path.clone()).collect(),
        ));
    }
    // handled: a fallible call that is neither `?` nor unwrapped
    let mut handlers: Vec<ItemId> = Vec::new();
    for c in &p.calls {
        let Some(t) = c.to else { continue };
        let callee_fallible = p.item(t).fn_info().map(|f| f.returns_result || f.returns_option).unwrap_or(false);
        if callee_fallible && !c.propagated && !c.unwrapped && c.kind != CallKind::Construct && fns.iter().any(|f| f.id == c.from) && !handlers.contains(&c.from) {
            handlers.push(c.from);
        }
    }
    if !handlers.is_empty() {
        cues.push(cue(
            tt::errors_handled(lang, names(p, &handlers, 5, lang)),
            None,
            handlers.iter().map(|h| p.item(*h).path.clone()).collect(),
        ));
    }
    if !panicky.is_empty() {
        cues.push(cue(
            tt::errors_panics(lang, names(p, &panicky, 5, lang)),
            None,
            panicky.iter().map(|h| p.item(*h).path.clone()).collect(),
        ));
    }
    cues
}

// ------------------------------------------------------------------ entry points

fn entry(p: &Project, lang: Lang) -> Vec<Cue> {
    let mut cues = Vec::new();
    if let Some(m) = p.main {
        let it = p.item(m);
        let mut seen = std::collections::HashSet::new();
        let callees: Vec<ItemId> = p.callees(m).iter().filter_map(|e| e.to).filter(|t| seen.insert(*t)).collect();
        let doc = told_doc(it.doc.as_ref(), lang).unwrap_or_default();
        let calls = if callees.is_empty() { String::new() } else { tt::entry_calls(lang, names(p, &callees, 5, lang)) };
        cues.push(cue(tr::sentences(lang, &[tt::entry_main(lang), &doc, &calls]), Some(format!("calls:{}", it.path)), vec![it.path.clone()]));
    }
    let public: Vec<&Item> = p
        .items
        .iter()
        .filter(|i| i.vis == Visibility::Public && Some(i.id) != p.main && !is_test_item(p, i))
        .filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Struct | ItemKind::Enum | ItemKind::Trait | ItemKind::TypeAlias))
        .collect();
    if public.is_empty() {
        if cues.is_empty() {
            cues.push(cue(tt::entry_nothing(lang).into(), Some("structure".into()), vec![]));
        }
        return cues;
    }
    let lib = p.files.iter().any(|f| f.path.ends_with("lib.rs"));
    let n_pub = public.len();
    cues.push(cue(
        match (cues.is_empty(), lib) {
            (true, true) => tt::entry_way_in_api(lang, n_pub),
            (true, false) => tt::entry_way_in_pub(lang, n_pub),
            (false, true) => tt::entry_then_api(lang, n_pub),
            (false, false) => tt::entry_then_pub(lang, n_pub),
        },
        Some("structure".into()),
        public.iter().map(|i| i.path.clone()).collect(),
    ));
    let n = public.len().min(MAX_STOPS);
    let base = cues.len() - 1;
    for (i, it) in public.iter().enumerate() {
        let what = match it.kind {
            ItemKind::Function => told_doc(it.doc.as_ref(), lang).unwrap_or_else(|| tt::entry_signature(lang, &it.signature)),
            _ => guide::type_sentence(p, it, lang),
        };
        let say = match it.kind {
            // "Then there is `f`." reads badly: a function gets "Then `f`."
            ItemKind::Function => {
                let l = if i > 0 && i + 1 != n && i % 3 == 2 { tt::lead_then(lang) } else { lead(i, n, lang) };
                tr::sentences(lang, &[&tt::entry_function(lang, l, label(p, it.id)), &what])
            }
            _ => led(lang, lead(i, n, lang), &what),
        };
        cues.push(cue(say, None, vec![it.path.clone()]));
    }
    // cap counts the stops after the public intro
    let mut head: Vec<Cue> = cues.drain(..base).collect();
    head.extend(cap(cues, public.len(), lang));
    head
}

// ------------------------------------------------------------------ one function

fn flow(p: &Project, f: &str, lang: Lang) -> Result<Vec<Cue>, String> {
    let id = resolve(p, f).and_then(|r| r.item).filter(|i| p.item(*i).is_callable()).ok_or_else(|| format!("`flow_tour:{}`: not a function", f))?;
    if p.item(id).fn_info().and_then(|i| i.cfg.as_ref()).is_none() {
        return Err(format!("`flow_tour:{}`: no body to walk", f));
    }
    let mut last = None;
    Ok(guide::function_steps(p, id, &GuideOptions { lang, ..Default::default() }).iter().map(|st| step_cue(p, st, &mut last)).collect())
}

// ------------------------------------------------------------------ rough edges

/// The smallest function around a line.
fn fn_at(p: &Project, file: FileId, line: usize) -> Option<&Item> {
    p.functions().filter(|f| f.span.file == file && f.span.contains_line(line)).min_by_key(|f| f.span.line_end - f.span.line_start)
}

/// `TODO` / `FIXME` / `XXX` / `HACK` in a `//` comment: (marker, text after it).
fn marker(comment: &str) -> Option<(&'static str, String)> {
    for m in ["TODO", "FIXME", "XXX", "HACK"] {
        if let Some(i) = comment.find(m) {
            let before = comment[..i].chars().last();
            let after = comment[i + m.len()..].chars().next();
            if before.is_none_or(|c| !c.is_alphanumeric()) && after.is_none_or(|c| !c.is_alphanumeric()) {
                let text = comment[i + m.len()..].trim_start_matches(|c: char| c == ':' || c == '(' || c.is_whitespace()).trim();
                return Some((m, text.to_string()));
            }
        }
    }
    None
}

fn todo(p: &Project, scope: Option<ModuleId>, lang: Lang) -> Vec<Cue> {
    // (file, line, what fn, sentence)
    let mut stops: Vec<(FileId, usize, Option<ItemId>, String)> = Vec::new();
    let (mut notes, mut unsafes, mut panics) = (0, 0, 0);
    for f in p.files.iter().filter(|f| f.is_code() && f.text) {
        let module = p.modules.iter().filter(|m| m.file == Some(f.id)).min_by_key(|m| m.path.len());
        if module.is_some_and(|m| !in_scope(p, m.id, scope) || crate::views::is_test_module(p, m.id)) || (module.is_none() && scope.is_some()) {
            continue;
        }
        for (i, line) in f.content.lines().enumerate() {
            let n = i + 1;
            let (code, comment) = line.split_once("//").map_or((line, ""), |(a, b)| (a, b));
            let owner = fn_at(p, f.id, n);
            if owner.is_some_and(|o| is_test_item(p, o)) {
                continue;
            }
            let at = owner.map(|o| label(p, o.id)).unwrap_or_else(|| f.path.clone());
            if let Some((m, text)) = marker(comment) {
                notes += 1;
                let say = if text.is_empty() { tt::todo_bare(lang, &at, m) } else { tt::todo_text(lang, &at, m, authored(lang, &text)) };
                stops.push((f.id, n, owner.map(|o| o.id), say));
            }
            if code.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == "unsafe") {
                unsafes += 1;
                stops.push((f.id, n, owner.map(|o| o.id), tt::todo_unsafe(lang, &at)));
            }
        }
    }
    for it in p.functions().filter(|f| in_scope(p, f.module, scope) && !is_test_item(p, f)) {
        let Some(cfg) = it.fn_info().and_then(|f| f.cfg.as_ref()) else { continue };
        for n in cfg.nodes.iter().filter(|n| n.kind == CfgNodeKind::Panic) {
            panics += 1;
            stops.push((n.span.file, n.span.line_start, Some(it.id), tt::todo_panic(lang, label(p, it.id), crate::shorten(n.detail.lines().next().unwrap_or(""), 60))));
        }
    }
    if stops.is_empty() {
        let say = match scope_name(p, scope) {
            Some(sc) => tt::todo_none_in(lang, sc),
            None => tt::todo_none(lang).to_string(),
        };
        return vec![cue(say, Some("arch".into()), vec![])];
    }
    stops.sort_by_key(|s| (s.0, s.1));
    stops.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1 && a.3 == b.3);
    let show_of = |s: &(FileId, usize, Option<ItemId>, String)| format!("code:{}", s.2.map(|i| p.item(i).path.clone()).unwrap_or_else(|| p.files[s.0].path.clone()));
    let (nn, nu, np) = (tt::todo_notes(lang, notes), tt::todo_unsafes(lang, unsafes), tt::todo_panics(lang, panics));
    let intro = match scope_name(p, scope) {
        Some(sc) => tt::todo_intro_in(lang, sc, nn, nu, np),
        None => tt::todo_intro(lang, nn, nu, np),
    };
    let mut cues = vec![cue(intro, Some(show_of(&stops[0])), vec![])];
    let mut last = cues[0].show.clone();
    for s in &stops {
        let show = Some(show_of(s));
        let changed = show != last;
        last = show.clone();
        cues.push(cue(s.3.clone(), if changed { show } else { None }, vec![format!("{}:{}", p.files[s.0].path, s.1)]));
    }
    cap(cues, stops.len(), lang)
}

// ------------------------------------------------------------------ files

/// How a file is said: `main`, `parser`, `net` for `net/mod.rs` (the view shows the full path).
fn file_name(f: &SourceFile) -> String {
    let mut parts = f.path.trim_end_matches(".rs").rsplit('/');
    let stem = parts.next().unwrap_or("");
    let stem = if stem == "mod" { parts.next().unwrap_or(stem) } else { stem };
    format!("`{}`", stem)
}

fn files(p: &Project, scope: Option<ModuleId>, lang: Lang) -> Tour {
    let mut stops: Vec<(&SourceFile, Option<&Module>)> = Vec::new();
    for f in p.files.iter().filter(|f| f.is_code()) {
        let module = p.modules.iter().filter(|m| m.file == Some(f.id)).min_by_key(|m| m.path.len());
        let keep = match module {
            Some(m) => in_scope(p, m.id, scope) && !crate::views::is_test_module(p, m.id),
            None => scope.is_none(),
        };
        if keep {
            stops.push((f, module));
        }
    }
    let intro = match scope_name(p, scope) {
        Some(sc) => tt::files_intro_in(lang, stops.len(), sc),
        None => tt::files_intro(lang, stops.len()),
    };
    let mut t = Tour::from(vec![cue(intro, stops.first().map(|s| format!("code:{}", s.0.path)), vec![])]);
    let n = stops.len().min(MAX_STOPS);
    for (i, (f, m)) in stops.iter().take(MAX_STOPS).enumerate() {
        let doc = m.and_then(|m| told_doc(m.doc.as_ref(), lang)).unwrap_or_else(|| crate::tr::t::guide::module_no_doc(lang).into());
        let defs: Vec<ItemId> = m.map(|m| m.items.iter().copied().filter(|i| !matches!(p.item(*i).kind, ItemKind::Method | ItemKind::Impl)).collect()).unwrap_or_default();
        let defines = if defs.is_empty() { String::new() } else { tt::files_defines(lang, names(p, &defs, 5, lang)) };
        let name = file_name(f);
        let lead = match i {
            0 => tt::files_first(lang, &name),
            _ if i + 1 == n => tt::files_last(lang, &name),
            _ => [tt::files_next, tt::files_then][i % 2](lang, &name),
        };
        let show = if i == 0 { None } else { Some(format!("code:{}", f.path)) };
        t.doc_stops(lang, &tr::sentences(lang, &[&lead, &defines]), &name, m.and_then(|m| m.doc.as_ref()), doc, show, vec![]);
    }
    more(&mut t.cues, stops.len(), lang);
    t
}
