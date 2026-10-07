## Pass one: parsing
@ flow:iwr-core::parse::collect
collect is pass one. It gets the file list and returns a Collected: the Project so far, plus the function bodies kept aside for pass two.
! iwr-core::parse::collect/b1 iwr-core::parse::collect/b2
First, an empty Project and a context around it. The context tracks the module tree and collects the bodies.
! iwr-core::parse::collect/b4
Then one loop over the files. Every file becomes a SourceFile, whether it is Rust or not, so the UI can show a Cargo.toml too.
! iwr-core::parse::collect/b9 iwr-core::parse::collect/b10
Non-Rust files stop here with a continue. Only Rust files go on to the parser.
! iwr-core::parse::collect/b12
= crates/iwr-core/src/parse.rs:143:25-49
The parser is syn, the same crate procedural macros use. parse_file turns the text into a syntax tree, or an error with a line number.
! iwr-core::parse::collect/b15 iwr-core::parse::collect/b17 iwr-core::parse::collect/b18
A file that does not parse is not fatal: the error goes into the diagnostics list and the loop continues. A half-broken project still gets a picture.
? What is syn?
  syn is the Rust parser library that procedural macros are built on. Give it source text, it gives back a typed syntax tree: items, expressions, patterns, with spans. IwannatRust walks that tree instead of writing its own parser, which is why it handles all of Rust's syntax.
! iwr-core::parse::collect/b19 iwr-core::parse::collect/b20
= crates/iwr-core/src/parse.rs:151:21-47
Next, the file path becomes a module path: `src/parser.rs` turns into the module parser. module_for finds or creates that module in the tree.
! iwr-core::parse::collect/b25
Finally collect_items walks the items of the file: functions, structs, enums, traits, impl blocks, and nested modules, recursively. Function bodies are stored, not analyzed yet.
! iwr-core::parse::collect/b28 iwr-core::parse::collect/b29
At the end, the context is taken apart and the bodies travel on to pass two, where the real linking happens.
