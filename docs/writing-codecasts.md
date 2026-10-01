# Writing a codecast (for agents and humans)

You are writing a **codecast**: a short spoken tour of a Rust project that the IwannatRust player
performs on top of its views — switching diagrams, glowing the block being talked about, highlighting
and underlining code. Input: the **brief** (`iwr brief <path>`). Output: a **script** in the markdown
format below, as one `codecast.md` or a `codecast/` directory with `index.md` (title + part list) and
one `NN-part-name.md` file per part (plus an optional `glossary.md` for project terms). Validate with
`iwr check codecast.md --path <path>` (or `iwr check codecast/ --path <path>`), read it back with
`iwr speak`.

## Format

```
# <Title>

## <Part name>               a part = one subject, 4–10 cues, playable alone
@ <view>[:<ref>]             what to show (sticky until the next @)
! <ref> [<ref> …]            what to glow (sticky; `!` alone clears)
= <file>:<line>:<c1>-<c2>    what to underline in the code (this cue only)
<One or two spoken sentences.>
? <A question the listener may have>      optional, after the cue it belongs to
  <The answer, indented, two to four sentences; may start with its own @ ! = lines>
```

Views: `code` `calls` `flow` `arch` `branches` `structure` `types` `errors`.
Refs: `crate::run` (item) · `crate::run/b5` (block 5 of `run`, ids as in the brief) · `crate::storage`
(module) · `src/parser.rs` (file: `@ code:src/parser.rs` opens it) · `src/main.rs:49-52` (lines) ·
`src/main.rs:49:17-44` (columns). `crate::` may be omitted
when the name is unique. `//` starts a comment line.

## How to structure it

1. **Write it as a tour, not a list of facts.** The whole script is played in a row (*full tour*),
   so it should read like one person showing a visitor around: a *Welcome* part that says what the
   project is for and where we will go, then parts that follow from each other, and a one-sentence
   wrap-up at the very end.
2. **Parts are subjects**, not files: *Welcome*, *How a request flows*, *Where errors go*,
   *The scheduler*. 4–12 parts. The first part is the big picture, later parts go deeper; for a
   listener new to Rust, a part on the language mechanism the code shows best (ownership, traits).
3. **Stay on one view per part and say everything there is to say.** It is better to spend ten cues
   on one function, moving only the glow, than to jump between views every sentence. Change `@` only
   when the subject really moves (e.g. from the control flow of a function to its code to read a
   loop). The listener keeps their bearings when the picture stays still.
4. **Link the parts.** End each part with a sentence that hands over to the next one ("Next, let's
   see what executing a task means."), and begin the next with a sentence that picks it up ("`execute`
   is the smallest function with the most interesting shape, because it calls itself."). The
   listener should never wonder why the view just changed.
5. **One idea per cue.** One or two sentences, ≤ 30 words each. The listener also sees the caption.
   Use natural connectors between cues: *First, … Then … Notice … Otherwise … Finally …*
6. **Point at code when it matters.** Use `= file:line:c1-c2` for the exact token you are talking
   about (the `?`, the `.await`, a condition). Column numbers are 1-based, `c2` is inclusive.
7. **Say what it does and why**, not what it is called. "The scheduler picks the heaviest task
   each round" beats "`next_task` returns an `Option<Task>`".
8. **Name the mechanism** when it is a Rust idiom the reader may not know: `?` propagation, trait
   dispatch, ownership moves, recursion base case. One sentence, in place.
9. **Every ref must exist.** Use ids from the brief, never invented line numbers. Prefer block refs
   (`fn/bN`) over line refs; they survive edits.
10. **Explain the Rust, don't assume it.** The listener may be new to Rust. Generic vocabulary
    (crate, impl, trait, `?`, ownership, closure…) is covered by the player's built-in glossary:
    when a cue says the word, a "What is a …?" question appears by itself, so *use the real words*
    rather than paraphrasing them. Add a `?` question of your own when the answer is specific to
    this project ("Why does deploy never run?", "Could we avoid the clone?"), one or two per part,
    with an answer of two to four sentences that can be read aloud. Put project vocabulary that
    recurs (a domain term, a constant) in `codecast/glossary.md` instead of repeating it.
11. **Write code as code; the player says it in words.** `parse_all`, `Task::weight`, `&tasks`,
    `Option<Task>` are spoken as "parse all", "Task weight", "a reference to tasks", "Option of
    Task", and the listener sees the real token in the caption. So don't spell things out for the
    voice ("parse underscore all"); write the identifier. Backticks are optional for single names,
    useful for expressions (`` `for t in &tasks` ``). Check the result with `iwr speak`, and fix a word
    the voice gets wrong with `pronounce: written = spoken` in the header.
12. **Be a good host, not a textbook.** A light touch of humour tied to the code (what the input
    actually does, a task that never runs, the compiler as the bouncer) keeps a listener awake. Keep
    it short, keep it kind, and make sure it survives a robot voice: no puns that need spelling.

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
# A tour of the demo task runner

## Welcome
@ arch
Welcome to the demo task runner. It reads a list of tasks from text, stores them, and runs them in priority order.
We start with the big picture: every box is a module, and the arrows show who depends on whom.
! crate::parser
The parser turns text lines into tasks. It is the first thing the program touches, and the first place things can go wrong.
! crate::storage
Storage keeps them, behind a trait so the backend can change.
!
That is the whole map. Now let's follow the program as it runs.

## How a run works
@ flow:crate::run
Everything starts in main, which only calls run. So run is where the story is, and we will stay here for a while.
! crate::run/b1
= src/main.rs:49:41-41
First, run hands the input to the parser. Notice the question mark: if parsing fails, run stops right here and gives the error to main.
? What is run's return type?
  = src/main.rs:48:24-46
  A Result of usize and AppError: Ok with the number of tasks that ran, or Err with what went wrong. Rust has no exceptions, so a function that can fail says so in its signature.
! crate::run/b5 crate::run/b6
= src/main.rs:55:22-30
Then every parsed task is saved into storage, one per loop iteration. `t.clone()` makes a copy, because the storage wants to own its task and the loop only borrowed it.
```

## Checklist before you hand it in

- `iwr check` prints "all refs resolve".
- Directory form: `index.md` has the `# Title` and a `- [Part](NN-file.md)` list in playing order; each
  part file starts with its `## Part name`.
- The first part welcomes the listener and says what the project is for; the last cue wraps up.
- Each part has a title a listener would pick from a menu, and ends by handing over to the next.
- No cue longer than two sentences; no part longer than twelve cues; `@` changes at most two or three times per part.
- Every `?` has an indented answer (`iwr check` lists the ones that don't); one or two per part, project-specific.
- `iwr speak` reads well: no "underscore", no "colon colon"; odd words have a `pronounce:` line.
- The first cue of every part sets `@`.
- At least one `=` per part that walks through code.
