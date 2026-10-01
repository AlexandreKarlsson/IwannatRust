## Welcome
@ arch
Welcome to the demo task runner. It is a tiny Rust program that reads a list of tasks from text, stores them, and runs them in priority order. Four tasks, five files, zero dependencies, one bug that we will pretend is a feature.
It exists to exercise everything IwannatRust can show: calls, branches, loops, recursion, traits and error propagation. It is small enough to understand in fifteen minutes, which is the whole point of this tour.
This is the architecture view. Every box is a module, and the arrows show who depends on whom. Think of it as the floor plan before we open any doors.
? Where does the input come from?
  @ code:src/main.rs
  ! src/main.rs:34
  = src/main.rs:34:7-11
  From a constant called INPUT, hard-coded at the top of main.rs: four lines, one task per line, in the form name, priority, steps. A real program would read a file; a demo reads a string and gets on with its life.
! crate::parser
The parser module turns those text lines into tasks. It is the first thing the program touches, so it is also the first place things can go wrong. Parsers always are.
! crate::storage
The storage module keeps the tasks. It hides the actual container behind a trait, so a database could replace the in-memory map without the rest of the program noticing. We will see how that trick works later.
! crate::scheduler
The scheduler decides what runs next. Each round it asks the storage for everything and picks the heaviest task. It is a buffet strategy: always go for the biggest plate.
! crate::model
And the model module holds the shared data: what a task is, how urgent it is, and a small trait for describing things in one line.
!
Four modules, one entry point in main.rs, and every arrow eventually points at the model. That is the whole map. Before we walk it, one word that will come up a lot: crate.
