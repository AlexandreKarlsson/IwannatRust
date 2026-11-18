# Notes you can point into

// A topic of the tour: a markdown note beside the view, whose headings, steps, rows, tabs and
// diagram a sentence can glow, and one block that serves every part.
plan: How a sentence glows one line of a note, opens a tab, and how a block is written once for all parts.

```md Order
## The trip
1. The client sends a request.
2. The server reads the length.
3. The server answers.

=== "On the wire"
    | field | size | holds |
    |---|---|---|
    | len | 2 | the size of the body |
    | body | len | the request itself |

=== "Exchange"
    ~~~mermaid
    sequenceDiagram
    participant c as Client
    participant s as Server
    c->>s: request
    s-->>c: reply
    ~~~

!!! tip "Written once"
    This note is written in the index of the topic. Every part that names it borrows it.
```

- [A note you can point at](01-a-note-you-can-point-at.md) — a heading, a step, a row, a tab and an arrow, each glowing when the voice names it
- [Names for everything](02-names-for-everything.md) — slug turns a title into a name, collect walks the page and names every block
- [Tabs that open themselves](03-tabs-that-open-themselves.md) — one line starts a tab, and the page opens the tab the voice points into
- [Written once](04-written-once.md) — a part names a block it did not write, and lend hands it a copy
