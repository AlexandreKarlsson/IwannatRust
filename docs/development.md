# Development

## Toolchain

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.10 --locked     # provides `dx` (in ~/.cargo/bin)
```

## Launcher

`scripts/iwr` with no argument prints every command with an icon (and lets you pick one on a TTY).
`scripts/iwr build | serve | codecast | export | brief | guide | check | summary | test | shot | clean`.

## Build / run

```sh
scripts/iwr build                            # or ./build.sh: dx build --release (UI) + cargo build --release -p iwr
./target/release/iwr serve examples/demo
```

Core-only iteration (no wasm): `cargo run -p iwr -- summary examples/demo` prints items, calls, relations, block trees, view sizes and guide steps.

UI type-check without `dx` noise: `cargo check -p iwr-ui --target wasm32-unknown-unknown`.

`build.sh` wipes `target/dx/iwr-ui/release/web/public` first; otherwise old hashed bundles accumulate and get embedded/exported.

## Tests

```sh
scripts/iwr test                # tests + clippy + wasm check of the core
cargo test -p iwr-core          # crates/iwr-core/tests/analyze.rs: parsing, resolution, CFG, blocks, views, guide, scripts, the tutorial filled for several projects
cargo test -p iwr               # the tutorial's recording matches its text
```

Views test asserts no dangling edges, every edge routed, no overlapping siblings — run it after touching `layout`.

## The tutorial

`crates/iwr-ui/assets/tutorial/` is a codecast directory: part files whose directive lines use
placeholders (`{dir}`, `{file}`, `{fn}`, `{fn_line}`, `{fn_b1}`, filled by `iwr_core::tutorial::fill`;
spoken text never names the project) and their `af_heart` recordings. After editing a sentence:

```sh
./target/release/iwr voice crates/iwr-ui/assets/tutorial --path examples/demo
```

then rebuild (the recordings are UI assets). New parts go in `PARTS` (`crates/iwr-ui/src/tutorial.rs`).

## Visual QA

Headless Chrome against a running `iwr serve` is the fastest loop (`scripts/iwr shot`, or drive it
over the DevTools protocol: click, press keys, screenshot each cue, read classes). A fresh
`--user-data-dir` shows the first-start dialog. Selectors: `.node` (svg nodes), `.iblk`/`.blk` (code
view blocks), `.sidebar .mod` (module headers), `.main.focus-*` / `.spot-on` / `.main.zoomed`
(codecast panels, spots, zoom).

## Adding a view

1. `views::Mode` — add the variant, label, description, to `GRAPHS`.
2. `views::build` — match arm calling a new `fn my_view(p, opts) -> Graph`; use `mk_node` for items, `layered_layout` or `layout::tree`.
3. `theme::legend` — colors/labels; new `NodeKind`/`EdgeKind` need entries in `node_color`/`edge_style`.
4. Nothing else: tabs, tooltips, selection and guide highlighting are generic over `VNode`.

## Adding a block or CFG kind

`blocks.rs` / `cfg.rs` builders + the `BlockKind` / `CfgNodeKind` enums in `model.rs`; UI colors in `code.rs::block_color` / `theme.rs`.

## Conventions

- Core must stay free of UI/CLI dependencies and keep compiling with `--no-default-features`.
- All UI colors through CSS variables (`theme.rs`), node/edge palette through `node_color`/`edge_style`.
- Node ids in views must be deterministic (they key the expanded/collapsed sets).
