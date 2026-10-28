# Notes for agents working in this repository

- Project map: `docs/architecture.md`. Model: `docs/model.md`. Commands: `docs/cli.md`.
- **To write a codecast for a Rust project:** read `docs/writing-codecasts.md`, run
  `iwr brief <path>` (or `iwr brief <path> --guide` to get the guide and the brief in one output),
  add missing `//!` / `///` / short `//` comments if you may edit the code (rules: `docs/commenting.md`),
  write the script (markdown: one `codecast.md`, or a `codecast/` directory with `index.md` + one file per part),
  run `iwr check codecast.md --path <path>` (or `iwr check codecast/ --path <path>`), read `iwr speak codecast/`
  to hear-check the TTS text, play it with `iwr serve <path>`.
- **Comment the Rust code you write** so it reads well in the UI: `skills/iwr-commenting/SKILL.md`
  (summary of `docs/commenting.md`, with how to check the result). It ships to users as a Claude
  Code plugin (`.claude-plugin/`); keep it in step with `docs/commenting.md` and generic (no
  `scripts/`, no repo-relative links).
- Build with `scripts/iwr build`; `scripts/iwr` lists everything else. Tests: `scripts/iwr test`.
- Core must stay UI-free and compile with `--no-default-features` (it runs in the browser as wasm).
