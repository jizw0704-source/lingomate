use crate::bridge::{Bridge, Confirmation, Frame};
use crate::punctuation::{Action, Language, Mode, Punctuation};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

fn memory_context() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!(
        "win-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

pub const PAGE_SIZE: usize = 5;
#[derive(Clone, Copy, Debug)]
pub enum Key {
    Letter(char),
    Literal(char),
    Backspace,
    Space(bool),
    Return,
    Escape,
    Tab(bool),
    Up,
    Down,
    Previous,
    Next,
    Digit(usize),
    Punctuation(char),
    ToggleBilingual,
    CyclePunctuation,
}
#[derive(Clone)]
pub struct Input {
    pub raw: String,
    pub frame: Frame,
    pub selected: usize,
    pub sense: usize,
    pub expanded: bool,
    pub english: bool,
    pub bilingual: bool,
    pub error: bool,
    pub last_english: bool,
    pub pending_study: Option<crate::learning::Word>,
    pub pending_memory: Option<Confirmation>,
    pub memory_context: String,
    pub remember: bool,
    pub punctuation: Punctuation,
    pub punctuation_mode: Mode,
}
impl Default for Input {
    fn default() -> Self {
        Self {
            raw: String::new(),
            frame: Frame::default(),
            selected: 0,
            sense: 0,
            expanded: false,
            english: false,
            bilingual: true,
            error: false,
            last_english: false,
            pending_study: None,
            pending_memory: None,
            memory_context: memory_context(),
            remember: true,
            punctuation: Punctuation::default(),
            punctuation_mode: Mode::default(),
        }
    }
}
impl Input {
    pub fn reset_memory_context(&mut self) -> String {
        self.pending_memory = None;
        std::mem::replace(&mut self.memory_context, memory_context())
    }
    pub fn break_memory(&mut self, engine: &mut Bridge) {
        engine.cancel_memory(&self.reset_memory_context());
    }
    pub fn finish_memory(&mut self, engine: &mut Bridge) {
        engine.retire_memory(&self.reset_memory_context());
    }
    pub fn normalized(&self) -> String {
        self.raw.to_ascii_lowercase()
    }
    pub fn page(&self) -> usize {
        self.selected / PAGE_SIZE
    }
    pub fn page_count(&self) -> usize {
        self.frame.candidates.len().div_ceil(PAGE_SIZE)
    }
    pub fn reset(&mut self) {
        self.punctuation.reset_context();
        self.raw.clear();
        self.frame = Frame::default();
        self.selected = 0;
        self.sense = 0;
        self.expanded = false;
        self.error = false;
        self.pending_study = None;
        self.pending_memory = None;
    }
    pub fn raw_commit(&mut self) -> String {
        let text = self.raw.clone();
        self.reset();
        if !text.is_empty() {
            self.last_english = true;
            self.punctuation.confirm(Language::English);
        }
        text
    }
    pub(crate) fn literal_commit(&mut self, ch: char) -> String {
        let mut text = if self.raw.is_empty() {
            String::new()
        } else {
            self.raw_commit()
        };
        text.push(ch);
        self.punctuation.observe_literal(&text);
        text
    }
    pub fn handle(&mut self, key: Key, engine: &mut Bridge) -> Result<Option<String>, String> {
        self.pending_study = None;
        self.pending_memory = None;
        // A replacement bridge has no frame for the retained composition.
        // Candidate actions must query it before selecting or paging anything.
        if self.error
            && !self.raw.is_empty()
            && !matches!(
                key,
                Key::Letter(_) | Key::Backspace | Key::Return | Key::Escape
            )
        {
            self.refresh(engine)?;
        }
        match key {
            Key::ToggleBilingual => {
                self.bilingual = !self.bilingual;
                if !self.bilingual {
                    self.expanded = false;
                }
            }
            Key::CyclePunctuation => {
                self.finish_memory(engine);
                let raw = self.raw_commit();
                let index = crate::punctuation::Mode::ALL
                    .iter()
                    .position(|m| *m == self.punctuation_mode)
                    .unwrap_or(0);
                self.punctuation_mode = crate::punctuation::Mode::ALL[(index + 1) % 3];
                self.punctuation.reset_context();
                return Ok((!raw.is_empty()).then_some(raw));
            }
            Key::Literal(ch) => {
                self.finish_memory(engine);
                return Ok(Some(self.literal_commit(ch)));
            }
            Key::Letter(ch) => {
                if self.punctuation.is_latin() && self.raw.is_empty() {
                    self.finish_memory(engine);
                    self.punctuation.observe_literal(&ch.to_string());
                    return Ok(Some(ch.to_string()));
                }
                if self.raw.len() < 240 && (ch.is_ascii_alphabetic() || ch == '\'') {
                    self.raw.push(ch);
                    self.refresh(engine)?;
                }
            }
            Key::Backspace => {
                // Editing Pinyin does not remove an opening quote already in the host.
                self.punctuation.reset_literal();
                self.break_memory(engine);
                self.raw.pop();
                self.refresh(engine)?;
            }
            Key::Return => {
                self.finish_memory(engine);
                return Ok(Some(self.raw_commit()));
            }
            Key::Escape => {
                if self.expanded {
                    self.expanded = false;
                } else {
                    self.break_memory(engine);
                    self.reset();
                }
            }
            Key::Tab(back) => {
                if !self.bilingual {
                    return Ok(None);
                }
                if let Some(candidate) = self.frame.candidates.get(self.selected) {
                    let count = candidate.translations.len();
                    if count > 0 {
                        if self.expanded {
                            self.sense = if back {
                                (self.sense + count - 1) % count
                            } else {
                                (self.sense + 1) % count
                            };
                        }
                        self.expanded = true;
                    }
                }
            }
            Key::Up => {
                if !self.selected.is_multiple_of(PAGE_SIZE) {
                    self.selected -= 1;
                }
                self.sense = 0;
                self.expanded = false;
            }
            Key::Down => {
                if self.selected + 1 < self.frame.candidates.len()
                    && self.selected % PAGE_SIZE + 1 < PAGE_SIZE
                {
                    self.selected += 1;
                }
                self.sense = 0;
                self.expanded = false;
            }
            Key::Previous => self.move_page(false),
            Key::Next => self.move_page(true),
            Key::Digit(slot) => {
                let index = self.page() * PAGE_SIZE + slot;
                if slot < PAGE_SIZE && index < self.frame.candidates.len() {
                    self.selected = index;
                    return self.commit(engine, false);
                }
            }
            Key::Space(english) => return self.commit(engine, english),
            Key::Punctuation(ch) => {
                let Some(action) = Punctuation::action(
                    ch,
                    &self.normalized(),
                    !self.frame.candidates.is_empty(),
                    self.punctuation_mode,
                ) else {
                    return self.handle(Key::Letter(ch), engine);
                };
                let chosen = match action {
                    Action::Symbol => None,
                    Action::Raw => {
                        let raw = self.raw_commit();
                        self.punctuation.observe_literal(&raw);
                        Some(raw)
                    }
                    Action::CommitChinese => self.commit(engine, false)?,
                };
                if !self.raw.is_empty() {
                    return Ok(chosen);
                }
                let Some(punctuation) = self.punctuation.render(ch, self.punctuation_mode) else {
                    return Ok(chosen);
                };
                if let Some(receipt) = &mut self.pending_memory {
                    receipt.output.push_str(&punctuation);
                    receipt.boundary_after = true;
                }
                let receipt = self.pending_memory.take();
                self.finish_memory(engine);
                self.pending_memory = receipt;
                return Ok(Some(format!("{}{punctuation}", chosen.unwrap_or_default())));
            }
        }
        Ok(None)
    }
    fn refresh(&mut self, engine: &mut Bridge) -> Result<(), String> {
        self.frame = engine.call(
            json!({"action":"query", "input":self.normalized(), "context":self.memory_context}),
        )?;
        self.selected = 0;
        self.sense = 0;
        self.expanded = false;
        self.error = false;
        Ok(())
    }
    pub fn move_page(&mut self, next: bool) {
        let page = self.page();
        if next && page + 1 < self.page_count() {
            self.selected = (page + 1) * PAGE_SIZE;
        }
        if !next && page > 0 {
            self.selected = (page - 1) * PAGE_SIZE;
        }
        self.sense = 0;
        self.expanded = false;
    }
    fn commit(&mut self, engine: &mut Bridge, english: bool) -> Result<Option<String>, String> {
        if english && !self.bilingual {
            return Ok(None);
        }
        let Some(candidate) = self.frame.candidates.get(self.selected) else {
            return Ok(None);
        };
        let mut request = json!({"action":"commit", "input":self.normalized(), "candidate":candidate.text, "syllables":candidate.syllables});
        if self.remember {
            request["context"] = json!(self.memory_context);
            request["deferred_output"] = json!(true);
        }
        if english {
            let Some(sense) = candidate.translations.get(self.sense) else {
                return Ok(None);
            };
            request["english"] = json!(sense.word);
        }
        let study = if english {
            candidate.translations.get(self.sense).map(|sense| {
                crate::learning::Word::new(
                    sense.word.clone(),
                    candidate.text.clone(),
                    sense.pos.clone(),
                )
            })
        } else {
            None
        };
        let expected = if english {
            study.as_ref().map(|w| w.english.clone())
        } else {
            Some(candidate.text.clone())
        };
        let result = engine.call(request)?;
        if result.committed != expected {
            return Err("Invalid committed word".into());
        }
        if let Some(word) = &study
            && result.committed.as_deref() != Some(word.english.as_str())
        {
            return Err("Invalid English commit".into());
        }
        let consumed = self
            .raw
            .len()
            .checked_sub(result.input.len())
            .ok_or("Invalid remaining input")?;
        if self.normalized().get(consumed..) != Some(result.input.as_str()) {
            return Err("Invalid engine suffix".into());
        }
        self.pending_study = study.filter(|w| w.valid());
        if self.remember {
            self.pending_memory = Confirmation::new(&result, self.memory_context.clone(), !english);
        }
        self.raw = self.raw[consumed..].to_string();
        self.frame = result.clone();
        self.selected = 0;
        self.sense = 0;
        self.expanded = false;
        self.last_english = english;
        self.punctuation.confirm(if english {
            Language::English
        } else {
            Language::Chinese
        });
        Ok(result.committed)
    }

    pub fn commit_sentence(
        &mut self,
        engine: &mut Bridge,
        source: &str,
        english: &str,
    ) -> Result<Option<String>, String> {
        if !self.bilingual || !crate::remote::valid_english(english) {
            return Ok(None);
        }
        let Some(candidate) = self.frame.candidates.get(self.selected) else {
            return Ok(None);
        };
        if candidate.text != source || !candidate.translations.is_empty() {
            return Ok(None);
        }
        let mut request = json!({"action":"commit","input":self.normalized(),"candidate":candidate.text,"syllables":candidate.syllables});
        if self.remember {
            request["context"] = json!(self.memory_context);
            request["deferred_output"] = json!(true);
        }
        let mut result = engine.call(request)?;
        if result.committed.as_deref() != Some(source) {
            return Err("Invalid sentence candidate".into());
        }
        let consumed = self
            .raw
            .len()
            .checked_sub(result.input.len())
            .ok_or("Invalid remaining input")?;
        if self.normalized().get(consumed..) != Some(result.input.as_str()) {
            return Err("Invalid engine suffix".into());
        }
        // The real engine consumes the validated Chinese candidate. Only this
        // already-current translation replaces the host output; it is never a glossary sense.
        result.committed = Some(english.into());
        self.pending_study = None;
        self.pending_memory = if self.remember {
            Confirmation::new(&result, self.memory_context.clone(), false)
        } else {
            None
        };
        self.raw = self.raw[consumed..].to_owned();
        self.frame = result;
        self.selected = 0;
        self.sense = 0;
        self.expanded = false;
        self.last_english = true;
        self.punctuation.confirm(Language::English);
        Ok(Some(english.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Candidate;
    #[test]
    fn raw_return_keeps_case_and_clears_expansion() {
        let mut input = Input {
            raw: "SwIFT".into(),
            expanded: true,
            ..Input::default()
        };
        assert_eq!(input.raw_commit(), "SwIFT");
        assert!(input.raw.is_empty());
        assert!(!input.expanded);
    }
    #[test]
    fn paging_has_no_wrap_and_retains_absolute_indices() {
        let mut input = Input::default();
        input.frame.candidates = vec![Candidate::default(); 12];
        input.move_page(false);
        assert_eq!(input.selected, 0);
        input.move_page(true);
        assert_eq!(input.selected, 5);
        input.move_page(true);
        assert_eq!(input.selected, 10);
        input.move_page(true);
        assert_eq!(input.selected, 10);
        input.move_page(false);
        assert_eq!(input.selected, 5);
        assert_eq!(input.page_count(), 3);
    }
}
