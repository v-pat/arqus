//! The clipboard helper shared by `/copy` and later stages.
//!
//! It shells out because no clipboard crate is approved. Linux tries Wayland
//! first, then X11. When no program exists it writes an OSC 52 escape, which
//! most modern terminals accept.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

#[cfg(target_os = "macos")]
fn candidates() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![("pbcopy", vec![])]
}

#[cfg(not(target_os = "macos"))]
fn candidates() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("wl-copy", vec![]),
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["-b"]),
    ]
}

/// Put `text` on the system clipboard: a program first, OSC 52 last.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut last_error = String::from("no clipboard program found");
    for (program, args) in candidates() {
        match run(program, &args, text) {
            Ok(()) => return Ok(()),
            Err(err) => last_error = err.to_string(),
        }
    }
    let mut out = std::io::stdout();
    out.write_all(osc52(text).as_bytes())
        .with_context(|| format!("no clipboard program ({last_error}) and OSC 52 failed"))?;
    out.flush()?;
    Ok(())
}

/// `ESC ] 52 ; c ; <base64> BEL`, the mouse-free clipboard escape.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Small standard base64 encoder: 3 bytes to 4 symbols, `=` padding.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 63] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(triple >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[triple as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn run(program: &str, args: &[&str], text: &str) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("cannot start {program}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("{program} exited with {status}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
