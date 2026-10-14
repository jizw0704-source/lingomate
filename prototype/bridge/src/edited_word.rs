//! Only confirmed candidate characters can be removed; no arbitrary host text input.
use qingjian_core::candidate::{Candidate, CandidateKind};
use std::time::{Duration, Instant};

const MAX_IDLE: Duration = Duration::from_secs(30);
const MAX_CHARACTERS: usize = 16;
const FUNCTION_CHARACTERS: &str = "我你他她它们的地得了着过吗呢吧啊和与或但而就在是把被给从到向于以为这那很更最不没只想要会能去来做用有说看让请将已经还";

#[derive(Default)]
pub struct EditedWord {
    pieces: Vec<(char, String)>,
    token: u64,
    edited: bool,
    last: Option<Instant>,
}

impl EditedWord {
    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }
    pub fn was_edited(&self) -> bool {
        self.edited
    }

    pub fn append(&mut self, candidate: &Candidate, token: u64) -> bool {
        let now = Instant::now();
        if !self.fresh(now) {
            *self = Self::default();
        }
        let chars: Vec<_> = candidate.text.chars().collect();
        if chars.is_empty()
            || chars.len() != candidate.syllables.len()
            || self.pieces.len() + chars.len() > MAX_CHARACTERS
            || !chars
                .iter()
                .all(|ch| ('\u{3400}'..='\u{9fff}').contains(ch))
        {
            return false;
        }
        self.pieces
            .extend(chars.into_iter().zip(candidate.syllables.clone()));
        self.token = token;
        self.last = Some(now);
        true
    }

    fn fresh(&self, now: Instant) -> bool {
        self.last.is_some_and(|last| {
            now.checked_duration_since(last)
                .is_some_and(|elapsed| elapsed <= MAX_IDLE)
        })
    }

    pub fn valid_token(&self, token: u64) -> bool {
        self.token == token && self.fresh(Instant::now())
    }

    pub fn renew_token(&mut self, token: u64) {
        self.token = token;
    }

    pub fn delete(&mut self, token: u64, start: usize, length: usize) -> bool {
        if !self.valid_token(token)
            || length == 0
            || start
                .checked_add(length)
                .is_none_or(|end| end > self.pieces.len())
        {
            return false;
        }
        self.pieces.drain(start..start + length);
        self.edited = true;
        self.last = Some(Instant::now());
        true
    }

    pub fn candidate(&self, known: impl Fn(&str, &[String]) -> bool) -> Option<Candidate> {
        let text: String = self.pieces.iter().map(|p| p.0).collect();
        let syllables: Vec<_> = self.pieces.iter().map(|p| p.1.clone()).collect();
        if !self.edited
            || !(2..=MAX_CHARACTERS).contains(&self.pieces.len())
            || !(known(&text, &syllables)
                || (self.pieces.len() <= 6
                    && !text.chars().any(|ch| FUNCTION_CHARACTERS.contains(ch))))
        {
            return None;
        }
        Some(Candidate {
            text,
            syllables,
            kind: CandidateKind::Chinese,
            reading: None,
            translation: None,
            aux_code: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, syllables: &[&str]) -> Candidate {
        Candidate {
            text: text.into(),
            syllables: syllables.iter().map(|s| (*s).into()).collect(),
            kind: CandidateKind::Chinese,
            reading: None,
            translation: None,
            aux_code: None,
        }
    }

    #[test]
    fn keeps_original_readings_and_only_the_trimmed_word() {
        let mut run = EditedWord::default();
        assert!(run.append(&word("银行", &["yin", "hang"]), 1));
        assert!(run.delete(1, 0, 1));
        assert!(run.append(&word("灵活", &["ling", "huo"]), 2));
        assert!(run.delete(2, 2, 1));
        let kept = run.candidate(|_, _| false).unwrap();
        assert_eq!(kept.text, "行灵");
        assert_eq!(kept.syllables, ["hang", "ling"]);
    }

    #[test]
    fn rejects_stale_tokens_bounds_overflow_unedited_and_expired_runs() {
        let mut run = EditedWord::default();
        assert!(run.append(&word("灵活", &["ling", "huo"]), 5));
        assert!(run.candidate(|_, _| true).is_none());
        assert!(!run.delete(4, 1, 1));
        assert!(!run.delete(5, usize::MAX, 1));
        assert!(!run.delete(5, 0, 3));
        assert!(!run.delete(5, 0, 0));
        run.last = Some(Instant::now() - MAX_IDLE - Duration::from_secs(1));
        assert!(!run.delete(5, 1, 1));
        assert!(run.append(&word("果实", &["guo", "shi"]), 6));
        assert!(run.delete(6, 1, 1));
        assert!(run.candidate(|_, _| false).is_none());
        assert!(!run.append(&word("a", &["a"]), 7));
    }

    #[test]
    fn known_words_can_contain_function_characters_but_unknown_sentences_are_rejected() {
        let mut run = EditedWord::default();
        assert!(run.append(&word("地点", &["di", "dian"]), 1));
        assert!(run.delete(1, 1, 1));
        assert!(run.append(&word("球体", &["qiu", "ti"]), 2));
        assert!(run.delete(2, 2, 1));
        assert!(run.candidate(|_, _| false).is_none());
        assert_eq!(
            run.candidate(|text, _| text == "地球").unwrap().text,
            "地球"
        );
        let mut long = EditedWord::default();
        for token in 1..=16 {
            assert!(long.append(&word("晶", &["jing"]), token));
        }
        assert!(!long.append(&word("晶", &["jing"]), 17));
        assert!(long.delete(16, 15, 1));
        assert!(long.candidate(|_, _| false).is_none());
        assert_eq!(
            long.candidate(|_, _| true).unwrap().text.chars().count(),
            15
        );
    }
}
