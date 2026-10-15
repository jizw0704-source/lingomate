//! Shared local Chinese memory lives outside the MSIX AppData redirection boundary.
use std::path::{Path, PathBuf};
pub fn path() -> Option<PathBuf> {
    if std::env::var_os("LINGOMATE_TEST_SESSION").is_some() {
        return std::env::var_os("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("LingoMate/PersonalVocabulary/words.json"));
    }
    std::env::var_os("USERPROFILE")
        .filter(|p| Path::new(p).is_absolute())
        .map(|p| PathBuf::from(p).join(".lingomate/PersonalVocabulary/words.json"))
}
pub fn preserve_legacy(old: &Path, new: &Path) -> Result<(), crate::remote::RemoteError> {
    use crate::remote::RemoteError;
    use std::io::{Read, Write};
    if !old.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(new.parent().ok_or(RemoteError::Storage)?)
        .map_err(|_| RemoteError::Storage)?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(new.with_extension("lock"))
        .map_err(|_| RemoteError::Storage)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(350);
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(2))
            }
            Err(_) => return Err(RemoteError::Storage),
        }
    }
    if new.exists() {
        return Ok(());
    }
    let meta = std::fs::symlink_metadata(old).map_err(|_| RemoteError::Storage)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 4_000_000 {
        return Err(RemoteError::Storage);
    }
    let mut bytes = Vec::new();
    std::fs::File::open(old)
        .map_err(|_| RemoteError::Storage)?
        .take(4_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| RemoteError::Storage)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| RemoteError::Storage)?;
    if bytes.len() > 4_000_000
        || value.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        || value
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|v| v.len() > 5000)
    {
        return Err(RemoteError::Storage);
    }
    std::fs::create_dir_all(new.parent().ok_or(RemoteError::Storage)?)
        .map_err(|_| RemoteError::Storage)?;
    let temp = new.with_file_name(format!("migration-{}.tmp", crate::storage::nonce()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| RemoteError::Storage)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| RemoteError::Storage)?;
        drop(file);
        match std::fs::hard_link(&temp, new) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(_) => Err(RemoteError::Storage),
        }
    })();
    let _ = std::fs::remove_file(&temp);
    result
}
#[cfg(test)]
#[test]
fn migration_copies_once_and_preserves_both_old_and_existing_new_data() {
    let root =
        std::env::temp_dir().join(format!("lingomate-migration-{}", crate::storage::nonce()));
    std::fs::create_dir_all(&root).unwrap();
    let old = root.join("old.json");
    let new = root.join("new/words.json");
    let fixture = b"{\"version\":1,\"sequence\":0,\"entries\":[]}";
    std::fs::write(&old, fixture).unwrap();
    preserve_legacy(&old, &new).unwrap();
    assert_eq!(std::fs::read(&old).unwrap(), fixture);
    assert_eq!(std::fs::read(&new).unwrap(), fixture);
    std::fs::write(&old, b"bad").unwrap();
    preserve_legacy(&old, &new).unwrap();
    assert_eq!(std::fs::read(&new).unwrap(), fixture);
    std::fs::remove_file(&new).unwrap();
    assert!(preserve_legacy(&old, &new).is_err());
    assert!(!new.exists());
    std::fs::remove_dir_all(root).unwrap();
}
