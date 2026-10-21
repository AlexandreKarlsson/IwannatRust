# User guide

```sh
iwr serve path/to/project      # opens http://127.0.0.1:4321
```

## Layout

| Area | Content |
|------|---------|
| Top bar | `Code` (default) · `Views ▸` reveals the analysis views as icons (hover for the name and what it shows) · split icon (source next to the blocks) · search · 📣 codecast · 📍 full tour · ⚙ settings |
| Sidebar (`☰` toggles, open by default) | Two tabs. **📁 files** (default, like an editor's explorer): the whole repository tree, Rust files with their item count, other text files viewable in the source pane; the file being shown is highlighted, files a codecast cue points at glow; click a file to open it in the Code view (item count on the right). **☰ items**: every item grouped by module. Click = select + show source. Double-click a function = open its control flow. Click a **module header** = scope the module-level views to that subtree (chip in the canvas clears it). |
| Center | Code view or graph canvas |
| Right | Details + source (graph modes) or the source pane (Code view) |

Panel edges are draggable (sidebar, details, blocks/source split); sizes are remembered. During codecast the camera, highlights and source scroll animate between cues; any manual pan or zoom switches back to instant.

## Code view (default)

The file as nested colored blocks. Nothing is expanded at start.

- **Item block** (fn/struct/enum/trait/impl/const). Click to open. Functions show their body; impls/traits show their methods; structs/enums show fields/variants as chips.
- **Body blocks**: `let`, `call`, `?`, `panic`, `return`, `break`/`continue`, `await`, `macro`, plain statements; compound blocks `loop`, `if`/`else if`/`else`, `match` → `arm`, `unsafe`, `closure`. Click a compound block to open its children. `▸ n` = number of hidden children.
- **Variable chips**: `＋x` = this block introduces `x`; `x` = this block reads `x`. Variables appear only where used.
- **Call chips** `→ name`: jump to that function (switches file, opens it).
- `flow` / `calls` on a function block: open its control flow / call tree.
- **Source ½**: source on the right half. Hover a block → its lines highlight. Hover a line → the innermost block highlights (its collapsed ancestor if closed). Click a block → source scrolls to it.
- File selector at the top when the project has several files.

## Analysis views (`Views ▸`)

| View | Root | What to look at |
|------|------|-----------------|
| Call tree | selected fn (default `main`) | Edge color = how the call happens: plain, in loop (thick cyan), conditional (yellow), recursion (orange dashed), `?` (rose dashed), unwrap (red dotted). `depth` control. ⊕ expands. |
| Control flow | selected fn | Statement-level graph. Green/red = true/false, orange = match arm, cyan = loop body / next iteration, rose dashed = error exit, red = return. Entry pill top (function name, big), exit pill bottom. A `//` comment of ≤ 60 chars on the line above a statement replaces its code as the label (code on hover). Loops start folded into one stacked node (⊕ unfolds); its exit edges say which `if` / comment leads out. Colours: new variable, state change, I/O, call, return / return Err, program stops (`process::exit`). Logging hidden unless commented. |
| Architecture | scope module | One box per module, labelled by its short `//!` line (file underneath). Layered by dependency: users above, what they use below. One faint arrow per module pair (thicker = more uses, two heads = mutual), full strength when an end is hovered / selected / glowing. Hubs (used by ≥ 4 modules, views of ≥ 6) get a coloured initials chip; their users carry the chip instead of an arrow; hovering a chip shows the hub's arrows. A module tree with no arrow inside (generated code) folds into its top box, "+N inside"; an empty workspace root is hidden. ⊕ shows items. `external` toggle adds crates. |
| Branch tree | selected fn | Every path as a tree; loops end in `↻ back to` leaves. ⊕ on a call inlines the callee. |
| Structure | scope module | Files/modules as columns of blocks; impls/traits expand to methods. |
| Types & traits | scope module | `contains` (green), `implements` (purple dashed, trait above), supertrait, alias. ⊕ on a type shows functions using it. |
| Error flow | scope module | Fallible fns + error enums. `? propagates` (rose dashed), `unwrap → panic` (red), `handled` (green), `Err` → error type, `From` conversions. |

Nodes show only their name; the signature, fields or badges appear in a small panel under the node when it is hovered, selected or narrated.

Every node: **hover** = tooltip (signature, doc, params, return, facts, callers/callees, location; statement source for flow nodes). **Click** = details + source highlight. **Double-click** = expand, or open a function's control flow. Drag = pan, wheel = zoom.

What the graph views include (external calls, macros, constructors, tests) is set on the settings page; with tests off, the Architecture view also hides test modules (`#[cfg(test)] mod tests`, files under `tests/`); the call-tree depth is also in the canvas toolbar.

## Codecast and the full tour

📣 *codecast* (or `g`) opens the player with the loaded script, or with the built-in tour of the
project. 📍 *full tour* (or `t`) plays it from the first cue and goes on to the next part when one
ends, until the last one; with the *full tour* box unticked (player bar or settings) playing stops
at the end of each part and `next part ▶` continues.

The built-in tour is told like a visit: a welcome with the project's goal (its crate doc), the
architecture module by module, the data types one by one, the call tree, then one part per function.
Each function is walked statement by statement in reading order, in plain sentences ("First, the
question mark: `parse_all` can fail, and if it does, `run` stops right here…"), and the tour stays
in that function until it is fully explained; the functions it calls come next, each once.
`codecast from here` in the details panel starts the same tour at any function.

Every cue switches view, glows the relevant boxes, highlights source lines and can underline a code
range. Voice: browser TTS (rate in settings), a recording, or silent reading (seconds per cue in
settings).

Scripts can also be written by an AI or by hand and loaded with `iwr serve --script`, or `load…` in
the player bar. See [codecast.md](codecast.md) for the format, the brief and the player.

## Keyboard

| Key | Action |
|-----|--------|
| `←` `→` | previous / next cue |
| `space` | play / pause |
| `g` | start / stop the codecast |
| `t` | start the full tour |
| `f` | fit graph to view |
| `,` | open / close the settings page |
| `Esc` | close the settings page, or the player |

## Settings

⚙ (or `,`) opens the settings page. A side menu (it follows the scroll) leads to: **Appearance** —
32 themes as small previews of the interface (filter Dark / Light) and a gallery of 31 bundled
fonts, drawn in themselves, for the box titles and for the interface text; **Codecast** — voice,
language, speech rate, reading time, full tour, spoken code, glossary questions; **Codecast
design** (authors, below); **Analysis views**; **Code view**; **Layout**; **Keyboard**.
Everything is remembered in the browser.

A codecast may ship its own look and defaults (`codecast.toml`: theme, colours, fonts, voice,
language). They are applied when you first open it, or when its author changes them; what you
change afterwards is kept. A locked codecast greys out the theme, font, voice and language controls. Authors design that file in
the *Codecast design* section: *Create the codecast configuration* opens the editor; pick the theme, fonts (upload a font file for the box
titles), colours, voice and language, see them at once, then *save codecast.toml* (with `iwr
serve`) or download it.

## Generate the voice

With `iwr serve` and a `codecast/` directory, **generate** in the top bar (its badge counts the
parts without an up-to-date English recording) opens a page that records the codecast with Kokoro,
like `iwr voice`: pick the language (other languages are translated from the English parts), a
voice (▶ plays a sample), the speed, the pause between cues and the compression, then the parts
(by default the ones missing or changed since their recording). A progress bar follows every cue
with the time left; *Cancel* keeps the parts already written. The recording runs in the server, so
you can leave the page (the top-bar button shows a ring meanwhile); when it ends the player loads
the new recordings. The first run installs Kokoro (~90 MB) under `~/.cache/iwr/kokoro`.
`?page=generate` (or `?page=settings`) in the URL opens the page directly.

## Live reload

`iwr serve` watches `.rs` files and `Cargo.toml`; the UI reloads the model within ~2 s of a save.
