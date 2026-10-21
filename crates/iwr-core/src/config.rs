//! The codecast's own look and defaults: `codecast.toml` next to the script (inside a
//! `codecast/` directory, or beside `codecast.md`). The author picks the theme, the fonts (box
//! titles above all), the colours, the voice and the language the viewer starts with.
//!
//! ```toml
//! theme = "forest"           # a built-in theme (THEMES)
//! lang = "en"                # language the codecast plays in (i18n::LANGS)
//! voice = "tts"              # tts | audio | read
//! tts_voice = "Samantha"     # browser voice whose name contains this, when there is one
//! rate = 1.1                 # speech rate, 0.5..2
//! tour = true                # play every part in a row
//! glossary = false           # built-in glossary questions
//! lock = false               # true: the viewer cannot change theme, fonts, language and voice
//!
//! [fonts]                    # a bundled font (FONTS), a font file next to the script, or an installed font
//! title = "lost-tumbler"                # titles in the boxes (and the headings)
//! text = "montserrat"                   # the rest of the interface
//! code = "fonts/FiraCode-Regular.ttf"   # code
//!
//! [colors]                   # theme variables to override
//! accent = "#ff7a00"
//! ```
//!
//! Without `lock`, these are defaults: they are applied when the viewer first opens the codecast
//! (or when the author changes a value), and a setting the viewer changes afterwards is kept.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// File name of the configuration, next to the script.
pub const FILE: &str = "codecast.toml";

/// (id, label, dark?) of every built-in theme; the id is the UI's CSS class suffix (`.theme-<id>`).
pub const THEMES: &[(&str, &str, bool)] = &[
    ("dark", "Dark", true),
    ("light", "Light", false),
    ("paper", "Paper", false),
    ("midnight", "Midnight", true),
    ("forest", "Forest", true),
    ("nord", "Nord", true),
    ("solar", "Solar", false),
    ("dusk", "Dusk", true),
    ("dracula", "Dracula", true),
    ("monokai", "Monokai", true),
    ("gruvbox", "Gruvbox", true),
    ("gruvbox-light", "Gruvbox Light", false),
    ("catppuccin", "Catppuccin", true),
    ("latte", "Latte", false),
    ("tokyo", "Tokyo Night", true),
    ("onedark", "One Dark", true),
    ("github", "GitHub", false),
    ("github-dark", "GitHub Dark", true),
    ("rosepine", "Rosé Pine", true),
    ("dawn", "Rosé Dawn", false),
    ("everforest", "Everforest", true),
    ("kanagawa", "Kanagawa", true),
    ("ayu", "Ayu Light", false),
    ("synthwave", "Synthwave", true),
    ("ocean", "Ocean", true),
    ("sakura", "Sakura", false),
    ("mint", "Mint", false),
    ("desert", "Desert", false),
    ("coffee", "Coffee", true),
    ("matrix", "Matrix", true),
    ("cyber", "Cyberpunk", true),
    ("contrast", "High Contrast", true),
];

pub fn is_theme(id: &str) -> bool {
    THEMES.iter().any(|(t, _, _)| *t == id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontKind {
    /// bold or decorated, for titles
    Display,
    /// handwriting
    Script,
    /// plain enough for the whole interface
    Text,
}

impl FontKind {
    pub fn label(self) -> &'static str {
        match self {
            FontKind::Display => "Display",
            FontKind::Script => "Handwritten",
            FontKind::Text => "Text",
        }
    }
}

/// A font bundled with the UI: `[fonts]` names it by `id` (`title = "akieir"`).
#[derive(Clone, Copy, Debug)]
pub struct Font {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: FontKind,
}

/// The bundled fonts (`crates/iwr-ui/assets/fonts/<id>.woff2`).
pub const FONTS: &[Font] = &[
    Font { id: "akieir", name: "Akieir", kind: FontKind::Display },
    Font { id: "anak-bocil", name: "Anak Bocil", kind: FontKind::Display },
    Font { id: "apoca", name: "Apoca", kind: FontKind::Display },
    Font { id: "asal-usul", name: "Asal Usul", kind: FontKind::Display },
    Font { id: "brimful", name: "Brimful", kind: FontKind::Display },
    Font { id: "catways", name: "Catways", kind: FontKind::Display },
    Font { id: "cozy-autumns", name: "Cozy Autumns", kind: FontKind::Display },
    Font { id: "digitalt", name: "Digitalt", kind: FontKind::Display },
    Font { id: "lost-tumbler", name: "Lost Tumbler", kind: FontKind::Display },
    Font { id: "north-kids", name: "North Kids", kind: FontKind::Display },
    Font { id: "outline-style", name: "Outline Style", kind: FontKind::Display },
    Font { id: "pasti", name: "Pasti", kind: FontKind::Display },
    Font { id: "quango", name: "Quango", kind: FontKind::Display },
    Font { id: "rubintek", name: "Rubintek", kind: FontKind::Display },
    Font { id: "slow-play", name: "Slow Play", kind: FontKind::Display },
    Font { id: "super-shiny", name: "Super Shiny", kind: FontKind::Display },
    Font { id: "thesead", name: "Thesead", kind: FontKind::Display },
    Font { id: "zerove", name: "Zerove", kind: FontKind::Display },
    Font { id: "library-3am", name: "Library 3 AM", kind: FontKind::Display },
    Font { id: "aicon", name: "Aicon", kind: FontKind::Display },
    Font { id: "rahull", name: "Rahull", kind: FontKind::Script },
    Font { id: "hey-august", name: "Hey August", kind: FontKind::Script },
    Font { id: "mocaroni", name: "Mocaroni", kind: FontKind::Script },
    Font { id: "montserrat", name: "Montserrat", kind: FontKind::Text },
    Font { id: "josefin-sans", name: "Josefin Sans", kind: FontKind::Text },
    Font { id: "zt-otez", name: "ZT Otez", kind: FontKind::Text },
    Font { id: "glametrix", name: "Glametrix", kind: FontKind::Text },
    Font { id: "timeburner", name: "Timeburner", kind: FontKind::Text },
    Font { id: "ovaline", name: "Ovaline", kind: FontKind::Text },
    Font { id: "steiner", name: "Steiner", kind: FontKind::Text },
    Font { id: "cambai", name: "Cambai", kind: FontKind::Text },
];

/// A bundled font by id.
pub fn font(id: &str) -> Option<&'static Font> {
    FONTS.iter().find(|f| f.id == id)
}

/// Theme variables `[colors]` may set (CSS `--name`).
pub const COLORS: &[&str] = &["bg", "panel", "panel2", "panel3", "border", "text", "muted", "accent", "hi", "hl-bg", "sel-bg", "code", "grid"];

/// Font files a `[fonts]` entry may name.
pub const FONT_EXTS: &[&str] = &["ttf", "otf", "woff", "woff2"];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub theme: Option<String>,
    pub lang: Option<String>,
    /// `tts`, `audio` (the recording, when there is one) or `read`
    pub voice: Option<String>,
    /// part of the name of the browser voice to speak with
    pub tts_voice: Option<String>,
    pub rate: Option<f64>,
    pub tour: Option<bool>,
    pub glossary: Option<bool>,
    /// the viewer cannot change theme, fonts, language and voice
    pub lock: bool,
    pub fonts: Fonts,
    pub colors: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Fonts {
    /// titles in the graph boxes, codecast title and headings
    pub title: Option<String>,
    /// the interface text
    pub text: Option<String>,
    /// code
    pub code: Option<String>,
}

impl Fonts {
    /// (role, value) of every font set: `title`, `text`, `code`.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [("title", &self.title), ("text", &self.text), ("code", &self.code)].into_iter().filter_map(|(r, v)| Some((r, v.as_deref()?)))
    }
}

/// Is this font value a file (relative to the script's directory) rather than an installed font's name?
pub fn is_font_file(v: &str) -> bool {
    v.rsplit_once('.').map(|(_, e)| FONT_EXTS.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false)
}

/// A relative path inside the codecast directory (no `..`, not absolute, no backslash), that
/// can also sit in a quoted CSS `url("…")`.
pub fn is_safe_path(p: &str) -> bool {
    !p.is_empty() && !p.split('/').any(|s| s == ".." || s.is_empty()) && !p.chars().any(|c| "\\:\"'".contains(c) || c.is_control())
}

/// A colour value that cannot break out of a CSS declaration (`#hex`, `rgb(…)`, a name, …).
pub fn is_safe_color(v: &str) -> bool {
    !v.is_empty() && v.len() <= 64 && v.chars().all(|c| c.is_ascii_alphanumeric() || " #(),.%-".contains(c))
}

/// An installed font's name, safe inside a quoted CSS string.
pub fn is_safe_font_name(v: &str) -> bool {
    !v.is_empty() && v.len() <= 64 && v.chars().all(|c| c.is_alphanumeric() || " -_".contains(c))
}

impl Config {
    #[cfg(feature = "fs")]
    pub fn from_toml(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The config as a commented `codecast.toml` (what the Settings page saves or downloads).
    /// Unset values are left out.
    pub fn to_toml(&self) -> String {
        fn q(s: &str) -> String {
            let mut o = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => o.push_str("\\\""),
                    '\\' => o.push_str("\\\\"),
                    c if c.is_control() => o.push_str(&format!("\\u{:04X}", c as u32)),
                    c => o.push(c),
                }
            }
            o.push('"');
            o
        }
        let mut t = String::from("# The look and defaults of this codecast (see docs/codecast.md, \"Look and defaults\").\n");
        let mut line = |k: &str, v: Option<String>, note: &str| {
            if let Some(v) = v {
                t.push_str(&format!("{:<26}# {}\n", format!("{} = {}", k, v), note));
            }
        };
        line("theme", self.theme.as_deref().map(q), "a built-in theme (Settings → Appearance)");
        line("lang", self.lang.as_deref().map(q), "language the codecast plays in");
        line("voice", self.voice.as_deref().map(q), "tts | audio | read");
        line("tts_voice", self.tts_voice.as_deref().map(q), "browser voice whose name contains this");
        line("rate", self.rate.map(|r| format!("{:?}", r)), "speech rate, 0.5..2");
        line("tour", self.tour.map(|b| b.to_string()), "play every part in a row");
        line("glossary", self.glossary.map(|b| b.to_string()), "built-in glossary questions");
        line("lock", self.lock.then(|| "true".to_string()), "the viewer cannot change theme, fonts, language and voice");
        if self.fonts.iter().next().is_some() {
            t.push_str("\n[fonts]                   # a bundled font, a font file next to this one, or an installed font\n");
            for (role, v) in self.fonts.iter() {
                t.push_str(&format!("{} = {}\n", role, q(v)));
            }
        }
        if !self.colors.is_empty() {
            t.push_str("\n[colors]                  # theme variables to override\n");
            for (k, v) in &self.colors {
                let k = if k.contains('-') { q(k) } else { k.clone() };
                t.push_str(&format!("{} = {}\n", k, q(v)));
            }
        }
        t
    }

    /// Font files to serve or export with the script, relative to its directory.
    pub fn font_files(&self) -> Vec<String> {
        self.fonts.iter().map(|(_, v)| v.to_string()).filter(|v| is_font_file(v) && is_safe_path(v)).collect()
    }

    /// What is wrong with the values (font files are checked on disk by the CLI).
    pub fn problems(&self) -> Vec<String> {
        let mut bad = Vec::new();
        if let Some(t) = self.theme.as_deref().filter(|t| !is_theme(t)) {
            bad.push(format!("unknown theme `{}` (one of: {})", t, THEMES.iter().map(|t| t.0).collect::<Vec<_>>().join(", ")));
        }
        if let Some(l) = self.lang.as_deref().filter(|l| crate::i18n::lang(l).is_none()) {
            bad.push(format!("unknown lang `{}` (one of: {})", l, crate::i18n::LANGS.iter().map(|l| l.code).collect::<Vec<_>>().join(", ")));
        }
        if let Some(v) = self.voice.as_deref().filter(|v| !["tts", "audio", "read"].contains(v)) {
            bad.push(format!("unknown voice `{}` (tts, audio or read)", v));
        }
        if let Some(r) = self.rate.filter(|r| !(0.5..=2.0).contains(r)) {
            bad.push(format!("rate {} out of 0.5..2", r));
        }
        for (role, v) in self.fonts.iter() {
            if is_font_file(v) {
                if !is_safe_path(v) {
                    bad.push(format!("fonts.{}: `{}` must be a relative path inside the codecast directory", role, v));
                }
            } else if !is_safe_font_name(v) {
                bad.push(format!("fonts.{}: `{}` is neither a font file ({}) nor a font name", role, v, FONT_EXTS.join(", ")));
            }
        }
        for (k, v) in &self.colors {
            if !COLORS.contains(&k.as_str()) {
                bad.push(format!("colors: unknown `{}` (one of: {})", k, COLORS.join(", ")));
            } else if !is_safe_color(v) {
                bad.push(format!("colors.{}: `{}` is not a colour", k, v));
            }
        }
        bad
    }
}

#[cfg(all(test, feature = "fs"))]
mod tests {
    use super::*;

    #[test]
    fn parses_and_checks() {
        let c = Config::from_toml("theme = \"forest\"\nvoice = \"read\"\n[fonts]\ntitle = \"fonts/Bangers.ttf\"\ntext = \"Nunito\"\n[colors]\naccent = \"#ff7a00\"\n").unwrap();
        assert_eq!(c.theme.as_deref(), Some("forest"));
        assert_eq!(c.font_files(), vec!["fonts/Bangers.ttf".to_string()]);
        assert!(c.problems().is_empty(), "{:?}", c.problems());
        assert!(Config::from_toml("colour = 1").is_err(), "unknown keys are refused");
        let bad = Config::from_toml("theme = \"neon\"\n[fonts]\ntitle = \"../x.ttf\"\n[colors]\naccent = \"red;}\"\nfoo = \"red\"\n").unwrap();
        assert_eq!(bad.problems().len(), 4, "{:?}", bad.problems());
    }

    #[test]
    fn writes_what_it_reads() {
        let mut c = Config { theme: Some("nord".into()), rate: Some(1.0), tour: Some(false), lock: true, tts_voice: Some("Name \"q\" \\".into()), ..Default::default() };
        c.fonts.title = Some("fonts/My Font.ttf".into());
        c.colors.insert("hl-bg".into(), "#112233".into());
        c.colors.insert("accent".into(), "rgb(1, 2, 3)".into());
        let back = Config::from_toml(&c.to_toml()).unwrap();
        assert_eq!(back, c, "{}", c.to_toml());
        assert_eq!(Config::from_toml(&Config::default().to_toml()).unwrap(), Config::default());
    }
}
