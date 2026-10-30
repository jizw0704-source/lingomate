//! Explicit local recommendation preferences; never usage history or learned counts.
use crate::memory::{MAX_BYTES, MAX_ENTRIES, valid_word};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    pub text: String,
    pub syllables: Vec<String>,
    pub value: i8,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    entries: Vec<Entry>,
}
pub(crate) fn path(words: &Path) -> PathBuf {
    words.with_extension("feedback.json")
}
pub(crate) fn load(words: &Path) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
    let p = path(words);
    let meta = match std::fs::symlink_metadata(&p) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        other => other?,
    };
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err("invalid feedback file".into());
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(p)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("invalid feedback size".into());
    }
    let data: File = serde_json::from_slice(&bytes)?;
    let mut seen = HashSet::new();
    let mut preferred = HashSet::new();
    if data.version != 1
        || data.entries.len() > MAX_ENTRIES
        || data.entries.iter().any(|e| {
            !valid_word(&e.text, &e.syllables)
                || ![-1, 1].contains(&e.value)
                || !seen.insert((e.text.clone(), e.syllables.clone()))
                || (e.value == 1 && !preferred.insert(e.syllables.clone()))
        })
    {
        return Err("invalid feedback".into());
    }
    Ok(data.entries)
}
/// Caller holds the vocabulary lock and validates both stores before writing.
pub(crate) fn set(
    words: &Path,
    mut entries: Vec<Entry>,
    text: &str,
    syllables: &[String],
    value: i8,
) -> Result<(), Box<dyn std::error::Error>> {
    entries.retain(|e| {
        !(e.text == text && e.syllables == syllables
            || value == 1 && e.value == 1 && e.syllables == syllables)
    });
    if value != 0 {
        entries.push(Entry {
            text: text.into(),
            syllables: syllables.into(),
            value,
        });
    }
    if entries.len() > MAX_ENTRIES {
        return Err("feedback full".into());
    }
    crate::vocabulary::atomic_write(
        &path(words),
        &serde_json::to_vec(&File {
            version: 1,
            entries,
        })?,
    )
}
