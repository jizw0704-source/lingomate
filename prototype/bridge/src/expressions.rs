//! Opt-in local fixed-expression completion; no host context or remote requests.
use crate::memory::{Memory, normalize};
use qingjian_core::candidate::{Candidate, CandidateKind};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_BYTES: u64 = 2 * 1024 * 1024;
const LIMIT: usize = 3;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    text: String,
    syllables: Vec<String>,
    weight: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u32,
    license: String,
    entries: Vec<Entry>,
}

#[derive(Default)]
pub struct Expressions {
    entries: Vec<Entry>,
    prefixes: HashMap<String, Vec<usize>>,
}

impl Expressions {
    pub fn load(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let packaged = root.join("fixed-expressions.json");
        let path = if packaged.is_file() {
            packaged
        } else {
            root.join("data/generated/fixed-expressions.json")
        };
        let meta = std::fs::symlink_metadata(&path)?;
        if !meta.is_file() || meta.len() > MAX_BYTES {
            return Err("invalid expression catalog file".into());
        }
        let catalog: Catalog = serde_json::from_slice(&std::fs::read(path)?)?;
        Self::from_catalog(catalog).map_err(Into::into)
    }

    fn from_catalog(catalog: Catalog) -> Result<Self, &'static str> {
        if catalog.version != 1
            || catalog.license != "CC-BY-SA-4.0"
            || catalog.entries.is_empty()
            || catalog.entries.len() > 10_000
        {
            return Err("invalid expression catalog");
        }
        let mut result = Self::default();
        let mut seen = HashSet::new();
        for entry in catalog.entries {
            let count = entry.text.chars().count();
            if !(4..=16).contains(&count)
                || count != entry.syllables.len()
                || !entry
                    .text
                    .chars()
                    .all(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
                || entry.syllables.iter().any(|s| {
                    s.is_empty() || s.len() > 6 || !s.bytes().all(|b| b.is_ascii_lowercase())
                })
                || !(1..=1_000_000).contains(&entry.weight)
                || !seen.insert((entry.text.clone(), entry.syllables.clone()))
            {
                return Err("invalid expression entry");
            }
            let index = result.entries.len();
            for end in 2..=count {
                result
                    .prefixes
                    .entry(entry.syllables[..end].concat())
                    .or_default()
                    .push(index);
            }
            result.entries.push(entry);
        }
        Ok(result)
    }

    fn matches(entry: &Entry, input: &str) -> bool {
        if input.starts_with('\'') || input.ends_with('\'') || input.contains("''") {
            return false;
        }
        let mut boundaries = HashSet::new();
        let mut end = 0;
        for syllable in &entry.syllables {
            end += syllable.len();
            boundaries.insert(end);
        }
        let mut letters = 0;
        input.bytes().all(|b| {
            if b == b'\'' {
                boundaries.contains(&letters)
            } else {
                letters += 1;
                b.is_ascii_lowercase()
            }
        })
    }

    pub fn is_completion(&self, input: &str, candidate: &Candidate) -> bool {
        let key = normalize(input);
        self.prefixes.get(&key).is_some_and(|indices| {
            indices.iter().any(|&index| {
                let entry = &self.entries[index];
                entry.text == candidate.text
                    && entry.syllables == candidate.syllables
                    && entry.syllables.concat() != key
                    && Self::matches(entry, input)
            })
        })
    }

    pub fn candidates(&self, input: &str, memory: &Memory) -> Vec<Candidate> {
        let key = normalize(input);
        let Some(indices) = self.prefixes.get(&key) else {
            return Vec::new();
        };
        let preferences = memory.preferences(input);
        let mut matches: Vec<_> = indices
            .iter()
            .map(|&index| &self.entries[index])
            .filter(|entry| Self::matches(entry, input))
            .map(|entry| {
                let identity = (entry.text.clone(), entry.syllables.clone());
                let full = memory.preferences(&entry.syllables.concat());
                let preference = preferences.get(&identity).or_else(|| full.get(&identity));
                let score = preference.map_or(0, |&(count, age)| {
                    count.min(20) * (256 - age.min(256) as u32)
                });
                (entry, score)
            })
            .collect();
        matches.sort_by(|(a, a_score), (b, b_score)| {
            b_score
                .cmp(a_score)
                .then(b.weight.cmp(&a.weight))
                .then(a.text.cmp(&b.text))
                .then(a.syllables.cmp(&b.syllables))
        });
        let mut seen = HashSet::new();
        matches
            .into_iter()
            .filter(|(entry, _)| seen.insert(entry.text.clone()))
            .take(LIMIT)
            .map(|(entry, _)| Candidate {
                text: entry.text.clone(),
                syllables: entry.syllables.clone(),
                // A verified local prefix expands the whole scope, not an arbitrary
                // client string. The real engine consumes that exact typed scope.
                kind: CandidateKind::Generated,
                reading: None,
                translation: None,
                aux_code: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_complete_syllable_prefixes_and_apostrophes() {
        let expressions = Expressions::from_catalog(Catalog {
            version: 1,
            license: "CC-BY-SA-4.0".into(),
            entries: vec![Entry {
                text: "吃葡萄不吐葡萄皮".into(),
                syllables: "chi pu tao bu tu pu tao pi"
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect(),
                weight: 1,
            }],
        })
        .unwrap();
        let memory = Memory::open(None);
        for input in ["chi", "chiput", "chiputaoxuexi", "ch'iputao", "chi''putao"] {
            assert!(expressions.candidates(input, &memory).is_empty());
        }
        for input in ["chipu", "chiputao", "chi'pu'tao"] {
            let items = expressions.candidates(input, &memory);
            assert_eq!(items.len(), 1);
            assert!(expressions.is_completion(input, &items[0]));
            assert_eq!(items[0].kind, CandidateKind::Generated);
        }
        let whole = expressions.candidates("chiputaobutuputaopi", &memory);
        assert_eq!(whole.len(), 1);
        assert!(!expressions.is_completion("chiputaobutuputaopi", &whole[0]));
    }

    #[test]
    fn rejects_unlicensed_or_invalid_catalogs() {
        for license in ["unknown", "GPL-3.0", "CC-BY-SA-4.0"] {
            assert!(
                Expressions::from_catalog(Catalog {
                    version: 1,
                    license: license.into(),
                    entries: vec![],
                })
                .is_err()
            );
        }
    }
}
