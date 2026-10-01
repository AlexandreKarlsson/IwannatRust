## Executing a task
@ flow:crate::execute
Execute is the smallest function with the most interesting shape, because it calls itself. This is its control flow, and the surprise is at the bottom.
! crate::execute/b1 crate::execute/b2
It starts by printing the task name, indented by the current depth. Depth zero for the tasks run passes in, deeper for the ones we are about to create.
! crate::execute/b3
Then it loops over the task's steps, zero to steps. The lint task has zero steps, so for lint this loop runs zero times. Lint is the laziest task in the history of task runners, and it still counts as executed.
! crate::execute/b4
Each step is a decision: even steps become a sub-task, odd steps are skipped. Why? Because the demo needed a branch inside a loop, and this one was cheap.
! crate::execute/b5 crate::execute/b6
= src/main.rs:76:13-36
For an even step it builds a sub-task with zero steps and calls execute again, one level deeper. That is the recursion: in the call tree it shows as an orange dashed edge that loops back onto execute itself.
? Why does the recursion stop?
  ! crate::execute/b5
  = src/main.rs:75:97-104
  Because every sub-task is built with steps set to zero. The inner call prints the sub-task's name, its loop runs zero times, and it returns. So the recursion is always exactly one level deep: not a spiral, more of a polite nod. A recursive function needs a case that does not recurse, and here that case is baked into the data.
! crate::execute/b7 crate::execute/b8
Odd steps just print a skip message. And that is all of execute: print, loop, branch, recurse, done. Now let's go back to the start of the data flow and see how a line of text became a task in the first place.
