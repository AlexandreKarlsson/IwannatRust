## Where things live
@ code:src/main.rs
main.rs holds the entry point, the top level error type with its two conversions, the run function, and execute. It also declares the four modules, which is what makes the other files part of the crate at all.
@ code:src/parser.rs
! crate::parser::parse_line
parser.rs is the three parsing functions we walked through, plus their error enum. Text goes in, tasks come out, question marks everywhere.
@ code:src/storage.rs
! crate::storage::Storage
storage.rs declares the Storage trait first, then the in-memory implementation below it. The contract on top, the signature at the bottom.
@ code:src/scheduler.rs
! crate::scheduler::Scheduler
scheduler.rs is generic over any Storage, which is why it never mentions MemoryStorage. It picks the heaviest task, logs it, and hands it over, one at a time.
@ code:src/model.rs
! crate::model::Describe
model.rs holds the data everyone shares, and ends with the Describe trait: a default method built on top of one required method.
!
And that is the end of the tour. You have seen every view: architecture, structure, types, control flow, code, errors and the files. You have also seen a crate, modules, structs, enums, traits, ownership, closures and more question marks than a quiz show. Click any box to explore on your own, and if deploy ever runs, let someone know.
