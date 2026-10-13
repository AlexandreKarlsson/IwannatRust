# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

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
