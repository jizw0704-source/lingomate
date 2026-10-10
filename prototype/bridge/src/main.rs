//! Local experimental adapter over Qingjian, GPL-3.0-or-later.
use qingjian_core::Engine;
use qingjian_core::candidate::{Candidate, Language, Sense, Translation};
use qingjian_dictionary::Dictionary;
use qingjian_translate::Glossary;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::Path;
mod memory;
mod ranking;

struct Pending {
    token: u64,
    input: String,
    remaining: String,
    candidate: Candidate,
    chinese: bool,
}

struct Chain {
    expected: String,
    input: String,
    text: String,
    syllables: Vec<String>,
    parts: usize,
}

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
    #[serde(default)]
    context: String,
    #[serde(default)]
    learning_token: Option<u64>,
    #[serde(default)]
    chinese_output: Option<bool>,
}

struct Adapter {
    engine: Engine,
    details: HashMap<String, Vec<Detail>>,
    memory: memory::Memory,
    ranking: ranking::Ranking,
    pending: HashMap<String, Pending>,
    chains: HashMap<String, Chain>,
    token: u64,
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
        let dictionary_path = data_path("dict.tsv", "data/generated/dict.tsv");
        let dictionary = Dictionary::from_path(&dictionary_path)?;
        let glossary = Glossary::from_path(
            Language::English,
            data_path("glossary-en.tsv", "data/generated/glossary-en.tsv"),
        )?;
        Ok(Self {
            engine: Engine::new(dictionary).with_translator(Box::new(glossary)),
            details: serde_json::from_str(&std::fs::read_to_string(data_path(
                "details.json",
                "prototype/data/details.json",
            ))?)?,
            memory: memory::Memory::open(None),
            ranking: ranking::Ranking::load(&dictionary_path)?,
            pending: HashMap::new(),
            chains: HashMap::new(),
            token: 0,
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
        let input = self.engine.composition().text();
        let mut items = self
            .engine
            .query()
            .map_or_else(|_| Vec::new(), |q| q.candidates.items);
        let mut seen: HashSet<_> = items
            .iter()
            .map(|c| (c.text.clone(), c.syllables.clone()))
            .collect();
        for remembered in self.memory.candidates(input) {
            if seen.insert((remembered.text.clone(), remembered.syllables.clone())) {
                items.push(remembered);
            }
        }
        self.ranking.order(input, &mut items, &self.memory);
        let mut list = qingjian_core::candidate::CandidateList { items };
        self.engine.annotate(&mut list);
        list.items
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
        if !matches!(
            request.action.as_str(),
            "query" | "commit" | "confirm" | "cancel"
        ) {
            return Err("不支持的操作".into());
        }
        if request.context.len() > 64 {
            return Err("无效输入会话。".into());
        }
        if request.action == "cancel" {
            self.pending.remove(&request.context);
            self.chains.remove(&request.context);
            return Ok(self.frame(None, None));
        }
        if request.action == "confirm" {
            let token = request.learning_token.ok_or("缺少选词确认。")?;
            if self
                .pending
                .get(&request.context)
                .is_none_or(|p| p.token != token)
            {
                return Err("选词确认已失效。".into());
            }
            let mut pending = self.pending.remove(&request.context).unwrap();
            pending.chinese &= request.chinese_output.unwrap_or(true);
            self.confirm(&request.context, pending);
            return Ok(self.frame(None, None));
        }
        if self
            .chains
            .get(&request.context)
            .is_some_and(|c| c.expected != request.input)
        {
            self.chains.remove(&request.context);
        }
        // A query/edit before confirmation invalidates the unconfirmed receipt.
        self.pending.remove(&request.context);
        self.engine.clear();
        for ch in request.input.chars() {
            self.engine.push(ch);
        }
        let mut committed = None;
        let mut learning_token = None;
        if request.action == "commit" {
            let chinese = request.english.is_none();
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
            if self.memory.enabled() && !request.context.is_empty() {
                self.token = self.token.saturating_add(1);
                learning_token = Some(self.token);
                if self.pending.len() >= 128 || self.chains.len() >= 128 {
                    self.pending.clear();
                    self.chains.clear();
                }
                self.pending.insert(
                    request.context.clone(),
                    Pending {
                        token: self.token,
                        input: request.input.clone(),
                        remaining: self.engine.composition().text().to_owned(),
                        candidate,
                        chinese,
                    },
                );
            }
        }
        Ok(self.frame(committed, learning_token))
    }

    fn confirm(&mut self, context: &str, pending: Pending) {
        let consumed = pending
            .input
            .strip_suffix(&pending.remaining)
            .unwrap_or(&pending.input);
        self.memory.remember(consumed, &pending.candidate);
        if pending.chinese {
            let previous = self
                .chains
                .remove(context)
                .filter(|c| c.expected == pending.input);
            let mut chain = previous.unwrap_or(Chain {
                expected: String::new(),
                input: pending.input.clone(),
                text: String::new(),
                syllables: Vec::new(),
                parts: 0,
            });
            chain.text.push_str(&pending.candidate.text);
            chain.syllables.extend(pending.candidate.syllables);
            chain.parts += 1;
            if pending.remaining.is_empty() {
                if chain.parts > 1 {
                    self.memory.remember(
                        &chain.input,
                        &Candidate {
                            text: chain.text,
                            syllables: chain.syllables,
                            kind: qingjian_core::candidate::CandidateKind::Chinese,
                            reading: None,
                            translation: None,
                            aux_code: None,
                        },
                    );
                }
            } else {
                chain.expected = pending.remaining;
                self.chains.insert(context.to_owned(), chain);
            }
        } else {
            self.chains.remove(context);
        }
        self.memory.save();
    }

    fn frame(&self, committed: Option<String>, learning_token: Option<u64>) -> Value {
        let remembered = self.memory.candidates(self.engine.composition().text());
        let candidates: Vec<_> = self
            .candidates()
            .iter()
            .map(|c| {
                json!({
                    "text": c.text,
                    "syllables": c.syllables,
                    "translations": self.translations(c),
                    "hasDetails": self.details.contains_key(&c.text),
                    "personal": remembered.iter().any(|m| m.text == c.text && m.syllables == c.syllables),
                })
            })
            .collect();
        let marked = self
            .engine
            .query()
            .map(|q| q.marked_text())
            .unwrap_or_default();
        json!({"input": self.engine.composition().text(), "marked": marked,
            "candidates": candidates, "committed": committed,
            "learningToken": learning_token, "memoryWarning": self.memory.warning})
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("research root argument required")?;
    let mut adapter = Adapter::load(Path::new(&root))?;
    let mut arguments = std::env::args().skip(2);
    if let Some(flag) = arguments.next() {
        if flag != "--memory" {
            return Err("unsupported bridge option".into());
        }
        let path = arguments.next().ok_or("memory path required")?;
        adapter.memory = memory::Memory::open(Some(path.into()));
        if arguments.next().is_some() {
            return Err("unexpected bridge option".into());
        }
    }
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
