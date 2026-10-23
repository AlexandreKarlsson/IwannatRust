# CLI

```
iwr serve   [PATH] [-p PORT] [--no-open] [--script S] [--audio A]   analyze + serve UI (default PATH ".", PORT 4321); <PATH>/codecast/, codecast.md or codecast.txt is loaded when present
iwr export  [PATH] [-o DIR] [--script S] [--audio A] [--lang ja,fr]   static site: index.html, assets/, project.json (+ codecast.md, codecast/*.mp3, codecast.json + its fonts, audio, translations/<lang>.json)
iwr analyze [PATH] [-o FILE] [--pretty]    model as JSON (stdout when -o omitted)
iwr summary [PATH]                         text dump: modules, items, calls, relations, block trees, view sizes, guide steps
iwr brief   [PATH] [--fn P] [--no-body] [--no-overview]   compact text brief for writing a codecast (see codecast.md)
iwr guide   [PATH] [--fn P]                built-in walkthrough as a codecast script (text) on stdout
iwr speak   SCRIPT                         print a codecast as it will be spoken (code said as words), to review the TTS text
iwr voice   DIR [--voice V] [--speed X] [--lang L] [--path P] [--gap S] [--level Q] [--force] [--retranslate] [--fresh]   record a codecast/ directory with Kokoro TTS: NN-part.<voice>.ogg (Opus) + .times per part and voice (each voice keeps its own files; the player picks one; unchanged parts and cues are skipped); --lang ja|fr|es|it|pt translates each part first and writes NN-part.<lang>.ogg + .times; --retranslate skips the translation cache, --fresh the cue clip cache
iwr translate SCRIPT --lang L [--path PATH] [--pieces]   translate a codecast offline (`translations/<lang>.json` next to it first, then the Argos model, cached in ~/.cache/iwr/translate) and print it: what the player shows in that language; --pieces prints the pieces as JSON instead (no model) to translate with another tool and save as `translations/<lang>.json` (the offline model is rough: see codecast.md § Translating with another tool)
iwr check   SCRIPT [--path PATH]           validate a script's refs (file or codecast/ directory) its glossary.md and codecast.toml; exit 1 if any is unresolved, a question has no answer, or a recording is missing (notes parts whose recording is stale: TTS speaks them)
```

`PATH` = directory with `Cargo.toml` (name taken from it), any directory of `.rs` files, or a single `.rs` file. Skipped dirs: `target`, `.git`, `node_modules`, `.dx`, `dist`.

HTTP API of `serve`:

| Route | Response |
|-------|----------|
| `GET /api/project` | `Project` JSON (see [model.md](model.md)) |
| `GET /api/version` | integer, increments on every re-analysis |
| `GET /api/script` | the `--script` file as text, a directory assembled into one (re-read each request), 404 if none |
| `GET /api/audio` | the `--audio` file, 404 if none |
| `GET /api/codecast/NAME` | a file of the script's directory: a recording named by an `audio:` line, or a font of `codecast.toml` (no path escapes) |
| `POST /api/translate/LANG` | JSON list of English texts → JSON list of translations (the codecast's `translations/<lang>.json` first, then the offline model, cached; the first call for a language downloads its model), 503 if it cannot |
| `GET /api/glossary` | the project glossary (`glossary.md` next to the script), re-read on every request |
| `GET /api/config` | the codecast's `codecast.toml` as JSON, re-read on every request; its font files are served by `/api/codecast/PATH` |
| `POST /api/config` | JSON `Config` → written as `codecast.toml` next to the script (400 with the problems if a value is bad, 409 without a codecast); same-origin loopback only |
| `POST /api/font/NAME` | raw font bytes (ttf/otf/woff/woff2, ≤ 10 MB) → `fonts/NAME` in the codecast directory; answers that path; same-origin loopback only |
| `GET /api/voice` | `VoiceStatus` JSON: every part file of a `codecast/` directory, its cue count and its recordings per language (`recorded`, `stale`, `own`, with voice, duration, size); `problem` when the codecast is a single file or missing; `ready` once Kokoro is installed |
| `POST /api/voice` | JSON `VoiceRequest` (voice, speed, lang, gap, level, force, parts, retranslate, fresh) → starts `iwr voice` in the background (202; 409 if one runs); same-origin loopback only |
| `GET /api/voice/job` | `VoiceJob` JSON: the running (or last) recording's stage, parts with cues done, elapsed time, error, log |
| `POST /api/voice/cancel` | stops the running recording (finished parts are kept); same-origin loopback only |
| `POST /api/voice/refresh` | JSON `{groups: ["en/am_adam", …]}` (empty: all) → records the out-of-date recordings again with the voice and delivery they were made with, then deletes the files of parts that are gone (a job, like `POST /api/voice`); same-origin loopback only |
| `POST /api/voice/clean` | same JSON → deletes those out-of-date recordings and the files of parts that are gone; answers how many files went; same-origin loopback only |
| `POST /api/voice/preview` | JSON `{voice, speed, lang}` → an Ogg Opus sample of that voice (cached); same-origin loopback only |
| anything else | embedded UI (SPA fallback to `index.html`) |

The binary embeds the UI built by `dx`; if it was built without it, `/` returns 503 with instructions.
