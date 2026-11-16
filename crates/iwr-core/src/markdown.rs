//! Markdown of notes, answers and captions, read into a small tree the UI draws.
//!
//! CommonMark with tables, strikethrough and task lists (parsed by `pulldown-cmark`). Three
//! things are ours: text keeps its code spans between backticks, so [`crate::speech::segments`]
//! can give each its spoken form; HTML is never interpreted: it stays text, because codecast
//! prose is full of `Vec<String>`; and admonitions are read the way documentation sites write
//! them (`!!! note "Title"` over an indented body, `???` when it folds), next to GitHub's
//! `> [!NOTE]`; so are content tabs (`=== "Name"` over an indented body).
//!
//! A page names what a codecast's `!` line can glow in it (see [`ids`]): headings, the items of
//! a list, the rows and columns of a table, titled admonitions, tabs, and the nodes of a
//! `mermaid` code block, which is drawn as a diagram.

use pulldown_cmark::{Alignment, BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::iter::Peekable;

/// A block of a markdown text.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Para(Vec<Inline>),
    /// Level 1 to 6.
    Heading(u8, Vec<Inline>),
    /// A fenced or indented code block; `lang` is the first word after the fence's backticks,
    /// `title` what a `title="src/main.rs"` after it gives (empty without one).
    Code { lang: String, title: String, text: String },
    Quote(Vec<Block>),
    /// A framed remark. `title`: `None` is the name of its kind, an empty one no title line.
    /// `fold`: `None` cannot be folded, else whether it starts open.
    Admonition { kind: Callout, title: Option<Vec<Inline>>, fold: Option<bool>, blocks: Vec<Block> },
    /// `start` is the first number of a numbered list.
    List { start: Option<u64>, items: Vec<Item> },
    /// One [`Align`] per column; cells are inline text.
    Table { align: Vec<Align>, head: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
    /// Content tabs: `=== "Name"` blocks that follow each other, one shown at a time.
    Tabs(Vec<Tab>),
    Rule,
}

/// One tab of a [`Block::Tabs`].
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub title: String,
    pub blocks: Vec<Block>,
}

/// What an admonition is about: its colour, its icon and its title when it has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callout {
    Note,
    Abstract,
    Info,
    Tip,
    Success,
    Question,
    Warning,
    Failure,
    Danger,
    Bug,
    Example,
    Quote,
}

impl Callout {
    /// The kind a word after `!!!` names, with the usual synonyms; an unknown word is a note.
    pub fn of(word: &str) -> Callout {
        match word.to_ascii_lowercase().as_str() {
            "abstract" | "summary" | "tldr" => Callout::Abstract,
            "info" | "todo" => Callout::Info,
            "tip" | "hint" | "important" => Callout::Tip,
            "success" | "check" | "done" => Callout::Success,
            "question" | "help" | "faq" => Callout::Question,
            "warning" | "caution" | "attention" => Callout::Warning,
            "failure" | "fail" | "missing" => Callout::Failure,
            "danger" | "error" => Callout::Danger,
            "bug" => Callout::Bug,
            "example" => Callout::Example,
            "quote" | "cite" => Callout::Quote,
            _ => Callout::Note,
        }
    }

    /// Its name in a CSS class.
    pub fn class(self) -> &'static str {
        match self {
            Callout::Note => "note",
            Callout::Abstract => "abstract",
            Callout::Info => "info",
            Callout::Tip => "tip",
            Callout::Success => "success",
            Callout::Question => "question",
            Callout::Warning => "warning",
            Callout::Failure => "failure",
            Callout::Danger => "danger",
            Callout::Bug => "bug",
            Callout::Example => "example",
            Callout::Quote => "quote",
        }
    }
}

/// An item of a list.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// `- [ ]` / `- [x]`: the box and whether it is ticked.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// How a table column is aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// A piece of a paragraph, heading or table cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    /// Plain text. Code spans are in it between backticks, as written.
    Text(String),
    /// A code span that itself contains a backtick.
    Code(String),
    Strong(Vec<Inline>),
    Em(Vec<Inline>),
    Strike(Vec<Inline>),
    /// `href` is empty when the address is not one a page may follow (see [`safe_url`]).
    Link { href: String, text: Vec<Inline> },
    /// `src` is empty when the address is not safe; `alt` is then all there is to show.
    Image { src: String, alt: String },
    /// A line break asked for by two trailing spaces or a `\`.
    Break,
}

/// Read a markdown text.
pub fn parse(text: &str) -> Vec<Block> {
    read(&lenient(text))
}

/// Standard markdown with its `!!!` admonitions: each is cut out with its indented body, read on
/// its own; the text between them goes to the parser.
fn read(src: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut fence = None;
    let mut lines = src.lines().peekable();
    while let Some(line) = lines.next() {
        let fenced = in_fence(&mut fence, line);
        let tab = if fenced { None } else { tab_head(line) };
        let head = if fenced || tab.is_some() { None } else { admonition_head(line) };
        if tab.is_none() && head.is_none() {
            plain.push_str(line);
            plain.push('\n');
            continue;
        }
        // a tab right after another one joins its set
        let joins = plain.trim().is_empty() && matches!(out.last(), Some(Block::Tabs(_)));
        out.extend(commonmark(&plain));
        plain.clear();
        let mut body = String::new();
        while let Some(l) = lines.peek() {
            let Some(inner) = l.strip_prefix("    ").or_else(|| l.strip_prefix('\t')).or_else(|| l.trim().is_empty().then_some("")) else { break };
            body.push_str(inner);
            body.push('\n');
            lines.next();
        }
        if let Some(title) = tab {
            let tab = Tab { title, blocks: read(&body) };
            match out.last_mut() {
                Some(Block::Tabs(tabs)) if joins => tabs.push(tab),
                _ => out.push(Block::Tabs(vec![tab])),
            }
            continue;
        }
        let Some((kind, title, fold)) = head else { continue };
        // a title is one line of inline text
        let title = title.map(|t| match commonmark(&t).into_iter().next() {
            Some(Block::Para(p)) => p,
            _ => Vec::new(),
        });
        out.push(Block::Admonition { kind, title, fold, blocks: read(&body) });
    }
    out.extend(commonmark(&plain));
    out
}

/// What the parser reads.
fn commonmark(src: &str) -> Vec<Block> {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_GFM;
    let mut events = Parser::new_ext(src, opts).peekable();
    blocks(&mut events)
}

/// The first line of an admonition: `!!! kind`, `!!! kind "Title"`, `??? kind` (folded) or
/// `???+ kind` (folds, open). Gives the kind, the title when one is written, and the fold.
fn admonition_head(line: &str) -> Option<(Callout, Option<String>, Option<bool>)> {
    let (fold, rest) = if let Some(r) = line.strip_prefix("!!!") {
        (None, r)
    } else if let Some(r) = line.strip_prefix("???+") {
        (Some(true), r)
    } else {
        (Some(false), line.strip_prefix("???")?)
    };
    let rest = rest.strip_prefix(' ')?.trim();
    let (words, title) = match rest.split_once('"') {
        Some((w, t)) => (w, Some(t.strip_suffix('"').unwrap_or(t).to_string())),
        None => (rest, None),
    };
    // more words (`inline end`) are placement hints we do not follow
    let kind = words.split_whitespace().next()?;
    kind.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).then(|| (Callout::of(kind), title, fold))
}

/// The first line of a tab: `=== "Name"`. Gives the name.
fn tab_head(line: &str) -> Option<String> {
    let name = line.strip_prefix("=== \"")?.trim_end().strip_suffix('"')?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Follow code fences from line to line: whether this line is a fence mark or between two.
fn in_fence(fence: &mut Option<String>, line: &str) -> bool {
    let t = line.trim_start();
    if let Some(f) = fence {
        if t.starts_with(f.as_str()) && t.trim_end().chars().all(|c| f.starts_with(c)) {
            *fence = None;
        }
        return true;
    }
    if t.starts_with("```") || t.starts_with("~~~") {
        let mark = t.chars().next().unwrap();
        *fence = Some(t.chars().take_while(|&c| c == mark).collect());
        return true;
    }
    false
}

/// The text of inline pieces without its marks, e.g. for an image's `alt`.
pub fn plain(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for i in inlines {
        match i {
            Inline::Text(t) | Inline::Code(t) => out.push_str(t),
            Inline::Strong(k) | Inline::Em(k) | Inline::Strike(k) | Inline::Link { text: k, .. } => out.push_str(&plain(k)),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::Break => out.push(' '),
        }
    }
    out
}

/// The id of a title: lower case, letters, digits and `_` kept, anything else a `-`
/// (`Step two` → `step-two`, `` `next_task` `` → `next_task`).
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().chars() {
        if c.is_alphanumeric() || c == '_' {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Gives the things of a page their ids, in reading order. The page that is drawn and
/// [`ids`] both ask it, so they agree.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Namer {
    /// id of the last heading read
    section: String,
    /// a list of this section has its numbers already
    listed: bool,
}

impl Namer {
    /// A heading starts a section: its id.
    pub fn heading(&mut self, text: &[Inline]) -> String {
        self.section = slug(&plain(text));
        self.listed = false;
        self.section.clone()
    }

    /// A list starts: what comes before the number in its items' ids (`` or `steps/`). Only
    /// the first list of a section has one.
    pub fn list(&mut self) -> Option<String> {
        if self.listed {
            return None;
        }
        self.listed = true;
        Some(if self.section.is_empty() { String::new() } else { format!("{}/", self.section) })
    }
}

/// The id of the `i`-th item (from 0) of a list: its number as written for a numbered list,
/// its rank otherwise (`3`, `steps/3`).
pub fn item_id(prefix: &str, start: Option<u64>, i: usize) -> String {
    format!("{}{}", prefix, start.unwrap_or(1) + i as u64)
}

/// The ids of blocks in reading order, added to `out`: a heading's [`slug`], list items
/// ([`item_id`]), a table's rows (slug of their first cell) then its columns (slug of their
/// header), an admonition's written title, tabs by name, the nodes of a `mermaid` block.
/// Nothing is left out: a title without letters gives an empty id, a name used twice comes
/// twice, so two texts of the same shape (a note and its translation) give lists that match
/// place by place.
pub fn collect(blocks: &[Block], namer: &mut Namer, out: &mut Vec<String>) {
    let push = |out: &mut Vec<String>, id: String| out.push(id);
    for b in blocks {
        match b {
            Block::Heading(_, h) => push(out, namer.heading(h)),
            Block::List { start, items } => {
                let key = namer.list();
                for (i, it) in items.iter().enumerate() {
                    if let Some(k) = &key {
                        push(out, item_id(k, *start, i));
                    }
                    collect(&it.blocks, namer, out);
                }
            }
            Block::Table { head, rows, .. } => {
                for r in rows {
                    push(out, r.first().map(|c| slug(&plain(c))).unwrap_or_default());
                }
                for c in head {
                    push(out, slug(&plain(c)));
                }
            }
            Block::Code { lang, text, .. } if lang == "mermaid" => {
                for id in crate::diagram::parse(text).ids() {
                    push(out, id.to_string());
                }
            }
            Block::Admonition { title, blocks, .. } => {
                if let Some(t) = title {
                    push(out, slug(&plain(t)));
                }
                collect(blocks, namer, out);
            }
            Block::Quote(q) => collect(q, namer, out),
            Block::Tabs(tabs) => {
                for t in tabs {
                    push(out, slug(&t.title));
                    collect(&t.blocks, namer, out);
                }
            }
            _ => {}
        }
    }
}

/// What a `!` line can glow in a markdown text shown as a page (see [`collect`]).
pub fn ids(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in all_ids(text) {
        if !id.is_empty() && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// Every id of a text in reading order, empty and repeated ones included (see [`collect`]).
pub fn all_ids(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    collect(&parse(text), &mut Namer::default(), &mut out);
    out
}

/// Whether a link or image address can be given to the browser: `http(s)`, `mailto`, an anchor
/// or a relative path. Anything with another scheme (`javascript:`, `data:`) is refused.
pub fn safe_url(url: &str) -> bool {
    let u = url.trim();
    let lower = u.to_ascii_lowercase();
    if ["http://", "https://", "mailto:"].iter().any(|p| lower.starts_with(p)) {
        return true;
    }
    // a `:` before the first `/`, `?` or `#` is a scheme
    let head = u.split(['/', '?', '#']).next().unwrap_or("");
    !u.is_empty() && !head.contains(':') && !u.chars().any(|c| c.is_control())
}

/// What codecasts wrote before this parser, made standard: `•` bullets, tables right under a
/// line of text, or without the `|---|` line under their first row. Code fences are left alone.
fn lenient(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let is_row = |l: &str| l.trim_start().starts_with('|');
    let is_rule = |l: &str| is_row(l) && l.contains('-') && l.trim().chars().all(|c| matches!(c, '|' | '-' | ':' | ' '));
    let mut out = String::with_capacity(text.len() + 16);
    let mut fence: Option<String> = None;
    for (n, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if in_fence(&mut fence, line) {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if let Some(rest) = t.strip_prefix("• ") {
            out.push_str(&line[..line.len() - t.len()]);
            out.push_str("- ");
            out.push_str(rest);
        } else {
            out.push_str(line);
        }
        // the first row of a table: on a line of its own block, with a rule under it
        let first_row = is_row(line) && !is_rule(line) && (n == 0 || !is_row(lines[n - 1]));
        if first_row && n > 0 && !lines[n - 1].trim().is_empty() {
            let at = out.len() - line.len();
            out.insert(at, '\n');
        }
        out.push('\n');
        if first_row && !lines.get(n + 1).is_some_and(|l| is_rule(l)) {
            let cells = t.trim().trim_matches('|').split('|').count();
            out.push_str(&line[..line.len() - t.len()]);
            out.push_str(&"|---".repeat(cells));
            out.push_str("|\n");
        }
    }
    out
}

/// Whether the event belongs to a run of inline text.
fn is_inline(e: &Event) -> bool {
    match e {
        Event::Text(_) | Event::Code(_) | Event::InlineHtml(_) | Event::Html(_) | Event::SoftBreak | Event::HardBreak | Event::TaskListMarker(_) => true,
        Event::InlineMath(_) | Event::DisplayMath(_) | Event::FootnoteReference(_) => true,
        Event::Start(t) => matches!(t, Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }),
        _ => false,
    }
}

/// Blocks up to the end of the container the parser is in (its `End` event is consumed).
fn blocks<'a, I: Iterator<Item = Event<'a>>>(it: &mut Peekable<I>) -> Vec<Block> {
    let mut out = Vec::new();
    loop {
        // a tight list item has its text directly inside, without a paragraph
        if it.peek().is_some_and(is_inline) {
            out.push(Block::Para(inlines(it)));
            continue;
        }
        let Some(ev) = it.next() else { return out };
        match ev {
            Event::End(_) => return out,
            Event::Rule => out.push(Block::Rule),
            Event::Start(Tag::Paragraph) => {
                out.push(Block::Para(inlines(it)));
                it.next();
            }
            Event::Start(Tag::Heading { level, .. }) => {
                out.push(Block::Heading(level as u8, inlines(it)));
                it.next();
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let (lang, title) = match kind {
                    CodeBlockKind::Fenced(info) => fence_info(&info),
                    CodeBlockKind::Indented => (String::new(), String::new()),
                };
                let mut text = String::new();
                for e in it.by_ref() {
                    match e {
                        Event::Text(t) => text.push_str(&t),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                }
                if text.ends_with('\n') {
                    text.pop();
                }
                out.push(Block::Code { lang, title, text });
            }
            // HTML is shown, not run
            Event::Start(Tag::HtmlBlock) => {
                let mut text = String::new();
                for e in it.by_ref() {
                    match e {
                        Event::Html(t) | Event::Text(t) => text.push_str(&t),
                        Event::End(TagEnd::HtmlBlock) => break,
                        _ => {}
                    }
                }
                out.push(Block::Para(vec![Inline::Text(text.trim_end().to_string())]));
            }
            Event::Start(Tag::BlockQuote(None)) => out.push(Block::Quote(blocks(it))),
            // GitHub's `> [!NOTE]`
            Event::Start(Tag::BlockQuote(Some(kind))) => {
                let kind = match kind {
                    BlockQuoteKind::Note => Callout::Note,
                    BlockQuoteKind::Tip => Callout::Tip,
                    BlockQuoteKind::Important => Callout::Info,
                    BlockQuoteKind::Warning => Callout::Warning,
                    BlockQuoteKind::Caution => Callout::Danger,
                };
                out.push(Block::Admonition { kind, title: None, fold: None, blocks: blocks(it) });
            }
            Event::Start(Tag::List(start)) => {
                let mut items = Vec::new();
                while let Some(Event::Start(Tag::Item)) = it.peek() {
                    it.next();
                    let task = match it.peek() {
                        Some(Event::TaskListMarker(done)) => Some(*done),
                        _ => None,
                    };
                    items.push(Item { task, blocks: blocks(it) });
                }
                it.next();
                out.push(Block::List { start, items });
            }
            Event::Start(Tag::Table(aligns)) => {
                let align = aligns
                    .iter()
                    .map(|a| match a {
                        Alignment::None => Align::None,
                        Alignment::Left => Align::Left,
                        Alignment::Center => Align::Center,
                        Alignment::Right => Align::Right,
                    })
                    .collect();
                let mut head = Vec::new();
                let mut rows = Vec::new();
                loop {
                    match it.next() {
                        Some(Event::Start(Tag::TableHead)) => head = cells(it),
                        Some(Event::Start(Tag::TableRow)) => rows.push(cells(it)),
                        _ => break,
                    }
                }
                out.push(Block::Table { align, head, rows });
            }
            // a container we do not draw (footnote, definition list): its content, flat
            Event::Start(_) => out.extend(blocks(it)),
            _ => {}
        }
    }
}

/// What follows a fence's backticks: the language (its first word) and the `title="…"` after it.
fn fence_info(info: &str) -> (String, String) {
    let lang = info.split_whitespace().next().unwrap_or("").to_string();
    let title = info.split_once("title=\"").and_then(|(_, t)| t.split_once('"')).map(|(t, _)| t.to_string()).unwrap_or_default();
    (lang, title)
}

/// The cells of a table row, up to its end.
fn cells<'a, I: Iterator<Item = Event<'a>>>(it: &mut Peekable<I>) -> Vec<Vec<Inline>> {
    let mut row = Vec::new();
    while let Some(Event::Start(Tag::TableCell)) = it.peek() {
        it.next();
        row.push(inlines(it));
        it.next();
    }
    it.next();
    row
}

/// A run of inline pieces. Stops before the event that ends the block (left to the caller),
/// or after the `End` of the emphasis, link or image it was called for.
fn inlines<'a, I: Iterator<Item = Event<'a>>>(it: &mut Peekable<I>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    // consecutive text and code go to one `Text`: speech reads them together
    fn text(out: &mut Vec<Inline>, s: &str) {
        match out.last_mut() {
            Some(Inline::Text(t)) => t.push_str(s),
            _ => out.push(Inline::Text(s.to_string())),
        }
    }
    loop {
        match it.peek() {
            Some(Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link | TagEnd::Image)) => {
                it.next();
                return out;
            }
            Some(e) if is_inline(e) => {}
            _ => return out,
        }
        match it.next().unwrap() {
            Event::Text(t) | Event::InlineHtml(t) | Event::Html(t) | Event::InlineMath(t) | Event::DisplayMath(t) => text(&mut out, &t),
            Event::FootnoteReference(t) => text(&mut out, &format!("[^{t}]")),
            Event::Code(c) if c.contains('`') => out.push(Inline::Code(c.to_string())),
            Event::Code(c) => text(&mut out, &format!("`{c}`")),
            Event::SoftBreak => text(&mut out, " "),
            Event::HardBreak => out.push(Inline::Break),
            Event::Start(Tag::Emphasis) => out.push(Inline::Em(inlines(it))),
            Event::Start(Tag::Strong) => out.push(Inline::Strong(inlines(it))),
            Event::Start(Tag::Strikethrough) => out.push(Inline::Strike(inlines(it))),
            Event::Start(Tag::Link { dest_url, .. }) => {
                let href = if safe_url(&dest_url) { dest_url.to_string() } else { String::new() };
                out.push(Inline::Link { href, text: inlines(it) });
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let src = if safe_url(&dest_url) { dest_url.to_string() } else { String::new() };
                let alt = plain(&inlines(it));
                out.push(Inline::Image { src, alt });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Inline {
        Inline::Text(s.to_string())
    }

    #[test]
    fn paragraphs_and_inline_marks() {
        let b = parse("A `Vec<u8>` and **bold**, *soft*, ~~gone~~\nsame line.\n\nNext.");
        assert_eq!(b.len(), 2);
        let Block::Para(p) = &b[0] else { panic!() };
        assert_eq!(p[0], t("A `Vec<u8>` and "));
        assert_eq!(p[1], Inline::Strong(vec![t("bold")]));
        assert_eq!(p[3], Inline::Em(vec![t("soft")]));
        assert_eq!(p[5], Inline::Strike(vec![t("gone")]));
        assert_eq!(p[6], t(" same line."));
        assert_eq!(b[1], Block::Para(vec![t("Next.")]));
    }

    #[test]
    fn html_stays_text() {
        assert_eq!(parse("returns Vec<String> or Option<T>."), vec![Block::Para(vec![t("returns Vec<String> or Option<T>.")])]);
        assert_eq!(parse("<script>alert(1)</script>"), vec![Block::Para(vec![t("<script>alert(1)</script>")])]);
        // code-looking words are not emphasis
        assert_eq!(parse("call next_task_id and a * b"), vec![Block::Para(vec![t("call next_task_id and a * b")])]);
    }

    #[test]
    fn headings_rules_quotes() {
        let b = parse("# One\n\n### Three\n\n---\n\n> quoted\n> text\n>\n> - item");
        assert_eq!(b[0], Block::Heading(1, vec![t("One")]));
        assert_eq!(b[1], Block::Heading(3, vec![t("Three")]));
        assert_eq!(b[2], Block::Rule);
        let Block::Quote(q) = &b[3] else { panic!() };
        assert_eq!(q[0], Block::Para(vec![t("quoted text")]));
        assert!(matches!(&q[1], Block::List { start: None, items } if items.len() == 1));
    }

    #[test]
    fn lists_nested_numbered_tasks() {
        let b = parse("1. one\n   - sub `x`\n   - sub two\n2. two\n\n• old bullet\n- [x] done\n- [ ] todo");
        let Block::List { start, items } = &b[0] else { panic!() };
        assert_eq!(*start, Some(1));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].blocks[0], Block::Para(vec![t("one")]));
        let Block::List { start: None, items: sub } = &items[0].blocks[1] else { panic!() };
        assert_eq!(sub[0].blocks, vec![Block::Para(vec![t("sub `x`")])]);
        let Block::List { items, .. } = &b[1] else { panic!() };
        assert_eq!(items.iter().map(|i| i.task).collect::<Vec<_>>(), vec![None, Some(true), Some(false)]);
        assert_eq!(items[0].blocks, vec![Block::Para(vec![t("old bullet")])]);
        assert_eq!(items[2].blocks, vec![Block::Para(vec![t("todo")])]);
    }

    #[test]
    fn code_blocks_keep_their_language() {
        let b = parse("```yaml\na: 1\n• not a bullet\n| not | a table |\n```\n\n    indented\n");
        assert_eq!(b[0], Block::Code { lang: "yaml".into(), title: String::new(), text: "a: 1\n• not a bullet\n| not | a table |".into() });
        assert_eq!(b[1], Block::Code { lang: String::new(), title: String::new(), text: "indented".into() });
        let b = parse("~~~rust title=\"src/main.rs\"\nfn main() {}\n~~~");
        assert_eq!(b[0], Block::Code { lang: "rust".into(), title: "src/main.rs".into(), text: "fn main() {}".into() });
    }

    #[test]
    fn admonitions() {
        let b = parse("Before.\n\n!!! warning \"Mind the `lock`\"\n    First line.\n\n    - item\n\n    ??? tip\n        Inside.\nAfter.\n\n???+ summary \"\"\n    Open.");
        assert_eq!(b[0], Block::Para(vec![t("Before.")]));
        let Block::Admonition { kind: Callout::Warning, title: Some(title), fold: None, blocks } = &b[1] else { panic!("{b:?}") };
        assert_eq!(title, &vec![t("Mind the `lock`")]);
        assert_eq!(blocks[0], Block::Para(vec![t("First line.")]));
        assert!(matches!(&blocks[1], Block::List { items, .. } if items.len() == 1));
        assert_eq!(blocks[2], Block::Admonition { kind: Callout::Tip, title: None, fold: Some(false), blocks: vec![Block::Para(vec![t("Inside.")])] });
        assert_eq!(b[2], Block::Para(vec![t("After.")]));
        assert_eq!(b[3], Block::Admonition { kind: Callout::Abstract, title: Some(vec![]), fold: Some(true), blocks: vec![Block::Para(vec![t("Open.")])] });
        // GitHub's form, and what is not one
        assert_eq!(parse("> [!CAUTION]\n> Hot."), vec![Block::Admonition { kind: Callout::Danger, title: None, fold: None, blocks: vec![Block::Para(vec![t("Hot.")])] }]);
        assert_eq!(parse("```\n!!! note\n    code\n```"), vec![Block::Code { lang: String::new(), title: String::new(), text: "!!! note\n    code".into() }]);
        assert_eq!(parse("!!!"), vec![Block::Para(vec![t("!!!")])]);
    }

    #[test]
    fn tables_with_and_without_rule() {
        let with = parse("| a | b |\n|:--|--:|\n| 1 | `x` |");
        let Block::Table { align, head, rows } = &with[0] else { panic!() };
        assert_eq!(align, &vec![Align::Left, Align::Right]);
        assert_eq!(head, &vec![vec![t("a")], vec![t("b")]]);
        assert_eq!(rows, &vec![vec![vec![t("1")], vec![t("`x`")]]]);
        let without = parse("- item\n| a | b |\n| 1 | 2 |\n");
        assert!(matches!(&without[0], Block::List { items, .. } if items.len() == 1));
        let Block::Table { head, rows, .. } = &without[1] else { panic!("{without:?}") };
        assert_eq!(head.len(), 2);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn links_and_images() {
        let b = parse("[docs](https://example.org/a) [bad](javascript:alert(1)) ![logo](img/logo.png) <https://example.org>");
        let Block::Para(p) = &b[0] else { panic!() };
        assert_eq!(p[0], Inline::Link { href: "https://example.org/a".into(), text: vec![t("docs")] });
        assert_eq!(p[2], Inline::Link { href: String::new(), text: vec![t("bad")] });
        assert_eq!(p[4], Inline::Image { src: "img/logo.png".into(), alt: "logo".into() });
        assert_eq!(p[6], Inline::Link { href: "https://example.org".into(), text: vec![t("https://example.org")] });
        assert!(safe_url("#top") && safe_url("../a.md") && safe_url("mailto:a@example.org"));
        assert!(!safe_url("data:text/html,x") && !safe_url(" JavaScript:x") && !safe_url(""));
    }

    #[test]
    fn tabs() {
        let b = parse("=== \"Rust\"\n    Fast.\n\n    - item\n\n=== \"C\"\n    Old.\n\nAfter.\n\n=== \"Alone\"\n    x\n\nTitle\n===\n");
        let Block::Tabs(tabs) = &b[0] else { panic!("{b:?}") };
        assert_eq!(tabs.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), vec!["Rust", "C"]);
        assert_eq!(tabs[0].blocks.len(), 2);
        assert_eq!(tabs[1].blocks, vec![Block::Para(vec![t("Old.")])]);
        assert_eq!(b[1], Block::Para(vec![t("After.")]));
        assert!(matches!(&b[2], Block::Tabs(tabs) if tabs.len() == 1));
        // a setext heading is not a tab
        assert_eq!(b[3], Block::Heading(1, vec![t("Title")]));
    }

    #[test]
    fn ids_of_a_page() {
        assert_eq!(slug(" Step two: `next_task`! "), "step-two-next_task");
        let text = "- a\n- b\n\n## The Steps\n3. three\n4. four\n   - sub\n\n- other list\n\n| field | size |\n|---|---|\n| `len` | 2 |\n\n!!! tip \"Mind it\"\n    x\n\n=== \"Wire\"\n    ~~~mermaid\n    sequenceDiagram\n    c->>s: hi\n    ~~~\n";
        assert_eq!(ids(text), ["1", "2", "the-steps", "the-steps/3", "the-steps/4", "len", "field", "size", "mind-it", "wire", "c", "s", "m1"]);
    }

    #[test]
    fn hard_breaks() {
        let b = parse("one  \ntwo\\\nthree");
        assert_eq!(b[0], Block::Para(vec![t("one"), Inline::Break, t("two"), Inline::Break, t("three")]));
    }
}
