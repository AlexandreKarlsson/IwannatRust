## How a run works
@ flow:crate::run
Everything starts in main, which prints a banner, calls run, and prints the result. So run is where the story is, and we will stay here for a while. This is its control flow: every box is a statement, and the edges are the paths execution can take.
! crate::run/b1
= src/main.rs:49:17-44
First, run hands the whole input to the parser and gets a list of tasks back. One call, and the parser does all the work; run just wants the list.
= src/main.rs:49:41-41
Notice the question mark at the end of that line. It means: if parsing failed, stop right here and hand the error to whoever called us. It is the Rust way of saying "nope, I'm out", and we never reach the rest of the function.
? What is run's return type?
  = src/main.rs:48:24-46
  It returns a Result of usize and AppError: either Ok with the number of tasks that ran, or Err with an AppError. Rust has no exceptions, so a function that can fail says so in its signature, and the caller has to open the box. The question mark is the short way to open it and pass an error along.
! crate::run/b2 crate::run/b3
Then the first decision. If the list came back empty, run refuses to go on and returns the Empty error. Nothing to do is treated as a failure, which tells you something about the author's work ethic.
! crate::run/b4
Otherwise it creates an in-memory storage. The storage starts empty, like most good intentions.
! crate::run/b5 crate::run/b6
= src/main.rs:55:32-32
And it saves every task into it, one per loop iteration. Saving can fail too, a duplicate name for instance, so the question mark is back, this time inside the loop. One bad task and the whole run bails out.
! crate::run/b7 crate::run/b8
With the tasks stored, run builds a scheduler on top of the storage and starts a counter. Note that it hands the storage over to the scheduler. It gives it away, and we come back to that in the next part, because it matters.
! crate::run/b9
= src/main.rs:59:5-44
Now the main loop. `while let Some(task)` keeps asking the scheduler for the next task until the scheduler answers None, which happens when the storage is empty. No index, no counter, no off-by-one. Just "give me the next one until there is none".
@ code:crate::run
! crate::run/b10 crate::run/b11
= src/main.rs:60:12-53
Let's read that loop as code. Inside it there is a small rule: once three tasks have run, low priority tasks are skipped with a continue. In our input that rule hits exactly one task: deploy. Deploy never runs. Some would call that a bug. Ops calls it a Friday.
? Why does deploy never run?
  = src/main.rs:34:21-78
  The scheduler serves tasks by weight: build first, then test, then lint, and deploy last because it is low priority. By the time deploy comes up, count is three, so "low and count greater than two" is true and the loop skips it. The rule was meant to drop low priority work once the important things are done; the demo input just happens to make the drop land on deploy.
! crate::run/b12 crate::run/b13
Every other task is executed, and the counter goes up. Execute is a separate function, and it has a surprise in it, so it gets its own part.
! crate::run/b14
When the loop ends, run returns how many tasks it executed, wrapped in Ok. So run has three ways out: two errors and one success. Before we follow the tasks any further, a short detour about who owns what, because this function is a perfect example.
