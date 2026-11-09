<div align="center">

<img src="images/hero.jpg" width="420" alt="IwannatRust logo: a crab behind a lattice">

# IwannatRust

**Understand any codebase in minutes, and give it a presentation that never goes out of date.**

*Rust today; [more languages](#supported-languages) on request.*

[![Rust](https://img.shields.io/badge/Rust-stable-f59e0b?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Dioxus](https://img.shields.io/badge/UI-Dioxus%200.7%20%E2%86%92%20WASM-60a5fa)](https://dioxuslabs.com)
[![No JS deps](https://img.shields.io/badge/JavaScript%20deps-none-34d399)](#how-it-is-built)
[![License: MIT](https://img.shields.io/badge/License-MIT-c084fc)](LICENSE)

</div>

## Why

Reading a project you did not write is slow. The docs are thin or out of date, and the person who
knows it is busy. IwannatRust reads the code and gives every project:

- 🗺️ **Drawings built from the code**: files, modules, calls, control flow, types, error paths.
  They are redrawn on every save, so they never go out of date.
- 🎙️ **A virtual presentation (a *codecast*)**: a guided talk that switches to the right view,
  lights up what it is talking about, underlines code, zooms in, and answers questions. It is
  spoken by a recorded voice or the browser, or you read it.
- 👥 **The same explanation for everyone**: newcomers, reviewers, a handover, someone learning
  Rust. The authors write it once, or have an AI write it from `codecast brief`, and `codecast check` keeps
  every reference in sync with the code.

A project without a codecast still gets one: the built-in tour, written from its code.

## Quick start

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.10 --locked
scripts/iwr build                  # web UI + CLI, one binary: target/release/codecast
scripts/iwr self                   # IwannatRust explaining itself, http://127.0.0.1:4321
```

On first start the app asks whether you have used it before. Answer *no* and the **integrated
tutorial** plays. It is itself a codecast, recorded and shipped with the app, and runs on the
project you opened (about five minutes). The **?** button in the top bar brings it back.

Example: [codecast/](codecast/), IwannatRust's own tour (`scripts/iwr self`).
Scripts put one directive per line. GitHub's markdown view joins those lines into paragraphs, so
read them raw (the *Code* button) or in the player.

<img src="images/shot-tour.png" alt="A codecast explaining the ? operator on the control-flow graph" width="100%">

**▶ Watch a part (46 s, with sound):** *The pipeline*, part 2 of IwannatRust's own tour, filmed
by `codecast video`.

https://github.com/user-attachments/assets/2b99506f-5d02-4b04-b611-26e007f69902

## Make a codecast for your project

You need the `codecast` binary (see *Quick start*) and an AI coding agent (Claude Code, Codex,
Cursor…). Three steps:

1. **Open your project in the app:** `codecast serve path/to/your/project`, then click
   **generate** in the top bar.
2. **Make the prompt:** the first card, *Write the script with an AI*, asks a few questions
   (subject, audience, humour, a metaphor, which pages to use…). Leave any of them empty and the
   AI decides. Click **Copy the prompt**.
3. **Paste it into your agent**, opened in your project folder. It reads your code and the writing
   guide by itself, shows you the plan, writes `codecast/` and checks it. The app reloads on its
   own: click **codecast** in the top bar to play it.

Optional: on the same page, record a natural voice for it (Kokoro, offline), then share it with
`codecast export path/to/your/project -o site/`.

## What is in it

| | |
|---|---|
| **Code view** | The file as nested blocks (one per item, one per step), next to the source; hover either side and both light up. |
| **Seven views** | Call tree, control flow, architecture, branch tree, structure, types & traits, error flow. |
| **Codecasts** | Markdown scripts: `@` view, `!` highlight (items, steps, files, folders), `=` underline, `+` enlarge a panel, light up a control or zoom, `?` questions, mermaid sketches, side notes, popups. |
| **Voice** | Kokoro recordings (`codecast voice`, or the *Generate* page), browser TTS, or silent reading; offline translation. |
| **Look** | 32 themes, 31 fonts; a `codecast.toml` sets the author's defaults. |
| **Interface** | English and 日本語: switch in the top bar. The built-in tour, generated tours and the tutorial are told natively in Japanese too ([docs/i18n.md](docs/i18n.md)). |
| **Sharing** | `codecast export` writes a static site that works from any host or sub-path. |

<img src="images/shot-code.png" alt="Code view: nested blocks next to the source" width="100%">

## Supported languages

| Language | Status |
|---|---|
| Rust | ✅ Supported |

Only Rust for now. More languages (Python, PHP, TypeScript, Go, ...) can be added.
If you want one, say so in an [issue](https://github.com/AlexandreKarlsson/IwannatRust/issues): languages people ask for come first.

## Commands

```
codecast serve  PATH          interactive UI, live reload on save
codecast export PATH -o DIR   static site
codecast brief  PATH          the code as compact text, for an AI (or you) to write a codecast
codecast check  codecast.md   every reference must resolve
codecast voice  codecast/     record the codecast with Kokoro
```

`scripts/iwr` with no argument lists everything. Full list: [docs/cli.md](docs/cli.md).

Keep only the script in git. Recordings and videos are generated and rebuilt from it, so ignore
them in your repository's `.gitignore`:

```gitignore
codecast/**/*.ogg
codecast/**/*.times
codecast/**/*.mp4
```

Publish the videos elsewhere (a release, GitHub Pages) if you want to share them.

## How it is built

All Rust, in one binary with no JavaScript dependencies: the analyzer (`syn`), the layout engine
(written here), the tour generator and the Dioxus → WebAssembly UI. The analysis is syntactic (no
type checker), so it is fast on any crate that parses and right most of the time.
`crates/iwr-core` compiles to wasm and can be used on its own ([docs/embedding.md](docs/embedding.md)).

## Documentation

[User guide](docs/user-guide.md) · [codecast format](docs/codecast.md) ·
[writing a codecast](docs/writing-codecasts.md) · [commenting your code](docs/commenting.md) ·
[CLI](docs/cli.md) · [architecture](docs/architecture.md) · [model](docs/model.md) ·
[development](docs/development.md)

## Skill for AI agents

[`skills/iwr-commenting`](skills/iwr-commenting/SKILL.md) teaches an agent to comment Rust code
so it reads well in IwannatRust. Copy it into your agent's skills folder, e.g. for Claude Code:

```sh
cp -r skills/iwr-commenting ~/.claude/skills/        # all projects
cp -r skills/iwr-commenting <project>/.claude/skills/ # one project
```

Or as a Claude Code plugin: `/plugin marketplace add AlexandreKarlsson/IwannatRust`, then
`/plugin install iwannatrust@iwannatrust`.

## License

MIT
