//! Bounded, same-editor confirmed characters only; no host text or persistent input trail.
use qingjian_core::candidate::{Candidate, CandidateKind};
use std::time::{Duration, Instant};

const MAX_CHARACTERS: usize = 16;
const MAX_UNKNOWN: usize = 6;
const MAX_IDLE: Duration = Duration::from_secs(20);
const FUNCTION_CHARACTERS: &str = "我你他她它们的地得了着过吗呢吧啊和与或但而就在是把被给从到向于以为这那很更最不没只想要会能去来做用有说看让请将已经还";

#[derive(Default)]
pub struct Sequence {
    pieces: Vec<(char, String)>,
    unknown_length: usize,
    last: Option<Instant>,
}
impl Sequence {
    pub fn push(
        &mut self,
        candidate: &Candidate,
        known: impl Fn(&str, &[String]) -> bool,
    ) -> Vec<Candidate> {
        self.push_at(candidate, &known, Instant::now())
    }

    fn push_at(
        &mut self,
        candidate: &Candidate,
        known: &impl Fn(&str, &[String]) -> bool,
        now: Instant,
    ) -> Vec<Candidate> {
        if candidate.text.chars().count() != 1 || candidate.syllables.len() != 1 {
            *self = Self::default();
            return Vec::new();
        }
        if self.last.is_some_and(|last| {
            now.checked_duration_since(last)
                .is_none_or(|elapsed| elapsed > MAX_IDLE)
        }) {
            *self = Self::default();
        }
        let Some(character) = candidate.text.chars().next() else {
            return Vec::new();
        };
        if !('\u{3400}'..='\u{9fff}').contains(&character) {
            *self = Self::default();
            return Vec::new();
        }
        self.last = Some(now);
        self.unknown_length = if FUNCTION_CHARACTERS.contains(character) {
            0
        } else {
            self.unknown_length.saturating_add(1)
        };
        self.pieces
            .push((character, candidate.syllables[0].clone()));
        if self.pieces.len() > MAX_CHARACTERS {
            self.pieces.remove(0);
        }
        let mut words = Vec::with_capacity(2);
        for length in (2..=self.pieces.len()).rev() {
            let word = self.word(length);
            if known(&word.text, &word.syllables) {
                words.push(word);
                break;
            }
        }
        // The user explicitly chose automatic unknown-word guessing. Keep only
        // short runs without common function characters, not rolling sentence chunks.
        // A technical compound may contain two existing words, so do not reject it
        // just because its components also have independent dictionary entries.
        if (2..=MAX_UNKNOWN).contains(&self.unknown_length) {
            let word = self.word(self.unknown_length);
            if !words
                .iter()
                .any(|known| known.text == word.text && known.syllables == word.syllables)
            {
                words.push(word);
            }
        }
        words
    }

    fn word(&self, length: usize) -> Candidate {
        let pieces = &self.pieces[self.pieces.len() - length..];
        Candidate {
            text: pieces.iter().map(|p| p.0).collect(),
            syllables: pieces.iter().map(|p| p.1.clone()).collect(),
            kind: CandidateKind::Chinese,
            reading: None,
            translation: None,
            aux_code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn character(text: &str) -> Candidate {
        Candidate {
            text: text.into(),
            syllables: vec!["shi".into()],
            kind: CandidateKind::Chinese,
            reading: None,
            translation: None,
            aux_code: None,
        }
    }
    #[test]
    fn short_unknown_words_expire_and_long_runs_do_not_become_sentences() {
        let mut sequence = Sequence::default();
        let now = Instant::now();
        assert!(
            sequence
                .push_at(&character("钙"), &|_, _| false, now)
                .is_empty()
        );
        assert_eq!(
            sequence.push_at(&character("钛"), &|_, _| false, now)[0].text,
            "钙钛"
        );
        assert!(
            sequence
                .push_at(
                    &character("矿"),
                    &|_, _| false,
                    now + MAX_IDLE + Duration::from_secs(1)
                )
                .is_empty()
        );
        for _ in 0..20 {
            sequence.push(&character("晶"), |_, _| false);
        }
        assert!(sequence.push(&character("矿"), |_, _| false).is_empty());
        assert!(sequence.pieces.len() <= MAX_CHARACTERS);
    }
    #[test]
    fn known_function_words_and_user_authorized_short_compounds() {
        let mut sequence = Sequence::default();
        let known = |text: &str, _: &[String]| ["地球", "学习", "中文"].contains(&text);
        assert!(sequence.push(&character("地"), known).is_empty());
        assert_eq!(sequence.push(&character("球"), known)[0].text, "地球");
        sequence = Sequence::default();
        for text in ["我", "想", "去", "看", "你"] {
            assert!(sequence.push(&character(text), known).is_empty());
        }
        sequence = Sequence::default();
        for text in ["学", "习", "中", "文"] {
            sequence.push(&character(text), known);
        }
        let words = sequence.push(&character("界"), known);
        assert!(words.iter().any(|w| w.text == "学习中文界"));
    }
}
