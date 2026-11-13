//! Stage 14: nasty stream shapes. Black-box tests against the fake server.
//! No network, no real key: every run gets its own temp project and ARQUS_HOME.

mod support;
use support::*;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = fs::read_dir(dir) else {
        return out;
    };
    for entry in read {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(jsonl_files(&path));
        } else if path.extension().map(|x| x == "jsonl").unwrap_or(false) {
            out.push(path);
        }
    }
    out
}

fn session_entries(home: &Path) -> Vec<Value> {
    let files = jsonl_files(&home.join("sessions"));
    assert_eq!(files.len(), 1, "expected exactly one session file");
    let text = fs::read_to_string(&files[0]).expect("read session file");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("session line is JSON"))
        .collect()
}

fn assistant_entries(home: &Path) -> Vec<Value> {
    session_entries(home)
        .into_iter()
        .filter(|entry| entry["message"]["role"] == json!("assistant"))
        .collect()
}

fn last_assistant(home: &Path) -> Value {
    assistant_entries(home)
        .pop()
        .expect("at least one assistant entry")
}

fn finish_chunk(reason: &str) -> Value {
    json!({"id":"fake-1","object":"chat.completion.chunk",
           "choices":[{"index":0,"delta":{},"finish_reason":reason}]})
}

fn usage_with_cache(finish: &str, prompt: u64, completion: u64, cached: u64) -> Value {
    json!({"id":"fake-1","object":"chat.completion.chunk",
           "choices":[{"index":0,"delta":{"content":""},"finish_reason":finish}],
           "usage":{"prompt_tokens":prompt,"completion_tokens":completion,
                    "total_tokens":prompt+completion,
                    "prompt_tokens_details":{"cached_tokens":cached}}})
}

#[test]
fn event_cut_in_the_middle_of_data_prefix() {
    let project = TempDir::new("cut_data_project");
    let home = TempDir::new("cut_data_home");
    let mut chunks = Vec::new();
    for chunk in sse_text(&["Hello"]) {
        chunks.extend(split_chunk(chunk, 3));
    }
    let fake = FakeLlm::start(vec![Reply::Sse {
        chunks,
        delay_ms: 30,
    }]);

    let out = run_arqus(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Hello"), "stdout: {}", stdout(&out));
    assert_eq!(fake.requests().len(), 1, "unexpected extra requests");
}

#[test]
fn multi_byte_character_cut_across_writes() {
    let project = TempDir::new("utf8_project");
    let home = TempDir::new("utf8_home");
    let mut chunks = Vec::new();
    for chunk in sse_text(&["aé🚀b"]) {
        // Split right after the first byte of a multi-byte character.
        let at = chunk
            .iter()
            .position(|b| *b == 0xC3 || *b == 0xF0)
            .map(|p| p + 1)
            .unwrap_or(chunk.len() / 2);
        chunks.extend(split_chunk(chunk, at));
    }
    let fake = FakeLlm::start(vec![Reply::Sse {
        chunks,
        delay_ms: 30,
    }]);

    let out = run_arqus(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("aé🚀b"), "stdout: {text:?}");
    assert!(
        !text.contains('\u{FFFD}'),
        "replacement char in stdout: {text:?}"
    );
}

#[test]
fn comment_lines_are_ignored() {
    let project = TempDir::new("comment_project");
    let home = TempDir::new("comment_home");
    let mut chunks = vec![sse_comment("OPENROUTER PROCESSING")];
    for (index, chunk) in sse_text(&["Hello"]).into_iter().enumerate() {
        if index == 1 {
            chunks.push(sse_comment("OPENROUTER PROCESSING"));
        }
        chunks.push(chunk);
    }
    let fake = FakeLlm::start(vec![Reply::sse(chunks)]);

    let out = run_arqus(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Hello"), "stdout: {}", stdout(&out));
}

#[test]
fn crlf_line_endings_parse() {
    let project = TempDir::new("crlf_project");
    let home = TempDir::new("crlf_home");
    let fake = FakeLlm::start(vec![Reply::sse(to_crlf(sse_text(&["Hello"])))]);

    let out = run_arqus(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("Hello"), "stdout: {text:?}");
    assert!(!text.contains('\r'), "carriage returns leaked: {text:?}");
}

fn assert_usage_split(home: &Path) {
    let entry = last_assistant(home);
    assert_eq!(
        entry["message"]["usage"]["input"],
        json!(70),
        "entry: {entry}"
    );
    assert_eq!(entry["message"]["usage"]["cache_read"], json!(30));
    assert_eq!(entry["message"]["usage"]["output"], json!(20));
}

#[test]
fn openrouter_usage_chunk_splits_cached_tokens() {
    let project = TempDir::new("usage_or_project");
    let home = TempDir::new("usage_or_home");
    let fake = FakeLlm::start(vec![Reply::sse(vec![
        sse_event(&content_chunk("Hi")),
        sse_event(&finish_chunk("stop")),
        sse_event(&usage_with_cache("stop", 100, 20, 30)),
        sse_done(),
    ])]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_usage_split(&home.path);
}

#[test]
fn openai_empty_choices_usage_chunk_splits_cached_tokens() {
    let project = TempDir::new("usage_oai_project");
    let home = TempDir::new("usage_oai_home");
    let fake = FakeLlm::start(vec![Reply::sse(vec![
        sse_event(&content_chunk("Hi")),
        sse_event(&finish_chunk("stop")),
        sse_event(&json!({"id":"fake-1","choices":[],"usage":{
            "prompt_tokens":100,"completion_tokens":20,"total_tokens":120,
            "prompt_tokens_details":{"cached_tokens":30}}})),
        sse_done(),
    ])]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_usage_split(&home.path);
}

#[test]
fn stream_ends_after_finish_reason_without_done_is_success() {
    let project = TempDir::new("no_done_project");
    let home = TempDir::new("no_done_home");
    let fake = FakeLlm::start(vec![Reply::sse(vec![
        sse_event(&content_chunk("Hello")),
        sse_event(&finish_chunk("stop")),
    ])]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Hello"), "stdout: {}", stdout(&out));
    let entry = last_assistant(&home.path);
    assert_eq!(entry["message"]["stop"], json!("stop"), "entry: {entry}");
}

#[test]
fn stream_cut_without_finish_saves_partial_text_as_error() {
    let project = TempDir::new("cut_stream_project");
    let home = TempDir::new("cut_stream_home");
    let fake = FakeLlm::start(vec![Reply::sse(vec![sse_event(&content_chunk("par"))])]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    let err = stderr(&out);
    assert!(
        err.to_lowercase().contains("ended early"),
        "stderr: {err:?}"
    );
    let entry = last_assistant(&home.path);
    assert_eq!(entry["message"]["stop"], json!("error"), "entry: {entry}");
    assert!(
        entry["message"]["blocks"].to_string().contains("par"),
        "entry: {entry}"
    );
}

fn error_chunk() -> Value {
    json!({"error":{"message":"Provider disconnected","code":502},
           "choices":[{"index":0,"delta":{"content":""},"finish_reason":"error"}]})
}

#[test]
fn mid_stream_error_after_text_is_reported() {
    let project = TempDir::new("stream_err_project");
    let home = TempDir::new("stream_err_home");
    let fake = FakeLlm::start(vec![Reply::sse(vec![
        sse_event(&content_chunk("par")),
        sse_event(&error_chunk()),
    ])]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    let err = stderr(&out);
    assert!(err.contains("Provider disconnected"), "stderr: {err:?}");
    let entry = last_assistant(&home.path);
    assert_eq!(entry["message"]["stop"], json!("error"), "entry: {entry}");
}

#[test]
fn mid_stream_error_as_first_event_is_reported() {
    let project = TempDir::new("stream_err_first_project");
    let home = TempDir::new("stream_err_first_home");
    let fake = FakeLlm::start(vec![Reply::sse(vec![sse_event(&error_chunk())])]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    let err = stderr(&out);
    assert!(err.contains("Provider disconnected"), "stderr: {err:?}");
    let entry = last_assistant(&home.path);
    assert_eq!(entry["message"]["stop"], json!("error"), "entry: {entry}");
}

#[test]
fn empty_tool_arguments_are_an_empty_object() {
    let project = TempDir::new("empty_args_project");
    let home = TempDir::new("empty_args_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call("call_empty", "bash", &[""])),
        Reply::sse(sse_text(&["done"])),
    ]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(fake.requests().len(), 2, "unexpected extra requests");
    let with_call = assistant_entries(&home.path)
        .into_iter()
        .find(|entry| entry["message"]["blocks"].to_string().contains("tool_call"))
        .expect("an assistant entry with a tool call");
    let args = &with_call["message"]["blocks"][0]["args"];
    assert_eq!(
        args,
        &json!({}),
        "blocks: {}",
        with_call["message"]["blocks"]
    );
}

#[test]
fn invalid_tool_arguments_become_an_error_and_the_loop_continues() {
    let project = TempDir::new("bad_args_project");
    let home = TempDir::new("bad_args_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call("call_bad", "bash", &["{\"command\": \"ec"])),
        Reply::sse(sse_text(&["recovered"])),
    ]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("recovered"),
        "stdout: {}",
        stdout(&out)
    );
    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "unexpected extra requests");
    let body = requests[1].json();
    let tool = body["messages"]
        .as_array()
        .expect("messages array")
        .iter()
        .find(|m| m["role"] == json!("tool"))
        .expect("a tool message");
    let content = tool["content"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        content.contains("not valid json"),
        "tool content: {content}"
    );
}

#[test]
fn two_interleaved_tool_calls_both_run_in_index_order() {
    let project = TempDir::new("two_calls_project");
    let home = TempDir::new("two_calls_home");
    let call = |index: usize, id: &str, args: &str| {
        json!({"id":"fake-1","object":"chat.completion.chunk",
               "choices":[{"index":0,"delta":{"tool_calls":[
                   {"index":index,"id":id,"type":"function",
                    "function":{"name":"bash","arguments":args}}]},
                   "finish_reason":null}]})
    };
    let frag = |index: usize, args: &str| {
        json!({"id":"fake-1","object":"chat.completion.chunk",
               "choices":[{"index":0,"delta":{"tool_calls":[
                   {"index":index,"function":{"arguments":args}}]},
                   "finish_reason":null}]})
    };
    let fake = FakeLlm::start(vec![
        Reply::sse(vec![
            sse_event(&call(0, "call_a", "")),
            sse_event(&call(1, "call_b", "")),
            sse_event(&frag(0, "{\"command\": \"echo a")),
            sse_event(&frag(1, "{\"command\": \"echo b")),
            sse_event(&frag(0, " > a.txt\"}")),
            sse_event(&frag(1, " > b.txt\"}")),
            sse_event(&finish_chunk("tool_calls")),
            sse_done(),
        ]),
        Reply::sse(sse_text(&["done"])),
    ]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(project.path.join("a.txt").exists(), "a.txt missing");
    assert!(project.path.join("b.txt").exists(), "b.txt missing");
    assert_eq!(
        fs::read_to_string(project.path.join("a.txt"))
            .unwrap()
            .trim(),
        "a"
    );
    assert_eq!(
        fs::read_to_string(project.path.join("b.txt"))
            .unwrap()
            .trim(),
        "b"
    );
    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "unexpected extra requests");
    let body = requests[1].json();
    let ids: Vec<&str> = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == json!("tool"))
        .filter_map(|m| m["tool_call_id"].as_str())
        .collect();
    assert_eq!(ids, vec!["call_a", "call_b"], "tool order: {ids:?}");
}

#[test]
fn chunk_without_choices_is_ignored() {
    let project = TempDir::new("no_choices_project");
    let home = TempDir::new("no_choices_home");
    let fake = FakeLlm::start(vec![Reply::sse(vec![
        sse_event(&content_chunk("Hello")),
        sse_event(&json!({"id":"x"})),
        sse_event(&finish_chunk("stop")),
        sse_done(),
    ])]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Hello"), "stdout: {}", stdout(&out));
    assert_eq!(fake.requests().len(), 1, "unexpected extra requests");
}

#[test]
fn reasoning_fields_become_thinking_blocks_and_stay_off_stdout() {
    let project = TempDir::new("reasoning_project");
    let home = TempDir::new("reasoning_home");
    let fake = FakeLlm::start(vec![Reply::sse(vec![
        sse_event(&json!({"id":"fake-1","object":"chat.completion.chunk",
            "choices":[{"index":0,"delta":{"reasoning_content":"think one "},"finish_reason":null}]})),
        sse_event(&json!({"id":"fake-1","object":"chat.completion.chunk",
            "choices":[{"index":0,"delta":{"reasoning":"think two"},"finish_reason":null}]})),
        sse_event(&content_chunk("Hi")),
        sse_event(&finish_chunk("stop")),
        sse_done(),
    ])]);

    let out = run_arqus(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("Hi"), "stdout: {text:?}");
    assert!(
        !text.contains("think"),
        "reasoning leaked to stdout: {text:?}"
    );
    let entry = last_assistant(&home.path);
    let blocks = entry["message"]["blocks"].as_array().expect("blocks");
    let thinking: String = blocks
        .iter()
        .filter(|b| b["type"] == json!("thinking"))
        .filter_map(|b| b["text"].as_str())
        .collect();
    assert_eq!(thinking, "think one think two", "blocks: {blocks:?}");
}
