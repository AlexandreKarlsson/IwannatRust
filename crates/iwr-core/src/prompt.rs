//! The prompt an author hands to an AI agent to write a codecast.
//!
//! Only the author's wishes go in it, in a few short lines: the format and the writing rules come
//! from `codecast brief <path> --guide`, which the agent runs first, so a prompt copied today still
//! works when the guide changes. The Generate page fills [`PromptOptions`] and shows [`build`].

use serde::{Deserialize, Serialize};

/// What the agent should do with the codecast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Task {
    #[default]
    New,
    Improve,
    AddPart,
}

/// Who listens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Audience {
    /// knows Rust, new to this code
    #[default]
    Newcomer,
    /// new to Rust too
    RustBeginner,
    /// knows the field: reviewers, maintainers
    Expert,
}

/// How much humour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Humour {
    None,
    #[default]
    Light,
    Playful,
}

/// Metaphors: none, the agent's, or the author's own (`PromptOptions::metaphor_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Metaphor {
    None,
    #[default]
    Ai,
    Mine,
}

/// How much a page (view or block kind) is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Want {
    #[default]
    Free,
    Prefer,
    Must,
    Avoid,
}

impl Want {
    /// The next state of a chip clicked in the UI.
    pub fn next(self) -> Want {
        match self {
            Want::Free => Want::Prefer,
            Want::Prefer => Want::Must,
            Want::Must => Want::Avoid,
            Want::Avoid => Want::Free,
        }
    }
}

/// The pages an author may favour, require or avoid: (key, how the prompt names it).
pub const PAGES: &[(&str, &str)] = &[
    ("arch", "`@ arch`"),
    ("flow", "`@ flow`"),
    ("code", "`@ code`"),
    ("calls", "`@ calls`"),
    ("sequence", "`@ sequence:fn`"),
    ("types", "`@ types`"),
    ("errors", "`@ errors`"),
    ("branches", "`@ branches`"),
    ("structure", "`@ structure`"),
    ("diagram", "mermaid flowchart (`@ diagram`)"),
    ("seqdiagram", "mermaid sequenceDiagram"),
    ("chart", "chart block"),
    ("bytes", "bytes block"),
    ("example", "data example block"),
    ("tours", "generated tours (`*_tour`)"),
];

/// The author's wishes. Empty texts and `parts == 0` leave the choice to the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptOptions {
    pub task: Task,
    /// what to explain: a module, a feature, a flow (empty: the whole project)
    pub subject: String,
    pub audience: Audience,
    /// one concrete thing to follow through the parts ("Ann's order")
    pub example: String,
    /// about how many parts (0: the agent decides)
    pub parts: u8,
    pub humour: Humour,
    pub metaphor: Metaphor,
    pub metaphor_text: String,
    /// page key (see [`PAGES`]) → how much it is wanted; missing = `Free`
    pub pages: Vec<(String, Want)>,
    /// show the outline and wait for an OK before writing
    pub plan_first: bool,
    /// may add `//!` `///` `//` comments to the code
    pub edit_code: bool,
    pub two_voices: bool,
    pub questions: bool,
    pub notes: String,
}

impl Default for PromptOptions {
    fn default() -> Self {
        PromptOptions {
            task: Task::New,
            subject: String::new(),
            audience: Audience::Newcomer,
            example: String::new(),
            parts: 0,
            humour: Humour::Light,
            metaphor: Metaphor::Ai,
            metaphor_text: String::new(),
            pages: vec![],
            plan_first: true,
            edit_code: true,
            two_voices: true,
            questions: true,
            notes: String::new(),
        }
    }
}

impl PromptOptions {
    pub fn want(&self, key: &str) -> Want {
        self.pages.iter().find(|(k, _)| k == key).map(|(_, w)| *w).unwrap_or_default()
    }

    pub fn set_want(&mut self, key: &str, w: Want) {
        self.pages.retain(|(k, _)| k != key);
        if w != Want::Free {
            self.pages.push((key.to_string(), w));
        }
    }

    /// Prompt names of the pages wanted that much, in [`PAGES`] order.
    fn named(&self, w: Want) -> String {
        PAGES.iter().filter(|(k, _)| self.want(k) == w).map(|(_, n)| *n).collect::<Vec<_>>().join(", ")
    }
}

/// The prompt for the project at `path`, named once: the commands run from there with `.`.
pub fn build(path: &str, o: &PromptOptions) -> String {
    let p = if path.trim().is_empty() { "." } else { path.trim() };
    let mut s = String::new();
    let task = match o.task {
        Task::New => "Write a codecast (a spoken, animated tour of the code)",
        Task::Improve => "Improve the existing codecast in `codecast/` (keep what works)",
        Task::AddPart => "Add one or more parts to the existing codecast in `codecast/`",
    };
    s += &format!("{task} for the Rust project in `{p}`. Run the commands below from there.\n\n");
    s += "Steps:\n";
    s += "1. Run `codecast brief . --guide`: the format, the writing rules and the code. Follow the rules.\n";
    s += if o.edit_code {
        "2. Add missing `//!` `///` `//` comments to the code first (see the guide).\n"
    } else {
        "2. Do not edit the code.\n"
    };
    if o.plan_first {
        s += "3. Show me the parts (title + one line each) and wait for my OK.\n";
    }
    let n = if o.plan_first { 4 } else { 3 };
    s += &format!("{n}. Write `codecast/` (`index.md` + one file per part).\n");
    s += &format!("{}. Run `codecast check codecast/ --path .` until clean, read `codecast speak codecast/` and fix what sounds wrong.\n", n + 1);
    s += &format!("{}. Tell me to run `codecast serve .`.\n", n + 2);

    let mut w: Vec<String> = vec![];
    let line = |w: &mut Vec<String>, label: &str, v: &str| {
        if !v.trim().is_empty() {
            w.push(format!("- {label}: {}", v.trim()));
        }
    };
    line(&mut w, "Subject", &o.subject);
    w.push(
        match o.audience {
            Audience::Newcomer => "- Audience: knows Rust, new to this code",
            Audience::RustBeginner => "- Audience: new to Rust too; explain the language mechanisms met",
            Audience::Expert => "- Audience: experienced (reviewers, maintainers); dense, skip basics",
        }
        .into(),
    );
    line(&mut w, "Follow this example through the parts", &o.example);
    if o.parts > 0 {
        w.push(format!("- Length: about {} parts", o.parts));
    }
    w.push(
        match o.humour {
            Humour::None => "- Humour: none",
            Humour::Light => "- Humour: a light touch",
            Humour::Playful => "- Humour: playful, a joke tied to the code in most parts",
        }
        .into(),
    );
    match o.metaphor {
        Metaphor::None => w.push("- Metaphors: none".into()),
        Metaphor::Ai => {}
        Metaphor::Mine if o.metaphor_text.trim().is_empty() => {}
        Metaphor::Mine => w.push(format!("- Main metaphor: {}", o.metaphor_text.trim())),
    }
    line(&mut w, "Use often", &o.named(Want::Prefer));
    line(&mut w, "Use at least once", &o.named(Want::Must));
    line(&mut w, "Avoid", &o.named(Want::Avoid));
    if !o.two_voices {
        w.push("- No second voice (`sp:`)".into());
    }
    if !o.questions {
        w.push("- No `?` questions".into());
    }
    line(&mut w, "Notes", &o.notes);
    s += "\nWishes:\n";
    s += &w.join("\n");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_short() {
        let s = build("", &PromptOptions::default());
        assert!(s.contains("codecast brief . --guide") && s.contains("project in `.`"));
        assert!(s.contains("3. Show me the parts"));
        assert!(s.contains("6. Tell me to run `codecast serve .`"));
        assert!(!s.contains("Subject") && !s.contains("Metaphor") && !s.contains("sp:"));
        assert!(s.len() < 900, "{} bytes", s.len());
    }

    #[test]
    fn wishes_and_pages() {
        let mut o = PromptOptions { task: Task::AddPart, subject: "the scheduler".into(), plan_first: false, edit_code: false, metaphor: Metaphor::Mine, metaphor_text: "a bakery queue".into(), parts: 3, ..Default::default() };
        o.set_want("seqdiagram", Want::Must);
        o.set_want("arch", Want::Prefer);
        o.set_want("chart", Want::Avoid);
        o.set_want("arch", Want::Prefer);
        let s = build("/p", &o);
        assert_eq!(s.matches("/p").count(), 1);
        assert!(s.starts_with("Add one or more parts"));
        assert!(s.contains("2. Do not edit"));
        assert!(s.contains("3. Write `codecast/`"));
        assert!(s.contains("- Subject: the scheduler"));
        assert!(s.contains("- Main metaphor: a bakery queue"));
        assert!(s.contains("- Use often: `@ arch`\n"));
        assert!(s.contains("- Use at least once: mermaid sequenceDiagram"));
        assert!(s.contains("- Avoid: chart block"));
        assert!(s.contains("about 3 parts"));
        // every chip state comes back to free
        assert_eq!(Want::Free.next().next().next().next(), Want::Free);
    }
}
