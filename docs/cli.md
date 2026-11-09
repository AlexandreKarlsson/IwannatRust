# CLI

```
codecast serve   [PATH] [-p PORT] [--no-open] [--script S] [--audio A]   analyze + serve UI (default PATH ".", PORT 4321); <PATH>/codecast/, codecast.md or codecast.txt is loaded when present
codecast export  [PATH] [-o DIR] [--script S] [--audio A] [--lang ja,fr]   static site: index.html, assets/, project.json (+ codecast.md, codecast/*.mp3, codecast.json + its fonts, audio, translations/<lang>.json)
codecast analyze [PATH] [-o FILE] [--pretty]    model as JSON (stdout when -o omitted)
codecast summary [PATH]                         text dump: modules, items, calls, relations, block trees, view sizes, guide steps
codecast brief   [PATH] [--fn P] [--no-body] [--no-overview]   compact text brief for writing a codecast (see codecast.md)
codecast guide   [PATH] [--fn P] [--lang en|ja]  built-in walkthrough as a codecast script (text) on stdout, told in that language (doc comments stay as written)
codecast speak   SCRIPT                         print a codecast as it will be spoken (code said as words), to review the TTS text
codecast voice   DIR [--voice V] [--voice2 V] [--speed X] [--lang L] [--path P] [--gap S] [--level Q] [--force] [--retranslate] [--fresh] [--written]   record a codecast/ directory with Kokoro TTS: NN-part_<lang>_<voice>.ogg (Opus; the voice's `_` written `-`: 01-welcome_en_am-michael.ogg) + .times per part and voice (each voice keeps its own files; the player picks one; unchanged parts and cues are skipped); with `sp:` lines, --voice2 says them (default: the other gender; the same voice higher when the language has one) and files are named NN-part_<lang>_<voice>+<voice2>.ogg; --lang ja|fr|es|it|pt translates each part first and writes NN-part_<lang>_<voice>.ogg + .times; older dotted names (NN-part.<lang>.<voice>.ogg) are renamed on load; --retranslate skips the translation cache, --fresh the cue clip cache; --written records parts already written in --lang as they are (no translation), e.g. the Japanese tutorial
codecast translate SCRIPT --lang L [--path PATH] [--pieces]   translate a codecast offline (`translations/<lang>.json` next to it first, then the Argos model, cached in ~/.cache/codecast/translate) and print it: what the player shows in that language; --pieces prints the pieces as JSON instead (no model) to translate with another tool and save as `translations/<lang>.json` (the offline model is rough: see codecast.md § Translating with another tool)
codecast video   DIR [--voice V] [--lang L] [--sub S|none] [--parts 1,4] [--fps 30] [--size 4k|1440p|1080p|720p|WxH] [--scale H/720] [--crf 18] [--tail S] [-o DIR] [--force] [--chrome BIN] [--path P]   film each recorded part as NN-part_<lang>.mp4 (H.264 + AAC) next to its recording: the player's render mode (the stage alone, no controls, theme and fonts of codecast.toml) in headless Chromium on virtual time, frame by frame, so picture and voice stay in step whatever the machine; 4K by default, the page laid out at 720 px high whatever the size (--scale changes that); size, fps, scale, crf and sub default to `[video]` of codecast.toml; --lang ja films the Japanese recordings with the translated captions (the translation must exist: `translations/<lang>.json` or the cache, mounted into Docker read-only); no text under the picture unless --sub: --sub en adds a subtitle band in that language (NN-part_<lang>_sub-en.mp4; --sub none turns off a `[video] sub`); skips parts without an up-to-date recording and videos newer than their recording and text (--force); given --voice, first records the parts that voice is missing or has a stale recording of (as `codecast voice` would, on this machine; `--voice am_michael` also finds its pair `am_michael+af_heart`), so one command records and films; films with chromium-headless-shell + ffmpeg on PATH (Linux), else in Docker by itself (image codecast-video, built from docker/video/Dockerfile of the repository the binary was built in, cached after the first time; the project, the codecast, -o and the translation cache are mounted)
codecast check   SCRIPT [--path PATH]           validate a script's refs (file or codecast/ directory) its glossary.md and codecast.toml; exit 1 if any is unresolved, a question has no answer, or a recording is missing (notes parts whose recording is stale: TTS speaks them)
```

`PATH` = directory with `Cargo.toml` (name taken from it), any directory of `.rs` files, or a single `.rs` file. Skipped dirs: `target`, `.git`, `node_modules`, `.dx`, `dist`.

HTTP API of `serve`:

| Route | Response |
|-------|----------|
| `GET /api/project` | `Project` JSON (see [model.md](model.md)) |
| `GET /api/version` | integer, increments on every re-analysis; `0` while the first analysis runs (`/api/project` answers 503 then) |
| `GET /api/status` | `Status` JSON: how far the running (or last) analysis is (`stage`, `done` / `total`, the file or function it is on, counts of files / items / calls, `elapsed_ms`, `error`); the loading page polls it |
| `GET /api/script` | the `--script` file as text, a directory assembled into one (re-read each request), 404 if none |
| `GET /api/audio` | the `--audio` file, 404 if none |
| `GET /api/codecast/NAME` | a file of the script's directory: a recording named by an `audio:` line, or a font of `codecast.toml` (no path escapes) |
| `POST /api/translate/LANG` | JSON list of English texts → JSON list of translations (the codecast's `translations/<lang>.json` first, then the offline model, cached; the first call for a language downloads its model), 503 if it cannot |
| `POST /api/subtitles/LANG` | JSON list of English texts → JSON list of the translations that already exist (`translations/<lang>.json`, then the cache), `null` for the others; never runs the model (subtitles) |
| `GET /api/glossary` | the project glossary (`glossary.md` next to the script), re-read on every request |
| `GET /api/config` | the codecast's `codecast.toml` as JSON, re-read on every request; its font files are served by `/api/codecast/PATH` |
| `POST /api/config` | JSON `Config` → written as `codecast.toml` next to the script (400 with the problems if a value is bad, 409 without a codecast); same-origin loopback only |
| `POST /api/font/NAME` | raw font bytes (ttf/otf/woff/woff2, ≤ 10 MB) → `fonts/NAME` in the codecast directory; answers that path; same-origin loopback only |
| `GET /api/voice` | `VoiceStatus` JSON: every part file of a `codecast/` directory, its cue count and its recordings per language (`recorded`, `stale`, `own`, with voice, duration, size); `problem` when the codecast is a single file or missing; `ready` once Kokoro is installed |
| `POST /api/voice` | JSON `VoiceRequest` (voice, voice2, speed, lang, gap, level, force, parts, retranslate, fresh) → starts `codecast voice` in the background (202; 409 if one runs); same-origin loopback only |
| `GET /api/voice/job` | `VoiceJob` JSON: the running (or last) recording's stage, parts with cues done, elapsed time, error, log |
| `POST /api/voice/cancel` | stops the running recording (finished parts are kept); same-origin loopback only |
| `POST /api/voice/refresh` | JSON `{groups: ["en/am_adam", …]}` (empty: all) → records the out-of-date recordings again with the voice and delivery they were made with, then deletes the files of parts that are gone (a job, like `POST /api/voice`); same-origin loopback only |
| `POST /api/voice/clean` | same JSON → deletes those out-of-date recordings and the files of parts that are gone; answers how many files went; same-origin loopback only |
| `POST /api/voice/preview` | JSON `{voice, speed, lang}` → an Ogg Opus sample of that voice (cached); same-origin loopback only |
| anything else | embedded UI (SPA fallback to `index.html`) |

The binary embeds the UI built by `dx`; if it was built without it, `/` returns 503 with instructions.
