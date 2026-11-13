//! Stage 17 live checks against a real OpenRouter model.
//!
//! Every test is `#[ignore]`, needs `ARQUS_LIVE=1`, `ARQUS_API_KEY` and
//! `ARQUS_LIVE_MODEL`, and costs real money. Run them with:
//!
//! `scripts/live-smoke.sh`
//!
//! The key is never printed. Each model call is capped at 300 output tokens
//! and every run must report a cost of at most 0.05 dollars.

mod live_helpers;
mod support;

use live_helpers::*;
use support::TempDir;

use std::fs;

// L1: a plain text reply.
#[test]
#[ignore]
fn live_l1_reply() {
    let Some(live) = live() else {
        return;
    };
    let project = TempDir::new("live_l1");
    let home = TempDir::new("live_l1_home");

    let output = run_live(
        &live,
        &project.path,
        &home.path,
        &["-p", "Reply with exactly: ARQUS_OK"],
    );
    ensure_exit_ok(&output, &home.path, "L1 reply");
    ensure(
        stdout(&output).contains("ARQUS_OK"),
        &home.path,
        format!("L1: stdout had no ARQUS_OK:\n{}", stdout(&output)),
    );
    ensure_cost(&output, &home.path, "L1 reply");
}

// L2: the bash tool runs in the project folder.
#[test]
#[ignore]
fn live_l2_tool() {
    let Some(live) = live() else {
        return;
    };
    let project = TempDir::new("live_l2");
    let home = TempDir::new("live_l2_home");

    let output = run_live(
        &live,
        &project.path,
        &home.path,
        &[
            "-p",
            "Use the bash tool to run: echo live-tool-test > live.txt , then say done.",
        ],
    );
    ensure_exit_ok(&output, &home.path, "L2 tool");
    let path = project.path.join("live.txt");
    let text = fs::read_to_string(&path).unwrap_or_default();
    ensure(
        text.trim() == "live-tool-test",
        &home.path,
        format!("L2: {:?} contains {:?}", path, text),
    );
    ensure_cost(&output, &home.path, "L2 tool");
}

// L3: the edit tool changes an existing line.
#[test]
#[ignore]
fn live_l3_edit() {
    let Some(live) = live() else {
        return;
    };
    let project = TempDir::new("live_l3");
    let home = TempDir::new("live_l3_home");
    let path = project.path.join("notes.txt");
    fs::write(&path, "colour=red\n").unwrap();

    let output = run_live(
        &live,
        &project.path,
        &home.path,
        &[
            "-p",
            "Use the edit tool to change colour=red to colour=blue in notes.txt, then say done.",
        ],
    );
    ensure_exit_ok(&output, &home.path, "L3 edit");
    let text = fs::read_to_string(&path).unwrap_or_default();
    ensure(
        text.contains("colour=blue") && !text.contains("colour=red"),
        &home.path,
        format!("L3: {:?} contains {:?}", path, text),
    );
    ensure_cost(&output, &home.path, "L3 edit");
}

// L4: `-c` resumes the same session and its history.
#[test]
#[ignore]
fn live_l4_resume() {
    let Some(live) = live() else {
        return;
    };
    let project = TempDir::new("live_l4");
    let home = TempDir::new("live_l4_home");

    let first = run_live(
        &live,
        &project.path,
        &home.path,
        &["-p", "Remember the secret word PELICAN. Reply: stored."],
    );
    ensure_exit_ok(&first, &home.path, "L4 first run");
    ensure_cost(&first, &home.path, "L4 first run");

    let second = run_live(
        &live,
        &project.path,
        &home.path,
        &["-c", "-p", "What was the secret word? One word only."],
    );
    ensure_exit_ok(&second, &home.path, "L4 second run");
    ensure(
        stdout(&second).contains("PELICAN"),
        &home.path,
        format!("L4: second stdout had no PELICAN:\n{}", stdout(&second)),
    );
    ensure_cost(&second, &home.path, "L4 second run");
}

// L5: `/budget` reads the key limit from OpenRouter.
#[test]
#[ignore]
fn live_l5_budget() {
    let Some(live) = live() else {
        return;
    };
    let project = TempDir::new("live_l5");
    let home = TempDir::new("live_l5_home");

    let output = run_live(&live, &project.path, &home.path, &["-p", "/budget"]);
    ensure_exit_ok(&output, &home.path, "L5 budget");
    ensure(
        stdout(&output).contains("limit_remaining"),
        &home.path,
        format!("L5: stdout had no limit_remaining:\n{}", stdout(&output)),
    );
    // `/budget` makes no model call, so no cost line is expected.
}

// L6: a small context window forces at least one compaction.
#[test]
#[ignore]
fn live_l6_compaction() {
    let Some(live) = live() else {
        return;
    };
    let project = TempDir::new("live_l6");
    let home = TempDir::new("live_l6_home");
    let settings_dir = project.path.join(".arqus");
    fs::create_dir_all(&settings_dir).unwrap();
    // The stage's 3000/500/800 cannot compact here: the system prompt and tool
    // schemas are about 850 tokens, and four 5-line replies add only about 19
    // estimated tokens each. A 1200-token window with 500 reserved is crossed
    // early, and 180 kept tokens leaves the first three turns in place while
    // the fourth is still worth summarizing.
    fs::write(
        settings_dir.join("settings.json"),
        r#"{"context_window":1200,"compaction":{"reserve_tokens":500,"keep_recent_tokens":180}}"#,
    )
    .unwrap();

    let prompts = [
        "Answer in exactly 5 short lines about the history of the bicycle: one line per era, starting with 1817, then the 1860s, 1870s, 1880s and 1900s. Name a place and a material in each line.",
        "Answer in exactly 5 short lines about the history of the telephone: one line per decade, from the 1870s to the 1910s. Mention a person and a place in each line.",
        "Answer in exactly 5 short lines about the history of radio: one line per decade, from the 1890s to the 1930s. Mention an inventor and a country in each line.",
        "Answer in exactly 5 short lines about the history of television: one line per decade, from the 1920s to the 1960s. Mention a company and a country in each line.",
    ];
    let mut total_requests = 0u64;
    for (index, prompt) in prompts.iter().enumerate() {
        let args: Vec<&str> = if index == 0 {
            vec!["-p", prompt]
        } else {
            vec!["-c", "-p", prompt]
        };
        let output = run_live(&live, &project.path, &home.path, &args);
        let label = format!("L6 run {}", index + 1);
        ensure_exit_ok(&output, &home.path, &label);
        ensure_cost(&output, &home.path, &label);
        let err = stderr(&output);
        let Some(count) = requests(&err) else {
            panic!("{label}: no requests= in stderr:\n{err}");
        };
        total_requests += count;
    }
    ensure(
        total_requests <= 6,
        &home.path,
        format!("L6: {total_requests} requests, cap is 6"),
    );

    let session = newest_session(&home.path);
    let text = fs::read_to_string(&session).unwrap_or_default();
    ensure(
        text.contains("\"type\":\"compaction\""),
        &home.path,
        format!("L6: no compaction entry in {:?}", session),
    );
}
