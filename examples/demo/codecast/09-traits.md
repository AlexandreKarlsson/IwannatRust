## Traits, or how to swap the database
@ code:src/storage.rs
! src/storage.rs:13-17
Here is the Storage trait, in full: save, take, list. Three method signatures and no bodies. A trait is a promise; the bodies come from whoever makes the promise.
! src/storage.rs:31
And here is MemoryStorage making it: `impl Storage for MemoryStorage`. Read it as "MemoryStorage can do Storage". Everything inside this block has to match the trait's signatures exactly, or the compiler sends it back.
? What is the difference between impl Task and impl Storage for MemoryStorage?
  @ code:src/model.rs
  ! src/model.rs:20
  `impl Task` adds methods that belong to Task alone, like weight. `impl Storage for MemoryStorage` fulfils a trait: it says MemoryStorage provides every method Storage asks for. The first form is "here are my own methods", the second is "here is how I meet this contract". A type can have one of the first and as many of the second as it wants.
@ code:src/storage.rs
! crate::storage::MemoryStorage::save
Save is the only one with a decision in it: refuse a task whose name is already there, with a Duplicate error, otherwise insert. The HashMap is keyed by name, which is why a duplicate name is the only thing that can go wrong.
! crate::storage::MemoryStorage::list
List copies every task out of the map and sorts by name. The sort is there so that two tasks with equal weight are picked in a predictable order. Determinism is boring, and boring is what you want from a scheduler.
@ code:src/scheduler.rs
! src/scheduler.rs:7-12
Now the payoff. The scheduler is declared over any S that implements Storage. It calls list and take and never once mentions MemoryStorage. Write a PostgresStorage with the same three methods, pass it to Scheduler::new, and the scheduler would not notice. That is the whole promise of traits: the code that uses the contract never has to know who signed it.
@ code:src/model.rs
! src/model.rs:33-40
One more trait trick, in Describe. The trait declares describe without a body, and describe_with with one. The second is a default method: it is written once, in the trait, in terms of the first. Implement describe, and describe_with comes for free.
! src/model.rs:42-46
Task implements only describe, one line, and the scheduler calls describe_with on it. The trait supplied the glue. Traits are not only contracts, they are also where shared behaviour lives. And with that, the happy path is done. Time to break things.
