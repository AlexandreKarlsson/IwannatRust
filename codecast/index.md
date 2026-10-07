# How IwannatRust works

// A tour of the tool by the tool: the two analysis passes, the model, views and layout,
// the codecast format, diagrams, the command line, and the player. Project terms: glossary.md.
plan: The tool explained by itself, from source files to a spoken tour.
pronounce: syn = sin
pronounce: clap = clap
pronounce: Sugiyama = soo gee yah mah
pronounce: barycenter = barry center
pronounce: stderr = standard error
pronounce: serde = sir-day

- [Welcome](01-welcome.md) — four crates, who depends on whom, and where this tour goes
- [The pipeline](02-the-pipeline.md) — from source files to a Project, and from the Project to pictures and to a codecast
- [Pass one: parsing](03-pass-one-parsing.md) — syn turns every file into items and modules; bodies wait for pass two
- [Pass two: linking](04-pass-two-linking.md) — impls to types, fields to types, calls to functions, then the CFG and the block tree of each body
- [The model](05-the-model.md) — Project, Item, FnInfo, Cfg, Block and Span: the plain data everything else reads
- [Views and layout](06-views-and-layout.md) — one builder per view, and the Sugiyama layout that positions every box, in five steps
- [The codecast format](07-the-codecast-format.md) — how a markdown script becomes parts and cues, and how check verifies every ref
- [Diagrams](08-diagrams.md) — a mermaid block the player draws, moves and reveals; a feature explaining itself
- [The command line and the server](09-the-command-line.md) — brief, check, serve: what the iwr binary does and how the file watcher works
- [The player](10-the-player.md) — apply_cue and apply_directives: the contract between a script and the screen
