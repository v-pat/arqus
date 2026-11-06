use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use tokio::sync::mpsc::Sender;

use super::{LlmRequest, StreamEvent};

/// Record the request, then behave like the plain mock. Tests read the log
/// to check what the caller sent.
pub async fn stream_capture(
    queue: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>,
    seen: Arc<Mutex<Vec<String>>>,
    req: LlmRequest,
    tx: Sender<StreamEvent>,
) -> Result<()> {
    if let Ok(text) = serde_json::to_string(&req) {
        if let Ok(mut guard) = seen.lock() {
            guard.push(text);
        }
    }
    stream(queue, req, tx).await
}

/// Send the next scripted event list, in order.
pub async fn stream(
    queue: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>,
    _req: LlmRequest,
    tx: Sender<StreamEvent>,
) -> Result<()> {
    let next = {
        let mut guard = queue
            .lock()
            .map_err(|_| anyhow!("mock provider: queue lock is poisoned"))?;
        guard.pop_front()
    };
    let events = next.ok_or_else(|| anyhow!("mock provider: no scripted response left"))?;
    for event in events {
        if tx.send(event).await.is_err() {
            break;
        }
    }
    Ok(())
}
