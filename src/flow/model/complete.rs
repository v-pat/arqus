use std::time::Instant;
use tokio::sync::mpsc::channel;

use crate::ai::{Llm, LlmRequest, StreamEvent};
use crate::flow::errors::FlowError;
use crate::types::{Block, Message, Usage};

#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub text: String,
    pub usage: Usage,
    pub latency_ms: u64,
    pub estimated: bool,
}

pub async fn complete_text(llm: &Llm, req: LlmRequest) -> Result<Completion, FlowError> {
    let start = Instant::now();
    let (tx, mut rx) = channel(128);

    let mut prompt_chars = req.system.chars().count() as u64;
    for msg in &req.messages {
        match msg {
            Message::User { text } => {
                prompt_chars = prompt_chars.saturating_add(text.chars().count() as u64);
            }
            Message::Assistant { blocks, .. } => {
                for b in blocks {
                    if let Block::Text { text } = b {
                        prompt_chars = prompt_chars.saturating_add(text.chars().count() as u64);
                    }
                }
            }
            Message::ToolResult { text, .. } => {
                prompt_chars = prompt_chars.saturating_add(text.chars().count() as u64);
            }
        }
    }

    let run = llm.stream(req, tx);
    tokio::pin!(run);

    let mut streamed_text = String::new();
    let mut finished_text = String::new();
    let mut provider_usage: Option<Usage> = None;
    let mut provider_error: Option<String> = None;
    let mut stream_done = false;
    let mut stream_res: Option<anyhow::Result<()>> = None;

    loop {
        tokio::select! {
            res = &mut run, if !stream_done => {
                stream_done = true;
                stream_res = Some(res);
            }
            event = rx.recv() => match event {
                Some(event) => {
                    match event {
                        StreamEvent::TextDelta(delta) => {
                            streamed_text.push_str(&delta);
                        }
                        StreamEvent::Done(Message::Assistant { blocks, usage, .. }) => {
                            if provider_usage.is_none() {
                                provider_usage = usage;
                            }
                            for block in blocks {
                                if let Block::Text { text } = block {
                                    finished_text.push_str(&text);
                                }
                            }
                        }
                        StreamEvent::Error(err) if provider_error.is_none() => {
                            provider_error = Some(err);
                        }
                        _ => {}
                    }
                }
                None => {
                    break;
                }
            }
        }
    }

    if let Some(err) = provider_error {
        return Err(FlowError::ProviderError(err));
    }

    if let Some(Err(err)) = stream_res {
        return Err(FlowError::ProviderError(format!("{err:#}")));
    }

    let text = if !streamed_text.is_empty() {
        streamed_text
    } else {
        finished_text
    };

    let latency_ms = start.elapsed().as_millis() as u64;

    let (usage, estimated) = match provider_usage {
        Some(u) => (u, false),
        None => {
            let output_chars = text.chars().count() as u64;
            let est_input = (prompt_chars / 4).max(1);
            let est_output = output_chars / 4;
            (
                Usage {
                    input: est_input,
                    output: est_output,
                    cache_read: 0,
                    cache_write: 0,
                    cost: None,
                },
                true,
            )
        }
    };

    Ok(Completion {
        text,
        usage,
        latency_ms,
        estimated,
    })
}

#[cfg(test)]
#[path = "complete_tests.rs"]
mod tests;
