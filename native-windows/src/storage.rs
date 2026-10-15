//! Bounded, atomic aggregate configuration stores; damaged files are preserved.
use crate::remote::RemoteError;
use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct Store<T> {
    pub directory: PathBuf,
    name: &'static str,
    limit: usize,
    marker: std::marker::PhantomData<T>,
}
impl<T: Default + Serialize + DeserializeOwned> Store<T> {
    pub fn new(directory: PathBuf, name: &'static str, limit: usize) -> Self {
        Self {
            directory,
            name,
            limit,
            marker: std::marker::PhantomData,
        }
    }
    pub fn transaction<R>(
        &self,
        write: bool,
        operation: impl FnOnce(&mut T) -> Result<R, RemoteError>,
    ) -> Result<R, RemoteError> {
        let run = || -> Result<R, RemoteError> {
            fs::create_dir_all(&self.directory).map_err(|_| RemoteError::Storage)?;
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(self.directory.join(format!("{}.lock", self.name)))
                .map_err(|_| RemoteError::Storage)?;
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                match lock.try_lock() {
                    Ok(()) => break,
                    Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    _ => return Err(RemoteError::Storage),
                }
            }
            let path = self.directory.join(self.name);
            let mut data = match File::open(&path) {
                Ok(file) => {
                    if file.metadata().map_err(|_| RemoteError::Storage)?.len() > self.limit as u64
                    {
                        return Err(RemoteError::Storage);
                    }
                    let mut bytes = Vec::new();
                    file.take(self.limit as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| RemoteError::Storage)?;
                    if bytes.len() > self.limit {
                        return Err(RemoteError::Storage);
                    }
                    serde_json::from_slice(&bytes).map_err(|_| RemoteError::Storage)?
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => T::default(),
                Err(_) => return Err(RemoteError::Storage),
            };
            let value = operation(&mut data)?;
            if write {
                let bytes = serde_json::to_vec(&data).map_err(|_| RemoteError::Storage)?;
                if bytes.len() > self.limit {
                    return Err(RemoteError::Storage);
                }
                let temp = self.directory.join(format!(
                    "{}.{}.{}.tmp",
                    self.name,
                    std::process::id(),
                    nonce()
                ));
                let result = (|| -> std::io::Result<()> {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&temp)?;
                    file.write_all(&bytes)?;
                    file.sync_all()?;
                    drop(file);
                    replace(&temp, &path)
                })();
                if result.is_err() {
                    let _ = fs::remove_file(&temp);
                    return Err(RemoteError::Storage);
                }
            }
            Ok(value)
        };
        run()
    }
}
pub fn nonce() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    format!(
        "{:x}-{:x}-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
pub fn fingerprint(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf29ce484222325u64, |value, b| {
            (value ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        })
        .max(1)
}
#[cfg(windows)]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    unsafe {
        MoveFileExW(
            &windows::core::HSTRING::from(source.as_os_str()),
            &windows::core::HSTRING::from(destination.as_os_str()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(std::io::Error::other)
}
#[cfg(not(windows))]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupted_configuration_is_not_replaced_by_defaults() {
        let root = std::env::temp_dir().join(format!("lingomate-store-{}", nonce()));
        fs::create_dir_all(&root).unwrap();
        let store = Store::<Vec<String>>::new(root.clone(), "settings.json", 4096);
        let path = root.join("settings.json");
        fs::write(&path, b"damaged").unwrap();
        assert_eq!(
            store.transaction(true, |v| {
                v.push("new".into());
                Ok(())
            }),
            Err(RemoteError::Storage)
        );
        assert_eq!(fs::read(&path).unwrap(), b"damaged");
        fs::remove_file(&path).unwrap();
        store
            .transaction(true, |v| {
                v.push("saved".into());
                Ok(())
            })
            .unwrap();
        assert_eq!(
            store.transaction(false, |v| Ok(v.clone())).unwrap(),
            vec!["saved"]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
