# Architecture

```
crates/iwr-core   analysis → model → views/layout → guide        (no UI deps; compiles to wasm)
crates/iwr-ui     Dioxus 0.7 web app → wasm (built by `dx`)
crates/iwr        CLI: axum server, rust-embed of the UI bundle, notify watcher, export
examples/demo     sample crate used in docs/tests
```

## Pipeline (iwr-core)

1. `loader` — walk the directory, read `.rs` files, entry points first.
2. `parse` — `syn::parse_file` per file. File path → module path (`src/a/b.rs` → `crate::a::b`, `crates/foo/src/…` → `foo::…`, multi-crate roots hang under a virtual `crate` root). Collects `Item`s (fn, method, struct, enum, trait, impl, alias, const, static), docs, attrs, signatures (token stream re-spaced by `text::clean_tokens`), spans (proc-macro2 `span-locations`). Function bodies kept for pass 2.
3. `resolve` — links impls to types/traits; field/signature/alias type relations; per body a `CallVisitor` resolves calls by name (same module first, `self.x()` → owner type, generic bounds + field types as hints, trait decl when receiver is `T: Trait`), tags each call `in_loop`/`in_branch`/`propagated`/`unwrapped`/`awaited`/`recursive`, counts stats; then builds the `Cfg` (`cfg`) and the `Block` tree (`blocks`) of the body.
4. `views::build(project, mode, opts)` → `Graph { nodes, edges }` with positions. One builder per mode; `ViewOptions` carries root, module scope, expanded/collapsed node ids, toggles.
5. `layout` — `layered` (Sugiyama-style: cycle removal, longest-path ranks, layer wrapping at `max_per_layer`, dummy nodes for long edges, barycenter ordering, relaxation, component packing into rows) and `tree` (tidy tree). Returns rects + edge polylines.
6. `guide::build` → `Vec<GuideStep>`: the built-in tour. Welcome + one step per module (arch), one per type (types), the call tree, then one chapter per function: its CFG in reading order, told in plain sentences with connectors ("First, … Then … Finally,"). Callees are queued and visited afterwards, breadth first, each once (depth ≤ 6), so the camera never bounces between functions. `GuideStep::part` names the chapter and `refs` carries explicit script refs; `script::from_guide` turns the steps into a `Script`.
7. `script` — codecast format (`parse` / `to_text`, incl. `?` questions on cues and ```` ```mermaid ```` diagram blocks per part), ref resolver (`resolve`: item, block, module, file:line:cols → span), `check` (tracks the sticky `@` view so `!` refs are checked against the diagram's node ids while one is shown). `diagram` — a mermaid-flowchart subset (`parse`: shapes, arrows with labels, `&` sets, nested `subgraph`, front-matter title; tolerant, errors collected) and its layout (`graph`: groups laid out recursively with `layout::layered`, members offset inside their group box, cross-group edges drawn straight) into the same `Graph` the UI draws for the other views; `Mode::Diagram` is the view, node ids are the refs. `glossary` — glossaries as questions: the **built-in Rust glossary** (`crates/iwr-core/glossary.md`, `include_str!`) and project ones (`glossary.md` next to a codecast), `parse` / `merge` / `mentioned` (whole-word match on a cue's text) / `questions_for`. `speech` — how code is spoken: `segments` (text / code with its spoken form, backtick spans + bare code-looking words), `say_code` (small tokenizer: paths, generics, references, closures, operators → words), `spoken` (TTS text), `pronounce:` overrides. `brief` — compact text brief for LLMs.

Analysis is syntactic; no type checking. See `resolve_method` for the heuristics and their order.

## UI (iwr-ui)

- `main.rs` — `State` (all `Signal`s, provided via context), fetch + 1.5 s version polling (static mode when `/api/version` is unreachable → `project.json`), keyboard via `document::eval` channel, theme persistence.
- `code.rs` — Code view: `ItemBlock` / `BlockView` render `FnInfo.blocks`; `hover_span` drives two-way highlighting with the source pane.
- `canvas.rs` — SVG canvas: pan/zoom transform, `Node`, `Edge` (bezier / spline through dummy points / orthogonal back edges, manual arrowheads), `Tooltip`, legend, guide highlighting (`focus_nodes`).
- `panels.rs` — top bar (icon buttons with hover labels), sidebar, details + source. `settings.rs` — settings page (theme, view toggles, depth, voice/rate/reading time, full tour, layout, keys) and its `localStorage` persistence (`iwr-settings` JSON). `icons.rs` — PNG icons embedded with `include_bytes!` and served as data URIs (work from `serve` and from a static export under any sub-path); `assets/icons/*.png` are 48 px black silhouettes recoloured by the `--icon-filter` CSS variable of each theme. `player.rs` — codecast player: parts, cue application (`apply_directives`: view/root/scope, `hl` refs, code mark, camera), `advance()` (next cue, or next part when the full tour is on), TTS via `speechSynthesis` through `document::eval`, recording sync via an `<audio>` element's `timeupdate`, script/audio/glossary loading, questions (`questions()` = script `?` + glossary matches not yet asked; `ask()` pauses and applies the answer's directives, `finish_answer()` restores the cue and resumes). `md.rs` — markdown subset for guide text; code tokens rendered with their spoken form (tooltip, or parentheses when the setting is on). `theme.rs` — colors per node/edge kind, legends, `THEMES`, CSS (themes = CSS variables on `.theme-*`).

Graph is recomputed by `use_memo` from `State::build_graph()` whenever mode/root/scope/expansion/toggles change; layout runs in the browser (wasm), so expanding is instant.

## CLI (iwr)

`codecast.rs`: finds the codecast next to a project (`codecast/` dir, `codecast.md`, legacy `codecast.txt`) and reads it; a directory is assembled into one script text by `iwr_core::script::assemble` (part order from `index.md`, sibling `NN-part.mp3` recordings become per-part `audio:` lines). `/api/codecast/NAME` serves those recordings; `/api/glossary` the project glossary (`glossary.md` next to the script).

`serve.rs`: `analyze()` → `Arc<Project>` in `RwLock`; `notify` watcher debounced 250 ms triggers re-analysis and bumps `/api/version`. Assets come from `target/dx/iwr-ui/release/web/public` via `rust-embed` (`build.rs` creates the folder so the crate compiles before the UI is built). `export()` copies assets, rewrites `"/./assets/` → `"./assets/` in html/js so the site works from any sub-path, and writes `project.json`.
