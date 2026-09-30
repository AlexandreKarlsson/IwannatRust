//! Data model shared between the analyzer, the layout engine and the UI.
//!
//! Everything here is plain data (serde-serializable) so the CLI can ship a
//! `Project` to the browser as JSON and the UI can rebuild any view from it.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type FileId = usize;
pub type ItemId = usize;
pub type ModuleId = usize;

/// A source file inside the analyzed project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    pub id: FileId,
    /// Path relative to the project root, using `/` separators.
    pub path: String,
    pub content: String,
    /// Rust source (parsed); other files are listed and shown but not analyzed.
    #[serde(default = "default_true")]
    pub rust: bool,
    /// False for binary or oversized files (content empty).
    #[serde(default = "default_true")]
    pub text: bool,
}

fn default_true() -> bool {
    true
}

/// Location of something inside a file. Lines and columns are 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    pub file: FileId,
    pub line_start: usize,
    pub col_start: usize,
    pub line_end: usize,
    pub col_end: usize,
}

impl Span {
    pub fn contains_line(&self, line: usize) -> bool {
        line >= self.line_start && line <= self.line_end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Const,
    Static,
    Module,
    Impl,
}

impl ItemKind {
    pub fn label(&self) -> &'static str {
        match self {
            ItemKind::Function => "fn",
            ItemKind::Method => "method",
            ItemKind::Struct => "struct",
            ItemKind::Enum => "enum",
            ItemKind::Trait => "trait",
            ItemKind::TypeAlias => "type",
            ItemKind::Const => "const",
            ItemKind::Static => "static",
            ItemKind::Module => "mod",
            ItemKind::Impl => "impl",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    #[default]
    Private,
    Crate,
    Public,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SelfKind {
    #[default]
    None,
    Ref,
    RefMut,
    Owned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub vis: Visibility,
    /// Item ids of project types referenced by this field's type.
    pub refs: Vec<ItemId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
}

/// Function-specific facts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FnInfo {
    pub is_async: bool,
    pub is_unsafe: bool,
    pub is_const: bool,
    pub self_kind: SelfKind,
    /// For methods: the impl block this method belongs to.
    pub impl_id: Option<ItemId>,
    /// For methods: the type the impl is for (resolved item id when it is a project type).
    pub owner_type: Option<ItemId>,
    pub owner_type_name: Option<String>,
    /// For trait impls / trait defs: the trait.
    pub trait_id: Option<ItemId>,
    pub trait_name: Option<String>,
    pub params: Vec<Param>,
    pub ret: Option<String>,
    pub returns_result: bool,
    pub returns_option: bool,
    /// Ok / Err type names when returning Result.
    pub result_ok: Option<String>,
    pub result_err: Option<String>,
    pub question_marks: usize,
    pub unwraps: usize,
    pub expects: usize,
    pub panics: usize,
    pub awaits: usize,
    pub loops: usize,
    pub branches: usize,
    pub is_recursive: bool,
    /// Control-flow graph of the body (None for trait method declarations without body).
    pub cfg: Option<Cfg>,
    /// Simplified nested block structure of the body (see `blocks`).
    pub blocks: Option<Block>,
    /// Number of source lines in the body.
    pub body_lines: usize,
    /// Is this a `#[test]`?
    pub is_test: bool,
    /// Is this the crate entry point?
    pub is_main: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StructInfo {
    pub fields: Vec<Field>,
    pub derives: Vec<String>,
    pub is_tuple: bool,
    pub is_unit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnumInfo {
    pub variants: Vec<Variant>,
    pub derives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraitInfo {
    pub supertraits: Vec<String>,
    pub supertrait_ids: Vec<ItemId>,
    /// Method item ids declared in the trait.
    pub methods: Vec<ItemId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImplInfo {
    pub target: String,
    pub target_id: Option<ItemId>,
    pub trait_name: Option<String>,
    pub trait_id: Option<ItemId>,
    pub methods: Vec<ItemId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemExtra {
    Fn(FnInfo),
    Struct(StructInfo),
    Enum(EnumInfo),
    Trait(TraitInfo),
    Impl(ImplInfo),
    TypeAlias { target: String },
    Const { ty: String },
    Module,
}

/// Any named thing in the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub kind: ItemKind,
    pub name: String,
    /// Fully qualified path, e.g. `crate::net::Client::connect`.
    pub path: String,
    pub module: ModuleId,
    pub vis: Visibility,
    pub span: Span,
    /// One-line signature reconstructed from the source, e.g. `pub fn foo(x: u32) -> Result<(), E>`.
    pub signature: String,
    /// Doc comment (`///` and `/** */`), if any.
    pub doc: Option<String>,
    /// Attributes as written, without `#[ ]`.
    pub attrs: Vec<String>,
    pub extra: ItemExtra,
}

impl Item {
    pub fn fn_info(&self) -> Option<&FnInfo> {
        match &self.extra {
            ItemExtra::Fn(f) => Some(f),
            _ => None,
        }
    }
    pub fn fn_info_mut(&mut self) -> Option<&mut FnInfo> {
        match &mut self.extra {
            ItemExtra::Fn(f) => Some(f),
            _ => None,
        }
    }
    pub fn is_callable(&self) -> bool {
        matches!(self.kind, ItemKind::Function | ItemKind::Method)
    }
    pub fn is_type(&self) -> bool {
        matches!(
            self.kind,
            ItemKind::Struct | ItemKind::Enum | ItemKind::Trait | ItemKind::TypeAlias
        )
    }
    /// Short human-readable summary (first doc line or synthesized).
    pub fn summary(&self) -> String {
        if let Some(d) = &self.doc {
            if let Some(first) = d.lines().find(|l| !l.trim().is_empty()) {
                return first.trim().to_string();
            }
        }
        match &self.extra {
            ItemExtra::Fn(f) => {
                let mut parts = Vec::new();
                if f.is_main {
                    parts.push("program entry point".to_string());
                }
                if f.is_async {
                    parts.push("async".to_string());
                }
                if f.returns_result {
                    parts.push("fallible (returns Result)".to_string());
                } else if f.returns_option {
                    parts.push("may return nothing (Option)".to_string());
                }
                if f.is_recursive {
                    parts.push("recursive".to_string());
                }
                if f.loops > 0 {
                    parts.push(format!("{} loop(s)", f.loops));
                }
                if f.branches > 0 {
                    parts.push(format!("{} branch(es)", f.branches));
                }
                if parts.is_empty() {
                    format!("{} {}", self.kind.label(), self.name)
                } else {
                    parts.join(", ")
                }
            }
            ItemExtra::Struct(s) => format!("struct with {} field(s)", s.fields.len()),
            ItemExtra::Enum(e) => format!("enum with {} variant(s)", e.variants.len()),
            ItemExtra::Trait(t) => format!("trait with {} method(s)", t.methods.len()),
            ItemExtra::Impl(i) => match &i.trait_name {
                Some(t) => format!("impl {} for {}", t, i.target),
                None => format!("inherent impl for {}", i.target),
            },
            ItemExtra::TypeAlias { target } => format!("alias of {}", target),
            ItemExtra::Const { ty } => format!("constant of type {}", ty),
            ItemExtra::Module => "module".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: ModuleId,
    pub name: String,
    /// `crate`, `crate::net`, ...
    pub path: String,
    pub file: Option<FileId>,
    pub parent: Option<ModuleId>,
    pub children: Vec<ModuleId>,
    pub items: Vec<ItemId>,
    /// `use` paths as written.
    pub uses: Vec<String>,
    pub span: Option<Span>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    /// `foo(..)` / `Type::foo(..)`
    Call,
    /// `x.foo(..)`
    MethodCall,
    /// `foo!(..)`
    Macro,
    /// `Type { .. }` / `Type(..)` constructor usage
    Construct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdge {
    pub from: ItemId,
    /// Resolved callee when it is a project item.
    pub to: Option<ItemId>,
    /// Callee as written, e.g. `client.send`, `Vec::new`, `println`.
    pub callee: String,
    pub kind: CallKind,
    pub span: Span,
    pub in_loop: bool,
    pub in_branch: bool,
    /// Call result is propagated with `?`.
    pub propagated: bool,
    /// Call result is `.unwrap()`ed / `.expect()`ed.
    pub unwrapped: bool,
    pub is_await: bool,
    pub is_recursive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeRelKind {
    /// struct/enum field holds the other type
    Contains,
    /// type implements trait
    Implements,
    /// trait requires supertrait
    Supertrait,
    /// function signature mentions type
    UsesInSignature,
    /// alias points to target
    Aliases,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeRel {
    pub from: ItemId,
    pub to: ItemId,
    pub kind: TypeRelKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CfgNodeKind {
    Entry,
    Exit,
    /// Plain statements (let / expression / assignment).
    Block,
    /// `if` / `if let`
    If,
    /// `match`
    Match,
    /// `loop` / `while` / `for`
    Loop,
    /// A call to a project function.
    Call,
    /// explicit `return`
    Return,
    /// `break` / `continue`
    Jump,
    /// `?` early-return on error
    Propagate,
    /// `unwrap` / `expect` / `panic!` / `unreachable!` / `todo!`
    Panic,
    /// `.await`
    Await,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgNode {
    pub id: usize,
    pub kind: CfgNodeKind,
    /// Short label shown in the box.
    pub label: String,
    /// Longer description (the source text, trimmed).
    pub detail: String,
    pub span: Span,
    /// Project functions called from this node.
    pub calls: Vec<ItemId>,
    /// Depth of nesting (for indentation in the branch tree).
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgEdge {
    pub from: usize,
    pub to: usize,
    /// `true`, `false`, `Some(x)`, `Err`, `break`, `loop`, ...
    pub label: Option<String>,
    pub kind: CfgEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CfgEdgeKind {
    Next,
    True,
    False,
    Arm,
    LoopBody,
    LoopBack,
    Break,
    Continue,
    Error,
    Return,
}

/// Control-flow graph of one function body.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cfg {
    pub nodes: Vec<CfgNode>,
    pub edges: Vec<CfgEdge>,
}

impl Cfg {
    pub fn entry(&self) -> usize {
        0
    }
    pub fn succ(&self, n: usize) -> impl Iterator<Item = &CfgEdge> + '_ {
        self.edges.iter().filter(move |e| e.from == n)
    }
}

/// The whole analyzed project.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Project {
    pub name: String,
    pub root: String,
    pub files: Vec<SourceFile>,
    pub modules: Vec<Module>,
    pub items: Vec<Item>,
    pub calls: Vec<CallEdge>,
    pub type_rels: Vec<TypeRel>,
    /// Item id of `fn main` if present.
    pub main: Option<ItemId>,
    /// External crates found in `use` statements (and Cargo.toml).
    pub external_crates: Vec<String>,
    /// Diagnostics produced while analysing (parse errors etc).
    pub diagnostics: Vec<String>,
}

impl Project {
    pub fn item(&self, id: ItemId) -> &Item {
        &self.items[id]
    }
    pub fn module(&self, id: ModuleId) -> &Module {
        &self.modules[id]
    }
    pub fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id]
    }
    pub fn callers(&self, id: ItemId) -> Vec<&CallEdge> {
        self.calls.iter().filter(|c| c.to == Some(id)).collect()
    }
    pub fn callees(&self, id: ItemId) -> Vec<&CallEdge> {
        self.calls.iter().filter(|c| c.from == id).collect()
    }
    pub fn find_by_path(&self, path: &str) -> Option<ItemId> {
        self.items.iter().find(|i| i.path == path).map(|i| i.id)
    }
    /// Source text of a span.
    pub fn source_of(&self, span: &Span) -> String {
        let file = &self.files[span.file];
        file.content
            .lines()
            .skip(span.line_start.saturating_sub(1))
            .take(span.line_end + 1 - span.line_start)
            .collect::<Vec<_>>()
            .join("\n")
    }
    /// Map from item name (last path segment) to ids, for quick lookups.
    pub fn name_index(&self) -> HashMap<&str, Vec<ItemId>> {
        let mut m: HashMap<&str, Vec<ItemId>> = HashMap::new();
        for it in &self.items {
            m.entry(it.name.as_str()).or_default().push(it.id);
        }
        m
    }
    /// Items that are functions or methods.
    pub fn functions(&self) -> impl Iterator<Item = &Item> + '_ {
        self.items.iter().filter(|i| i.is_callable())
    }
}

// ---- simplified block structure (built by `blocks`, usable without the `parse` feature)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Fn,
    Let,
    Stmt,
    Call,
    Macro,
    If,
    ElseIf,
    Else,
    Match,
    Arm,
    Loop,
    Return,
    Break,
    Continue,
    Propagate,
    Panic,
    Await,
    Unsafe,
    Closure,
}

impl BlockKind {
    pub fn is_compound(&self) -> bool {
        matches!(self, BlockKind::Fn | BlockKind::If | BlockKind::ElseIf | BlockKind::Else | BlockKind::Match | BlockKind::Arm | BlockKind::Loop | BlockKind::Unsafe | BlockKind::Closure)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub id: usize,
    pub kind: BlockKind,
    /// Short text shown in the block ("for t in &tasks", "if tasks.is_empty()", "let tasks = parse_all(input)?").
    pub label: String,
    /// Full source text of the construct's head/statement.
    pub detail: String,
    pub span: Span,
    pub children: Vec<Block>,
    /// Variables introduced by this block (let bindings, loop patterns, match arm patterns, closure args).
    pub defines: Vec<String>,
    /// Local variables read by this block's own head/expression (not by its children).
    pub uses: Vec<String>,
    /// Project functions called from this block's head/expression.
    pub calls: Vec<ItemId>,
}

impl Block {
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(|c| c.count()).sum::<usize>()
    }
    /// Deepest block whose span contains `line`.
    pub fn deepest_at(&self, line: usize) -> Option<&Block> {
        if !self.span.contains_line(line) {
            return None;
        }
        for c in &self.children {
            if let Some(b) = c.deepest_at(line) {
                return Some(b);
            }
        }
        Some(self)
    }
}

