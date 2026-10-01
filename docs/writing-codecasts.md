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

1. **Write it as a tour, not a list of facts.** The whole script is played in a row (*full tour*),
   so it should read like one person showing a visitor around: a *Welcome* part that says what the
   project is for and where we will go, then parts that follow from each other, and a one-sentence
   wrap-up at the very end.
2. **Parts are subjects**, not files: *Welcome*, *How a request flows*, *Where errors go*,
   *The scheduler*. 4–8 parts. The first part is the big picture, later parts go deeper.
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
! crate::run/b5 crate::run/b6
Then every parsed task is saved into storage, one per loop iteration.
```

## Checklist before you hand it in

- `iwr check` prints "all refs resolve".
- The first part welcomes the listener and says what the project is for; the last cue wraps up.
- Each part has a title a listener would pick from a menu, and ends by handing over to the next.
- No cue longer than two sentences; no part longer than twelve cues; `@` changes at most two or three times per part.
- The first cue of every part sets `@`.
- At least one `=` per part that walks through code.
