//! Parse `name:priority:steps` lines into tasks.

use crate::model::{Priority, Task};

#[derive(Debug)]
pub enum ParseError {
    MissingField(usize),
    BadPriority(String),
    BadNumber(String),
}

/// Parse every non-empty line.
pub fn parse_all(input: &str) -> Result<Vec<Task>, ParseError> {
    let mut out = Vec::new();
    for (i, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let task = parse_line(i, line)?;
        out.push(task);
    }
    Ok(out)
}

/// Parse one line.
fn parse_line(idx: usize, line: &str) -> Result<Task, ParseError> {
    let mut parts = line.split(':');
    let name = parts.next().ok_or(ParseError::MissingField(idx))?.to_string();
    let prio = parts.next().ok_or(ParseError::MissingField(idx))?;
    let steps = parts.next().ok_or(ParseError::MissingField(idx))?;
    let priority = parse_priority(prio)?;
    let steps: u32 = steps.trim().parse().map_err(|_| ParseError::BadNumber(steps.to_string()))?;
    Ok(Task { name, priority, steps })
}

fn parse_priority(s: &str) -> Result<Priority, ParseError> {
    match s.trim() {
        "low" => Ok(Priority::Low),
        "normal" => Ok(Priority::Normal),
        "high" => Ok(Priority::High),
        other => Err(ParseError::BadPriority(other.to_string())),
    }
}
