//! Pick tasks in priority order.

use crate::model::{Describe, Task};
use crate::storage::Storage;

/// Pops the heaviest task each time.
pub struct Scheduler<S: Storage> {
    storage: S,
    served: usize,
}

impl<S: Storage> Scheduler<S> {
    pub fn new(storage: S) -> Self {
        Scheduler { storage, served: 0 }
    }

    /// Next task by weight, or None when the storage is empty.
    pub fn next_task(&mut self) -> Option<Task> {
        let all = self.storage.list();
        let best = all.iter().max_by_key(|t| t.weight())?;
        let name = best.name.clone();
        let task = self.storage.take(&name).ok()?;
        self.served += 1;
        log(&task.describe_with("scheduling "));
        Some(task)
    }
}

fn log(msg: &str) {
    println!("[sched] {}", msg);
}
