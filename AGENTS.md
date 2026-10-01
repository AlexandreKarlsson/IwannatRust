# Notes for agents working in this repository

- Project map: `docs/architecture.md`. Model: `docs/model.md`. Commands: `docs/cli.md`.
- **To write a codecast for a Rust project:** read `docs/writing-codecasts.md`, run
  `iwr brief <path>` (or `iwr brief <path> --guide` to get the guide and the brief in one output),
  write the script (markdown: one `codecast.md`, or a `codecast/` directory with `index.md` + one file per part),
  run `iwr check codecast.md --path <path>` (or `iwr check codecast/ --path <path>`), read `iwr speak codecast/`
  to hear-check the TTS text, play it with `iwr serve <path>`.
- Build with `scripts/iwr build`; `scripts/iwr` lists everything else. Tests: `scripts/iwr test`.
- Core must stay UI-free and compile with `--no-default-features` (it runs in the browser as wasm).
