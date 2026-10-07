# A tour of the demo task runner

// A tiny Rust program explained from the top: what a crate is, how a run works, where errors go.
// Questions under the caption pause the tour, answer, and carry on. Project terms: glossary.md.
plan: A tiny task runner, read from the top: the data, one run, the Rust ideas it shows, and where errors go.

- [Welcome](01-welcome.md) — what the program is for, and its four modules on one map
- [What is a crate anyway](02-what-is-a-crate.md) — the word that comes up all the time, and the files it names
- [The data](03-the-data.md) — Task, Priority and the Describe trait: the types everything else passes around
- [How a run works](04-how-a-run-works.md) — run, statement by statement: parse, store, schedule, execute, and three ways out
- [Ownership in sixty seconds](05-ownership.md) — borrowing, cloning and moving, drawn first and then read in run
- [Executing a task](06-executing-a-task.md) — a function that calls itself, and how recursion stops
- [Parsing a line](07-parsing-a-line.md) — text in, task or error out: the question mark and the error enum
- [Scheduling](08-scheduling.md) — the heaviest task first, with an iterator chain and an Option
- [Traits, or how to swap the database](09-traits.md) — the Storage trait and why the scheduler never sees a HashMap
- [Where errors go](10-where-errors-go.md) — every error path from the parser and the storage up to main
- [Where things live](11-where-things-live.md) — a last look at the files, and who declares what
