## Pass two: linking
@ flow:iwr-core::resolve::finish
finish is pass two, and it is long because this is where everything gets linked: impl blocks to their types, fields to the types they mention, calls to the functions they call.
! iwr-core::resolve::finish/b2
First an index: every type, function and method by name, so the lookups below are cheap.
! iwr-core::resolve::finish/b4
The first loop is about impl blocks. For each one, the target type and the trait are resolved by name, and every method learns who owns it.
! iwr-core::resolve::finish/b33 iwr-core::resolve::finish/b34
When both the type and the trait resolve, that is an Implements relation, and the Types view draws it as a dashed arrow.
! iwr-core::resolve::finish/b50
The third loop reads field types. A struct field of type Vec of Task links the struct to Task: that becomes a Contains edge.
! iwr-core::resolve::finish/b100
Now the bodies. This loop is the heart of the analyzer: one CallVisitor per function body.
! iwr-core::resolve::finish/b116 iwr-core::resolve::finish/b117 iwr-core::resolve::finish/b120
= crates/iwr-core/src/resolve.rs:559:9-106
Notice the body is visited twice. The first walk only collects where the question marks, unwraps and awaits are; the second attributes them to the calls, so an edge knows it was propagated with a question mark.
? How does it know which function a call points at?
  By name, with hints. Same module first, then self dot something goes to the owner type, then the types in the signature and in the fields are tried, then anything unique with that name. It is not the compiler, so it can be wrong on ambiguous names, but it needs no cargo build and works on a single file.
! iwr-core::resolve::finish/b122 iwr-core::resolve::finish/b123
With the calls known, two more pictures are built per function: the control-flow graph, and the block tree you see in the Code view.
! iwr-core::resolve::finish/b127
Then the facts land in each function's FnInfo: its CFG, its blocks, how many loops, branches, question marks, whether it recurses.
! iwr-core::resolve::finish/b148 iwr-core::resolve::finish/b156
Last, the use paths tell which external crates the project depends on. The Project is complete. Let's look at what is in it.
