# <img src="images/logo.png" width="40" align="top"> IwannatRust

**Make unfamiliar Rust code visually understandable.**

IwannatRust parses a Rust project and turns it into interactive, colour-coded
schematics: call trees, control-flow graphs, module architecture, branch trees,
block structure, type/trait relationships and error flow. Hover anything to see
what the code says about it, click to jump to the source, expand nodes to go
deeper, or let the **full tour** walk you through the whole program, from a welcome
to the last function.

Everything is Rust: the analyzer (`syn`), the layout engine and the web UI
([Dioxus](https://dioxuslabs.com) → WebAssembly). No JavaScript dependencies.

```
iwr serve path/to/project     # analyze + open the interactive UI (live reload on edits)
iwr export path/to/project    # static site (index.html + project.json), embeddable in docs
iwr brief  path/to/project    # compact text brief for writing a codecast (LLM-friendly)
iwr analyze path/to/project   # raw model as JSON
```

## Code view (default)

The landing page shows the file **as simple nested blocks**: every function,
loop, branch, match arm, call, `?`, return or panic is a coloured block. All
complexity is hidden until you click: functions open to their body, loops and
branches open to their children. Variables appear only where they are used, as
small chips (`＋name` where a variable is introduced, `name` where it is read),
and calls to project functions are clickable chips that jump to the callee.

`Source ½` splits the page: blocks on the left, real source on the right.
Hovering a block highlights its lines; hovering a line highlights its block.
Every function block has `flow` / `calls` shortcuts into the analysis views,
which are otherwise tucked away behind **Views ▸**.

The analysis views are icons in the top bar (hover one for its name). Eight themes
(Dark, Light, Paper, Midnight, Forest, Nord, Solar, Dusk), the view toggles, the voice
and the layout live on the **settings page** (⚙ or `,`); everything is remembered.

## Analysis views

| Mode | What it shows |
|------|---------------|
| **Code** | The file as nested blocks (see above). |
| **Call tree** | Who calls whom from the entry point. Edges are coloured by *how* the call happens: inside a loop, conditionally, recursively, propagated with `?`, or `unwrap`ped. Expand nodes progressively. |
| **Control flow** | Statement-level flow of one function: `if`/`match` branches, loops with back edges, early returns, `?` error exits, panics, awaits. |
| **Architecture** | Modules (and crates in a workspace), their contents, and weighted dependency edges between them. |
| **Branch tree** | Every possible path through a function as a tree; calls can be expanded inline to walk into the callee. |
| **Structure** | Files/modules as blocks with items, impl blocks and their methods nested inside. |
| **Types & traits** | Structs, enums, traits, aliases: `contains`, `implements`, supertraits, and (on expand) functions using a type. |
| **Error flow** | `Result`/`Option` functions, error enums, `From` conversions, and whether each call propagates (`?`), handles, or panics. |

Interactions, in every mode:

- **Hover** a node → signature, doc comment, parameters, return type, facts (loops, branches, `?` count…), callers/callees, location.
- **Click** a node → details panel + source with the relevant lines highlighted.
- **⊕ / double-click** → expand or collapse; double-click a function to open its control flow.
- **Sidebar** lists every item; click a module header to *scope* the module-level views to that subtree (useful on big projects).
- Pan by dragging, zoom with the wheel, `f` to fit.
- Settings page (⚙): show external calls, macros, constructors, tests; call-tree depth
  (also in the call-tree toolbar).

### Codecast

📣 *codecast* (or `g`) plays a walkthrough on top of the views, and 📍 *full tour* (or `t`)
plays every part of it in a row. The built-in tour is told like a visit: a welcome with the
project's goal, the architecture module by module, the data types, the call tree, then one
part per function, walked statement by statement and staying there until it is fully
explained ("First, the question mark: parse_all can fail, and if it does, run stops right
here…"). Each cue switches to the right view, glows the relevant boxes, highlights source
lines, and is spoken by the browser (or played from a recording, or read silently).
Scripts are split into parts you can also play alone.

Scripts are plain text and cheap to write, by hand or by an AI:

```sh
iwr brief examples/demo            # compact, LLM-friendly description with block ids
iwr check codecast.txt --path examples/demo
iwr serve examples/demo --script codecast.txt [--audio voice.mp3]
```

```
## Running a task list
@ flow:crate::run
! crate::run/b1
= src/main.rs:49:17-44
run starts by parsing the input. If that fails, the ? hands the error to main.
```

See [docs/codecast.md](docs/codecast.md).

## Building

Requirements: Rust (stable), the `wasm32-unknown-unknown` target and the Dioxus CLI.

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.10 --locked
scripts/iwr build               # builds the web UI, then the CLI with the UI embedded
scripts/iwr serve examples/demo
scripts/iwr                     # menu of everything: build, serve, codecast, export, brief, guide, check, …
```

`iwr` embeds the compiled UI, so the release binary is self-contained. The
`examples/demo` crate is a small task runner that exercises calls, branches,
loops, recursion, traits and error propagation — a good first thing to open.

Full documentation: [docs/](docs/README.md) — user guide, CLI, architecture, model reference, embedding, development.

## Layout of this repository

```
crates/iwr-core   analysis + graph model + layout + guide (no UI dependencies)
crates/iwr-ui     Dioxus web app (compiled to WASM by `dx`)
crates/iwr        CLI: serve / export / analyze / summary
examples/demo     sample project
```

### iwr-core

The core is independent of any UI or documentation platform and can be reused
(it also compiles to WASM; disable default features to drop `syn`/filesystem):

```rust
let project = iwr_core::analyze_path(Path::new("."))?;          // -> Project (serde)
let graph   = iwr_core::views::build(&project, Mode::CallTree, &ViewOptions::default());
let steps   = iwr_core::guide::build(&project, None, &GuideOptions::default());
```

Pipeline: `parse` (syn → items, modules) → `resolve` (calls, type relations,
stats, per-function CFG via `cfg` and block tree via `blocks`) → `views`
(per-mode graph, positioned by `layout`) → `guide`.

Analysis is purely syntactic (no type checker). Calls are resolved by name with
heuristics (same module first, `self.` methods to the owner type, generic bounds
and field types as hints, trait declarations when the receiver is a `T: Trait`).
This is right most of the time and always fast; ambiguous method names may
resolve to a sibling implementation.

## Embedding in documentation (Zensical etc.)

`iwr export -o site/` writes a static folder (`index.html`, `assets/`,
`project.json`) that works from any static host or sub-path — embed it in an
`<iframe>` or serve it next to your docs. A dedicated Zensical plugin can build
on the same core later; nothing in `iwr-core` depends on the CLI or the UI.

## License

MIT
