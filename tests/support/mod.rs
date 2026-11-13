//! Fake OpenAI-compatible LLM server for black-box tests. Standard library only: no async, no extra crates.
//! Put this file at tests/support/mod.rs and add `mod support;` at the top of each tests/*.rs file.
#![allow(dead_code)]

use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// What the fake server does for ONE incoming request. Replies are used in order.
#[derive(Clone, Debug)]
pub enum Reply {
    /// 200 + text/event-stream. Each chunk is written and flushed separately, with a pause between.
    Sse { chunks: Vec<Vec<u8>>, delay_ms: u64 },
    /// A plain HTTP response (use for 401, 402, 429, 500 ...).
    Http {
        status: u16,
        body: String,
        headers: Vec<(String, String)>,
    },
    /// Read the request, then sleep without answering (for timeout tests).
    Hang { secs: u64 },
    /// Read the request, then close the connection without answering.
    Drop,
}

impl Reply {
    pub fn sse(chunks: Vec<Vec<u8>>) -> Reply {
        Reply::Sse {
            chunks,
            delay_ms: 5,
        }
    }
    pub fn status(status: u16, body: &str) -> Reply {
        Reply::Http {
            status,
            body: body.to_string(),
            headers: vec![],
        }
    }
    pub fn status_with(status: u16, body: &str, headers: &[(&str, &str)]) -> Reply {
        Reply::Http {
            status,
            body: body.to_string(),
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}

/// One request the server received.
#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>, // header names are lowercased
    pub body: String,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

pub struct FakeLlm {
    /// Use as ARQUS_BASE_URL, for example http://127.0.0.1:41233/v1
    pub url: String,
    recorded: Arc<Mutex<Vec<Recorded>>>,
}

impl FakeLlm {
    pub fn start(replies: Vec<Reply>) -> FakeLlm {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        FakeLlm::serve(listener, replies)
    }

    /// Bind one explicit port. Used by `examples/fake_llm.rs`, which prints the
    /// port so a shell script can point `ARQUS_BASE_URL` at it.
    pub fn start_on(port: u16, replies: Vec<Reply>) -> std::io::Result<FakeLlm> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        Ok(FakeLlm::serve(listener, replies))
    }

    fn serve(listener: TcpListener, replies: Vec<Reply>) -> FakeLlm {
        let port = listener.local_addr().unwrap().port();
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let queue = Arc::new(Mutex::new(VecDeque::from(replies)));
        let rec = recorded.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let stream = match stream {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let rec = rec.clone();
                let queue = queue.clone();
                thread::spawn(move || handle(stream, rec, queue));
            }
        });
        FakeLlm {
            url: format!("http://127.0.0.1:{port}/v1"),
            recorded,
        }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.recorded.lock().unwrap().clone()
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn read_request(stream: &mut TcpStream) -> Option<Recorded> {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end;
    loop {
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let first = lines.next()?;
    let mut parts = first.split(' ');
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_lowercase();
            let v = v.trim().to_string();
            if k == "content-length" {
                content_length = v.parse().unwrap_or(0);
            }
            headers.push((k, v));
        }
    }
    while buf.len() < header_end + content_length {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let end = (header_end + content_length).min(buf.len());
    let body = String::from_utf8_lossy(&buf[header_end..end]).to_string();
    Some(Recorded {
        method,
        path,
        headers,
        body,
    })
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn handle(
    mut stream: TcpStream,
    rec: Arc<Mutex<Vec<Recorded>>>,
    queue: Arc<Mutex<VecDeque<Reply>>>,
) {
    let _ = stream.set_nodelay(true);
    let req = match read_request(&mut stream) {
        Some(r) => r,
        None => return,
    };
    rec.lock().unwrap().push(req);
    let reply = queue.lock().unwrap().pop_front().unwrap_or(Reply::Http {
        status: 500,
        body: "{\"error\":{\"message\":\"fake server: no more scripted replies\"}}".to_string(),
        headers: vec![],
    });
    match reply {
        Reply::Sse { chunks, delay_ms } => {
            let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n";
            if stream.write_all(head.as_bytes()).is_err() {
                return;
            }
            let _ = stream.flush();
            for c in chunks {
                if stream.write_all(&c).is_err() {
                    return;
                }
                let _ = stream.flush();
                thread::sleep(Duration::from_millis(delay_ms));
            }
        }
        Reply::Http {
            status,
            body,
            headers,
        } => {
            let mut head = format!(
                "HTTP/1.1 {status} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
                reason(status),
                body.len()
            );
            for (k, v) in headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(body.as_bytes());
            let _ = stream.flush();
        }
        Reply::Hang { secs } => thread::sleep(Duration::from_secs(secs)),
        Reply::Drop => {}
    }
    // dropping the stream closes the connection
}

// ---------- builders for OpenAI-style SSE bodies ----------

pub fn sse_event(value: &Value) -> Vec<u8> {
    format!("data: {}\n\n", value).into_bytes()
}

pub fn sse_done() -> Vec<u8> {
    b"data: [DONE]\n\n".to_vec()
}

pub fn content_chunk(text: &str) -> Value {
    json!({"id":"fake-1","object":"chat.completion.chunk",
           "choices":[{"index":0,"delta":{"content":text},"finish_reason":null}]})
}

/// OpenRouter-style final accounting chunk: a content-free delta that repeats finish_reason, plus usage.
pub fn usage_chunk(finish: &str, prompt: u64, completion: u64, cost: f64) -> Value {
    json!({"id":"fake-1","object":"chat.completion.chunk",
           "choices":[{"index":0,"delta":{"content":""},"finish_reason":finish}],
           "usage":{"prompt_tokens":prompt,"completion_tokens":completion,"total_tokens":prompt+completion,
                    "cost":cost,"prompt_tokens_details":{"cached_tokens":0}}})
}

/// A complete text reply: one event per piece, finish chunk, usage chunk, [DONE].
pub fn sse_text(pieces: &[&str]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = pieces
        .iter()
        .map(|p| sse_event(&content_chunk(p)))
        .collect();
    out.push(sse_event(
        &json!({"id":"fake-1","object":"chat.completion.chunk",
        "choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
    ));
    out.push(sse_event(&usage_chunk("stop", 100, 20, 0.0001)));
    out.push(sse_done());
    out
}

/// A tool-call reply. `arg_pieces` joined together must be the JSON arguments string.
pub fn sse_tool_call(call_id: &str, name: &str, arg_pieces: &[&str]) -> Vec<Vec<u8>> {
    let mut out = vec![sse_event(
        &json!({"id":"fake-1","object":"chat.completion.chunk",
        "choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[
            {"index":0,"id":call_id,"type":"function","function":{"name":name,"arguments":""}}]},
            "finish_reason":null}]}),
    )];
    for p in arg_pieces {
        out.push(sse_event(
            &json!({"id":"fake-1","object":"chat.completion.chunk",
            "choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":p}}]},
                "finish_reason":null}]}),
        ));
    }
    out.push(sse_event(
        &json!({"id":"fake-1","object":"chat.completion.chunk",
        "choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ));
    out.push(sse_event(&usage_chunk("tool_calls", 100, 20, 0.0001)));
    out.push(sse_done());
    out
}

/// Shortcut: a tool call whose arguments are one JSON value.
pub fn sse_tool_call_json(call_id: &str, name: &str, args: &Value) -> Vec<Vec<u8>> {
    let s = args.to_string();
    sse_tool_call(call_id, name, &[s.as_str()])
}

// ---------- temp folders and running the real binary ----------

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> TempDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("arqus_test_{label}_{}_{n}_{t}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        TempDir { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Run the real compiled `arqus` binary against the fake server.
/// `cwd` is the project folder, `home` becomes ARQUS_HOME. Any ARQUS_* variables of your own shell are removed first.
/// Adds `--yolo`, so tools run without approval like before stage 22.
pub fn run_arqus(
    cwd: &Path,
    home: &Path,
    base_url: &str,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Output {
    run_arqus_extra(cwd, home, base_url, args, extra_env, true)
}

/// Same as `run_arqus` but keeps the default approval mode (`ask`).
pub fn run_arqus_without_yolo(
    cwd: &Path,
    home: &Path,
    base_url: &str,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Output {
    run_arqus_extra(cwd, home, base_url, args, extra_env, false)
}

fn run_arqus_extra(
    cwd: &Path,
    home: &Path,
    base_url: &str,
    args: &[&str],
    extra_env: &[(&str, &str)],
    yolo: bool,
) -> Output {
    let mut cmd = arqus_command(cwd, home, base_url, yolo, extra_env);
    cmd.args(args).output().expect("failed to run arqus binary")
}

/// A configured `arqus` command, for tests that need a child process they can kill.
pub fn arqus_command(
    cwd: &Path,
    home: &Path,
    base_url: &str,
    yolo: bool,
    extra_env: &[(&str, &str)],
) -> Command {
    let mut cmd = Command::new(
        std::env::var("CARGO_BIN_EXE_arqus")
            .expect("CARGO_BIN_EXE_arqus; run through cargo test or cargo run --example"),
    );
    for (k, _) in std::env::vars() {
        if k.starts_with("ARQUS_") {
            cmd.env_remove(k);
        }
    }
    cmd.current_dir(cwd)
        .env("ARQUS_HOME", home)
        .env("ARQUS_API_KEY", "test-key-123")
        .env("ARQUS_PROVIDER", "openai")
        .env("ARQUS_MODEL", "fake-model")
        .env("ARQUS_BASE_URL", base_url)
        .env("ARQUS_RETRY_BASE_MS", "10");
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    if yolo {
        cmd.arg("--yolo");
    }
    cmd
}

pub fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

pub fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

// ---------- helpers for nasty stream shapes ----------

/// An SSE comment line, like OpenRouter's keep-alive.
pub fn sse_comment(text: &str) -> Vec<u8> {
    format!(": {text}\n\n").into_bytes()
}

/// Cut ONE chunk into two writes at byte index `at` (for example in the middle of "data:" or in the middle of a UTF-8 character).
pub fn split_chunk(chunk: Vec<u8>, at: usize) -> Vec<Vec<u8>> {
    let at = at.min(chunk.len());
    vec![chunk[..at].to_vec(), chunk[at..].to_vec()]
}

/// Replace "\n" with "\r\n" in every chunk (CRLF line endings).
pub fn to_crlf(chunks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    chunks
        .into_iter()
        .map(|c| {
            String::from_utf8_lossy(&c)
                .replace('\n', "\r\n")
                .into_bytes()
        })
        .collect()
}
