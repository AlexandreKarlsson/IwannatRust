//! Markdown of notes, answers and captions, read into a small tree the UI draws.
//!
//! CommonMark with tables, strikethrough and task lists (parsed by `pulldown-cmark`). Two things
//! are ours: text keeps its code spans between backticks, so [`crate::speech::segments`] can
//! give each its spoken form, and HTML is never interpreted: it stays text, because codecast
//! prose is full of `Vec<String>`.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::iter::Peekable;

/// A block of a markdown text.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Para(Vec<Inline>),
    /// Level 1 to 6.
    Heading(u8, Vec<Inline>),
    /// A fenced or indented code block; `lang` is what follows the fence's backticks.
    Code { lang: String, text: String },
    Quote(Vec<Block>),
    /// `start` is the first number of a numbered list.
    List { start: Option<u64>, items: Vec<Item> },
    /// One [`Align`] per column; cells are inline text.
    Table { align: Vec<Align>, head: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
    Rule,
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
    let src = lenient(text);
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut events = Parser::new_ext(&src, opts).peekable();
    blocks(&mut events)
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
        if let Some(f) = &fence {
            if t.starts_with(f.as_str()) && t.trim_end().chars().all(|c| f.starts_with(c)) {
                fence = None;
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            let mark = t.chars().next().unwrap();
            fence = Some(t.chars().take_while(|&c| c == mark).collect());
            out.push_str(line);
        } else if let Some(rest) = t.strip_prefix("• ") {
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
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => String::new(),
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
                out.push(Block::Code { lang, text });
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
            Event::Start(Tag::BlockQuote(_)) => out.push(Block::Quote(blocks(it))),
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
        assert_eq!(b[0], Block::Code { lang: "yaml".into(), text: "a: 1\n• not a bullet\n| not | a table |".into() });
        assert_eq!(b[1], Block::Code { lang: String::new(), text: "indented".into() });
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
    fn hard_breaks() {
        let b = parse("one  \ntwo\\\nthree");
        assert_eq!(b[0], Block::Para(vec![t("one"), Inline::Break, t("two"), Inline::Break, t("three")]));
    }
}
