//! Bounded Mac-only word lattice using the licensed dictionary and confirmed memory.
//! No host context, new corpus, model, persistence format or upstream changes.
use crate::memory::Memory;
use qingjian_core::candidate::{Candidate, CandidateKind};
use qingjian_core::engine::Query;
use std::collections::HashMap;

pub struct Words {
    readings: HashMap<Vec<String>, Vec<(String, u32)>>,
}

impl Words {
    pub fn new(weights: &HashMap<(String, String), u32>, dictionary: &str) -> Self {
        let mut readings: HashMap<Vec<String>, Vec<(String, u32)>> = HashMap::new();
        for line in dictionary.lines().filter(|l| !l.starts_with('#')) {
            let parts: Vec<_> = line.split('\t').collect();
            if parts.len() != 3 {
                continue;
            }
            let syllables: Vec<String> = parts[1].split_whitespace().map(str::to_owned).collect();
            if syllables.is_empty()
                || syllables.len() > 8
                || parts[0].chars().count() != syllables.len()
            {
                continue;
            }
            if let Some(&weight) = weights.get(&(parts[0].to_owned(), syllables.concat())) {
                readings
                    .entry(syllables)
                    .or_default()
                    .push((parts[0].to_owned(), weight));
            }
        }
        for words in readings.values_mut() {
            words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            words.truncate(6);
        }
        Self { readings }
    }

    pub fn candidate(
        &self,
        input: &str,
        query: Option<&Query>,
        memory: &Memory,
    ) -> Option<Candidate> {
        let query = query?;
        if !query.tail.is_empty() {
            return None;
        }
        for segmentation in query.segmentations.iter().take(4) {
            if !(3..=32).contains(&segmentation.syllables.len())
                || segmentation.syllables.iter().any(|s| !s.complete)
            {
                continue;
            }
            let syllables: Vec<String> = segmentation
                .syllables
                .iter()
                .map(|s| s.text.clone())
                .collect();
            if syllables.concat() != crate::memory::normalize(input) {
                continue;
            }
            if let Some(candidate) = self.convert(syllables, memory) {
                return Some(candidate);
            }
        }
        None
    }

    fn convert(&self, syllables: Vec<String>, memory: &Memory) -> Option<Candidate> {
        // Dynamic programming over exact dictionary spans. Only a repeatedly confirmed
        // word can activate this path; cold queries retain the original engine output.
        let mut paths: Vec<Option<(f64, String, bool)>> = vec![None; syllables.len() + 1];
        paths[0] = Some((0.0, String::new(), false));
        for end in 1..=syllables.len() {
            for start in end.saturating_sub(8)..end {
                let Some((prior, text, learned)) = paths[start].clone() else {
                    continue;
                };
                let span = &syllables[start..end];
                let Some(words) = self.readings.get(span) else {
                    continue;
                };
                let preferences = memory.preferences(&span.concat());
                let mut choices = words.clone();
                // Also retain confirmed alternatives outside the dictionary's top six.
                for ((word, reading), (count, _)) in &preferences {
                    if *count >= 2 && reading == span && !choices.iter().any(|(w, _)| w == word) {
                        choices.push((word.clone(), 1));
                    }
                }
                let maximum = f64::from(words[0].1).ln_1p();
                for (word, weight) in choices {
                    let base = f64::from(weight).ln_1p();
                    let preference = preferences.get(&(word.clone(), span.to_vec()));
                    let repeated = preference.is_some_and(|(count, _)| *count >= 2);
                    let bonus =
                        preference
                            .filter(|(count, _)| *count >= 2)
                            .map_or(0.0, |(count, age)| {
                                let freshness = 1.0 / (1.0 + *age as f64 / 256.0);
                                (maximum - base).max(0.0)
                                    + 0.8 * f64::from((*count).min(20)).ln_1p() * freshness
                            });
                    let score = prior + base + bonus - 14.0;
                    if paths[end].as_ref().is_none_or(|p| score > p.0) {
                        paths[end] = Some((score, format!("{text}{word}"), learned || repeated));
                    }
                }
            }
        }
        let (_, text, learned) = paths.pop()??;
        learned.then_some(Candidate {
            text,
            kind: CandidateKind::Sentence,
            syllables,
            reading: None,
            translation: None,
            aux_code: None,
        })
    }
}
