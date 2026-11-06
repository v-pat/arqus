use crate::session::path_to_root;
use crate::types::*;

/// Pure function: session entries + leaf -> messages to send to the model.
pub fn build_context(entries: &[Entry], leaf: Option<&str>) -> Vec<Message> {
    let path = path_to_root(entries, leaf);

    // find the LAST compaction entry on the path
    let last_c = path
        .iter()
        .rposition(|e| matches!(e.kind, EntryKind::Compaction { .. }));

    let mut out: Vec<Message> = Vec::new();
    let start_of_kept;
    let after;
    match last_c {
        Some(ci) => {
            if let EntryKind::Compaction {
                summary,
                first_kept_id,
                read_files,
                modified_files,
                ..
            } = &path[ci].kind
            {
                let mut text = format!("[Summary of the earlier conversation]\n\n{summary}");
                if !read_files.is_empty() {
                    text.push_str(&format!(
                        "\n\n<read-files>\n{}\n</read-files>",
                        read_files.join("\n")
                    ));
                }
                if !modified_files.is_empty() {
                    text.push_str(&format!(
                        "\n\n<modified-files>\n{}\n</modified-files>",
                        modified_files.join("\n")
                    ));
                }
                out.push(Message::User { text });
                start_of_kept = path[..ci]
                    .iter()
                    .position(|e| &e.id == first_kept_id)
                    .unwrap_or(ci);
            } else {
                start_of_kept = 0;
            }
            after = ci + 1;
            for e in &path[start_of_kept..ci] {
                push_entry(&mut out, e);
            }
            for e in &path[after..] {
                push_entry(&mut out, e);
            }
        }
        None => {
            for e in &path {
                push_entry(&mut out, e);
            }
        }
    }
    sanitize(out)
}

fn push_entry(out: &mut Vec<Message>, e: &Entry) {
    match &e.kind {
        EntryKind::Message { message } => out.push(message.clone()),
        EntryKind::BranchSummary { summary, .. } => out.push(Message::User {
            text: format!("[Summary of another branch of this conversation]\n\n{summary}"),
        }),
        _ => {} // model_change, custom, compaction: not sent to the model
    }
}

/// Make the history legal for every provider:
/// every tool call needs a result right after it; results without a call are dropped;
/// failed/aborted assistant messages with no content are dropped.
pub fn sanitize(messages: Vec<Message>) -> Vec<Message> {
    let mut out: Vec<Message> = Vec::new();
    let mut pending: Vec<(String, String)> = Vec::new(); // (call_id, tool_name) not yet answered

    for m in messages {
        match &m {
            Message::ToolResult { call_id, .. } => {
                if let Some(pos) = pending.iter().position(|(id, _)| id == call_id) {
                    pending.remove(pos);
                    out.push(m);
                } // else: orphan result, drop it
                continue;
            }
            _ => {
                flush_pending(&mut out, &mut pending);
            }
        }
        if let Message::Assistant { blocks, stop, .. } = &m {
            if blocks.is_empty() && matches!(stop, StopReason::Error | StopReason::Aborted) {
                continue;
            }
            for b in blocks {
                if let Block::ToolCall { id, name, .. } = b {
                    pending.push((id.clone(), name.clone()));
                }
            }
        }
        out.push(m);
    }
    flush_pending(&mut out, &mut pending);
    out
}

fn flush_pending(out: &mut Vec<Message>, pending: &mut Vec<(String, String)>) {
    for (id, name) in pending.drain(..) {
        out.push(Message::ToolResult {
            call_id: id,
            tool_name: name,
            text: "No result: this tool call was interrupted.".to_string(),
            is_error: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(id: &str, parent: Option<&str>, kind: EntryKind) -> Entry {
        Entry {
            id: id.to_string(),
            parent_id: parent.map(|p| p.to_string()),
            ts: 0,
            kind,
        }
    }

    fn user(id: &str, parent: Option<&str>, text: &str) -> Entry {
        entry(
            id,
            parent,
            EntryKind::Message {
                message: Message::User {
                    text: text.to_string(),
                },
            },
        )
    }

    fn assistant(id: &str, parent: Option<&str>, blocks: Vec<Block>, stop: StopReason) -> Entry {
        entry(
            id,
            parent,
            EntryKind::Message {
                message: Message::Assistant {
                    blocks,
                    provider: "p".to_string(),
                    model: "m".to_string(),
                    usage: None,
                    stop,
                },
            },
        )
    }

    fn text(t: &str) -> Block {
        Block::Text {
            text: t.to_string(),
        }
    }

    fn call(id: &str) -> Block {
        Block::ToolCall {
            id: id.to_string(),
            name: "read".to_string(),
            args: json!({"path": "a.rs"}),
        }
    }

    fn result(id: &str, parent: &str) -> Entry {
        entry(
            id,
            Some(parent),
            EntryKind::Message {
                message: Message::ToolResult {
                    call_id: "c1".to_string(),
                    tool_name: "read".to_string(),
                    text: "ok".to_string(),
                    is_error: false,
                },
            },
        )
    }

    fn users(ctx: &[Message]) -> Vec<String> {
        ctx.iter()
            .filter_map(|m| match m {
                Message::User { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn plain_chat_keeps_messages_in_order() {
        let entries = vec![
            user("u1", None, "hi"),
            assistant("a2", Some("u1"), vec![text("hello")], StopReason::Stop),
            user("u3", Some("a2"), "bye"),
        ];
        let ctx = build_context(&entries, Some("u3"));
        assert_eq!(ctx.len(), 3);
        assert_eq!(
            ctx[0],
            Message::User {
                text: "hi".to_string()
            }
        );
        assert!(
            matches!(&ctx[1], Message::Assistant { blocks, .. } if blocks == &vec![text("hello")])
        );
        assert_eq!(
            ctx[2],
            Message::User {
                text: "bye".to_string()
            }
        );
    }

    #[test]
    fn branch_only_shows_leaf_path() {
        let entries = vec![
            user("u1", None, "shared"),
            user("b2", Some("u1"), "main branch"),
            user("b3", Some("u1"), "side branch"),
        ];
        let ctx = build_context(&entries, Some("b3"));
        assert_eq!(users(&ctx), vec!["shared", "side branch"]);
    }

    #[test]
    fn tool_call_without_result_gets_synthetic_error() {
        let entries = vec![
            user("u1", None, "go"),
            assistant("a2", Some("u1"), vec![call("c1")], StopReason::ToolUse),
            user("u3", Some("a2"), "still there?"),
        ];
        let ctx = build_context(&entries, Some("u3"));
        assert_eq!(ctx.len(), 4);
        match &ctx[2] {
            Message::ToolResult {
                call_id,
                is_error,
                text,
                ..
            } => {
                assert_eq!(call_id, "c1");
                assert!(is_error);
                assert!(text.contains("interrupted"));
            }
            other => panic!("expected synthetic tool result, got {other:?}"),
        }
        assert_eq!(users(&ctx), vec!["go", "still there?"]);
    }

    #[test]
    fn tool_result_without_call_is_dropped() {
        let entries = vec![user("u1", None, "hi"), result("r2", "u1")];
        let ctx = build_context(&entries, Some("r2"));
        assert_eq!(ctx.len(), 1);
        assert!(matches!(&ctx[0], Message::User { .. }));
    }

    #[test]
    fn compaction_summary_first_and_old_messages_gone() {
        let entries = vec![
            user("u1", None, "old user"),
            assistant("a2", Some("u1"), vec![text("old answer")], StopReason::Stop),
            user("u3", Some("a2"), "recent user"),
            assistant(
                "a4",
                Some("u3"),
                vec![text("recent answer")],
                StopReason::Stop,
            ),
            entry(
                "c5",
                Some("a4"),
                EntryKind::Compaction {
                    summary: "SUMMARY".to_string(),
                    first_kept_id: "u3".to_string(),
                    tokens_before: 1000,
                    read_files: vec!["src/a.rs".to_string()],
                    modified_files: vec!["src/b.rs".to_string()],
                },
            ),
            user("u6", Some("c5"), "after"),
        ];
        let ctx = build_context(&entries, Some("u6"));
        assert_eq!(ctx.len(), 4);
        match &ctx[0] {
            Message::User { text } => {
                assert!(text.contains("SUMMARY"));
                assert!(text.contains("<read-files>\nsrc/a.rs\n</read-files>"));
                assert!(text.contains("<modified-files>\nsrc/b.rs\n</modified-files>"));
            }
            other => panic!("expected summary user message, got {other:?}"),
        }
        let user_texts = users(&ctx);
        assert_eq!(user_texts.len(), 3);
        assert_eq!(user_texts[1], "recent user");
        assert_eq!(user_texts[2], "after");
        let all = format!("{ctx:?}");
        assert!(!all.contains("old user"));
        assert!(!all.contains("old answer"));
    }

    #[test]
    fn aborted_assistant_with_no_blocks_is_dropped() {
        let entries = vec![
            user("u1", None, "hi"),
            assistant("a2", Some("u1"), vec![], StopReason::Aborted),
            user("u3", Some("a2"), "again"),
        ];
        let ctx = build_context(&entries, Some("u3"));
        assert_eq!(users(&ctx), vec!["hi", "again"]);
        assert!(ctx
            .iter()
            .all(|m| !matches!(m, Message::Assistant { blocks, .. } if blocks.is_empty())));
    }
}
