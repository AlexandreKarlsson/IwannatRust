# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

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
