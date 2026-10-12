//! Confirmed local vocabulary only; never raw composition, host context or translations.
use qingjian_core::candidate::{Candidate, CandidateKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, DirBuilder, OpenOptions, TryLockError};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

const MAX_ENTRIES: usize = 5000;
const MAX_BYTES: u64 = 4 * 1024 * 1024;
pub type Preferences = HashMap<(String, Vec<String>), (u32, u64)>;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    pinyin: String,
    text: String,
    syllables: Vec<String>,
    count: u32,
    last: u64,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Data {
    version: u32,
    sequence: u64,
    entries: Vec<Entry>,
}

pub struct Memory {
    path: Option<PathBuf>,
    data: Data,
    pub warning: Option<&'static str>,
    dirty: Vec<(String, Candidate)>,
    fingerprint: Option<(SystemTime, u64)>,
}

pub fn normalize(input: &str) -> String {
    input.chars().filter(|ch| *ch != '\'').collect()
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 240 && key.bytes().all(|b| b.is_ascii_lowercase())
}

fn valid_word(text: &str, syllables: &[String]) -> bool {
    let count = text.chars().count();
    (1..=16).contains(&count)
        && count == syllables.len()
        && text
            .chars()
            .all(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
        && syllables.iter().all(|s| valid_key(s) && s.len() <= 6)
}

fn empty_data() -> Data {
    Data {
        version: 1,
        ..Data::default()
    }
}
fn fingerprint(path: &std::path::Path) -> Option<(SystemTime, u64)> {
    let meta = fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}
fn load_data(path: &std::path::Path) -> Result<Data, Box<dyn std::error::Error>> {
    let meta = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(empty_data()),
        other => other?,
    };
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err("invalid memory file".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("memory too large".into());
    }
    let data: Data = serde_json::from_slice(&bytes)?;
    if data.version != 1
        || data.entries.len() > MAX_ENTRIES
        || data.entries.iter().any(|e| {
            !valid_key(&e.pinyin)
                || !valid_word(&e.text, &e.syllables)
                || e.count == 0
                || e.last > data.sequence
        })
    {
        return Err("invalid memory".into());
    }
    Ok(data)
}

impl Memory {
    pub fn open(path: Option<PathBuf>) -> Self {
        let mut memory = Self {
            path,
            data: empty_data(),
            warning: None,
            dirty: Vec::new(),
            fingerprint: None,
        };
        if let Some(path) = &memory.path {
            let observed = fingerprint(path);
            match load_data(path) {
                Ok(data) => {
                    memory.data = data;
                    memory.fingerprint = observed;
                }
                Err(_) => {
                    memory.path = None;
                    memory.warning = Some("个人词库无法载入，原文件保留；本次不写入选词记忆。");
                }
            }
        }
        memory
    }

    pub fn refresh(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        let observed = fingerprint(path);
        if self.dirty.is_empty() && observed != self.fingerprint {
            match load_data(path) {
                Ok(data) => {
                    self.data = data;
                    self.fingerprint = observed;
                }
                Err(_) => {
                    self.fingerprint = observed;
                    self.warning = Some("个人词库暂时无法重读，原文件保留；记忆未保存。");
                }
            }
        }
    }

    pub fn enabled(&self) -> bool {
        self.path.is_some()
    }

    /// Usage age measures subsequent confirmed choices, not wall-clock time.
    pub fn preferences(&self, input: &str) -> Preferences {
        let key = normalize(input);
        self.data
            .entries
            .iter()
            .filter(|e| e.pinyin == key)
            .map(|e| {
                (
                    (e.text.clone(), e.syllables.clone()),
                    (e.count, self.data.sequence.saturating_sub(e.last)),
                )
            })
            .collect()
    }

    pub fn candidates(&self, input: &str) -> Vec<Candidate> {
        let key = normalize(input);
        let mut entries: Vec<_> = self
            .data
            .entries
            .iter()
            .filter(|e| e.pinyin == key)
            .collect();
        entries.sort_by(|a, b| b.count.cmp(&a.count).then(b.last.cmp(&a.last)));
        entries
            .into_iter()
            .map(|e| Candidate {
                text: e.text.clone(),
                kind: CandidateKind::Chinese,
                syllables: e.syllables.clone(),
                reading: None,
                translation: None,
                aux_code: None,
            })
            .collect()
    }

    pub fn remember(&mut self, input: &str, candidate: &Candidate) {
        if !self.enabled() || !valid_word(&candidate.text, &candidate.syllables) {
            return;
        }
        self.dirty.push((input.to_owned(), candidate.clone()));
        Self::record(&mut self.data, input, candidate);
    }

    fn record(data: &mut Data, input: &str, candidate: &Candidate) {
        let raw = normalize(input);
        let mut keys = vec![raw];
        if !keys.contains(&candidate.syllables.concat()) {
            keys.push(candidate.syllables.concat());
        }
        data.sequence = data.sequence.saturating_add(1);
        for key in keys.into_iter().filter(|key| valid_key(key)) {
            if let Some(entry) = data.entries.iter_mut().find(|e| {
                e.pinyin == key && e.text == candidate.text && e.syllables == candidate.syllables
            }) {
                entry.count = entry.count.saturating_add(1);
                entry.last = data.sequence;
            } else {
                data.entries.push(Entry {
                    pinyin: key,
                    text: candidate.text.clone(),
                    syllables: candidate.syllables.clone(),
                    count: 1,
                    last: data.sequence,
                });
            }
        }
        if data.entries.len() > MAX_ENTRIES {
            data.entries.sort_by_key(|e| std::cmp::Reverse(e.last));
            data.entries.truncate(MAX_ENTRIES);
        }
    }

    pub fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let dirty = std::mem::take(&mut self.dirty);
        if dirty.is_empty() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let parent = path.parent().ok_or("missing parent")?;
            let mut directory = DirBuilder::new();
            directory.recursive(true);
            #[cfg(unix)]
            directory.mode(0o700);
            directory.create(parent)?;
            let mut lock_options = OpenOptions::new();
            lock_options
                .read(true)
                .write(true)
                .create(true)
                .truncate(false);
            #[cfg(unix)]
            lock_options.mode(0o600);
            let lock = lock_options.open(path.with_extension("lock"))?;
            let started = Instant::now();
            loop {
                match lock.try_lock() {
                    Ok(()) => break,
                    Err(TryLockError::WouldBlock)
                        if started.elapsed() < Duration::from_millis(150) =>
                    {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => return Err(std::io::Error::from(error).into()),
                }
            }
            let mut latest = load_data(&path)?;
            for (input, candidate) in &dirty {
                Self::record(&mut latest, input, candidate);
            }
            let temporary = parent.join(format!(
                ".memory-{}-{}.tmp",
                std::process::id(),
                latest.sequence
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options.open(&temporary)?;
            let write = (|| -> Result<(), Box<dyn std::error::Error>> {
                file.write_all(&serde_json::to_vec(&latest)?)?;
                file.sync_all()?;
                replace_file(&temporary, &path)?;
                Ok(())
            })();
            if write.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            write?;
            self.data = latest;
            self.fingerprint = fingerprint(&path);
            Ok(())
        })();
        self.warning = result
            .err()
            .map(|_| "文字已输出，选词记忆暂未保存；请检查本机存储。");
    }
}

#[cfg(not(windows))]
fn replace_file(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn replace_file(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(source: *const u16, target: *const u16, flags: u32) -> i32;
    }
    let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // Replace atomically; never delete the previous vocabulary before a successful save.
    if unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), 0x1 | 0x8) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
