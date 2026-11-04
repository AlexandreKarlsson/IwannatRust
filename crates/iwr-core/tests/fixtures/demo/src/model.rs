//! Core data types.

/// How urgent a task is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Normal,
    High,
}

/// A unit of work.
#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub priority: Priority,
    /// Number of sub-steps to run.
    pub steps: u32,
}

impl Task {
    /// A task's weight used for ordering.
    pub fn weight(&self) -> u32 {
        let base = match self.priority {
            Priority::High => 100,
            Priority::Normal => 10,
            Priority::Low => 1,
        };
        base + self.steps
    }
}

/// Anything that can be described in one line.
pub trait Describe {
    fn describe(&self) -> String;

    /// Default: describe with a prefix.
    fn describe_with(&self, prefix: &str) -> String {
        format!("{}{}", prefix, self.describe())
    }
}

impl Describe for Task {
    fn describe(&self) -> String {
        format!("{} ({:?}, {} steps)", self.name, self.priority, self.steps)
    }
}
