//! What iwr-core says to people, in each interface language: view names and descriptions, the
//! notes of an empty view, the sentences of generated tours. Not the content translation of a
//! codecast (that is [`crate::i18n`]).
//!
//! Texts live in `crates/iwr-core/i18n/<lang>/<module>.toml`; the build turns each key into a
//! function that takes the language first: `t::views::label_code(lang)`. A key missing in one
//! language, or an argument that differs, stops the build.

include!(concat!(env!("OUT_DIR"), "/i18n.rs"));

/// Opens a piece of author text (a doc comment, a `//` note, a TODO) inside a sentence told in
/// another language than English: [`crate::i18n`] machine-translates only these pieces of a
/// told text and drops the marks. English text carries no marks.
pub const OPEN: char = '⟦';
/// Closes a piece opened by [`OPEN`].
pub const CLOSE: char = '⟧';

/// What goes between two sentences: a space in English, nothing in Japanese.
pub fn gap(lang: Lang) -> &'static str {
    match lang {
        Lang::En => " ",
        Lang::Ja => "",
    }
}

/// Sentences joined with [`gap`], empty ones left out.
pub fn sentences(lang: Lang, parts: &[&str]) -> String {
    parts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(gap(lang))
}

/// A lead word and the sentence it opens: "First, `A`…" in English; in Japanese no space, except
/// before inline code after a word ("まずは `A`…", but "まず、`A`…").
pub fn led(lang: Lang, lead: &str, rest: &str) -> String {
    match lang {
        Lang::En => format!("{lead} {rest}"),
        Lang::Ja if rest.starts_with('`') && !lead.ends_with(['、', '。', '：', '「', '（']) => format!("{lead} {rest}"),
        Lang::Ja => format!("{lead}{rest}"),
    }
}

/// Author text inside a told sentence: as it is in English, else marked for translation
/// ([`OPEN`] … [`CLOSE`]).
pub fn authored(lang: Lang, text: &str) -> String {
    if lang == Lang::En || text.trim().is_empty() {
        text.to_string()
    } else {
        format!("{OPEN}{text}{CLOSE}")
    }
}

/// "a, b and c" in `lang`.
pub fn join_and(lang: Lang, items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => t::guide::list_and(lang, items[..n - 1].join(t::guide::list_sep(lang)), &items[n - 1]),
    }
}

/// The text without its [`OPEN`] / [`CLOSE`] marks (author text left as written).
pub fn unmark(text: &str) -> String {
    text.chars().filter(|c| *c != OPEN && *c != CLOSE).collect()
}
