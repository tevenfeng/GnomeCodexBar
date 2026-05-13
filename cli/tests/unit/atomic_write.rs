use super::*;
use std::{fs, path::PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "codex-bar-{name}-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn test_atomic_write_content() {
    let dir = temp_dir("atomic-content");
    let path = dir.join("status.json");

    atomic_write(&path, b"hello").expect("atomic write");

    assert_eq!(fs::read_to_string(&path).expect("read back"), "hello");
    let _ = fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn test_atomic_write_creates_0600_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("atomic-mode");
    let path = dir.join("config.toml");

    atomic_write(&path, b"secret = true\n").expect("atomic write");

    let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let _ = fs::remove_dir_all(dir);
}
