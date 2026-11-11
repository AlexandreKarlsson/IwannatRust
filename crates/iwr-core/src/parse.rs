//! First pass: hand every source file to the front-end of its language ([`crate::lang`]) and
//! collect items + module tree. Bodies are kept around so the second pass (`resolve`) can build
//! CFGs and resolve calls once every item is known.

use crate::lang::{self, Source};
use crate::model::*;
use crate::progress::{Progress, Stage};
use std::collections::HashMap;

pub use crate::lang::Body;

/// Output of the first pass.
pub struct Collected {
    pub project: Project,
    /// (function item id, body, file) for every function that has a body.
    pub bodies: Vec<FnBody>,
    /// Raw `use` paths per module, for external crate detection.
    pub use_paths: Vec<(ModuleId, Vec<String>)>,
}

pub struct FnBody {
    pub item: ItemId,
    pub file: FileId,
    pub body: Body,
}

/// State of the first pass, shared by the front-ends.
pub(crate) struct Ctx<'a> {
    pub(crate) project: &'a mut Project,
    pub(crate) bodies: Vec<FnBody>,
    pub(crate) use_paths: Vec<(ModuleId, Vec<String>)>,
    module_index: HashMap<String, ModuleId>,
    report: &'a mut dyn FnMut(&Progress),
    /// Source files parsed so far, of `total`.
    parsed: usize,
    total: usize,
}

impl<'a> Ctx<'a> {
    /// Say that `file` is being parsed: a front-end calls it once per file, before reading it.
    pub(crate) fn parsing(&mut self, file: FileId) {
        self.parsed += 1;
        let p = Progress { stage: Stage::Parsing, done: self.parsed, total: self.total, current: self.project.files[file].path.clone(), files: self.project.files.len(), items: self.project.items.len(), ..Default::default() };
        (self.report)(&p);
    }

    /// The module at `path` (`crate::a::b`), created with its parents when missing.
    pub(crate) fn module_for(&mut self, path: &str) -> ModuleId {
        if let Some(id) = self.module_index.get(path) {
            return *id;
        }
        let (parent, name) = match path.rsplit_once("::") {
            Some((p, n)) => (Some(self.module_for(p)), n.to_string()),
            // crate roots of a multi-crate workspace hang under the virtual `crate` root (module 0)
            None if path != "crate" => (Some(self.module_for("crate")), path.to_string()),
            None => (None, path.to_string()),
        };
        let id = self.project.modules.len();
        self.project.modules.push(Module {
            id,
            name,
            path: path.to_string(),
            file: None,
            parent,
            children: vec![],
            items: vec![],
            uses: vec![],
            span: None,
            doc: None,
            is_test: false,
        });
        if let Some(p) = parent {
            self.project.modules[p].children.push(id);
        }
        self.module_index.insert(path.to_string(), id);
        id
    }

    pub(crate) fn push_item(&mut self, mut item: Item) -> ItemId {
        let id = self.project.items.len();
        item.id = id;
        self.project.modules[item.module].items.push(id);
        self.project.items.push(item);
        id
    }
}

/// Parse all files and collect items. `report` hears about every source file parsed.
pub fn collect(name: &str, root: &str, files: Vec<(String, String)>, report: &mut dyn FnMut(&Progress)) -> Collected {
    let mut project = Project { name: name.to_string(), root: root.to_string(), ..Default::default() };
    // every file is listed first (the file tree shows them all), sources sorted by language
    let mut sources: Vec<(SourceLang, Source)> = Vec::new();
    for (path, content) in files {
        let file = project.files.len();
        let lang = SourceLang::of_path(&path);
        let text = !content.is_empty() || lang.is_some();
        if let Some(l) = lang {
            sources.push((l, Source { file, path: path.clone(), content: content.clone() }));
        }
        project.files.push(SourceFile { id: file, path, content, lang, text });
    }
    let total = sources.len();
    let mut ctx = Ctx { project: &mut project, bodies: vec![], use_paths: vec![], module_index: HashMap::new(), report, parsed: 0, total };
    ctx.module_for("crate");

    // one front-end after the other, in the order of `SourceLang::ALL`: in a mixed project the
    // first language that has an entry point gets the root module
    for l in SourceLang::ALL {
        let (mine, rest): (Vec<_>, Vec<_>) = sources.into_iter().partition(|(sl, _)| *sl == l);
        sources = rest;
        if !mine.is_empty() {
            lang::collect(l, &mut ctx, mine.into_iter().map(|(_, s)| s).collect());
        }
    }

    // sort children/items deterministically by name for stable output
    for m in &mut ctx.project.modules {
        m.children.sort();
    }
    let Ctx { bodies, use_paths, .. } = ctx;
    Collected { project, bodies, use_paths }
}
