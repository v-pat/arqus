//! The separate, tool-free LLM call that writes a compaction summary.

use anyhow::{anyhow, Result};

use crate::ai::{Llm, LlmRequest, StreamEvent};
use crate::types::{Block, Message};

const SUMMARY_SYSTEM: &str = "You are a context summarization assistant. Read the conversation and output ONLY the structured summary. Do not continue the conversation and do not answer questions in it.";

const SUMMARY_SECTIONS: &str = "Output these sections exactly:\n\
## Goal\n\
## Constraints & Preferences\n\
## Progress\n\
### Done\n\
### In Progress\n\
### Blocked\n\
## Key Decisions\n\
## Next Steps\n\
## Critical Context\n\n\
Keep each section short. Keep exact file paths, function names and error messages.";

/// Build the summary request: conversation plus optional previous summary.
pub fn summary_request(
    conversation: &str,
    previous_summary: Option<&str>,
    custom_instructions: Option<&str>,
) -> LlmRequest {
    let mut text = String::from("Summarize the conversation below.\n\n<conversation>\n");
    text.push_str(conversation);
    text.push_str("\n</conversation>\n\n");

    if let Some(previous) = previous_summary {
        text.push_str("<previous-summary>\n");
        text.push_str(previous);
        text.push_str("\n</previous-summary>\n\n");
        text.push_str(
            "Update the existing summary: keep all still-relevant information, add new progress and decisions, move finished items to Done, update Next Steps.\n\n",
        );
    }

    text.push_str(SUMMARY_SECTIONS);

    if let Some(extra) = custom_instructions {
        if !extra.trim().is_empty() {
            text.push_str("\n\nAdditional focus: ");
            text.push_str(extra.trim());
        }
    }

    LlmRequest {
        system: SUMMARY_SYSTEM.to_string(),
        messages: vec![Message::User { text }],
        tools: vec![],
        max_tokens: None,
    }
}

pub async fn generate_summary(
    llm: &Llm,
    conversation_text: &str,
    previous_summary: Option<&str>,
    custom_instructions: Option<&str>,
) -> Result<String> {
    let request = summary_request(conversation_text, previous_summary, custom_instructions);

    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let task = tokio::spawn({
        let llm = llm.clone();
        async move { llm.stream(request, tx).await }
    });

    let mut streamed = String::new();
    let mut finished = String::new();
    let mut error: Option<String> = None;
    while let Some(event) = rx.recv().await {
        match event {
            StreamEvent::TextDelta(delta) => streamed.push_str(&delta),
            StreamEvent::Done(Message::Assistant { blocks, .. }) => {
                for block in blocks {
                    if let Block::Text { text } = block {
                        finished.push_str(&text);
                    }
                }
            }
            StreamEvent::Error(text) => {
                error.get_or_insert(text);
            }
            _ => {}
        }
    }
    if let Err(err) = task.await? {
        error.get_or_insert_with(|| format!("{err:#}"));
    }
    if let Some(err) = error {
        return Err(anyhow!("summary request failed: {err}"));
    }

    let summary = if finished.trim().is_empty() {
        streamed
    } else {
        finished
    };
    if summary.trim().is_empty() {
        return Err(anyhow!("summary request returned no text"));
    }
    Ok(summary.trim().to_string())
}
