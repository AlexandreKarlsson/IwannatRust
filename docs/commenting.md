# Commenting code so it looks good in IwannatRust

IwannatRust draws your code: boxes for modules and types, a flow chart for each function, block
headings in the Code view. **Your comments become the titles of those boxes.** A short comment
in the right place turns `fn next_task` into "Next task by weight", and a whole loop into "Find
the next biggest task". If a comment is missing, or doesn't follow the rules below, the box shows
the raw name or code.

Nothing here is IwannatRust syntax. These are ordinary Rust comments, and they read just as well
in your editor, on GitHub and in `cargo doc`.

## The short version

| Comment | Where | Becomes | Rule for a title |
|---------|-------|---------|------------------|
| `//!` | first lines of a file | the **module's box** in Architecture (name and file go underneath) | first line ≤ 60 characters, followed by a blank `//!` line |
| `///` | above a struct / enum / trait | the **type's box** in Types ("struct Task" goes underneath) | same rule |
| `///` | above a fn / method | the item's **heading** in the Code view, the tooltip, the details panel | same rule |
| `//` | the line just above a statement, `let`, `if`, `match` or loop | the **node's label** in Flow, the **block heading** in Code | one line, ≤ 60 characters, no comment line above it |
| `//` | above an `if` that `break`s or `return`s out of a loop | the label of that **exit arrow** | same rule |
| `// TODO:` `FIXME` `XXX` `HACK` | anywhere | a stop in `todo_tour` | the text after the marker is read |

What happens when a comment breaks the rule: a title that's too long or spans several lines isn't
an error. The box just keeps the code's name, and the text still appears in the tooltip, the
details panel and the spoken tours.

## `//!`: what a module is for

```rust
//! Pick tasks in priority order.
//!
//! The scheduler keeps no state of its own besides a counter: every call asks
//! the storage for the remaining tasks and returns the heaviest one.
```

- **Line 1 is the title.** In the Architecture view, the box says *Pick tasks in priority order*,
  with `scheduler.rs` in small underneath. The trailing period is dropped.
- **Line 2 must be an empty `//!`.** If line 2 carries on the sentence, line 1 is no longer a
  title and the box shows the module name.
- **Write what the module is *for*, not what it contains.** "Pick tasks in priority order" beats
  "Scheduler struct and impl".
- **The first paragraph is spoken** by `architecture_tour` and the built-in tour (`g`). Keep it
  to two or three sentences you'd say out loud.

### Long module docs: write a page

If a `//!` doc has `#` headings, a `| table |` or `- lists`, it's treated as a **page**. During a
tour, the side panel shows the whole page, then each heading's section is shown and read in turn
(at most 8 table rows are read; code blocks are only shown).

```rust
//! Storage backends: where tasks live between runs.
//!
//! Every backend implements [`Storage`]; the scheduler never knows which one it has.
//!
//! # Backends
//!
//! | Backend | Keeps tasks | Survives a restart |
//! |---------|-------------|--------------------|
//! | `Memory` | in a `Vec` | no |
//! | `File` | one line per task | yes |
//!
//! # Adding one
//!
//! Implement `save`, `list` and `remove`; nothing else changes.
```

Use a page for the crate root (`main.rs` / `lib.rs`) and for the two or three modules a newcomer
must understand. Leave the others at one title line plus one paragraph.

## `///`: what a type or function is for

```rust
/// One unit of work, with a priority and a cost.
pub struct Task {
    /// shown in the schedule
    pub name: String,
    /// 1 (low) to 9 (urgent)
    pub priority: u8,
}

/// Next task by weight, or None when the storage is empty.
pub fn next_task(&mut self) -> Option<Task> {
```

- **Types view:** the box title is *One unit of work, with a priority and a cost*, with
  `struct Task` underneath. Field and variant docs appear when the box is expanded and in the
  details panel.
- **Code view:** the item's heading is *Next task by weight, or None when the storage is empty*;
  hover it to see the signature.
- **Tooltip, details panel, brief, `types_tour`, `entry_tour`:** the whole doc.
- Same title rule as `//!`: a short first line, then a blank `///` line before any details.

## `//`: the steps inside a function

Flow charts are where comments matter most. Without them, every node shows a line of code. With
them, the chart reads like a recipe.

```rust
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

In `flow:run_all` this gives:
- a big `run_all` title at the top,
- a node *Read what is left* for the first paragraph (both `let`s),
- a loop node *Work through the queue*,
- an exit arrow labelled *Out of budget*,
- a node *Run it and pay for it* for the three statements under it.

The Code view shows the same words as block headings, with the code tucked underneath.

### The rules, and why

| Rule | Why |
|------|-----|
| **Directly above** the statement, no blank line in between | that's how the comment is tied to the code |
| **One line.** The line above must not be a comment too | a multi-line `//` block is an explanation, so it stays in the code instead of becoming a title |
| **60 characters or fewer** | it has to fit in a box |
| **One comment per paragraph** | it titles every plain statement down to the next blank line, so leave a blank line where a new step starts |
| **Intent, not code**: `// Find the next biggest value`, not `// loop over v` | the box already shows the code on hover |
| Above the `if`, not inside it | a comment inside the branch titles the branch's own body |

Longer explanations are still welcome: write them as a multi-line `//` block (or put a blank line
between them and the code). They stay in the source pane and don't become a title.

```rust
// The budget is checked before running, not after: a task that would
// overspend is skipped entirely rather than cut short.
if t.cost > budget {        // ← no title here: the comment above is two lines
```

```rust
// Out of budget
if t.cost > budget {        // ← title "Out of budget" on the exit arrow
```

### Logging and small things

- Logging macros (`log::info!`, `debug!`, …) are hidden in Flow unless a `//` comment titles
  them. Title the ones that matter to the story.
- `else if` chains: put the comment on its own line above the `if`, not after `} else`.

## Writing code inside comments

Put code in backticks: `` `parse_all` ``, `` `Task::weight` ``, `` `&tasks` ``. In the UI it is
drawn as code, and when spoken it is read as words ("parse all", "a reference to tasks"). Bare
code-looking words like `snake_case` names are recognised too, but backticks are safer. If a word
is read wrongly, fix it in the codecast with a `pronounce: written = spoken` line.

## Checklist

1. Every file starts with a `//!` title line, then a blank `//!`, then a short paragraph.
   `iwr brief <path>` marks the modules that have none with `(no //! doc)`.
2. Every public type and function has a `///` title line.
3. Every function longer than a screen has a `//` title for each paragraph, and for each early
   `break` / `return`.
4. Titles: one line, ≤ 60 characters, saying what the code is *for*.
5. The crate root, and one or two key modules, have a page doc (headings, a table).
6. Look at it: `iwr serve <path>`, then open Architecture, Types, and Flow for your main function.
   A box showing a raw name instead of your sentence means its comment broke a rule above.

## For AI agents

The rules above, condensed for an agent, are an Agent Skill:
[skills/iwr-commenting/SKILL.md](../skills/iwr-commenting/SKILL.md). To install it, copy the
folder into your agent's skills folder (see the [README](../README.md#skill-for-ai-agents)).
Keep it in step with this page when a rule changes.

See also: [writing a codecast](writing-codecasts.md) (comments are its "Step 0"),
[the codecast format](codecast.md) (tours that read these comments).
