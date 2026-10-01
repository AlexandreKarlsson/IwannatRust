## The data
@ types
This is the types view. Structs and enums hold the values, traits describe behaviour several types can share. Green arrows mean contains, purple dashed arrows mean implements.
! crate::model::Task
A task is a struct: a name, a priority and a number of steps. A struct is a type with named fields, like a record or a plain object, minus the inheritance drama.
? Why no methods inside the struct?
  @ code:crate::model::Task::weight
  ! crate::model::Task
  In Rust the struct declares only the data. Behaviour goes in a separate `impl Task` block next to it, which can be in the same file or a different one. The data is the noun, the impl is the verbs, and the compiler is fine with you adding verbs later.
! crate::model::Priority
Priority is an enum with three variants: low, normal and high. An enum is a type whose value is exactly one of a fixed list. Later, the scheduler turns it into a number, and that is where the buffet strategy comes from.
@ code:crate::model::Task::weight
= src/model.rs:4
Both types carry a `derive` line above them. It asks the compiler to write the boring code for you: how to print the value for debugging, how to copy it, how to compare it. Code generation you never have to see.
? Why does Priority derive Ord and Task does not?
  Priority has a natural order, low before normal before high, and deriving Ord gives you comparisons for free, in declaration order. A Task has no obvious order: by name, by weight, by steps? When the answer is "it depends", you leave Ord out and write a weight method instead, which is exactly what the demo does.
@ types
! crate::storage::Storage crate::storage::MemoryStorage
Storage is a trait, with save, take and list. A trait is a contract: a list of methods a type promises to provide. MemoryStorage is the only type that signs the contract, with a HashMap keyed by task name underneath.
! crate::model::Describe
Describe is a tiny trait with one required method and one default method built on top of it. Task implements it so the scheduler can log what it picked, in one line, without caring what a task is.
! crate::AppError crate::parser::ParseError crate::storage::StorageError
Then the three error types. The parser and the storage each have their own enum, and AppError at the top wraps both. Keep them in mind, they get their own part at the end.
!
Now that we know the data, let's follow the program as it runs.
