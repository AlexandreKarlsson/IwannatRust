# CLI

```
iwr serve   [PATH] [-p PORT] [--no-open] [--script S] [--audio A]   analyze + serve UI (default PATH ".", PORT 4321); <PATH>/codecast.txt is loaded when present
iwr export  [PATH] [-o DIR] [--script S] [--audio A]                 static site: index.html, assets/, project.json (+ script.txt, audio)
iwr analyze [PATH] [-o FILE] [--pretty]    model as JSON (stdout when -o omitted)
iwr summary [PATH]                         text dump: modules, items, calls, relations, block trees, view sizes, guide steps
iwr brief   [PATH] [--fn P] [--no-body] [--no-overview]   compact text brief for writing a codecast (see codecast.md)
iwr guide   [PATH] [--fn P]                built-in walkthrough as a codecast script (text) on stdout
iwr check   SCRIPT [--path PATH]           validate a script's refs; exit 1 if any is unresolved
```

`PATH` = directory with `Cargo.toml` (name taken from it), any directory of `.rs` files, or a single `.rs` file. Skipped dirs: `target`, `.git`, `node_modules`, `.dx`, `dist`.

HTTP API of `serve`:

| Route | Response |
|-------|----------|
| `GET /api/project` | `Project` JSON (see [model.md](model.md)) |
| `GET /api/version` | integer, increments on every re-analysis |
| `GET /api/script` | the `--script` file as text (re-read each request), 404 if none |
| `GET /api/audio` | the `--audio` file, 404 if none |
| anything else | embedded UI (SPA fallback to `index.html`) |

The binary embeds the UI built by `dx`; if it was built without it, `/` returns 503 with instructions.
