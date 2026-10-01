# Demo glossary

// Project-specific terms. Offered as questions when a cue says them, on top of the built-in
// Rust glossary (crates/iwr-core/glossary.md). An entry here with the same term as a built-in
// one replaces it. Directives (`@` `!` `=`) are allowed and point at this project.
// `pronounce:` lines fix how the voice says a word (the script header may have them too).

pronounce: sched = shed

## weight
aliases: weights, heaviest
ask: What is a task's weight?
@ code:crate::model::Task::weight
! crate::model::Task::weight
A number the scheduler sorts by: one hundred for a high priority task, ten for normal, one for low, plus the number of steps. The heaviest task is served first, so a high task with no steps still beats a normal one with ninety steps.

## steps
aliases: step, sub-step, sub-steps
ask: What are a task's steps?
@ flow:crate::execute
! crate::execute/b3
How many times execute loops for that task. Even steps become a sub-task with zero steps and are executed recursively; odd steps are skipped with a message. Lint has zero steps and does nothing, very efficiently.

## INPUT
aliases: the input, input constant
ask: What is in the input?
@ code:src/main.rs
! src/main.rs:34
= src/main.rs:34:21-78
Four tasks, one per line, as name, priority, steps: build with high priority and three steps, test with normal and two, deploy with low and one, lint with normal and zero. In this order they are parsed; in weight order they are run: build, test, lint, and deploy never, because of the "skip low after three" rule.
