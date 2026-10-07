//! Local experimental adapter over Qingjian, GPL-3.0-or-later.
use qingjian_core::Engine;
use qingjian_core::candidate::{Candidate, Language, Sense, Translation};
use qingjian_dictionary::Dictionary;
use qingjian_translate::Glossary;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;

#[derive(Clone, Deserialize, Serialize)]
struct Detail {
    word: String,
    pos: String,
    note: String,
    example: String,
}

#[derive(Deserialize)]
struct Request {
    action: String,
    input: String,
    #[serde(default)]
    candidate: String,
    #[serde(default)]
    syllables: Vec<String>,
    #[serde(default)]
    english: Option<String>,
}

struct Adapter {
    engine: Engine,
    details: HashMap<String, Vec<Detail>>,
}

impl Adapter {
    fn load(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let data_path = |bundled: &str, research: &str| {
            let packaged = root.join(bundled);
            if packaged.is_file() {
                packaged
            } else {
                root.join(research)
            }
        };
        let dictionary = Dictionary::from_path(data_path(
            "dict.tsv",
            "upstream/qingjian/assets/lexicon/dict.tsv",
        ))?;
        let glossary = Glossary::from_path(
            Language::English,
            data_path(
                "glossary-en.tsv",
                "upstream/qingjian/assets/glossary/glossary-en.tsv",
            ),
        )?;
        Ok(Self {
            engine: Engine::new(dictionary).with_translator(Box::new(glossary)),
            details: serde_json::from_str(&std::fs::read_to_string(data_path(
                "details.json",
                "prototype/data/details.json",
            ))?)?,
        })
    }

    fn translations(&self, candidate: &Candidate) -> Vec<Detail> {
        if let Some(details) = self.details.get(&candidate.text) {
            return details.clone();
        }
        candidate.translation.as_ref().map_or_else(Vec::new, |t| {
            t.senses()
                .iter()
                .map(|sense| Detail {
                    word: sense.text.clone(),
                    pos: sense
                        .part_of_speech
                        .map_or_else(String::new, |p| p.to_string()),
                    note: "本地词表释义；暂未提供用法说明。".into(),
                    example: String::new(),
                })
                .collect()
        })
    }

    fn candidates(&self) -> Vec<Candidate> {
        let Ok(mut query) = self.engine.query() else {
            return Vec::new();
        };
        self.engine.annotate(&mut query.candidates);
        query
            .candidates
            .items
            .into_iter()
            .filter(|c| {
                c.text
                    .chars()
                    .any(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
            })
            .take(64)
            .collect()
    }

    fn respond(&mut self, request: Request) -> Result<Value, String> {
        if request.input.len() > 240
            || !request
                .input
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'\'')
        {
            return Err("请输入不超过 240 个拼音字母，可使用单引号分隔音节。".into());
        }
        if !matches!(request.action.as_str(), "query" | "commit") {
            return Err("不支持的操作".into());
        }
        self.engine.clear();
        for ch in request.input.chars() {
            self.engine.push(ch);
        }
        let mut committed = None;
        if request.action == "commit" {
            let mut candidate = self
                .candidates()
                .into_iter()
                .find(|c| c.text == request.candidate && c.syllables == request.syllables)
                .ok_or("候选已变化，请重新选择。")?;
            if let Some(english) = request.english {
                let detail = self
                    .translations(&candidate)
                    .into_iter()
                    .find(|d| d.word == english)
                    .ok_or("该译法不属于当前候选。")?;
                // Detail lookup is separate from the compact two-sense annotation.
                // Submit exactly the selected, validated sense through the real engine.
                candidate.translation = Some(Translation::new(
                    Language::English,
                    vec![Sense {
                        text: detail.word,
                        part_of_speech: detail.pos.parse().ok(),
                        reading: None,
                        fresh: false,
                    }],
                ));
                committed = self.engine.commit_translation(&candidate, 0);
                if committed.is_none() {
                    return Err("译法提交失败".into());
                }
            } else {
                committed = Some(self.engine.commit(&candidate));
            }
        }
        let candidates: Vec<_> = self
            .candidates()
            .iter()
            .map(|c| {
                json!({
                    "text": c.text,
                    "syllables": c.syllables,
                    "translations": self.translations(c),
                    "hasDetails": self.details.contains_key(&c.text),
                })
            })
            .collect();
        let marked = self
            .engine
            .query()
            .map(|q| q.marked_text())
            .unwrap_or_default();
        Ok(
            json!({"input": self.engine.composition().text(), "marked": marked,
            "candidates": candidates, "committed": committed}),
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("research root argument required")?;
    let mut adapter = Adapter::load(Path::new(&root))?;
    eprintln!(
        "Qingjian adapter ready; details={} words; local-only",
        adapter.details.len()
    );
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let response = match serde_json::from_str::<Request>(&line?) {
            Ok(request) => adapter
                .respond(request)
                .unwrap_or_else(|e| json!({"error": e})),
            Err(_) => json!({"error": "请求格式错误"}),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
