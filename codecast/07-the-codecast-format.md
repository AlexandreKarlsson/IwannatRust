## The codecast format
@ code:iwr-core::script::parse
A codecast is a markdown file: headings for parts, one line per spoken cue, and short directive lines above a cue that say what to show. parse reads it, and it never fails: an unknown line is simply spoken.
! iwr-core::script::parse/b2 iwr-core::script::parse/b3
Two accumulators: the part being filled, and pending, the cue being built from the directive lines above its text.
! iwr-core::script::parse/b7
One loop over the lines, and a chain of prefixes. Each branch either changes state and continues, or finishes a cue.
! iwr-core::script::parse/b12 iwr-core::script::parse/b14
A mermaid fence is captured whole: when it closes, the block is parsed as a diagram and attached to the part.
! iwr-core::script::parse/b20 iwr-core::script::parse/b31
Indented lines after a question mark are the answer. An answer may carry its own directives, so it can point at the code it talks about.
! iwr-core::script::parse/b43 iwr-core::script::parse/b46 iwr-core::script::parse/b47
A second-level heading closes the current part and starts a new one. Parts are what the player lets you pick from a menu.
! iwr-core::script::parse/b70 iwr-core::script::parse/b73 iwr-core::script::parse/b78 iwr-core::script::parse/b81
= crates/iwr-core/src/script.rs:210:29-47
The four directives: at sign for the view, bang for the highlights, equals for the code to underline, greater-than for a diagram move. Each one fills a field of pending and continues.
! iwr-core::script::parse/b96 iwr-core::script::parse/b97
= crates/iwr-core/src/script.rs:244:24-51
Any other line is spoken text. It completes the cue: pending is taken out with `mem::take`, which leaves a fresh default behind for the next one.
? What does mem take do?
  `std::mem::take` swaps a value with its default and gives you the old one. Here it hands the finished cue to the list and leaves an empty cue in pending, in one move, without cloning and without a second variable. It is the idiom for "pop the current thing and start over".
@ flow:iwr-core::script::check
Writers make mistakes, so check walks the parsed script and verifies every ref against the Project: items, blocks, lines, files, and diagram node ids. iwr check prints whatever does not resolve.
! iwr-core::script::check/b9 iwr-core::script::check/b10 iwr-core::script::check/b11 iwr-core::script::check/b12
The at sign is sticky, so a small Track remembers which view is current. While a diagram is shown, the bang refs are node ids, not code. Which brings us to diagrams.
