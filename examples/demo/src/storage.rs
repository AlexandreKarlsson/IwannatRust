//! Storage backends.

use crate::model::Task;
use std::collections::HashMap;

#[derive(Debug)]
pub enum StorageError {
    Duplicate(String),
    NotFound(String),
}

/// Abstract task store.
pub trait Storage {
    fn save(&mut self, task: Task) -> Result<(), StorageError>;
    fn take(&mut self, name: &str) -> Result<Task, StorageError>;
    fn list(&self) -> Vec<Task>;
}

/// In-memory store backed by a HashMap.
#[derive(Default)]
pub struct MemoryStorage {
    tasks: HashMap<String, Task>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for MemoryStorage {
    fn save(&mut self, task: Task) -> Result<(), StorageError> {
        if self.tasks.contains_key(&task.name) {
            return Err(StorageError::Duplicate(task.name));
        }
        self.tasks.insert(task.name.clone(), task);
        Ok(())
    }

    fn take(&mut self, name: &str) -> Result<Task, StorageError> {
        self.tasks.remove(name).ok_or_else(|| StorageError::NotFound(name.to_string()))
    }

    fn list(&self) -> Vec<Task> {
        let mut v: Vec<Task> = self.tasks.values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }
}
