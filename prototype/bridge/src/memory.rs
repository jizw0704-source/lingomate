//! Confirmed local vocabulary only; never raw composition, host context or translations.
use qingjian_core::candidate::{Candidate, CandidateKind};
use serde::{Deserialize, Serialize};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

const MAX_ENTRIES: usize = 5000;
const MAX_BYTES: u64 = 4 * 1024 * 1024;

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

impl Memory {
    pub fn open(path: Option<PathBuf>) -> Self {
        let mut memory = Self {
            path,
            data: Data {
                version: 1,
                ..Data::default()
            },
            warning: None,
        };
        if let Some(path) = &memory.path {
            let loaded = (|| -> Result<Option<Data>, Box<dyn std::error::Error>> {
                match fs::metadata(path) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(e.into()),
                    Ok(meta) if meta.len() > MAX_BYTES => return Err("memory too large".into()),
                    Ok(_) => {}
                }
                let data: Data = serde_json::from_slice(&fs::read(path)?)?;
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
                Ok(Some(data))
            })();
            match loaded {
                Ok(Some(data)) => memory.data = data,
                Ok(None) => {}
                Err(_) => {
                    memory.path = None;
                    memory.warning = Some("个人词库无法载入，原文件保留；本次不写入选词记忆。");
                }
            }
        }
        memory
    }

    pub fn enabled(&self) -> bool {
        self.path.is_some()
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
        let raw = normalize(input);
        let mut keys = vec![raw];
        if !keys.contains(&candidate.syllables.concat()) {
            keys.push(candidate.syllables.concat());
        }
        self.data.sequence = self.data.sequence.saturating_add(1);
        for key in keys.into_iter().filter(|key| valid_key(key)) {
            if let Some(entry) = self.data.entries.iter_mut().find(|e| {
                e.pinyin == key && e.text == candidate.text && e.syllables == candidate.syllables
            }) {
                entry.count = entry.count.saturating_add(1);
                entry.last = self.data.sequence;
            } else {
                self.data.entries.push(Entry {
                    pinyin: key,
                    text: candidate.text.clone(),
                    syllables: candidate.syllables.clone(),
                    count: 1,
                    last: self.data.sequence,
                });
            }
        }
        if self.data.entries.len() > MAX_ENTRIES {
            self.data.entries.sort_by_key(|e| std::cmp::Reverse(e.last));
            self.data.entries.truncate(MAX_ENTRIES);
        }
    }

    pub fn save(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let parent = path.parent().ok_or("missing parent")?;
            let mut directory = DirBuilder::new();
            directory.recursive(true);
            #[cfg(unix)]
            directory.mode(0o700);
            directory.create(parent)?;
            let temporary = parent.join(format!(
                ".memory-{}-{}.tmp",
                std::process::id(),
                self.data.sequence
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options.open(&temporary)?;
            let write = (|| -> Result<(), Box<dyn std::error::Error>> {
                file.write_all(&serde_json::to_vec(&self.data)?)?;
                file.sync_all()?;
                replace_file(&temporary, path)?;
                Ok(())
            })();
            if write.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            write
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
