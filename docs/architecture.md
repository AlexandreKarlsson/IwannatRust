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
6. `guide::build` → `Vec<GuideStep>`: DFS over CFGs from `main`, diving into callees (depth ≤ 6, each fn walked once). `script::from_guide` turns it into a `Script`.
7. `script` — narration format (`parse` / `to_text`), ref resolver (`resolve`: item, block, module, file:line:cols → span), `check`. `brief` — compact text brief for LLMs.

Analysis is syntactic; no type checking. See `resolve_method` for the heuristics and their order.

## UI (iwr-ui)

- `main.rs` — `State` (all `Signal`s, provided via context), fetch + 1.5 s version polling (static mode when `/api/version` is unreachable → `project.json`), keyboard via `document::eval` channel, theme persistence.
- `code.rs` — Code view: `ItemBlock` / `BlockView` render `FnInfo.blocks`; `hover_span` drives two-way highlighting with the source pane.
- `canvas.rs` — SVG canvas: pan/zoom transform, `Node`, `Edge` (bezier / spline through dummy points / orthogonal back edges, manual arrowheads), `Tooltip`, legend, guide highlighting (`focus_nodes`).
- `panels.rs` — top bar, sidebar, details + source. `player.rs` — narration player: parts, cue application (view/root/scope, `hl` refs, code mark, camera), TTS via `speechSynthesis` through `document::eval`, recording sync via an `<audio>` element's `timeupdate`, script/audio loading. `md.rs` — markdown subset for guide text. `theme.rs` — colors per node/edge kind, legends, CSS (themes = CSS variables on `.app.theme-*`).

Graph is recomputed by `use_memo` from `State::build_graph()` whenever mode/root/scope/expansion/toggles change; layout runs in the browser (wasm), so expanding is instant.

## CLI (iwr)

`serve.rs`: `analyze()` → `Arc<Project>` in `RwLock`; `notify` watcher debounced 250 ms triggers re-analysis and bumps `/api/version`. Assets come from `target/dx/iwr-ui/release/web/public` via `rust-embed` (`build.rs` creates the folder so the crate compiles before the UI is built). `export()` copies assets, rewrites `"/./assets/` → `"./assets/` in html/js so the site works from any sub-path, and writes `project.json`.
