//! Demo program for IwannatRust: a tiny task runner with a parser, a scheduler
//! and a storage backend. Exercises calls, branches, loops, recursion, traits
//! and error propagation.

mod parser;
mod scheduler;
mod storage;
mod model;

use model::{Task, Priority};
use scheduler::Scheduler;
use storage::{Storage, MemoryStorage, StorageError};

/// Top-level error type of the demo.
#[derive(Debug)]
pub enum AppError {
    Parse(parser::ParseError),
    Storage(StorageError),
    Empty,
}

impl From<parser::ParseError> for AppError {
    fn from(e: parser::ParseError) -> Self {
        AppError::Parse(e)
    }
}

impl From<StorageError> for AppError {
    fn from(e: StorageError) -> Self {
        AppError::Storage(e)
    }
}

const INPUT: &str = "build:high:3\ntest:normal:2\ndeploy:low:1\nlint:normal:0";

fn main() {
    println!("demo task runner");
    match run(INPUT) {
        Ok(n) => println!("ran {} tasks", n),
        Err(e) => {
            eprintln!("error: {:?}", e);
            std::process::exit(1);
        }
    }
}

/// Parse the input, schedule the tasks and execute them.
fn run(input: &str) -> Result<usize, AppError> {
    let tasks = parser::parse_all(input)?;
    if tasks.is_empty() {
        return Err(AppError::Empty);
    }
    let mut storage = MemoryStorage::new();
    for t in &tasks {
        storage.save(t.clone())?;
    }
    let mut sched = Scheduler::new(storage);
    let mut count = 0;
    while let Some(task) = sched.next_task() {
        if task.priority == Priority::Low && count > 2 {
            continue;
        }
        execute(&task, 0);
        count += 1;
    }
    Ok(count)
}

/// Execute a task, recursing into its sub-steps.
fn execute(task: &Task, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{}> {}", indent, task.name);
    for i in 0..task.steps {
        if i % 2 == 0 {
            let sub = Task { name: format!("{}-step{}", task.name, i), priority: task.priority, steps: 0 };
            execute(&sub, depth + 1);
        } else {
            println!("{}  skip step {}", indent, i);
        }
    }
}
