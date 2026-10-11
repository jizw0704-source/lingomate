//! Display metadata is separate from English output and engine commit validation.
use std::collections::HashMap;
use std::path::Path;

pub struct PartsOfSpeech(HashMap<String, String>);
impl PartsOfSpeech {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let mut index = HashMap::new();
        for line in std::fs::read_to_string(path)?.lines() {
            if let Some(metadata) = line.strip_prefix("# POS\t") {
                let Some((word, pos)) = metadata.split_once('\t') else {
                    continue;
                };
                if !word.is_empty() && valid(pos) {
                    index.insert(word.to_owned(), pos.to_owned());
                }
            }
        }
        Ok(Self(index))
    }
    pub fn get(&self, word: &str) -> &str {
        self.0.get(word).map_or("", String::as_str)
    }
}
fn valid(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('/')
            .all(|p| ["n.", "v.", "adj.", "adv."].contains(&p))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_is_only_english_abbreviations() {
        assert!(valid("n./v."));
        assert!(!valid("名词"));
        assert!(!valid(""));
        assert!(!valid("n.\tother"));
        let index = PartsOfSpeech(HashMap::from([("study".into(), "n./v.".into())]));
        assert_eq!(index.get("study"), "n./v.");
        assert_eq!(index.get("unknown"), "");
    }
}
