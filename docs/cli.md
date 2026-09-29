# CLI

```
iwr serve   [PATH] [-p PORT] [--no-open]   analyze + serve UI (default PATH ".", PORT 4321)
iwr export  [PATH] [-o DIR]                static site: index.html, assets/, project.json (default DIR "iwr-site")
iwr analyze [PATH] [-o FILE] [--pretty]    model as JSON (stdout when -o omitted)
iwr summary [PATH]                         text dump: modules, items, calls, relations, block trees, view sizes, guide steps
```

`PATH` = directory with `Cargo.toml` (name taken from it), any directory of `.rs` files, or a single `.rs` file. Skipped dirs: `target`, `.git`, `node_modules`, `.dx`, `dist`.

HTTP API of `serve`:

| Route | Response |
|-------|----------|
| `GET /api/project` | `Project` JSON (see [model.md](model.md)) |
| `GET /api/version` | integer, increments on every re-analysis |
| anything else | embedded UI (SPA fallback to `index.html`) |

The binary embeds the UI built by `dx`; if it was built without it, `/` returns 503 with instructions.
