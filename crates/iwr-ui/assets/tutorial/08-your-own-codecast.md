## Your own codecast
```md Lines
| Line | Does |
|---|---|
| `@ flow:crate::run` | show a view: here the control flow of `run` |
| `! crate::run/b2` | light up an item, a step, a file or a folder (`! src/store/`) |
| `= src/main.rs:12` | underline code |
| `+ files` | enlarge a panel: `files`, `items`, `view`, `source` |
| `+ search` | spotlight a part of the app: `search`, `player`, `settings`… |
| `+ zoom` | zoom in on what is lit up (or `+ zoom files`, `+ zoom side`…) |
| `+` | back to normal |
| `? What is…?` | a question, its answer indented below |
```
@ code
+
| md:Lines
^ codecast brief .
To give your own project its virtual presentation, start with the brief: a short text that tells you, or an AI, what the project holds.
+ zoom side
On the right, the note lists the lines of a script. It is a markdown file next to the project: one sentence per line, and short lines above it that say what to show.
+
^ codecast check codecast.md
Then check that every reference still points at the code,
^ codecast serve .
and serve it. That is the end of the tutorial. Have fun.
