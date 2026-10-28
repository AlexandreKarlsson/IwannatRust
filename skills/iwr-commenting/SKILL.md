---
name: iwr-commenting
description: Comment Rust code so IwannatRust (iwr) turns the comments into box titles, flow-chart labels and block headings. Use when writing or editing Rust code in a project shown with IwannatRust (a `codecast/` or `codecast.md` is present, or the user mentions iwr / IwannatRust / codecasts), when asked to comment or document Rust code for it, and as Step 0 before writing a codecast.
---

# Commenting Rust code for IwannatRust

[IwannatRust](https://github.com/AlexandreKarlsson/IwannatRust) draws Rust code as boxes and flow charts,
and **comments become the titles**. A comment that breaks a rule is not an error: the box
silently falls back to the raw name or code. So follow the rules exactly.

These are ordinary Rust comments: they read just as well in an editor, on GitHub and in
`cargo doc`. Full guide with rationale: `docs/commenting.md` in the IwannatRust repository.

## Rules

| Comment | Where | Becomes | Title rule |
|---------|-------|---------|------------|
| `//!` | top of every file | module box (Architecture), spoken by tours | line 1 ≤ 60 chars, then a blank `//!`, then 2–3 spoken sentences |
| `///` | above every pub type / fn / method | type box (Types), heading (Code), tooltip | line 1 ≤ 60 chars, then a blank `///` before details |
| `///` | above fields / variants | expanded box, details panel | short phrase |
| `//` | line directly above a statement, `let`, `if`, `match`, loop | flow node label, Code block heading | **one** line, ≤ 60 chars, no comment line above it, no blank line below it |
| `//` | above an `if` that `break`s / `return`s out of a loop | exit-arrow label | same |
| `// TODO:` `FIXME` `XXX` `HACK` | anywhere | a stop in the TODO tour | text after the marker is read |

- One `//` titles every plain statement down to the next blank line. Leave a blank line where a
  new step starts, and give each step its own title.
- Say what the code is *for*, not what it does mechanically: `// Find the next biggest task`,
  not `// loop over tasks`. Module titles say purpose, not contents.
- Put the `//` above the `if`, not inside the branch (inside titles the branch body). For
  `else if`, put it on its own line above the `if`, not after `} else`.
- Explanations longer than a title: write a multi-line `//` block, or leave a blank line between
  it and the code. It then stays in the source and does not become a title.
- Code names inside comments go in backticks: `` `parse_all` ``. They are drawn as code and
  spoken as words.
- Logging macros are hidden in Flow unless titled; title only the ones that matter.
- Crate root (`main.rs` / `lib.rs`) and one or two key modules get a page doc: `#` headings, a
  table or a list in the `//!`. Other modules: title + one paragraph.
- Match the density of the surrounding code: functions under a screen with obvious steps need
  only the `///` title. Don't title every line.

## Example

```rust
//! Pick tasks in priority order.
//!
//! The scheduler keeps no state besides a counter: every call asks the
//! storage for the remaining tasks and returns the heaviest one.

/// Run every task, heaviest first, until the budget runs out.
pub fn run_all(tasks: &[Task], mut budget: u32) -> Result<usize, Error> {
    // Read what is left
    let mut left = storage.list()?;
    let mut done = 0;

    // Work through the queue
    while let Some(t) = scheduler.next_task() {
        // Out of budget
        if t.cost > budget {
            break;
        }
        // Run it and pay for it
        t.run()?;
        budget -= t.cost;
        done += 1;
    }
    Ok(done)
}
```

## Check your work

If `iwr` is installed (inside an IwannatRust checkout, use `scripts/iwr`):

1. `iwr brief <path> --no-body`: every module and public item should show a quoted title.
   Modules without one are marked `(no //! doc)`; items without one have no quote.
2. `iwr summary <path> | grep '^step'`: each `//` title shows as a step label
   (`step 30 [Statement] Start counting`). A step you titled that still reads `Loop`,
   `Call ...` or similar lost its title: its comment broke a rule above.
3. For a visual check: `iwr serve <path>`, then Architecture, Types and Flow.

Without `iwr`, re-read each comment you wrote against the table above.
