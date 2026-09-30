# Writing a codecast (for agents and humans)

You are writing a **codecast**: a short spoken tour of a Rust project that the IwannatRust player
performs on top of its views — switching diagrams, glowing the block being talked about, highlighting
and underlining code. Input: the **brief** (`iwr brief <path>`). Output: a **script** in the text
format below. Validate with `iwr check script.txt --path <path>`.

## Format

```
# <Title>

## <Part name>               a part = one subject, 4–10 cues, playable alone
@ <view>[:<ref>]             what to show (sticky until the next @)
! <ref> [<ref> …]            what to glow (sticky; `!` alone clears)
= <file>:<line>:<c1>-<c2>    what to underline in the code (this cue only)
<One or two spoken sentences.>
```

Views: `code` `calls` `flow` `arch` `branches` `structure` `types` `errors`.
Refs: `crate::run` (item) · `crate::run/b5` (block 5 of `run`, ids as in the brief) · `crate::storage`
(module) · `src/parser.rs` (file: `@ code:src/parser.rs` opens it) · `src/main.rs:49-52` (lines) ·
`src/main.rs:49:17-44` (columns). `crate::` may be omitted
when the name is unique. `//` starts a comment line.

## How to structure it

1. **Parts are subjects**, not files: *Architecture*, *How a request flows*, *Error handling*,
   *The scheduler*. 2–5 parts. The first part is the big picture, later parts go deeper.
2. **One idea per cue.** One or two sentences, ≤ 30 words each. The listener also sees the caption.
3. **Show, then glow.** Start each part with `@` on the view that fits the subject, then move the
   `!` highlight cue by cue. Do not change `@` on every cue: staying on one diagram while the glow
   moves is what makes it feel like a tour.
4. **Point at code when it matters.** Use `= file:line:c1-c2` for the exact token you are talking
   about (the `?`, the `.await`, a condition). Column numbers are 1-based, `c2` is inclusive.
5. **Say what it does and why**, not what it is called. "The scheduler picks the heaviest task
   each round" beats "`next_task` returns an `Option<Task>`".
6. **Name the mechanism** when it is a Rust idiom the reader may not know: `?` propagation, trait
   dispatch, ownership moves, recursion base case. One sentence, in place.
7. **Every ref must exist.** Use ids from the brief, never invented line numbers. Prefer block refs
   (`fn/bN`) over line refs; they survive edits.
8. **End each part with a one-sentence takeaway.**

## Which view for which subject

| Subject | `@` | Good `!` targets |
|---------|-----|------------------|
| big picture, modules, dependencies | `arch` | module refs |
| data model, traits, who implements what | `types[:module]` | struct / enum / trait refs |
| what one function does, step by step | `flow:fn` | `fn/bN` blocks |
| the same, reading like code | `code:fn` | `fn/bN` blocks (parents open automatically) |
| who calls whom from an entry point | `calls:fn` | fn refs |
| all the paths through a function | `branches:fn` | `fn/bN` blocks |
| where errors come from and go | `errors[:module]` | fn and error-enum refs |
| files and impl blocks at a glance | `structure[:module]` | item refs |
| a tour of the files ("where things live") | `code:<file>` per cue | item refs in that file |

## Example

```
# How the demo task runner works

## Architecture
@ arch
The demo is four modules around a tiny entry point: parser, scheduler, storage and model.
! crate::parser
The parser turns text lines into tasks.
! crate::storage
Storage keeps them, behind a trait so the backend can change.

## Running a task list
@ flow:crate::run
! crate::run/b1
= src/main.rs:49:17-44
run starts by parsing the input. The question mark means: if parsing fails, stop here and hand the error to main.
! crate::run/b5 crate::run/b6
Every parsed task is saved into storage, one per loop iteration.
```

## Checklist before you hand it in

- `iwr check` prints "all refs resolve".
- Each part has a title a listener would pick from a menu.
- No cue longer than two sentences; no part longer than ten cues.
- The first cue of every part sets `@`.
- At least one `=` per part that walks through code.
