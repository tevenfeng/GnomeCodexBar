use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Atomically write bytes to `path` using a unique temp file in the same directory.
pub fn atomic_write(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "target path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;

    let temp_path = unique_temp_path(path);
    let result = (|| {
        let mut file = create_temp_file(&temp_path)?;
        file.write_all(contents.as_ref())?;
        file.flush()?;
        file.sync_all()?;
        drop(file);

        fs::rename(&temp_path, path)?;
        sync_parent_dir(parent);
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

fn unique_temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("atomic-write");
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{name}.{pid}.{nanos}.{counter}.tmp"))
}

#[cfg(unix)]
fn create_temp_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_temp_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn sync_parent_dir(parent: &Path) {
    if let Ok(dir) = File::open(parent) {
        let _ = dir.sync_all();
    }
}

#[cfg(test)]
#[path = "../tests/unit/atomic_write.rs"]
mod tests;
