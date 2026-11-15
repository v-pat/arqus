use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("arqus_atom_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_atomic_write_creates_file_and_parents() {
    let dir = temp_dir();
    let target = dir.join("nested").join("sub").join("file.txt");
    write_atomic(&target, b"hello atomic").unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello atomic");
    std::fs::remove_dir_all(&dir).ok();
}

#[cfg(unix)]
#[test]
fn test_atomic_write_replaces_existing_file_and_preserves_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir();
    let target = dir.join("exec.sh");
    std::fs::write(&target, "echo old").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();

    write_atomic(&target, b"echo new").unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "echo new");
    let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_atomic_write_leaves_no_temp_file_behind() {
    let dir = temp_dir();
    let target = dir.join("test.txt");
    write_atomic(&target, b"content v1").unwrap();
    write_atomic(&target, b"content v2").unwrap();

    let entries: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["test.txt".to_string()]);
    std::fs::remove_dir_all(&dir).ok();
}
