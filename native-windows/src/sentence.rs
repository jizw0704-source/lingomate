//! Display state is tied to an immutable current-candidate request, never input history.
use crate::remote::RemoteError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TranslationRequest {
    pub revision: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationResponse {
    pub english: Option<String>,
    pub error: Option<RemoteError>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    None,
    Waiting,
    Ready {
        source: String,
        english: String,
    },
    Failed(RemoteError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateToken {
    pub input_revision: u64,
    pub engine_generation: u64,
    pub selected: usize,
    pub page: usize,
    pub source: String,
}
#[derive(Clone, Debug, Default)]
pub struct Sentence {
    pub token: Option<CandidateToken>,
    pub phase: Phase,
    pub request_id: u64,
}
impl Sentence {
    pub fn cancel(&mut self) {
        self.request_id = self.request_id.wrapping_add(1);
        self.token = None;
        self.phase = Phase::None;
    }
    pub fn begin(&mut self, token: CandidateToken) -> u64 {
        self.cancel();
        self.token = Some(token);
        self.phase = Phase::Waiting;
        self.request_id
    }
    pub fn accept(
        &mut self,
        id: u64,
        token: &CandidateToken,
        response: TranslationResponse,
    ) -> bool {
        if id != self.request_id
            || self.token.as_ref() != Some(token)
            || self.phase != Phase::Waiting
        {
            return false;
        }
        self.phase = match response.english {
            Some(english) if crate::remote::valid_english(&english) => Phase::Ready {
                source: token.source.clone(),
                english,
            },
            _ => Phase::Failed(response.error.unwrap_or(RemoteError::InvalidResponse)),
        };
        true
    }
    pub fn english(&self, token: &CandidateToken) -> Option<&str> {
        if self.token.as_ref() != Some(token) {
            return None;
        }
        match &self.phase {
            Phase::Ready { source, english } if source == &token.source => Some(english),
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn token(i: usize) -> CandidateToken {
        CandidateToken {
            input_revision: 2,
            engine_generation: 7,
            selected: i,
            page: i / 5,
            source: format!("候选{i}"),
        }
    }
    #[test]
    fn old_response_never_joins_a_new_candidate_or_cancelled_context() {
        let mut sentence = Sentence::default();
        let first = token(0);
        let old = sentence.begin(first.clone());
        let current = token(5);
        let now = sentence.begin(current.clone());
        let response = || TranslationResponse {
            english: Some("Selected candidate.".into()),
            error: None,
        };
        assert!(!sentence.accept(old, &first, response()));
        assert!(sentence.english(&current).is_none());
        assert!(sentence.accept(now, &current, response()));
        assert!(sentence.english(&first).is_none());
        sentence.cancel();
        assert!(!sentence.accept(now, &current, response()));
        assert!(sentence.english(&current).is_none());
    }
}
