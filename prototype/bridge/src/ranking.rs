//! Corpus weights and bounded local preference; never host text or remote scoring.
use crate::memory::{Memory, normalize};
use qingjian_core::candidate::Candidate;
use std::collections::HashMap;
use std::path::Path;

pub struct Ranking {
    weights: HashMap<(String, String), u32>,
}

impl Ranking {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut weights = HashMap::new();
        for line in std::fs::read_to_string(path)?
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 3 {
                return Err("invalid ranking dictionary".into());
            }
            let weight: u32 = fields[2].parse()?;
            if !(1..=1_000_000).contains(&weight) {
                return Err("invalid ranking weight".into());
            }
            weights.insert(
                (fields[0].into(), fields[1].split_whitespace().collect()),
                weight,
            );
        }
        if weights.is_empty() {
            return Err("empty ranking dictionary".into());
        }
        Ok(Self { weights })
    }

    fn weight(&self, candidate: &Candidate) -> Option<u32> {
        self.weights
            .get(&(candidate.text.clone(), candidate.syllables.concat()))
            .copied()
    }

    pub fn order(&self, input: &str, items: &mut [Candidate], memory: &Memory) {
        let key = normalize(input);
        // A real whole-word match must precede synthetic unigram concatenations.
        // Preserve the engine's ordering for abbreviation, prefix and correction paths.
        items.sort_by_key(|c| {
            if c.syllables.concat() == key {
                if self.weight(c).is_some() { 0 } else { 1 }
            } else {
                2
            }
        });
        let preferences = memory.preferences(input);
        if preferences.is_empty() {
            return;
        }
        let mut groups: HashMap<Vec<String>, Vec<usize>> = HashMap::new();
        let selected: Vec<_> = items
            .iter()
            .map(|c| preferences.get(&(c.text.clone(), c.syllables.clone())))
            .collect();
        let bases: Vec<_> = items
            .iter()
            .map(|c| f64::from(self.weight(c).unwrap_or(1)).ln_1p())
            .collect();
        for (i, c) in items.iter().enumerate() {
            groups.entry(c.syllables.clone()).or_default().push(i);
        }
        // Only compete within identical pronunciation/consumption groups. Group slots
        // stay in place, so personal short words never displace complete sentences.
        for indices in groups.values() {
            if !indices.iter().any(|i| selected[*i].is_some()) {
                continue;
            }
            let maximum = indices.iter().map(|i| bases[*i]).fold(0.0_f64, f64::max);
            let score = |i: usize| {
                let base = bases[i];
                let Some(&(count, age)) = selected[i] else {
                    return base;
                };
                let freshness = 1.0 - (age.min(256) as f64 / 256.0);
                base + ((maximum - base).max(0.0) + 0.8 * f64::from(count.min(20)).ln_1p())
                    * freshness
            };
            let mut choices: Vec<_> = indices
                .iter()
                .map(|i| (items[*i].clone(), score(*i)))
                .collect();
            choices.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (i, (candidate, _)) in indices.iter().zip(choices) {
                items[*i] = candidate;
            }
        }
    }
}
