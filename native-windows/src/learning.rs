//! Offline aggregate word records. No host context, account identity or input history.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Word {
    pub english: String,
    pub chinese: String,
    pub pos: String,
    pub uses: u32,
    pub mastered: bool,
}
impl Word {
    pub fn new(english: String, chinese: String, pos: String) -> Self {
        Self {
            english,
            chinese,
            pos,
            uses: 1,
            mastered: false,
        }
    }
    pub fn id(&self) -> String {
        format!(
            "{}|{}|{}",
            self.english.to_ascii_lowercase(),
            self.chinese,
            self.pos
        )
    }
    pub fn valid(&self) -> bool {
        (1..=80).contains(&self.english.len())
            && self
                .english
                .bytes()
                .all(|b| (32..127).contains(&b) && b != b'|')
            && (1..=16).contains(&self.chinese.chars().count())
            && self
                .chinese
                .chars()
                .all(|c| ('\u{3400}'..='\u{9FFF}').contains(&c))
            && self.pos.len() <= 24
            && self
                .pos
                .bytes()
                .all(|b| (32..127).contains(&b) && b != b'|')
    }
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    version: u32,
    words: Vec<Word>,
}

pub struct Store {
    directory: PathBuf,
}
impl Store {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    fn transaction<T>(
        &self,
        write: bool,
        action: impl FnOnce(&mut Data) -> Result<T, String>,
    ) -> Result<T, String> {
        let run = || -> Result<T, Box<dyn std::error::Error>> {
            fs::create_dir_all(&self.directory)?;
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(self.directory.join("words.lock"))?;
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                match lock.try_lock() {
                    Ok(()) => break,
                    Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            // Dropping the lock file releases the cross-process lock on all exit paths.
            let path = self.directory.join("words.json");
            let mut data = Data {
                version: 1,
                words: vec![],
            };
            match fs::File::open(&path) {
                Ok(file) => {
                    if file.metadata()?.len() > 4_000_000 {
                        return Err("Record file too large".into());
                    }
                    let mut bytes = Vec::new();
                    file.take(4_000_001).read_to_end(&mut bytes)?;
                    if bytes.len() > 4_000_000 {
                        return Err("Record file too large".into());
                    }
                    data = serde_json::from_slice(&bytes)?;
                    let mut ids = HashSet::new();
                    if data.version != 1
                        || data.words.len() > 5000
                        || data.words.iter().any(|w| {
                            !w.valid()
                                || w.uses == 0
                                || w.uses > 1_000_000_000
                                || !ids.insert(w.id())
                        })
                    {
                        return Err("Invalid word records".into());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            let result = action(&mut data)?;
            if write {
                let bytes = serde_json::to_vec(&data)?;
                if bytes.len() > 4_000_000 {
                    return Err("Record file too large".into());
                }
                let temp = self.directory.join(format!(
                    "words-{}-{}.tmp",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_nanos()
                ));
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temp)?;
                let saved = (|| -> std::io::Result<()> {
                    file.write_all(&bytes)?;
                    file.sync_all()?;
                    drop(file);
                    replace(&temp, &path)
                })();
                if saved.is_err() {
                    let _ = fs::remove_file(&temp);
                }
                saved?;
            }
            Ok(result)
        };
        run().map_err(|_| "本地学习记录无法读取或保存，原文件已保留。请检查存储权限后刷新。".into())
    }
    pub fn snapshot(&self) -> Result<Vec<Word>, String> {
        self.transaction(false, |d| {
            let mut words = d.words.clone();
            words.sort_by_key(|w| {
                (
                    w.english.to_ascii_lowercase(),
                    w.chinese.clone(),
                    w.pos.clone(),
                )
            });
            Ok(words)
        })
    }
    pub fn record(&self, word: Word) -> Result<(), String> {
        if !word.valid() {
            return Err("Invalid word".into());
        }
        self.transaction(true, |d| {
            if let Some(existing) = d.words.iter_mut().find(|w| w.id() == word.id()) {
                existing.uses = existing.uses.saturating_add(1).min(1_000_000_000);
            } else {
                if d.words.len() >= 5000 {
                    return Err("Record limit reached".into());
                }
                d.words.push(Word {
                    uses: 1,
                    mastered: false,
                    ..word
                });
            }
            Ok(())
        })
    }
    pub fn mastered(&self, id: &str, value: bool) -> Result<(), String> {
        self.transaction(true, |d| {
            d.words
                .iter_mut()
                .find(|w| w.id() == id)
                .ok_or("Word no longer exists")?
                .mastered = value;
            Ok(())
        })
    }
    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.transaction(true, |d| {
            d.words.retain(|w| w.id() != id);
            Ok(())
        })
    }
}
#[cfg(not(windows))]
fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}
#[cfg(windows)]
fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn csv(words: &[Word]) -> Vec<u8> {
    fn cell(text: &str) -> String {
        let prefix = if text.trim_start().starts_with(['=', '+', '-', '@']) {
            "'"
        } else {
            ""
        };
        format!("\"{prefix}{}\"", text.replace('"', "\"\""))
    }
    let mut text = String::from("\u{FEFF}English,Chinese,POS,Uses,Mastered\r\n");
    for word in words {
        text.push_str(
            &[
                word.english.clone(),
                word.chinese.clone(),
                word.pos.clone(),
                word.uses.to_string(),
                word.mastered.to_string(),
            ]
            .iter()
            .map(|v| cell(v))
            .collect::<Vec<_>>()
            .join(","),
        );
        text.push_str("\r\n");
    }
    text.into_bytes()
}
#[cfg(windows)]
pub fn local_store() -> Result<Store, String> {
    let root = std::env::var_os("LOCALAPPDATA")
        .filter(|p| Path::new(p).is_absolute())
        .ok_or("无法找到本地用户存储目录。")?;
    Ok(Store::new(
        PathBuf::from(root).join("LingoMate").join("LocalWords"),
    ))
}
#[cfg(windows)]
pub static SAVE_WARNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(windows)]
pub fn confirmed(word: Word) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PENDING: AtomicUsize = AtomicUsize::new(0);
    if PENDING.fetch_add(1, Ordering::SeqCst) >= 128 {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        SAVE_WARNING.store(true, Ordering::Relaxed);
        return;
    }
    // Hold a COM DLL lifetime reference before spawning; release only after disk work ends.
    let guard = crate::tsf::Guard::new();
    let result = std::thread::Builder::new()
        .name("lingomate-learning".into())
        .spawn(move || {
            let _guard = guard;
            if local_store().and_then(|store| store.record(word)).is_err() {
                SAVE_WARNING.store(true, Ordering::Relaxed);
            }
            PENDING.fetch_sub(1, Ordering::SeqCst);
        });
    if result.is_err() {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        SAVE_WARNING.store(true, Ordering::Relaxed);
    }
}

/// The only learning boundary: the entire host edit operation must succeed.
pub fn after_output<T, E>(
    result: Result<T, E>,
    commit: Option<&str>,
    study: Option<Word>,
    save: impl FnOnce(Word),
) -> Result<T, E> {
    if result.is_ok()
        && let Some(word) = study.filter(|w| w.valid() && commit == Some(w.english.as_str()))
    {
        save(word);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_failure_and_mismatched_output_never_create_records() {
        let root = temp();
        let store = Store::new(root.clone());
        let save = |word| store.record(word).unwrap();
        assert!(
            after_output(
                Err::<(), _>("host rejected"),
                Some("learn"),
                Some(word()),
                save
            )
            .is_err()
        );
        after_output(Ok::<_, ()>(()), Some("AI"), Some(word()), save).unwrap();
        after_output(Ok::<_, ()>(()), None, Some(word()), save).unwrap();
        assert!(store.snapshot().unwrap().is_empty());
        after_output(Ok::<_, ()>(()), Some("learn"), Some(word()), save).unwrap();
        assert_eq!(store.snapshot().unwrap()[0].uses, 1);
        fs::remove_dir_all(root).unwrap();
    }
    fn temp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "lingomate-learning-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    fn word() -> Word {
        Word::new("learn".into(), "学习".into(), "v.".into())
    }
    #[test]
    fn aggregate_manual_mastery_restart_delete_and_safe_export() {
        let root = temp();
        let store = Store::new(root.clone());
        let w = word();
        assert!(store.snapshot().unwrap().is_empty());
        store.record(w.clone()).unwrap();
        store.record(w.clone()).unwrap();
        store.mastered(&w.id(), true).unwrap();
        store.record(w.clone()).unwrap();
        let words = Store::new(root.clone()).snapshot().unwrap();
        assert_eq!(words[0].uses, 3);
        assert!(words[0].mastered);
        assert!(
            String::from_utf8(csv(&words))
                .unwrap()
                .contains("\"learn\",\"学习\",\"v.\",\"3\",\"true\"")
        );
        assert!(
            String::from_utf8(csv(&[Word::new("=value".into(), "示例".into(), "".into())]))
                .unwrap()
                .contains("\"'=value\"")
        );
        store.delete(&w.id()).unwrap();
        assert!(store.snapshot().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn concurrent_counts_preserve_mastery_and_corruption_is_not_overwritten() {
        let root = temp();
        let store = Store::new(root.clone());
        store.record(word()).unwrap();
        store.mastered(&word().id(), true).unwrap();
        let workers: Vec<_> = (0..20)
            .map(|_| {
                let root = root.clone();
                std::thread::spawn(move || Store::new(root).record(word()).unwrap())
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let words = store.snapshot().unwrap();
        assert_eq!(words[0].uses, 21);
        assert!(words[0].mastered);
        for bytes in [
            b"{bad".as_slice(),
            b"{\"version\":2,\"words\":[]}".as_slice(),
            b"{\"version\":1,\"words\":[],\"host\":\"secret\"}".as_slice(),
        ] {
            fs::write(root.join("words.json"), bytes).unwrap();
            assert!(store.record(word()).is_err());
            assert_eq!(fs::read(root.join("words.json")).unwrap(), bytes);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
