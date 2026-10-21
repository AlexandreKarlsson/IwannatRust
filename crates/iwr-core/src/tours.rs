//! Generated tours: a script line such as `architecture_tour` or `flow_tour:crate::run` becomes
//! cues built from the project when the script is played, so the tour follows the code.

use crate::guide::{self, doc_sentence, join_and, label, GuideOptions};
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
    fn doc_stops(&mut self, lead: &str, title: &str, doc: Option<&String>, plain: String, show: Option<String>, hl: Vec<String>) {
        match guide::doc_page(title, doc) {
            Some(stops) => {
                for (k, st) in stops.into_iter().enumerate() {
                    let c = if k == 0 { cue(format!("{} {}", lead, st.say), show.clone(), hl.clone()) } else { Cue { say: st.say, ..Default::default() } };
                    self.push(c, Some(st.note));
                }
            }
            None => self.push(cue(format!("{} {}", lead, plain), show, hl), None),
        }
    }
}

/// Stops per tour; the rest is counted in the last cue.
const MAX_STOPS: usize = 20;

/// The cues of a tour line (`name_tour` or `name_tour:<ref>`), or why it cannot be built.
pub fn generate(p: &Project, line: &str) -> Result<Tour, String> {
    let (name, arg) = line.split_once(':').map_or((line, None), |(n, a)| (n, Some(a)));
    let scope = match (name, arg) {
        ("flow_tour" | "entry_tour", _) | (_, None) => None,
        (_, Some(a)) => Some(resolve(p, a).and_then(|r| r.module).ok_or_else(|| format!("`{}`: `{}` is not a module", line, a))?),
    };
    Ok(match name {
        "architecture_tour" => architecture(p, scope),
        "types_tour" => types(p, scope).into(),
        "errors_tour" => errors(p, scope).into(),
        "entry_tour" => entry(p).into(),
        "flow_tour" => flow(p, arg.ok_or("`flow_tour` needs a function: `flow_tour:crate::run`")?)?.into(),
        "todo_tour" => todo(p, scope).into(),
        "files_tour" => files(p, scope),
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

/// " in `net`", or nothing without a scope.
fn within(p: &Project, scope: Option<ModuleId>) -> String {
    scope.map(|m| format!(" in `{}`", p.module(m).path.trim_start_matches("crate::"))).unwrap_or_default()
}

fn plural(n: usize, word: &str) -> String {
    format!("{} {}{}", n, word, if n == 1 { "" } else { "s" })
}

/// "First,", "Next,", "Then", … "And finally,".
fn lead(i: usize, n: usize) -> &'static str {
    match i {
        0 => "First,",
        _ if i + 1 == n => "And finally,",
        _ => ["Next,", "Then", "Then there is"][i % 3],
    }
}

fn cue(say: String, show: Option<String>, hl: Vec<String>) -> Cue {
    Cue { say, show, hl: Some(hl), ..Default::default() }
}

/// Keep `MAX_STOPS` cues after the intro; say how many were left out.
fn cap(mut cues: Vec<Cue>, total: usize) -> Vec<Cue> {
    if total > MAX_STOPS {
        cues.truncate(MAX_STOPS + 1);
    }
    more(&mut cues, total);
    cues
}

/// Say how many of `total` stops were left out after `MAX_STOPS`.
fn more(cues: &mut [Cue], total: usize) {
    if let Some(last) = cues.last_mut().filter(|_| total > MAX_STOPS) {
        last.say.push_str(&format!(" And {} more, not shown here.", total - MAX_STOPS));
    }
}

fn is_test_item(p: &Project, it: &Item) -> bool {
    crate::views::is_test_module(p, it.module) || it.fn_info().map(|f| f.is_test).unwrap_or(false)
}

// ------------------------------------------------------------------ architecture

/// The architecture view, then one cue per module, glowing it and reading its `//!` doc.
fn architecture(p: &Project, scope: Option<ModuleId>) -> Tour {
    let mods: Vec<&Module> = guide::tour_modules(p, scope);
    let g = crate::views::build(p, crate::views::Mode::Architecture, &crate::views::ViewOptions { module: scope, ..Default::default() });
    let root = match scope {
        Some(m) => p.module(m),
        None => &p.modules[0],
    };
    // the root's doc as a page: shown with the map, its sections read before the modules
    let mut page = guide::doc_page(&root.path.trim_start_matches("crate::").replace("crate", &p.name), root.doc.as_ref()).unwrap_or_default().into_iter();
    let first = page.next();
    let goal = match (&first, scope) {
        (Some(st), _) => Some(st.say.clone()).filter(|s| !s.is_empty()),
        (None, Some(_)) => doc_sentence(root.doc.as_ref()),
        (None, None) => root.doc.as_ref().and_then(|d| d.split("\n\n").next()).map(|d| d.split_whitespace().collect::<Vec<_>>().join(" ")),
    };
    let visit = format!("Let's visit the {} from the top down: first what starts the program, last the foundations everything else uses.", plural(mods.len(), "module"));
    let map = format!("Here is the map of {}{}: every box is a module, every arrow a dependency.{}", p.name, within(p, scope), goal.map(|g| format!(" {}", g)).unwrap_or_default());
    let mut t = Tour::default();
    match first {
        Some(st) => {
            t.push(cue(map, Some("arch".into()), vec![]), Some(st.note));
            for st in page {
                t.push(Cue { say: st.say, ..Default::default() }, Some(st.note));
            }
            t.push(Cue { say: format!("Now the modules. {}", visit), ..Default::default() }, None);
        }
        None => t.push(cue(format!("{} {}", map, visit), Some("arch".into()), vec![]), None),
    }
    let n = mods.len().min(MAX_STOPS);
    for (i, m) in mods.iter().take(MAX_STOPS).enumerate() {
        let inside = folded_count(&g, m);
        let folded = if inside > 0 { format!(" It holds {} smaller modules, folded into its box.", inside) } else { String::new() };
        let name = m.path.trim_start_matches("crate::");
        let lead = format!("{} `{}` module.{}", guide::tour_lead(i, n), name, folded);
        t.doc_stops(&lead, name, m.doc.as_ref(), guide::module_sentence(p, m), None, vec![m.path.clone()]);
    }
    more(&mut t.cues, mods.len());
    t
}

/// How many modules the Architecture view folds into this box (its "+N inside" badge).
fn folded_count(g: &crate::views::Graph, m: &Module) -> usize {
    g.node(&format!("m{}", m.id)).and_then(|n| n.badges.iter().find_map(|b| b.strip_prefix('+')?.strip_suffix(" inside")?.parse().ok())).unwrap_or(0)
}

// ------------------------------------------------------------------ types

fn types(p: &Project, scope: Option<ModuleId>) -> Vec<Cue> {
    let tys: Vec<&Item> = guide::tour_types(p).into_iter().filter(|i| in_scope(p, i.module, scope)).collect();
    if tys.is_empty() {
        return vec![cue(format!("There are no types{}: only functions.", within(p, scope)), Some(view("types", p, scope)), vec![])];
    }
    let intro = format!(
        "Now the data{}: {}. Structs and enums hold the values; traits describe behaviour that several types share. Green arrows mean contains, purple dashed arrows mean implements.",
        within(p, scope),
        plural(tys.len(), "type")
    );
    let mut cues = vec![cue(intro, Some(view("types", p, scope)), vec![])];
    let n = tys.len().min(MAX_STOPS);
    for (i, it) in tys.iter().enumerate() {
        cues.push(cue(format!("{} {}", lead(i, n), guide::type_sentence(p, it)), None, vec![it.path.clone()]));
    }
    cap(cues, tys.len())
}

// ------------------------------------------------------------------ errors

/// The type item named by the last segment of a type text (`crate::E`, `E<T>` → `E`).
fn type_named<'a>(p: &'a Project, ty: &str) -> Option<&'a Item> {
    let last = ty.rsplit("::").next().unwrap_or(ty).split('<').next().unwrap_or(ty).trim();
    p.items.iter().find(|i| i.is_type() && i.name == last)
}

fn names(p: &Project, ids: &[ItemId], max: usize) -> String {
    let mut v: Vec<String> = ids.iter().take(max).map(|i| format!("`{}`", label(p, *i))).collect();
    if ids.len() > max {
        v.push(format!("{} more", ids.len() - max));
    }
    join_and(&v)
}

fn errors(p: &Project, scope: Option<ModuleId>) -> Vec<Cue> {
    let fns: Vec<&Item> = p.functions().filter(|f| in_scope(p, f.module, scope) && !is_test_item(p, f)).collect();
    let fallible: Vec<&Item> = fns.iter().copied().filter(|f| f.fn_info().map(|i| i.returns_result).unwrap_or(false)).collect();
    let panicky: Vec<ItemId> = fns.iter().filter(|f| f.fn_info().map(|i| i.unwraps + i.expects + i.panics > 0).unwrap_or(false)).map(|f| f.id).collect();
    let show = Some(view("errors", p, scope));
    if fallible.is_empty() && panicky.is_empty() {
        return vec![cue(format!("Nothing{} returns a `Result` or can panic: no error paths to follow.", within(p, scope)), show, vec![])];
    }
    let intro = format!(
        "Now, where things go wrong{}. {} return a `Result`, so their callers must deal with failure; {} can panic. Rust has no exceptions: an error is a value that travels back up, step by step.",
        within(p, scope),
        plural(fallible.len(), "function"),
        match panicky.len() {
            0 => "none".to_string(),
            n => n.to_string(),
        }
    );
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
        let mut say = format!("{} It comes back from {}.", guide::type_sentence(p, it), names(p, producers, 5));
        if !froms.is_empty() {
            say.push_str(&format!(" {} turn{} into it through `From`, which is what lets `?` pass one as the other.", join_and(&froms), if froms.len() == 1 { "s" } else { "" }));
        }
        cues.push(cue(say, None, vec![it.path.clone()]));
    }
    // propagation with `?`
    let mut q: Vec<&Item> = fns.iter().copied().filter(|f| f.fn_info().map(|i| i.question_marks > 0).unwrap_or(false)).collect();
    q.sort_by_key(|f| std::cmp::Reverse(f.fn_info().unwrap().question_marks));
    if !q.is_empty() {
        let list: Vec<String> = q.iter().take(5).map(|f| format!("`{}` ({})", label(p, f.id), plural(f.fn_info().unwrap().question_marks, "time"))).collect();
        cues.push(cue(
            format!("The question mark passes an error straight up to the caller. It is used in {}{}.", join_and(&list), if q.len() > 5 { format!(" and {} more", q.len() - 5) } else { String::new() }),
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
            format!("Errors stop in {}: they check the result themselves, with a `match` or an `if let`, instead of passing it on.", names(p, &handlers, 5)),
            None,
            handlers.iter().map(|h| p.item(*h).path.clone()).collect(),
        ));
    }
    if !panicky.is_empty() {
        cues.push(cue(
            format!("And these can panic instead: {}. A panic, from `unwrap`, `expect` or `panic!`, stops the thread; no caller gets a chance to handle it.", names(p, &panicky, 5)),
            None,
            panicky.iter().map(|h| p.item(*h).path.clone()).collect(),
        ));
    }
    cues
}

// ------------------------------------------------------------------ entry points

fn entry(p: &Project) -> Vec<Cue> {
    let mut cues = Vec::new();
    if let Some(m) = p.main {
        let it = p.item(m);
        let mut seen = std::collections::HashSet::new();
        let callees: Vec<ItemId> = p.callees(m).iter().filter_map(|e| e.to).filter(|t| seen.insert(*t)).collect();
        let doc = doc_sentence(it.doc.as_ref()).map(|d| format!(" {}", d)).unwrap_or_default();
        let calls = if callees.is_empty() { String::new() } else { format!(" It calls {}.", names(p, &callees, 5)) };
        cues.push(cue(format!("Everything starts in `main`, the entry point of the program.{}{}", doc, calls), Some(format!("calls:{}", it.path)), vec![it.path.clone()]));
    }
    let public: Vec<&Item> = p
        .items
        .iter()
        .filter(|i| i.vis == Visibility::Public && Some(i.id) != p.main && !is_test_item(p, i))
        .filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Struct | ItemKind::Enum | ItemKind::Trait | ItemKind::TypeAlias))
        .collect();
    if public.is_empty() {
        if cues.is_empty() {
            cues.push(cue("This project has no `main` and nothing marked `pub`: no way in from outside.".into(), Some("structure".into()), vec![]));
        }
        return cues;
    }
    let lib = p.files.iter().any(|f| f.path.ends_with("lib.rs"));
    cues.push(cue(
        format!(
            "{} {}: {} that other code can use.",
            if cues.is_empty() { "The way in is" } else { "Then," },
            if lib { "the library's public API" } else { "what is marked `pub`" },
            plural(public.len(), "item")
        ),
        Some("structure".into()),
        public.iter().map(|i| i.path.clone()).collect(),
    ));
    let n = public.len().min(MAX_STOPS);
    let base = cues.len() - 1;
    for (i, it) in public.iter().enumerate() {
        let what = match it.kind {
            ItemKind::Function => doc_sentence(it.doc.as_ref()).unwrap_or_else(|| format!("Its signature: `{}`.", it.signature)),
            _ => guide::type_sentence(p, it),
        };
        let say = match it.kind {
            ItemKind::Function => format!("{} `{}`. {}", lead(i, n).replace(" there is", ""), label(p, it.id), what),
            _ => format!("{} {}", lead(i, n), what),
        };
        cues.push(cue(say, None, vec![it.path.clone()]));
    }
    // cap counts the stops after the public intro
    let mut head: Vec<Cue> = cues.drain(..base).collect();
    head.extend(cap(cues, public.len()));
    head
}

// ------------------------------------------------------------------ one function

fn flow(p: &Project, f: &str) -> Result<Vec<Cue>, String> {
    let id = resolve(p, f).and_then(|r| r.item).filter(|i| p.item(*i).is_callable()).ok_or_else(|| format!("`flow_tour:{}`: not a function", f))?;
    if p.item(id).fn_info().and_then(|i| i.cfg.as_ref()).is_none() {
        return Err(format!("`flow_tour:{}`: no body to walk", f));
    }
    let mut last = None;
    Ok(guide::function_steps(p, id, &GuideOptions::default()).iter().map(|st| step_cue(p, st, &mut last)).collect())
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

fn todo(p: &Project, scope: Option<ModuleId>) -> Vec<Cue> {
    // (file, line, what fn, sentence)
    let mut stops: Vec<(FileId, usize, Option<ItemId>, String)> = Vec::new();
    let (mut notes, mut unsafes, mut panics) = (0, 0, 0);
    for f in p.files.iter().filter(|f| f.rust && f.text) {
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
            let at = owner.map(|o| format!("In `{}`", label(p, o.id))).unwrap_or_else(|| format!("In `{}`", f.path));
            if let Some((m, text)) = marker(comment) {
                notes += 1;
                let say = if text.is_empty() { format!("{}, a {} left in the code.", at, m) } else { format!("{}, a {}: {}", at, m, text) };
                stops.push((f.id, n, owner.map(|o| o.id), say));
            }
            if code.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == "unsafe") {
                unsafes += 1;
                stops.push((f.id, n, owner.map(|o| o.id), format!("{}, `unsafe`: here the compiler trusts the author instead of checking.", at)));
            }
        }
    }
    for it in p.functions().filter(|f| in_scope(p, f.module, scope) && !is_test_item(p, f)) {
        let Some(cfg) = it.fn_info().and_then(|f| f.cfg.as_ref()) else { continue };
        for n in cfg.nodes.iter().filter(|n| n.kind == CfgNodeKind::Panic) {
            panics += 1;
            stops.push((n.span.file, n.span.line_start, Some(it.id), format!("In `{}`, `{}` can panic.", label(p, it.id), crate::shorten(n.detail.lines().next().unwrap_or(""), 60))));
        }
    }
    if stops.is_empty() {
        return vec![cue(format!("No rough edges{}: no TODO left, no `unsafe`, nothing that can panic.", within(p, scope)), Some("arch".into()), vec![])];
    }
    stops.sort_by_key(|s| (s.0, s.1));
    stops.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1 && a.3 == b.3);
    let show_of = |s: &(FileId, usize, Option<ItemId>, String)| format!("code:{}", s.2.map(|i| p.item(i).path.clone()).unwrap_or_else(|| p.files[s.0].path.clone()));
    let intro = format!(
        "Now the rough edges{}: {} left in comments, {}, and {} that can panic. A good list to start from when you want to improve the code.",
        within(p, scope),
        plural(notes, "note"),
        plural(unsafes, "unsafe block"),
        plural(panics, "place")
    );
    let mut cues = vec![cue(intro, Some(show_of(&stops[0])), vec![])];
    let mut last = cues[0].show.clone();
    for s in &stops {
        let show = Some(show_of(s));
        let changed = show != last;
        last = show.clone();
        cues.push(cue(s.3.clone(), if changed { show } else { None }, vec![format!("{}:{}", p.files[s.0].path, s.1)]));
    }
    cap(cues, stops.len())
}

// ------------------------------------------------------------------ files

/// How a file is said: `main`, `parser`, `net` for `net/mod.rs` (the view shows the full path).
fn file_name(f: &SourceFile) -> String {
    let mut parts = f.path.trim_end_matches(".rs").rsplit('/');
    let stem = parts.next().unwrap_or("");
    let stem = if stem == "mod" { parts.next().unwrap_or(stem) } else { stem };
    format!("`{}`", stem)
}

fn files(p: &Project, scope: Option<ModuleId>) -> Tour {
    let mut stops: Vec<(&SourceFile, Option<&Module>)> = Vec::new();
    for f in p.files.iter().filter(|f| f.rust) {
        let module = p.modules.iter().filter(|m| m.file == Some(f.id)).min_by_key(|m| m.path.len());
        let keep = match module {
            Some(m) => in_scope(p, m.id, scope) && !crate::views::is_test_module(p, m.id),
            None => scope.is_none(),
        };
        if keep {
            stops.push((f, module));
        }
    }
    let intro = format!("Let's walk through the {}{}, one by one, the way you would open them in an editor.", plural(stops.len(), "file"), within(p, scope));
    let mut t = Tour::from(vec![cue(intro, stops.first().map(|s| format!("code:{}", s.0.path)), vec![])]);
    let n = stops.len().min(MAX_STOPS);
    for (i, (f, m)) in stops.iter().take(MAX_STOPS).enumerate() {
        let doc = m.and_then(|m| doc_sentence(m.doc.as_ref())).unwrap_or_else(|| "It has no module doc yet.".into());
        let defs: Vec<ItemId> = m.map(|m| m.items.iter().copied().filter(|i| !matches!(p.item(*i).kind, ItemKind::Method | ItemKind::Impl)).collect()).unwrap_or_default();
        let defines = if defs.is_empty() { String::new() } else { format!(" It defines {}.", names(p, &defs, 5)) };
        let lead = match i {
            0 => "First,",
            _ if i + 1 == n => "And last,",
            _ => ["Next,", "Then"][i % 2],
        };
        let show = if i == 0 { None } else { Some(format!("code:{}", f.path)) };
        t.doc_stops(&format!("{} the {} file.{}", lead, file_name(f), defines), &file_name(f), m.and_then(|m| m.doc.as_ref()), doc, show, vec![]);
    }
    more(&mut t.cues, stops.len());
    t
}
