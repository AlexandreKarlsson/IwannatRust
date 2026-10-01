## What is a crate anyway
@ structure
Rust code comes in crates. A crate is one library or one program, compiled as a whole. This demo is a single crate called demo, and everything you see in this tour lives inside it.
This is the structure view: the crate at the top, its modules below, and inside each module the functions, types and traits. It is the table of contents of the program.
@ code:src/main.rs
! src/main.rs:5-8
A module is a named box inside the crate. These four `mod` lines in main.rs are what create them: each one says "there is a file with this name, go read it and make it a module".
? Why is main.rs special?
  Every Rust program has one root file, and for a binary that is main.rs. It is the only file the compiler opens on its own. Every other file becomes part of the crate only because some `mod` line points at it, which is why a file you forget to declare simply does not exist as far as the compiler is concerned.
! src/main.rs:10-12
The `use` lines are imports. They bring names into scope so the code can say Task instead of model::Task. Nothing moves; the code stays where it was written. It is a shortcut, not a shipment.
= src/main.rs:48:1-6
Notice that run has no `pub` in front of it. In Rust everything is private to its module unless you say `pub`, so run can only be called from inside main.rs. Which is fine, because main is right there.
@ code:src/parser.rs
! crate::parser::parse_all crate::parser::parse_line
Compare with the parser: parse_all is `pub` because run, in another module, needs it. parse_line is not, because it is the parser's own business. The module shows a small door and hides the furniture.
@ arch
So when a path like `crate::parser::parse_all` shows up in this tour, read it from left to right: this crate, the parser module, the function parse_all. Now let's meet the data those functions pass around.
