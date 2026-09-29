# Embedding

## Static export

```sh
iwr export path/to/project -o site/
```

`site/` = `index.html` + `assets/*.js|wasm` + `project.json`. Relative paths only, so it works from any static host and any sub-path (`/docs/viz/`). The UI detects the missing `/api/version` and loads `project.json` once (no live reload).

Embed in a page:

```html
<iframe src="viz/index.html" style="width:100%;height:80vh;border:0"></iframe>
```

Regenerate on each docs build (fast: ~100 ms for 300 items) — e.g. a `build.rs`, a Makefile target, or a CI step before the site generator runs.

## Deep links (planned)

URL hash state (`#mode=control_flow&root=<item path>&step=<n>`) is not implemented yet; today the export always opens on the Code view of the first file.

## Zensical plugin (planned)

Only `iwr-core` is needed: it exposes `analyze_path`, `views::build`, `guide::build` and serializable types. A plugin can either

1. shell out to `iwr export` and copy the folder into the site, or
2. link `iwr-core` and render its own front end from `Project` / `Graph`.

Nothing in the core references the CLI, axum or Dioxus.

## Using the core as a library

```toml
iwr-core = { path = "crates/iwr-core" }                          # parse + fs
iwr-core = { path = "...", default-features = false }            # model + views + layout + guide only (wasm-friendly)
```

```rust
let p = iwr_core::analyze_path(Path::new("."))?;
let g = iwr_core::views::build(&p, Mode::ErrorFlow, &ViewOptions::default());
serde_json::to_writer(std::io::stdout(), &g)?;
```
