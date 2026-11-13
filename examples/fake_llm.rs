//! Standalone fake OpenAI-compatible server for `scripts/tui-smoke.sh`.
//!
//! It reuses the in-process test server from `tests/support/mod.rs`, so the
//! wire format can never drift from the e2e tests. Usage:
//!
//!     cargo run --example fake_llm -- 18931 script.json
//!
//! It prints `listening on <port>` and runs until killed.

#[path = "../tests/support/mod.rs"]
mod support;

use std::fs;
use std::process;
use std::thread;
use std::time::Duration;

use serde_json::Value;

use support::{sse_text, sse_tool_call_json, FakeLlm, Reply};

fn main() {
    let mut args = std::env::args().skip(1);
    let port = match args.next().and_then(|text| text.parse::<u16>().ok()) {
        Some(port) => port,
        None => {
            eprintln!("usage: fake_llm <port> <script.json>");
            process::exit(2);
        }
    };
    let script = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("usage: fake_llm <port> <script.json>");
            process::exit(2);
        }
    };
    if let Err(err) = run(port, &script) {
        eprintln!("fake_llm: {err}");
        process::exit(1);
    }
}

fn run(port: u16, script_path: &str) -> Result<(), String> {
    let text = fs::read_to_string(script_path)
        .map_err(|err| format!("cannot read {script_path}: {err}"))?;
    let script: Value =
        serde_json::from_str(&text).map_err(|err| format!("{script_path} is not JSON: {err}"))?;
    let entries = script
        .as_array()
        .ok_or_else(|| format!("{script_path} must be a JSON array of replies"))?;

    let mut replies = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        replies.push(reply_from(entry, index)?);
    }

    let _fake =
        FakeLlm::start_on(port, replies).map_err(|err| format!("cannot bind {port}: {err}"))?;
    println!("listening on {port}");
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn reply_from(entry: &Value, index: usize) -> Result<Reply, String> {
    let kind = entry
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("reply {index}: missing kind"))?;
    match kind {
        "json" => {
            let body = entry.get("body").cloned().unwrap_or(Value::Null);
            Ok(Reply::status(200, &body.to_string()))
        }
        "status" => {
            let code = entry
                .get("code")
                .and_then(Value::as_u64)
                .and_then(|code| u16::try_from(code).ok())
                .ok_or_else(|| format!("reply {index}: status needs a code"))?;
            let body = match entry.get("body") {
                Some(Value::String(text)) => text.clone(),
                Some(value) => value.to_string(),
                None => String::new(),
            };
            Ok(Reply::status(code, &body))
        }
        "text" => {
            let pieces: Vec<String> = entry
                .get("pieces")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("reply {index}: text needs pieces"))?
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            let refs: Vec<&str> = pieces.iter().map(String::as_str).collect();
            Ok(Reply::sse(sse_text(&refs)))
        }
        "tool" => {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("reply {index}: tool needs a name"))?;
            let args = entry.get("args").cloned().unwrap_or(Value::Null);
            Ok(Reply::sse(sse_tool_call_json(
                &format!("call_{index}"),
                name,
                &args,
            )))
        }
        other => Err(format!("reply {index}: unknown kind {other:?}")),
    }
}
