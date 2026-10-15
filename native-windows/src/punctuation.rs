//! Session-only output language and character classes, never host text or input history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    Chinese,
    English,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Automatic,
    Chinese,
    English,
}
impl Mode {
    pub const ALL: [Self; 3] = [Self::Automatic, Self::Chinese, Self::English];
    pub fn title(self) -> &'static str {
        match self {
            Self::Automatic => "自动跟随输出语言",
            Self::Chinese => "固定中文标点",
            Self::English => "固定英文标点",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Symbol,
    Raw,
    CommitChinese,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Literal {
    #[default]
    None,
    Number,
    Latin,
}
#[derive(Clone, Debug)]
pub struct Punctuation {
    literal: Literal,
    double_opening: bool,
    single_opening: bool,
    applied_mode: Mode,
    language: Language,
}
impl Default for Punctuation {
    fn default() -> Self {
        Self {
            literal: Literal::None,
            double_opening: true,
            single_opening: true,
            applied_mode: Mode::Automatic,
            language: Language::Chinese,
        }
    }
}
impl Punctuation {
    pub fn mapped(key: char) -> Option<&'static str> {
        Some(match key {
            ',' => "，",
            '.' => "。",
            '?' => "？",
            '!' => "！",
            ':' => "：",
            ';' => "；",
            '(' => "（",
            ')' => "）",
            '[' => "【",
            ']' => "】",
            '<' => "《",
            '>' => "》",
            '/' | '\\' => "、",
            '"' | '\'' => "",
            '@' => "@",
            '_' => "_",
            _ => return None,
        })
    }
    pub fn action(key: char, input: &str, has_candidate: bool, mode: Mode) -> Option<Action> {
        if key == '\'' && !input.is_empty() {
            return None;
        }
        Self::mapped(key)?;
        if input.is_empty() {
            return Some(Action::Symbol);
        }
        let scheme = key == ':' && ["http", "https", "ftp", "file", "ws", "wss"].contains(&input);
        let www = input == "www" && key == '.';
        Some(
            if matches!(key, '@' | '_') || scheme || www || mode == Mode::English || !has_candidate
            {
                Action::Raw
            } else {
                Action::CommitChinese
            },
        )
    }
    pub fn language(&self, mode: Mode) -> Language {
        match mode {
            Mode::Automatic => self.language,
            Mode::Chinese => Language::Chinese,
            Mode::English => Language::English,
        }
    }
    pub fn is_latin(&self) -> bool {
        self.literal == Literal::Latin
    }
    pub fn confirm(&mut self, language: Language) {
        if language != self.language {
            self.reset_quotes();
        }
        self.language = language;
        self.literal = Literal::None;
    }
    pub fn observe_literal(&mut self, text: &str) {
        for ch in text.chars() {
            if ch.is_whitespace() {
                self.literal = Literal::None;
            } else if ch.is_ascii_alphabetic() {
                if self.language != Language::English {
                    self.reset_quotes();
                }
                self.language = Language::English;
                self.literal = Literal::Latin;
            } else if ch.is_ascii_digit() && self.literal != Literal::Latin {
                self.literal = Literal::Number;
            } else if self.literal != Literal::Latin && !".:/-+".contains(ch) {
                self.literal = Literal::None;
            }
        }
    }
    fn reset_quotes(&mut self) {
        self.double_opening = true;
        self.single_opening = true;
    }
    pub fn reset_literal(&mut self) {
        self.literal = Literal::None;
    }
    pub fn reset_context(&mut self) {
        self.reset_literal();
        self.reset_quotes();
    }
    pub fn render(&mut self, key: char, mode: Mode) -> Option<String> {
        let chinese = Self::mapped(key)?;
        if self.applied_mode != mode {
            self.reset_quotes();
            self.applied_mode = mode;
        }
        if self.literal == Literal::Latin
            || (mode != Mode::Chinese
                && self.literal == Literal::Number
                && matches!(key, '.' | ':' | '/'))
        {
            return Some(key.to_string());
        }
        self.literal = Literal::None;
        if self.language(mode) == Language::English {
            return Some(key.to_string());
        }
        Some(match key {
            '"' => {
                let text = if self.double_opening { "“" } else { "”" };
                self.double_opening = !self.double_opening;
                text.to_owned()
            }
            '\'' => {
                let text = if self.single_opening { "‘" } else { "’" };
                self.single_opening = !self.single_opening;
                text.to_owned()
            }
            _ => chinese.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_composition_clears_literal_classes_without_resetting_quotes_or_language() {
        let mut state = Punctuation::default();
        assert_eq!(state.render('"', Mode::Automatic).unwrap(), "“");
        assert_eq!(state.render('\'', Mode::Automatic).unwrap(), "‘");
        state.observe_literal("3");
        assert_eq!(state.render('.', Mode::Automatic).unwrap(), ".");
        state.reset_literal();
        assert_eq!(state.render('.', Mode::Automatic).unwrap(), "。");
        assert_eq!(state.render('"', Mode::Automatic).unwrap(), "”");
        assert_eq!(state.render('\'', Mode::Automatic).unwrap(), "’");
        state.observe_literal("https");
        assert!(state.is_latin());
        state.reset_literal();
        assert!(!state.is_latin());
        assert_eq!(state.language(Mode::Automatic), Language::English);
        assert_eq!(state.render(',', Mode::Automatic).unwrap(), ",");
        state.confirm(Language::Chinese);
        assert_eq!(state.render('"', Mode::Automatic).unwrap(), "“");
        state.reset_context();
        assert_eq!(state.render('"', Mode::Automatic).unwrap(), "“");
    }
    #[test]
    fn mappings_manual_modes_quotes_and_context_match_mac() {
        let keys = [
            (',', "，"),
            ('.', "。"),
            ('?', "？"),
            ('!', "！"),
            (':', "："),
            (';', "；"),
            ('(', "（"),
            (')', "）"),
            ('[', "【"),
            (']', "】"),
            ('<', "《"),
            ('>', "》"),
            ('/', "、"),
            ('\\', "、"),
        ];
        let mut state = Punctuation::default();
        for (key, expected) in keys {
            assert_eq!(state.render(key, Mode::Automatic).unwrap(), expected);
        }
        state.confirm(Language::English);
        for (key, _) in keys {
            assert_eq!(state.render(key, Mode::Automatic).unwrap(), key.to_string());
        }
        assert_eq!(state.render(',', Mode::Chinese).unwrap(), "，");
        state.confirm(Language::Chinese);
        assert_eq!(state.render('"', Mode::Automatic).unwrap(), "“");
        state.confirm(Language::Chinese);
        assert_eq!(state.render('"', Mode::Automatic).unwrap(), "”");
        assert_eq!(state.render('\'', Mode::Automatic).unwrap(), "‘");
        assert_eq!(state.render('\'', Mode::Automatic).unwrap(), "’");
        state.reset_context();
        assert_eq!(state.render('"', Mode::Automatic).unwrap(), "“");
        assert_eq!(state.render(',', Mode::English).unwrap(), ",");
    }
    #[test]
    fn numeric_addresses_and_syllable_separator_are_protected() {
        let mut state = Punctuation::default();
        state.observe_literal("3");
        assert_eq!(state.render('.', Mode::Automatic).unwrap(), ".");
        state.observe_literal("14");
        assert_eq!(state.render(',', Mode::Automatic).unwrap(), "，");
        state.observe_literal("12");
        assert_eq!(state.render(':', Mode::Automatic).unwrap(), ":");
        state.reset_context();
        assert_eq!(state.render('.', Mode::Automatic).unwrap(), "。");
        state.observe_literal("https");
        for key in [':', '/', '/', '.', '?', '@', '_', '\''] {
            assert_eq!(state.render(key, Mode::Chinese).unwrap(), key.to_string());
        }
        state.observe_literal(" ");
        state.confirm(Language::Chinese);
        assert_eq!(state.render(',', Mode::Automatic).unwrap(), "，");
        assert_eq!(Punctuation::action('\'', "xi", true, Mode::Automatic), None);
        assert_eq!(
            Punctuation::action(':', "https", true, Mode::Automatic),
            Some(Action::Raw)
        );
        assert_eq!(
            Punctuation::action('.', "www", true, Mode::Chinese),
            Some(Action::Raw)
        );
        assert_eq!(
            Punctuation::action('@', "test", true, Mode::Chinese),
            Some(Action::Raw)
        );
        assert_eq!(
            Punctuation::action(',', "nihao", true, Mode::Automatic),
            Some(Action::CommitChinese)
        );
        assert_eq!(
            Punctuation::action(',', "nihao", true, Mode::English),
            Some(Action::Raw)
        );
    }
}
