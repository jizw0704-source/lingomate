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
        let mut best: Option<(Candidate, f64)> = None;
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
            if let Some((candidate, score)) = self.convert(syllables, memory)
                && best.as_ref().is_none_or(|(_, prior)| score > *prior)
            {
                best = Some((candidate, score));
            }
        }
        best.map(|(candidate, _)| candidate)
    }

    fn convert(&self, syllables: Vec<String>, memory: &Memory) -> Option<(Candidate, f64)> {
        #[derive(Clone)]
        struct Path {
            score: f64,
            text: String,
            last: String,
            learned: bool,
            pairs: usize,
        }
        // Retain alternatives at each word boundary: the best prefix alone may have
        // the wrong final word for a subsequent collocation. Bounded beam, no host text.
        let mut paths: Vec<Vec<Path>> = vec![Vec::new(); syllables.len() + 1];
        paths[0].push(Path {
            score: 0.0,
            text: String::new(),
            last: String::new(),
            learned: false,
            pairs: 0,
        });
        for end in 1..=syllables.len() {
            let mut next = Vec::new();
            for start in end.saturating_sub(8)..end {
                let span = &syllables[start..end];
                let Some(words) = self.readings.get(span) else {
                    continue;
                };
                let preferences = memory.preferences(&span.concat());
                let mut choices = words.clone();
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
                                (maximum - base).max(0.0) * freshness
                                    + crate::collocation::preference_bonus(*count, *age)
                            });
                    for prior in &paths[start] {
                        let pair = crate::collocation::score(&prior.last, &word);
                        next.push(Path {
                            score: prior.score + base + bonus + pair - 14.0,
                            text: format!("{}{word}", prior.text),
                            last: word.clone(),
                            learned: prior.learned || repeated,
                            pairs: prior.pairs + usize::from(pair > 0.0),
                        });
                    }
                }
            }
            next.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.text.cmp(&b.text)));
            // Distinct last words preserve contextual alternatives without duplicate paths.
            let mut seen = std::collections::HashSet::new();
            next.retain(|p| seen.insert((p.last.clone(), p.learned, p.pairs > 0)));
            next.truncate(12);
            paths[end] = next;
        }
        let path = paths
            .pop()?
            .into_iter()
            .find(|p| p.learned || p.pairs >= 2)?;
        Some((
            Candidate {
                text: path.text,
                kind: CandidateKind::Sentence,
                syllables,
                reading: None,
                translation: None,
                aux_code: None,
            },
            path.score,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingjian_core::parser::{Segmentation, Syllable};
    #[test]
    fn collocations_generate_complete_paths_but_not_unrelated_or_partial_input() {
        let dictionary = "我们\two men\t1000000\n需要\txu yao\t1000000\n研究\tyan jiu\t1000000\n材料\tcai liao\t1000000\n许要\txu yao\t1000000\n";
        let weights = dictionary
            .lines()
            .map(|l| {
                let p: Vec<_> = l.split('\t').collect();
                (
                    (p[0].to_owned(), p[1].split_whitespace().collect()),
                    p[2].parse().unwrap(),
                )
            })
            .collect();
        let words = Words::new(&weights, dictionary);
        let memory = Memory::open(None);
        let syllables = ["wo", "men", "xu", "yao", "yan", "jiu", "cai", "liao"];
        let mut query = Query {
            segmentations: vec![Segmentation {
                syllables: syllables.iter().map(|s| Syllable::complete(s)).collect(),
            }],
            ..Default::default()
        };
        let result = words
            .candidate(&syllables.concat(), Some(&query), &memory)
            .unwrap();
        assert_eq!(result.text, "我们需要研究材料");
        assert_eq!(result.syllables.concat(), syllables.concat());
        query.segmentations[0].syllables[7] = Syllable::partial("l");
        assert!(
            words
                .candidate("womenxuyaoyanjiucail", Some(&query), &memory)
                .is_none()
        );
        assert!(words.candidate("AI", Some(&query), &memory).is_none());
    }
}
