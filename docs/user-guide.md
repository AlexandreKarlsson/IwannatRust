# User guide

```sh
iwr serve path/to/project      # opens http://127.0.0.1:4321
```

## Layout

| Area | Content |
|------|---------|
| Top bar | `Code` (default) · `Views ▸` reveals analysis tabs · `Source ½` split · theme · search · `▶ Codecast` |
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
| Control flow | selected fn | Statement-level graph. Green/red = true/false, orange = match arm, cyan = loop body / next iteration, rose dashed = error exit, red = return. Entry pill top, exit pill bottom. |
| Architecture | scope module | Modules with fn/type counts; `contains` edges; grey `n uses` dependency arcs (dimmed unless a module is hovered). ⊕ shows items. `external` toggle adds crates. |
| Branch tree | selected fn | Every path as a tree; loops end in `↻ back to` leaves. ⊕ on a call inlines the callee. |
| Structure | scope module | Files/modules as columns of blocks; impls/traits expand to methods. |
| Types & traits | scope module | `contains` (green), `implements` (purple dashed, trait above), supertrait, alias. ⊕ on a type shows functions using it. |
| Error flow | scope module | Fallible fns + error enums. `? propagates` (rose dashed), `unwrap → panic` (red), `handled` (green), `Err` → error type, `From` conversions. |

Nodes show only their name; the signature, fields or badges appear in a small panel under the node when it is hovered, selected or narrated.

Every node: **hover** = tooltip (signature, doc, params, return, facts, callers/callees, location; statement source for flow nodes). **Click** = details + source highlight. **Double-click** = expand, or open a function's control flow. Drag = pan, wheel = zoom.

Toggles (graph modes): external calls, macros, constructors, tests.

## Codecast

`▶ Codecast` or `g` plays the built-in walkthrough: overview → types → call tree → execution from `main()` (branches, loops, `?`, panics, recursion, trait dispatch), one part per top-level function. Every cue switches view, glows the relevant boxes, highlights source lines and can underline a code range. Voice: browser TTS, a recording, or silent reading. `codecast from here` in the details panel starts at any function.

Scripts can also be written by an AI or by hand and loaded with `iwr serve --script`, or `load…` in the player bar. See [codecast.md](codecast.md) for the format, the brief and the player.

## Keyboard

| Key | Action |
|-----|--------|
| `←` `→` | previous / next cue |
| `space` | play / pause |
| `g` | start / stop the built-in codecast |
| `f` | fit graph to view |
| `Esc` | close the player |

## Themes

Top-bar selector: **Dark**, **Light**, **Paper**. Remembered in the browser.

## Live reload

`iwr serve` watches `.rs` files and `Cargo.toml`; the UI reloads the model within ~2 s of a save.
