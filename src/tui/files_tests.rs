use super::*;

fn temp_root(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("arqus-files-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn walk_skips_hidden_and_build_folders() {
    let root = temp_root("walk");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join(".secret")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::create_dir_all(root.join("node_modules")).unwrap();
    std::fs::write(root.join("src/main.rs"), "").unwrap();
    std::fs::write(root.join("README.md"), "").unwrap();
    std::fs::write(root.join(".secret/key"), "").unwrap();
    std::fs::write(root.join("target/out"), "").unwrap();
    std::fs::write(root.join("node_modules/pkg"), "").unwrap();

    let files = scan(&root);
    assert_eq!(
        files,
        vec!["README.md".to_string(), "src/main.rs".to_string()]
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scan_on_an_empty_folder_is_empty() {
    let root = temp_root("empty");
    assert!(scan(&root).is_empty());
    let _ = std::fs::remove_dir_all(&root);
}
