# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Generate page (top bar → *generate*, with `iwr serve`): record the codecast's voice with Kokoro
  from the browser. Pick the language, a voice from a gallery (▶ plays a sample), speed, pause
  between cues and compression, and the parts (missing or changed ones by default); a live
  progress bar follows every cue with the time left, and *Cancel* keeps finished parts. The top
  bar shows how many parts need a recording, or a progress ring while one runs; the player loads
  the new recordings when it ends. API: `/api/voice`, `/api/voice/job`, `/api/voice/cancel`,
  `/api/voice/preview`. `?page=generate` / `?page=settings` open a page from the URL.

- Codecast examples: a ```` ```json Body ```` block (also csv, tsv, yaml, toml, xml, http, sql, sh,
  text) shows sample data in the side panel with `| example:Body`; `!` glows its keys or csv columns.
- Writing guide: a *Voice* section (plain words, one metaphor per idea, show the data) and an
  *Examples* section, both in `iwr brief --guide`.
- Coloured Rust code: the source panes (Code view and details) are syntax highlighted, and each
  line carries the colour of its item (function, method, struct, impl…) as a gutter stripe and a
  light tint, plus the colour of its block (if, match, loop, `?`…), matching the boxes. Signatures
  and types in the blocks, details and tooltips are coloured too.
- `codecast.toml` next to the script: the author's look and defaults for the viewer — theme,
  colour overrides (`[colors]`), fonts (`[fonts] title` for the titles in the graph boxes and the
  codecast headings, `text`, `code`; a font file shipped with the codecast or an installed font's
  name), voice mode, browser voice (`tts_voice`), language, rate, full tour, glossary. Values are
  applied when new or changed by the author, then the viewer's own choices stay; `lock = true`
  fixes theme, fonts, language and voice. Served at `/api/config` (fonts through `/api/codecast/`),
  exported as `codecast.json` + `codecast/<font>`, validated by `iwr check`.
- 31 bundled fonts (woff2, from the Club-App font folder) shown as a gallery in Settings →
  Appearance: the viewer picks one for the box titles and one for the interface text;
  `[fonts] title = "lost-tumbler"` picks one for a codecast. `lock` now covers fonts too.
- 24 more themes (32 in all): Dracula, Monokai, Gruvbox (dark and light), Catppuccin, Latte,
  Tokyo Night, One Dark, GitHub (light and dark), Rosé Pine, Rosé Dawn, Everforest, Kanagawa, Ayu
  Light, Synthwave, Ocean, Sakura, Mint, Desert, Coffee, Matrix, Cyberpunk, High Contrast. The
  catalogue lives in `iwr_core::config::THEMES`.
- `docs/commenting.md`: how to comment code so it reads well in the UI — which comment becomes
  which box title, flow node or exit arrow, the title rules (one line, ≤ 60 characters, blank line
  after a `//!` / `///` title), doc pages, a checklist.
- Settings page redesigned: side menu that follows the scroll, card sections, theme cards drawn as
  a small interface in their colours (All / Dark / Light), font cards drawn in their font,
  switches, steppers.
- Settings → *Codecast design*, behind a *Create the codecast configuration* button with an ⓘ explanation: build `codecast.toml` in the UI with a live preview —
  theme, language, voice, browser voice, rate, tour, glossary, lock, fonts (upload a font file into
  `codecast/fonts/`), a colour picker per theme variable (folded under *Custom colours*) — then save it next to the script
  (`POST /api/config`, `POST /api/font/NAME`, loopback same-origin only) or download it.
- Codecasts in other languages, translated when played: a language menu in the player (and
  Settings, or `?lang=ja`) translates the script, generated tours, questions and glossary answers
  with an offline Argos model behind `iwr serve` (`POST /api/translate/LANG`, cached in
  `~/.cache/iwr/translate`), and the browser speaks it with a voice of that language. Code stays as
  written (protected by backticks during translation). No translated files next to the codecast;
  `iwr translate codecast/ --lang ja` prints the translation, `iwr export --lang ja` bakes it into a
  static site.
- `iwr voice codecast/ --lang ja` (also fr, es, it, pt): records each part translated, with a
  Kokoro voice of that language (Japanese phonemes by misaki), into `NN-part.ja.ogg` +
  `NN-part.ja.times` next to the English ones. The player plays them when the codecast is played
  in that language; they go stale with the English text.
- Tours read `//!` docs written as pages (`#` headings, tables, lists) in full: the side panel shows
  the doc, then each section while it is read (table rows spoken as "cell: header value"). Applies
  to `architecture_tour` (crate doc in the intro, then each module), `files_tour` and the built-in
  tour. Speech reads paths and `<placeholders>` (`gw/data/<node>` → "gw slash data slash node").
- `iwr voice codecast/`: records each part file with Kokoro TTS (Python sidecar `kokoro-onnx`, set up
  on first use) into `NN-part.ogg` (Opus, ~18 kbps, meant to be committed) + `NN-part.times`; the
  loader feeds the times to the cues as a `times:` line, so recording mode follows the script
  without `[t]` edits. Per-cue clip cache. A part whose text changed since its recording is left
  out by the loader and spoken by browser TTS in recording mode (`serve` / `iwr check` name it).
- Codecasts: `spell: IP, CoAP, HTTP` (script header or glossary) reads words letter by letter.
- Codecasts: a **side panel** next to the view, opened by `|` lines (sticky inside a part, `|` alone
  closes it): `| bytes:Frame` shows a ```` ```bytes Frame ```` block, `| md:Steps` a ```` ```md ````
  block, `| diagram:2` a mermaid block drawn small. `!` refs glow its fields and nodes beside code refs.
- ```` ```bytes ```` blocks: byte layouts (`id size [format] description`, number types with
  endianness, sizes from earlier fields, `*` for the rest) with an optional `sample:` decoded into
  each field's value; drawn as a coloured byte strip plus a field table (`iwr_core::bytes`).
- Markdown in answers and notes: numbered and nested lists, tables, `###` headings.
- Common initials (`ID`, `IP`, `URL`, `API`, `JSON`, `HTTP`, `SQL`, `CPU`… `speech::BUILTIN_SPELL`) are
  spelled out by default, plurals included; a script's `spell:` or `pronounce:` line still wins.

### Changed

- Architecture view hides test modules (`#[cfg(test)] mod tests`, files under `tests/`) unless
  Settings → *tests* is on; the built-in tour skips them too.

### Fixed

- Graph arrows readable on big graphs: boxes are opaque (arrows no longer show through their text),
  long arrows cross each layer straight instead of cutting corners through boxes, ends spread
  along a box side instead of piling into its centre, fewer crossings (best ordering kept, then
  neighbour swaps), and edge labels are drawn above the boxes at a spot on their arrow that
  covers no box, group title or other label.
- Diagram arrows between groups go around boxes and other groups (straight runs, rounded
  corners) instead of cutting straight through them.
- `pronounce:` lines in a script or glossary now win over the built-in pronunciations, as documented.

## [0.1.0] - 2026-10-07

First release.

### Added

- **Analysis** (`iwr-core`): parses a crate or workspace into a model of modules, items, calls,
  control flow, types and errors. UI-free, also compiles to wasm.
- **Eight views**: code blocks next to the source, call tree, control flow, architecture,
  branch tree, structure, types & traits, error flow. Hover for details, click to open,
  expand to inline, pan/zoom/fit.
- **Codecasts**: a narrated tour on top of the graphs. Each cue switches view, glows nodes,
  highlights source lines and tokens. Built-in tour when no script is given.
  - Markdown scripts: one `codecast.md` or a `codecast/` directory with one file per part.
  - Voices: browser text-to-speech, recorded audio per part, or silent reading.
  - Questions from the script and a built-in Rust glossary; clicking one pauses and answers.
  - Mermaid diagrams drawn by the player, with glow, grouping and reveal steps.
  - Plan page (`@ plan`) listing every part; both tours open on it.
- **CLI** (`iwr`): `serve` (live reload), `export` (static site), `brief` (text brief for writing
  a codecast), `check` (validate refs), `speak`, `analyze` (raw JSON).
- **UI**: settings page, eight themes, resizable panels, logo and favicon.
- **Docs and examples**: writing guide, demo project with an eleven-part codecast, and a
  ten-part codecast of this repository (`scripts/iwr self`).

### Fixed

- Arrow heads on side-attached edges in left-to-right diagrams now point along the line.
- A live reload during a codecast keeps the camera; the server ignores file-access events.
- Text-to-speech stops with a clear message when the browser has no working voice.

[0.1.0]: https://github.com/AlexandreKarlsson/IwannatRust/releases/tag/v0.1.0
