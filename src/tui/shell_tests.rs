use super::*;

#[test]
fn local_command_needs_a_bang_and_a_command() {
    assert_eq!(local_command("!ls -l"), Some("ls -l".to_string()));
    assert_eq!(
        local_command("  !git status"),
        Some("git status".to_string())
    );
    assert_eq!(local_command("!"), None);
    assert_eq!(local_command("echo hi"), None);
    assert_eq!(local_command("/compact"), None);
}

#[test]
fn message_wraps_output_in_a_code_block() {
    let text = message("ls", "a\nb\n");
    assert_eq!(text, "I ran `ls` locally. Output:\n\n```text\na\nb\n```");
}

#[test]
fn message_keeps_empty_output_readable() {
    assert_eq!(
        message("true", ""),
        "I ran `true` locally. Output:\n\n```text\n\n```"
    );
}
