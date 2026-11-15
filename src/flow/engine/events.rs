use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FlowEvent {
    pub ts: u64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub task_id: Option<String>,
    pub data: Value,
}

pub fn append_event(path: &Path, event: &FlowEvent) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut line = serde_json::to_string(event)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    line.push('\n');
    file.write_all(line.as_bytes())?;
    file.flush()
}

pub fn emit_event(path: &Path, event_type: &str, task_id: Option<&str>, data: Value) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let event = FlowEvent {
        ts,
        event_type: event_type.to_string(),
        task_id: task_id.map(|s| s.to_string()),
        data,
    };
    let _ = append_event(path, &event);
    crate::debug::log(
        "flow_event",
        serde_json::json!({
            "type": event_type,
            "task_id": task_id,
            "data": event.data,
        }),
    );
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
