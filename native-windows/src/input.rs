use crate::bridge::{Bridge, Frame};
use serde_json::json;

pub const PAGE_SIZE: usize = 5;
#[derive(Clone, Copy, Debug)]
pub enum Key {
    Letter(char),
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
}
#[derive(Clone, Default)]
pub struct Input {
    pub raw: String,
    pub frame: Frame,
    pub selected: usize,
    pub sense: usize,
    pub expanded: bool,
    pub english: bool,
    pub error: bool,
    pub last_english: bool,
}
impl Input {
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
        self.raw.clear();
        self.frame = Frame::default();
        self.selected = 0;
        self.sense = 0;
        self.expanded = false;
        self.error = false;
    }
    pub fn raw_commit(&mut self) -> String {
        let text = self.raw.clone();
        self.reset();
        self.last_english = true;
        text
    }
    pub fn handle(&mut self, key: Key, engine: &mut Bridge) -> Result<Option<String>, String> {
        match key {
            Key::Letter(ch) => {
                if self.raw.len() < 240 && (ch.is_ascii_alphabetic() || ch == '\'') {
                    self.raw.push(ch);
                    self.refresh(engine)?;
                }
            }
            Key::Backspace => {
                self.raw.pop();
                self.refresh(engine)?;
            }
            Key::Return => return Ok(Some(self.raw_commit())),
            Key::Escape => {
                if self.expanded {
                    self.expanded = false;
                } else {
                    self.reset();
                }
            }
            Key::Tab(back) => {
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
                let chosen = if self.raw.is_empty() {
                    None
                } else {
                    self.commit(engine, false)?
                };
                if !self.raw.is_empty() {
                    return Ok(chosen);
                }
                let punctuation = if self.last_english {
                    ch
                } else {
                    match ch {
                        ',' => '，',
                        '.' => '。',
                        '?' => '？',
                        '!' => '！',
                        ':' => '：',
                        ';' => '；',
                        _ => ch,
                    }
                };
                return Ok(Some(format!("{}{punctuation}", chosen.unwrap_or_default())));
            }
        }
        Ok(None)
    }
    fn refresh(&mut self, engine: &mut Bridge) -> Result<(), String> {
        self.frame = engine.call(json!({"action":"query", "input":self.normalized()}))?;
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
        let Some(candidate) = self.frame.candidates.get(self.selected) else {
            return Ok(None);
        };
        let mut request = json!({"action":"commit", "input":self.normalized(), "candidate":candidate.text, "syllables":candidate.syllables});
        if english {
            let Some(sense) = candidate.translations.get(self.sense) else {
                return Ok(None);
            };
            request["english"] = json!(sense.word);
        }
        let result = engine.call(request)?;
        let consumed = self
            .raw
            .len()
            .checked_sub(result.input.len())
            .ok_or("Invalid remaining input")?;
        if self.normalized().get(consumed..) != Some(result.input.as_str()) {
            return Err("Invalid engine suffix".into());
        }
        self.raw = self.raw[consumed..].to_string();
        self.frame = result.clone();
        self.selected = 0;
        self.sense = 0;
        self.expanded = false;
        self.last_english = english;
        Ok(result.committed)
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
