## Scheduling
@ flow:crate::scheduler::Scheduler::next_task
next_task is the heart of the scheduler. It returns an Option: a task, or None when there is nothing left. That None is what ends the main loop in run.
! crate::scheduler::Scheduler::next_task/b1
It starts by asking the storage for every task it has. This call goes through the Storage trait, so the scheduler never knows it is talking to a HashMap. It could be a database, a file, or a very patient intern.
! crate::scheduler::Scheduler::next_task/b2
= src/scheduler.rs:20:31-56
Then it finds the task with the highest weight, with an iterator chain: iter walks the list, max_by_key picks the biggest according to a closure, and the closure says "biggest means weight". max_by_key returns None on an empty list, and the question mark turns that into an early None for the whole function.
? What is a closure?
  = src/scheduler.rs:20:42-55
  A closure is a small anonymous function written inline, with its arguments between vertical bars: `|t| t.weight()` means "given a task t, give me its weight". It can also use variables from the surrounding code, which a normal function cannot. Iterator methods like map, filter and max_by_key take closures to say what to do with each item.
@ code:crate::model::Task::weight
! crate::model::Task::weight/b2
The weight is where Priority becomes a number: high counts a hundred, normal ten, low one, plus the number of steps. So a high task always beats a normal one, whatever its size, and among equals the one with more steps goes first. Build, with high and three steps, scores one hundred and three. Deploy scores one.
@ flow:crate::scheduler::Scheduler::next_task
! crate::scheduler::Scheduler::next_task/b6
= src/scheduler.rs:22:20-48
Back in next_task, the winner is removed from the storage by name. take returns a Result, and ok turns an error into None, so a task that vanished between list and take also ends the loop quietly. In this program that cannot happen, but the scheduler does not know that, and defensive code is cheap.
! crate::scheduler::Scheduler::next_task/b8
Finally it logs what it picked, using the Describe trait we saw earlier, and hands the task back inside Some. One task per call, heaviest first, until the plate is empty. That is the buffet.
? Why is Scheduler generic?
  @ code:src/scheduler.rs
  ! src/scheduler.rs:7-12
  = src/scheduler.rs:7:21-32
  The struct is declared as Scheduler of S, where S is any type that implements Storage. That is a generic with a trait bound: the scheduler is written once and works for every storage, and the compiler generates a specialised copy for each one actually used. No run-time cost, no pointer indirection, and the scheduler never has to mention MemoryStorage. The next part is about exactly that trick.
