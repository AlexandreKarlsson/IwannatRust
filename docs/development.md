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
./build.sh                                   # dx build --release (UI) + cargo build --release -p iwr
./target/release/iwr serve examples/demo
```

Core-only iteration (no wasm): `cargo run -p iwr -- summary examples/demo` prints items, calls, relations, block trees, view sizes and guide steps.

UI type-check without `dx` noise: `cargo check -p iwr-ui --target wasm32-unknown-unknown`.

`build.sh` wipes `target/dx/iwr-ui/release/web/public` first; otherwise old hashed bundles accumulate and get embedded/exported.

## Tests

```sh
cargo test -p iwr-core          # crates/iwr-core/tests/analyze.rs: parsing, resolution, CFG, blocks, views, guide, module paths
cargo clippy --workspace --exclude iwr-ui
```

Views test asserts no dangling edges, every edge routed, no overlapping siblings — run it after touching `layout`.

## Visual QA

Headless Chrome + `playwright-core` against a running `iwr serve` is the fastest loop: click tabs/blocks, hover, screenshot each mode, capture console errors. Selectors: `.node` (svg nodes), `.iblk`/`.blk` (code view blocks), `.sidebar .mod` (module headers), `button:has-text('…')`.

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
