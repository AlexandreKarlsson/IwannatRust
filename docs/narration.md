# Narration

A **script** is spoken text with references to what to show and highlight. The player runs it on top
of the visualizer: switches views, glows the right boxes, highlights and underlines code, and speaks
(browser TTS), plays a recording, or just lets you read. Scripts are split into **parts** you can play
independently ("Architecture", "Error handling", …). Any LLM, or a person, can write one from the
**brief**. No API key, no provider, no JSON.

```
iwr brief  <path> [--fn P] [--no-body] [--no-overview]   # what the AI reads
iwr guide  <path> [--fn P]  > script.txt                  # the built-in walkthrough, as a script
iwr check  script.txt --path <path>                       # every ref must resolve
iwr serve  <path> --script script.txt [--audio voice.mp3] # play it (also load… in the player bar)
iwr export <path> --script script.txt [--audio voice.mp3] # bundled in the static site
```

## Script format

Line based. One spoken line = one cue. Directive lines above a cue attach to it.

```
# How the demo task runner works        title (first line)
audio: voice.mp3                         optional recording

## Architecture                          part (play it alone)
@ arch                                   show: <view>[:<ref>]       sticky
The demo is four modules around main.    cue: spoken + shown as caption
! crate::parser                          highlight refs (space separated)   sticky; `!` alone clears
The parser turns text lines into tasks.

## Running a task list
@ flow:crate::run
! crate::run/b1
= src/main.rs:49:17-44                   underline this code            this cue only
[12.5] run starts by parsing the input.  optional start time in seconds (recording mode)
! crate::run/b5 crate::run/b6
Every task is saved, in a loop.
// comment lines are ignored
```

Roughly 10 tokens of directives per sentence. Directives: `@` show, `!` highlight, `=` code, `[t]` time, `##` part, `#` title, `audio:`.

## Refs

| Ref | Means | Example |
|-----|-------|---------|
| item path | fn, method, struct, enum, trait, impl (`crate::` optional; unique suffix OK) | `crate::run`, `Task::weight`, `parse_all` |
| module path | module (scopes module views, glows the module box) | `crate::storage`, `storage` |
| `<item>/b<n>` | block *n* of that function, ids as printed by `iwr brief` | `crate::run/b5` |
| `<file>:<l>[-<l>]` | source lines | `src/main.rs:49-52` |
| `<file>:<l>:<c1>-<c2>` | columns on a line (word underline, use with `=`) | `src/main.rs:49:17-44` |

Views: `code` `calls` `flow` `arch` `branches` `structure` `types` `errors`. `@ flow:crate::run` opens the
control flow of `run`; `@ code:crate::run` opens its blocks; `@ types:crate::model` scopes the types
view to that module; `@ errors` keeps the current scope.

A ref resolves to a span, and every view highlights what intersects it: the block and its lines in the
Code view, the matching statement nodes in Control flow / Branch tree, the item's node in Call tree,
Types, Error flow, the module box in Architecture. The script never names UI nodes. Unknown refs are
skipped (`iwr check` lists them).

## Brief

`iwr brief` prints the project as compact text (~4 chars per token): modules with docs, types with fields
or variants, traits with implementors, then each function's signature, facts and body as block lines:

```
fn crate::run  src/main.rs:47-67  "Parse the input, schedule the tasks and execute them."
  fn run(input: &str) -> Result<usize, AppError>   Result · 2 loop · 2 branch · 2×?
  b1 ?      let tasks = parser::parse_all(input)?  ＋tasks  input  → crate::parser::parse_all
  b2 if     if tasks.is_empty()  tasks
  b3   return return Err(AppError::Empty)
  b5 loop   for t in &tasks  ＋t  tasks
  b6   ?    storage.save(t.clone())?  storage t  → crate::storage::Storage::save
```

`b<n> <kind> <label> ＋defined used → callee`. Demo crate: ~2.3k tokens with bodies, ~600 without.
`--fn crate::run` prints one function plus the signatures of what it calls and who calls it.

Prompt that works: *"Here is a brief of a Rust project and the script format. Write a narration in
2–4 parts that explains how it works to a newcomer. Reference blocks by their ids."* Then `iwr check`.

## Player

| Voice | Advance | Notes |
|-------|---------|-------|
| 🔊 TTS | when the utterance ends | browser `speechSynthesis`; markdown/code stripped before speaking |
| 🎧 recording | by time: cue with the largest `[t] ≤` position | `--audio` file, `audio:` line, or a file picked in *load…* |
| 📖 read | prev / next, or autoplay every 3 s | |

Parts are buttons in the player bar; playing stops at the end of a part (`next part ▶`).
`load…` accepts pasted text, a script file and an audio file. `▶ Narrate` (or `g`) plays the built-in
walkthrough as a script; `narrate from here` in the details panel starts it at a function.
Keys: `←` `→` step, `space` play/pause, `Esc` close.

`iwr serve` re-reads the script file on every page load, so edit and refresh.
