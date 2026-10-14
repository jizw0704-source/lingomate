//! Explicit local vocabulary management, separate from input and host receipts.
//! Priority lives in a sidecar so older clients can still read version-1 words.json.
use crate::memory::{MAX_BYTES, MAX_ENTRIES, load_data, replace_file, valid_word};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, DirBuilder, OpenOptions, TryLockError};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type Error = Box<dyn std::error::Error>;
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Priority {
    text: String,
    syllables: Vec<String>,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Priorities {
    version: u32,
    entries: Vec<Priority>,
}
impl Priorities {
    pub fn keys(&self) -> HashSet<(String, Vec<String>)> {
        self.entries
            .iter()
            .map(|p| (p.text.clone(), p.syllables.clone()))
            .collect()
    }
    pub fn contains(&self, text: &str, syllables: &[String]) -> bool {
        self.entries
            .iter()
            .any(|p| p.text == text && p.syllables == syllables)
    }
}
pub(crate) fn priority_path(path: &Path) -> PathBuf {
    path.with_extension("preferences.json")
}
pub(crate) fn load_priorities(path: &Path) -> Result<Priorities, Error> {
    let meta = match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Priorities {
                version: 1,
                entries: Vec::new(),
            });
        }
        other => other?,
    };
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err("invalid priorities".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let data: Priorities = serde_json::from_slice(&bytes)?;
    if bytes.len() as u64 > MAX_BYTES
        || data.version != 1
        || data.entries.len() > MAX_ENTRIES
        || data
            .entries
            .iter()
            .any(|p| !valid_word(&p.text, &p.syllables))
    {
        return Err("invalid priorities".into());
    }
    let unique: HashSet<_> = data.entries.iter().map(|p| &p.syllables).collect();
    if unique.len() != data.entries.len() {
        return Err("duplicate priority reading".into());
    }
    Ok(data)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    action: String,
    text: Option<String>,
    syllables: Option<Vec<String>>,
    prioritized: Option<bool>,
}
#[derive(Serialize)]
struct Word {
    text: String,
    syllables: Vec<String>,
    aliases: Vec<String>,
    count: u32,
    last: u64,
    prioritized: bool,
}

fn lock(path: &Path) -> Result<fs::File, Error> {
    let parent = path.parent().ok_or("missing parent")?;
    let mut directory = DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    directory.mode(0o700);
    directory.create(parent)?;
    let path = path.with_extension("lock");
    if let Ok(meta) = fs::symlink_metadata(&path)
        && !meta.is_file()
    {
        return Err("invalid lock".into());
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    options.mode(0o600);
    let lock = options.open(path)?;
    let started = Instant::now();
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::WouldBlock) if started.elapsed() < Duration::from_millis(350) => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temporary = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let result = (|| -> Result<(), Error> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
fn apply(path: &Path, request: Request) -> Result<Value, Error> {
    if !matches!(request.action.as_str(), "list" | "delete" | "prioritize") {
        return Err("invalid action".into());
    }
    if request.action == "list" {
        if request.text.is_some() || request.syllables.is_some() || request.prioritized.is_some() {
            return Err("invalid list".into());
        }
    } else if !valid_word(
        request.text.as_deref().unwrap_or(""),
        request.syllables.as_deref().unwrap_or(&[]),
    ) || (request.action == "delete" && request.prioritized.is_some())
        || (request.action == "prioritize" && request.prioritized.is_none())
    {
        return Err("invalid identity".into());
    }
    // A missing store is a read-only empty list; no preferences or vocabulary created.
    if request.action == "list" && !path.exists() {
        // Broken symlinks must fail rather than appear to be an empty store.
        if fs::symlink_metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
            return Ok(json!({"words": []}));
        }
    }
    let _lock = lock(path)?;
    let mut data = load_data(path)?;
    let preference_path = priority_path(path);
    let mut priorities = load_priorities(&preference_path)?;
    if request.action != "list" {
        let text = request.text.as_deref().unwrap();
        let syllables = request.syllables.as_ref().unwrap();
        if !data
            .entries
            .iter()
            .any(|e| e.text == text && e.syllables == *syllables)
        {
            return Err("stale selection".into());
        }
        let known: HashSet<_> = data
            .entries
            .iter()
            .map(|e| (e.text.as_str(), e.syllables.as_slice()))
            .collect();
        priorities
            .entries
            .retain(|p| known.contains(&(p.text.as_str(), p.syllables.as_slice())));
        priorities.entries.retain(|p| {
            if request.action == "prioritize" && request.prioritized == Some(true) {
                p.syllables != *syllables
            } else {
                p.text != text || p.syllables != *syllables
            }
        });
        if request.action == "prioritize" && request.prioritized == Some(true) {
            priorities.entries.push(Priority {
                text: text.into(),
                syllables: syllables.clone(),
            });
        }
        // Preserve legacy word schema. Prioritization never changes counts or receipts.
        atomic_write(&preference_path, &serde_json::to_vec(&priorities)?)?;
        if request.action == "delete" {
            data.entries
                .retain(|e| e.text != text || e.syllables != *syllables);
            atomic_write(path, &serde_json::to_vec(&data)?)?;
        }
    }
    let mut words: BTreeMap<(String, Vec<String>), Word> = BTreeMap::new();
    for entry in data.entries {
        let word = words
            .entry((entry.text.clone(), entry.syllables.clone()))
            .or_insert_with(|| Word {
                prioritized: priorities.contains(&entry.text, &entry.syllables),
                text: entry.text,
                syllables: entry.syllables,
                aliases: Vec::new(),
                count: 0,
                last: 0,
            });
        if !word.aliases.contains(&entry.pinyin) {
            word.aliases.push(entry.pinyin);
        }
        // Aliases are the same selection, not separate usages.
        word.count = word.count.max(entry.count);
        word.last = word.last.max(entry.last);
    }
    let mut words: Vec<_> = words.into_values().collect();
    words.sort_by(|a, b| {
        b.prioritized
            .cmp(&a.prioritized)
            .then(b.last.cmp(&a.last))
            .then(a.text.cmp(&b.text))
    });
    Ok(json!({"words": words}))
}
/// One bounded explicit request; errors never expose file paths, words or parser detail.
pub fn request(path: &Path, input: &str) -> Value {
    if input.len() > 4096 {
        return json!({"error": "词库操作参数无效。"});
    }
    let Ok(request) = serde_json::from_str::<Request>(input) else {
        return json!({"error": "词库操作参数无效。"});
    };
    apply(path, request).unwrap_or_else(|_| json!({"error": "词库操作未完成。请刷新后重试，或检查文件格式与存储权限；原有文件不会被重建。"}))
}
