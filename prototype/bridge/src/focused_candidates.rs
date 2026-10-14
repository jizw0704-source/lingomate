//! Mac-only selection over real engine matches. No new corpus, text or scores.
use crate::memory::normalize;
use qingjian_core::candidate::{Candidate, CandidateKind};
use qingjian_core::engine::Query;
use qingjian_dictionary::canonical_syllable;
use std::collections::{BTreeMap, HashSet};

const PARTIAL_LIMIT: usize = 8;
const SINGLE_LIMIT: usize = 16;
const COMPLETION_LIMIT: usize = 5;
const SENTENCE_LIMIT: usize = 3;

// Exact dictionary/personal words, whole synthetic sentences, full abbreviations,
// partial consumption, then completion. Existing frequency/memory order is stable.
fn class(input: &str, query: Option<&Query>, candidate: &Candidate) -> Option<u8> {
    let reading = candidate.syllables.concat();
    if reading.is_empty() {
        return None;
    }
    let key = normalize(input);
    let corrected = query
        .and_then(|q| q.correction.as_ref())
        .map(|c| c.corrected.as_str());
    let complete_alias = query.is_some_and(|q| {
        q.tail.is_empty()
            && q.segmentations.iter().any(|s| {
                s.syllables.len() == candidate.syllables.len()
                    && s.syllables.iter().all(|typed| typed.complete)
                    && candidate
                        .syllables
                        .iter()
                        .zip(&s.syllables)
                        .all(|(reading, typed)| {
                            canonical_syllable(reading) == canonical_syllable(&typed.text)
                        })
            })
    });
    if reading == key || corrected == Some(reading.as_str()) || complete_alias {
        return Some(if candidate.kind == CandidateKind::Sentence {
            1
        } else {
            0
        });
    }
    if let Some(query) = query
        && let Some(segmentation) = query.segmentations.first()
    {
        // Weak alternative splits must not turn a fully typed syllable into
        // unrelated initial matches. Flat exact matches above still keep ambiguity.
        let tokens = &segmentation.syllables;
        if tokens.is_empty() {
            return None;
        }
        let mut prefix = String::new();
        for token in tokens {
            prefix.push_str(&token.text);
            if prefix == reading && key.starts_with(&reading) {
                return Some(3);
            }
        }
        let strict = candidate
            .syllables
            .iter()
            .zip(tokens)
            .all(|(reading, typed)| {
                if typed.complete {
                    canonical_syllable(reading) == canonical_syllable(&typed.text)
                } else {
                    reading.starts_with(&typed.text)
                }
            });
        let relaxed = candidate
            .syllables
            .iter()
            .zip(tokens)
            .all(|(reading, typed)| reading.starts_with(&typed.text));
        return if strict && candidate.syllables.len() == tokens.len() && query.tail.is_empty() {
            Some(2)
        } else if strict && candidate.syllables.len() <= tokens.len() {
            Some(3)
        } else if relaxed && candidate.syllables.len() >= tokens.len() {
            Some(4)
        } else {
            None
        };
    }
    if key.starts_with(&reading) {
        Some(3)
    } else {
        reading.starts_with(&key).then_some(4)
    }
}

fn partial(items: &[(usize, usize)], limit: usize) -> Vec<usize> {
    // Give every available consumption length a slot before filling a group.
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(index, length) in items {
        groups.entry(length).or_default().push(index);
    }
    let mut selected = Vec::new();
    for offset in 0..limit {
        for group in groups.values().rev() {
            if let Some(&index) = group.get(offset) {
                selected.push(index);
                if selected.len() == limit {
                    return selected;
                }
            }
        }
    }
    selected
}

pub fn select(input: &str, query: Option<&Query>, items: Vec<Candidate>) -> Vec<Candidate> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut buckets: [Vec<usize>; 5] = Default::default();
    let mut single = Vec::new();
    let mut segments = Vec::new();
    let mut seen = HashSet::new();
    let mut sentences = 0;
    for (index, candidate) in items.iter().enumerate() {
        if !candidate
            .text
            .chars()
            .any(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
            || !seen.insert((candidate.text.clone(), candidate.syllables.clone()))
        {
            continue;
        }
        if let Some(class) = class(input, query, candidate) {
            if candidate.kind == CandidateKind::Sentence {
                if sentences == SENTENCE_LIMIT {
                    continue;
                }
                sentences += 1;
            }
            if class == 3 {
                let entry = (index, candidate.syllables.len());
                if candidate.text.chars().count() == 1 {
                    single.push(entry);
                } else {
                    segments.push(entry);
                }
            } else {
                buckets[class as usize].push(index);
            }
        }
    }
    let mut indices = buckets[0].clone();
    indices.extend(buckets[1].iter().take(SENTENCE_LIMIT).copied());
    indices.extend(&buckets[2]);
    indices.extend(partial(&segments, PARTIAL_LIMIT));
    indices.extend(partial(&single, SINGLE_LIMIT));
    indices.extend(buckets[4].iter().take(COMPLETION_LIMIT).copied());
    // Preserve bounded typo recovery only if nothing matches. Never invent candidates.
    if indices.is_empty() {
        seen.clear();
        indices.extend(
            items
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    c.text
                        .chars()
                        .any(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
                        && seen.insert((c.text.clone(), c.syllables.clone()))
                })
                .take(5)
                .map(|(i, _)| i),
        );
    }
    indices
        .into_iter()
        .take(64)
        .map(|index| items[index].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingjian_core::parser::{Segmentation, Syllable};
    fn word(text: &str, readings: &[&str]) -> Candidate {
        Candidate {
            text: text.into(),
            syllables: readings.iter().map(|s| (*s).into()).collect(),
            kind: CandidateKind::Chinese,
            reading: None,
            translation: None,
            aux_code: None,
        }
    }
    #[test]
    fn complete_syllable_does_not_become_a_strong_longer_syllable_match() {
        let mut query = Query {
            segmentations: vec![Segmentation {
                syllables: vec![Syllable::complete("xue"), Syllable::complete("xi")],
            }],
            ..Default::default()
        };
        assert_eq!(
            class("xuexi", Some(&query), &word("学校", &["xue", "xiao"])),
            Some(4)
        );
        query.segmentations[0].syllables[1] = Syllable::partial("x");
        assert_eq!(
            class("xuex", Some(&query), &word("学校", &["xue", "xiao"])),
            Some(2)
        );
    }
    #[test]
    fn synthetic_sentences_are_bounded_and_partial_lengths_and_single_words_survive() {
        let mut items = vec![word("学习", &["xue", "xi"])];
        for i in 0..20 {
            let mut candidate = word(
                &format!("学{}", char::from_u32(0x4e00 + i).unwrap()),
                &["xue", "xi"],
            );
            candidate.kind = CandidateKind::Sentence;
            items.push(candidate);
        }
        items.extend([word("学", &["xue"]), word("雪", &["xue"])]);
        let mut partial_sentence = word("血", &["xue"]);
        partial_sentence.kind = CandidateKind::Sentence;
        items.push(partial_sentence);
        let selected = select("xuexi", None, items);
        assert_eq!(selected[0].text, "学习");
        assert_eq!(
            selected
                .iter()
                .filter(|c| c.kind == CandidateKind::Sentence)
                .count(),
            3
        );
        assert!(selected.iter().any(|c| c.text == "学"));
        assert_eq!(partial(&[(0, 3), (1, 3), (2, 2), (3, 1)], 3), vec![0, 2, 3]);
    }
    #[test]
    fn many_exact_single_character_choices_are_not_forced_to_a_small_page_count() {
        let items = (0..64)
            .map(|i| word(&char::from_u32(0x4e00 + i).unwrap().to_string(), &["shi"]))
            .collect();
        assert_eq!(select("shi", None, items).len(), 64);
        assert!(select("", None, Vec::new()).is_empty());
    }
}
