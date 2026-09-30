# Notes for agents working in this repository

- Project map: `docs/architecture.md`. Model: `docs/model.md`. Commands: `docs/cli.md`.
- **To write a codecast for a Rust project:** read `docs/writing-codecasts.md`, run
  `iwr brief <path>` (or `iwr brief <path> --guide` to get the guide and the brief in one output),
  write the script, run `iwr check script.txt --path <path>`, play it with `iwr serve <path> --script script.txt`.
- Build with `scripts/iwr build`; `scripts/iwr` lists everything else. Tests: `scripts/iwr test`.
- Core must stay UI-free and compile with `--no-default-features` (it runs in the browser as wasm).
