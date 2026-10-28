---
name: iwr-commenting
description: Comment Rust (.rs) code so IwannatRust (`codecast`) shows comments as box titles and flow labels. Use when writing or commenting Rust in an iwr project (has codecast/) or before writing a codecast. Not for other languages.
---

# Commenting Rust for IwannatRust

Comments become box titles. A comment breaking a rule silently falls back to the raw name/code.
Scope: `.rs` files only. Never apply to Python, JS, C, etc., even in the same repo.
When commenting existing code, touch comments only: no code changes, keep existing meanings.

Full guide: `docs/commenting.md` in https://github.com/AlexandreKarlsson/IwannatRust

| Comment | Where | Becomes |
|---------|-------|---------|
| `//!` | top of every file | module box; first paragraph spoken by tours |
| `///` | every pub type, fn, method | type box, Code heading, tooltip |
| `///` | fields, variants | expanded box |
| `//` | line above a statement, `let`, `if`, `match`, loop | flow node, Code block heading |
| `//` | above an `if` that `break`s/`return`s from a loop | exit-arrow label |
| `// TODO:` etc. | anywhere | TODO tour stop |

Title rules:
- `//!` `///`: line 1 ≤ 60 chars, then a blank `//!`/`///` line, then details.
- `//`: one line, ≤ 60 chars, directly above the code; not preceded by another comment line.
  A multi-line `//` block is an explanation, not a title.
- One `//` titles all statements down to the next blank line: blank line between steps.
- Say intent, not mechanics: `// Find the biggest task`, not `// loop over v`.
- Comment above the `if`, not inside the branch; for `else if`, own line above the `if`.
- Code names in backticks. Title only logging that matters (untitled logs are hidden).
- Crate root and 1–2 key modules: page doc (`#` headings, table). Others: title + one paragraph.
- Short obvious fns need only `///`. Don't title every line.

```rust
//! Pick tasks in priority order.
//!
//! Every call asks the storage for the remaining tasks and returns the heaviest.

/// Run every task, heaviest first, until the budget runs out.
pub fn run_all(mut budget: u32) {
    // Work through the queue
    while let Some(t) = next_task() {
        // Out of budget
        if t.cost > budget {
            break;
        }
        // Run it and pay for it
        t.run();
        budget -= t.cost;
    }
}
```

Check (`scripts/iwr` inside an iwr checkout):
- `codecast brief <path> --no-body`: items show a quoted title; `(no //! doc)` marks missing module docs.
- `codecast summary <path> | grep '^step'`: titled steps show your text; `Loop`/`Call …` means the title was lost.
