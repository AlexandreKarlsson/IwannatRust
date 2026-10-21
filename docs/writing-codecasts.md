# Writing a codecast (for agents and humans)

A **codecast** is a short spoken tour of a Rust project. The IwannatRust player performs it on top of
its views: switching diagrams, glowing the block being talked about, underlining code. Input: the
**brief** (`iwr brief <path>`). Output: a **script** in the markdown format below, as one `codecast.md`
or a `codecast/` directory (`index.md` with title + part list, one `NN-part-name.md` per part, optional
`glossary.md`, optional `codecast.toml` for the theme, fonts, voice and language the viewer starts
with — see [codecast.md](codecast.md#look-and-defaults-codecasttoml)). Validate with `iwr check <script> --path <path>`, hear-check with `iwr speak <script>`.

## Format

```
# <Title>

## <Part name>               one subject, 4–10 cues, playable alone
plan: <one line about it>    optional: its line on the plan page (or write it after the link in index.md)
@ <view>[:<ref>]             what to show (sticky until the next @); `@ plan` shows the plan page
! <ref> [<ref> …]            what to glow (sticky; `!` alone clears)
= <file>:<line>:<c1>-<c2>    what to underline in the code (this cue only)
> <node> [<group|node>]      diagrams only: move a node from this cue on (sticky in the part)
> +<node> -<node>            diagrams only: show a `:::hidden` node / hide one
| bytes|md|diagram[:<name>]  side panel next to the view (sticky in the part; `|` alone closes)
<name>_tour[:<ref>]          generated cues (see Tours): architecture types errors entry flow todo files
<One or two spoken sentences.>
? <A question the listener may have>      optional, after the cue it belongs to
  <The answer, indented, two to four sentences; may start with its own @ ! = lines>
```

Views: `code` `calls` `flow` `arch` `branches` `structure` `types` `errors`, and `diagram` (below).
Refs: `crate::run` (item) · `crate::run/b5` (block 5 of `run`, ids as in the brief) · `crate::storage`
(module) · `src/parser.rs` (file; `@ code:src/parser.rs` opens it) · `src/main.rs:49-52` (lines) ·
`src/main.rs:49:17-44` (columns, 1-based, `c2` inclusive). `crate::` may be omitted when the name
is unique. `//` starts a comment line.

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
| a tour of the files | `code:<file>` per cue | item refs in that file |

## Step 0: comment the code

The player shows the code's own comments. Better comments = clearer views and less script to write.
If you may edit the project, comment first, then write the script. The full guide, with examples
and what each comment turns into on screen: [commenting.md](commenting.md).

| Comment | Put it | Player uses it as |
|---------|--------|-------------------|
| `//!` | top of every file | the module's line in `architecture_tour` and the guide (1st paragraph only) |
| `///` | above every fn / struct / enum / trait | tooltip, details panel, brief, guide |
| `//` | line above a statement, `let`, `if`, `match`, loop | the node's label in `flow` (code moves to hover); opens the guide's step |
| `//` | above an `if` that `break`s / `return`s out of a loop | the label of that exit edge |

`//` label rules: one line, directly above (no blank line), ≤ 60 chars, not part of a multi-line
`//` block. Write intent, not code: `// Find the next biggest value`, not `// loop over v`.
One comment titles one paragraph of plain statements (up to the next blank line).
`//!` rule: first sentence says what the module is *for*; blank `//!` line; then details.
`iwr brief` marks modules without one: `(no //! doc)`.

```rust
//! Pick tasks in priority order.

/// Run every task, heaviest first.
pub fn run_all(tasks: &[Task]) -> Result<usize, Error> {
    // Find the next biggest task
    for t in tasks {
        // Out of budget
        if t.cost > budget {
            break;
        }
        …
```

In `flow:run_all` this shows: a big `run_all` title; one folded loop node "Find the next biggest
task"; its exit edge "Out of budget".

## What `flow` shows

- Top node: the function name, big. Signature on hover.
- Box colours say what a statement does: new variable, changes state, input/output, call, return
  (green) / `return Err` (red), program stops (`process::exit`, merged with the `eprintln!` before it).
- Logging macros (`log::info!`, `debug!`…) are hidden unless a `//` comment titles them.
  `let x = match …` is one node; arms that only hand a value on (`Ok(c) => c`) are edge labels.
- Loops start **folded**: one stacked node (comment or loop head), "N steps inside", exit edges
  labelled by the comment or condition that leads out (`done` = normal end). The viewer unfolds
  with ⊕; a script cannot. `!` on a block inside a folded loop glows the whole loop. To walk a
  loop body step by step, use `@ code:fn`.

## Tours (generated cues)

A line holding only a tour name becomes cues built from the code at play time. It stays in sync
with the code.

| Line | Becomes |
|------|---------|
| `architecture_tour[:mod]` | `@ arch`, intro (project + crate root `//!`), then per box in reading order (top down = entry point first, foundations last; then left to right): `!` it + its `//!` first paragraph (or what it holds; a doc page in full, see below). Folded boxes say how many modules they hold. |
| `types_tour[:mod]` | `@ types`, then per type (traits, errors, structs, enums): kind, fields / variants / implementors, `///` doc |
| `errors_tour[:mod]` | `@ errors`: each error type + who returns it + `From` conversions; where `?` passes errors up; who handles them; who can panic |
| `entry_tour` | `@ calls:main`: what `main` calls; then `@ structure`: each `pub` item with its doc |
| `flow_tour:fn` | `@ flow:fn`: the function step by step in reading order, each step opened by its `//` comment |
| `todo_tour[:mod]` | `@ code:fn` per stop: each `TODO`/`FIXME`/`XXX`/`HACK` comment, `unsafe`, `unwrap`/`expect`/`panic!` |
| `files_tour[:mod]` | `@ code:<file>` per file: its `//!` doc and what it defines |

`:mod` limits a tour to a module subtree (`types_tour:crate::storage`). At most 20 stops; the
last says how many were left out. Tests are skipped.

**Doc pages.** A `//!` doc with `#` headings, `| tables |` or `- lists` is read in full, not just its
first paragraph: the side panel shows the whole doc (tables, headings) while the text before the
first heading is read, then one cue per heading shows that section and reads it (table rows as
"first cell: header value, …", at most 8; code blocks are only shown). The panel closes at the next
stop without a page. Same in the built-in tour (`g`): the crate doc from the welcome on, each
module's on its stop. Paths and `<placeholders>` are read as "gw slash data slash node".

Directives above the line and `?` questions below it attach to its first cue. Write your own cue
before it (why we look) and after it (hand-over). Use them for the
inventory parts (map, data, errors) and write by hand what needs judgement: why, how parts connect.
`flow_tour` is only as good as the function's `//` comments. `iwr speak` prints the generated text; `iwr check` flags unknown tours.

## Voice: explain it to a curious twelve-year-old

The listener is smart but new to this code. Aim for "I could explain it to a friend after".

- **Plain words first, jargon second.** Say what happens ("the server writes the order down so it
  survives a crash"), then name it ("that is the write-ahead log").
- **One metaphor per big idea**: a queue is a line at the bakery, a mutex is the bathroom key, a
  channel is a mail slot, a trait is a job description. Say it once, reuse it, drop it when it
  stops fitting. Draw it if it moves (see *Diagrams*).
- **Show, don't describe data.** Talking about an API? Show the request body. A file parser? Show
  three lines of the file. A table? Show two rows. Use an example block (see *Examples*), real-looking
  values (`"ann@example.com"`, not `"string"`), 3–10 lines.
- **Follow one concrete thing.** "Let's follow Ann's order from the click to the database" beats a
  tour of layers. Reuse the same sample across parts.
- **Questions before answers.** "So what happens if the disk is full?" then the code that handles it.
- **Short sentences, active voice, *we*.** "We check the token" not "The token is checked".
- **Say why it matters**: the bug it avoids, the cost it saves. Skip what the listener can't see.

## How to write it

1. **A tour, not a list of facts.** The script plays in a row: a *Welcome* part saying what the
   project is for and where we will go (put `@ plan` on that cue: the player shows the parts as a
   list, so give each one a line after its link in `index.md`), parts that follow from each other,
   a one-sentence wrap-up.
2. **Parts are subjects**, with titles a listener would pick from a menu (*How a request flows*,
   *Where errors go*, *The scheduler*), 4–12 of them. First the big picture, then deeper; for a
   listener new to Rust, one part on the language mechanism the code shows best (ownership, traits).
3. **One view per part.** Ten cues on one function, moving only the glow, beat jumping between views.
   Change `@` only when the subject really moves, two or three times per part at most.
4. **Link the parts.** End each with a hand-over ("Next, let's see what executing a task means."),
   begin the next by picking it up. The listener should never wonder why the view changed.
5. **One idea per cue**, one or two sentences, ≤ 30 words each (the listener sees the caption).
   Connect cues naturally: *First, … Then … Notice … Otherwise … Finally …*
6. **Point at code**: `=` on the exact token you are talking about (the `?`, the `.await`, a
   condition). At least one per part that walks through code.
7. **Say what it does and why**, not what it is called: "The scheduler picks the heaviest task
   each round" beats "`next_task` returns an `Option<Task>`".
8. **Name the Rust mechanism** the reader may not know (`?` propagation, trait dispatch, ownership
   moves, recursion base case), one sentence, in place. Use the real words (crate, impl, trait,
   closure…): the player's glossary then offers a "What is a …?" question by itself.
9. **Add your own `?`** when the answer is specific to this project ("Why does deploy never run?"),
   one or two per part, with an answer of two to four sentences readable aloud. Recurring project
   vocabulary goes in `codecast/glossary.md`, not repeated in cues.
10. **Every ref must exist**: ids from the brief, never invented line numbers. Prefer block refs
    (`fn/bN`) over line refs; they survive edits.
11. **Write code as code**: `parse_all`, `Task::weight`, `&tasks`, `Option<Task>` are spoken as
    "parse all", "Task weight", "a reference to tasks", "Option of Task", and the caption shows the
    real token. Never spell for the voice ("parse underscore all"). Backticks are optional for single
    names, useful for expressions. Fix a word the voice gets wrong with `pronounce: written = spoken`
    in the header, and list acronyms to read letter by letter with `spell: IP, CoAP, HTTP`.
12. **Be a host, not a textbook.** A light touch of humour tied to the code keeps a listener awake:
    short, kind, and no puns that need spelling.

## Diagrams (when the code has no picture)

For a metaphor, a physical effect, a protocol or any schema that is not in the code, draw it: a
```` ```mermaid ```` flowchart block in the part, `@ diagram` to show it, `!` with node ids to glow.
Shapes set the colour: `[box]` blue · `(round)` green · `([pill])` purple · `{diamond}` yellow ·
`[(store)]` teal; `subgraph id[Title] … end` draws a group. Emoji in labels are fine (`b[🚌 Bus]`).
To show something flowing (a request through layers, a value changing owner), move a node:
`> alice bus` puts it into group `bus` (or next to node `bus`) from that cue on, `> alice` brings
it back to the top; one move per line, undone when the listener steps back. A node written
`copy[📄 copy]:::hidden` waits until `> +copy` draws it (`> -copy` hides it again), so one block
can hold the whole story and reveal it cue by cue. Keep 3–8 nodes, one block per part, and come
back to the code afterwards.

````
## Why tasks queue up
```mermaid
graph LR
  subgraph stop[🚏 Stop]
    alice[🧍 Alice]
  end
  stop --> bus[(🚌 Bus)] --> office{Office?}
```
@ diagram
! stop
Think of tasks as passengers waiting at a stop.
> alice bus
! bus
Alice boards: the scheduler took the heaviest task first.
````

## Side panel (bytes, lists, a small schema)

When the listener needs two things at once, the code *and* what it reads, put the second one in
the side panel instead of switching views. `| bytes:<title>` shows a ```` ```bytes <title> ````
block: one field per line, `id size [format] description`, and an optional `sample:` line of hex
bytes decoded into each field. `!` with field ids glows them in the strip and the table, next to
the code refs on the same line. `| md:<title>` shows a ```` ```md <title> ```` block (a numbered
list of steps, a table of fields, short notes); `| diagram:<title>` a small, still mermaid block.

````
## Reading a frame
```bytes Frame
endian: le
magic  4    ascii  Magic number
len    u16         Body length
body   len         The payload
crc    4    hex    CRC32 of the body
sample: 49 57 52 31 05 00 68 65 6c 6c 6f de ad be ef
```
@ code:crate::proto::read_frame
| bytes:Frame
! crate::proto::read_frame/b2 len
The two bytes after the magic say how long the body is.
````

Sizes: a number, a number type (`u8` `u16` `u32` `u64`, `i…`, `f32` `f64`, `le`/`be` suffix),
an earlier field's id (its value), or `*` for whatever the sample has left. `_` is padding.
Use it for headers, packets, file formats, register maps; skip it when the format is text.

## Examples (a body sent, a file read, a row stored)

Data is easier to see than to describe. A ```` ```json <title> ```` block (also `csv`, `tsv`, `yaml`,
`toml`, `xml`, `http`, `sql`, `sh`, `text`) is an example; `| example:<title>` (or `| json:<title>`)
shows it in the side panel next to the code or diagram, `|` hides it again. `!` with a key (json,
yaml, toml, xml, http header) glows its line; with a column name it glows a csv column. Keep it short
and real; show it on the cue that talks about it, close it when the subject moves on.

````
## Sending an order
```json Order
{
  "user": "ann@example.com",
  "items": [{ "sku": "BREAD-01", "qty": 2 }],
  "pay": "card"
}
```
@ code:crate::api::post_order
| example:Order
! crate::api::post_order/b1 items
The shop sends this little parcel. Each item says what and how many.
|
````

## Example

```
# A tour of the demo task runner

## Welcome
@ arch
Welcome to the demo task runner. It reads a list of tasks from text, stores them, and runs them in priority order.
! crate::parser
The parser turns text lines into tasks. It is the first thing the program touches, and the first place things can go wrong.
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

## Checklist

- `iwr check` prints "all refs resolve"; every `?` has an indented answer.
- Directory form: `index.md` has `# Title` and a `- [Part](NN-file.md)` list in playing order; each
  part file starts with its `## Part name`.
- First part welcomes and says what the project is for; last cue wraps up; each part hands over.
- First cue of every part sets `@`; no part longer than twelve cues.
- Data the code sends, reads or stores is shown with an example block, not described.
- Each big idea has one metaphor; jargon comes after the plain words.
- Every module has a `//!`, loops and key `if`s have a short `//` (if you may edit the code).
- Inventory parts may use tours (`architecture_tour`, `types_tour`, `errors_tour`…) instead of hand-written cue lists.
- `iwr speak` reads well: no "underscore", no "colon colon"; odd words have a `pronounce:` line, acronyms a `spell:` one.
