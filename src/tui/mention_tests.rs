use super::*;

fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

#[test]
fn token_at_finds_the_word_at_the_cursor() {
    let text = "please read @src/main.rs now";
    let chars = chars(text);
    let cursor = text.find(" now").unwrap();
    let (start, query) = token_at(&chars, cursor).unwrap();
    assert_eq!(start, 12);
    assert_eq!(query, "src/main.rs");
    assert_eq!(token_end(&chars, start), 24);
}

#[test]
fn token_at_rejects_plain_words_and_bare_at() {
    let at_alone = chars("hello @");
    assert!(token_at(&at_alone, 7).is_some());
    let words = chars("hello world");
    assert!(token_at(&words, 5).is_none());
    let email = chars("user@host");
    assert!(token_at(&email, 9).is_none());
}

#[test]
fn files_for_prefers_prefixes_and_filters() {
    let files = vec![
        "docs/guide.md".to_string(),
        "src/main.rs".to_string(),
        "src/mail.rs".to_string(),
        "tests/main.rs".to_string(),
    ];
    assert_eq!(
        files_for(&files, "mai", 2),
        vec!["src/main.rs".to_string(), "src/mail.rs".to_string()]
    );
    assert_eq!(files_for(&files, "MAI", 1), vec!["src/main.rs".to_string()]);
    assert_eq!(files_for(&files, "zzz", 4).len(), 0);
}

#[test]
fn mentioned_dedupes_and_trims_punctuation() {
    let text = "look at @src/a.rs and @src/a.rs, plus @b.rs.";
    assert_eq!(
        mentioned(text),
        vec!["src/a.rs".to_string(), "b.rs".to_string()]
    );
}

#[test]
fn expand_inlines_small_files_and_notes_large_ones() {
    let dir = std::env::temp_dir().join(format!("arqus-mention-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("small.txt"), "one\ntwo\n").unwrap();
    std::fs::write(dir.join("big.txt"), "x\n".repeat(5000)).unwrap();

    let out = expand("see @small.txt and @big.txt and @missing.txt", &dir);
    assert!(out.contains("<file path=\"small.txt\">\none\ntwo\n\n</file>"));
    assert!(out.contains("(file mentioned: big.txt, 5000 lines; use the read tool)"));
    assert!(out.contains("missing.txt"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expand_stops_inlining_after_three_files() {
    let dir = std::env::temp_dir().join(format!("arqus-mention-cap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for index in 0..4 {
        std::fs::write(dir.join(format!("f{index}.txt")), format!("file {index}\n")).unwrap();
    }
    let out = expand("@f0.txt @f1.txt @f2.txt @f3.txt", &dir);
    assert_eq!(out.matches("<file path=").count(), 3);
    assert!(out.contains("(file mentioned: f3.txt, 1 lines; use the read tool)"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expand_without_mentions_keeps_the_text() {
    let dir = std::env::temp_dir();
    assert_eq!(expand("plain prompt", &dir), "plain prompt");
}
