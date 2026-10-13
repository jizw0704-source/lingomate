//! Real dictionary coverage, partial consumption, receipt persistence and legacy policy.
use lingomate_engine::Adapter;
use serde_json::{Value, json};
use std::path::PathBuf;

fn adapter(focused: bool) -> Adapter {
    let root = std::env::var_os("LINGOMATE_TEST_RESOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let mut adapter = Adapter::load(&root).expect("prepared test resources required");
    if focused {
        adapter.focus_candidates();
    }
    adapter
}
fn query(adapter: &mut Adapter, input: &str) -> Value {
    adapter.request_json(&json!({"action":"query","input":input}).to_string())
}
fn candidate(frame: &Value, text: &str) -> Value {
    frame["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["text"] == text)
        .unwrap_or_else(|| panic!("missing {text}"))
        .clone()
}

#[test]
fn exact_words_rare_single_characters_and_abbreviations_survive_reduced_pages() {
    let mut focused = adapter(true);
    let mut legacy = adapter(false);
    for (input, first) in [("xuexi", "学习"), ("zhongguo", "中国"), ("wo", "我")] {
        let old = query(&mut legacy, input);
        let new = query(&mut focused, input);
        assert_eq!(old["candidates"].as_array().unwrap().len(), 64);
        assert!(new["candidates"].as_array().unwrap().len() < 64);
        assert_eq!(new["candidates"][0]["text"], first);
        assert!(!new["candidates"].as_array().unwrap().is_empty());
    }
    let shi = query(&mut focused, "shi");
    for text in ["市", "示", "是", "十"] {
        candidate(&shi, text);
    }
    for (input, text) in [("zg", "中国"), ("nih", "你好"), ("xi'an", "西安")] {
        candidate(&query(&mut focused, input), text);
    }
    let sentence = query(&mut focused, "xuexizhongwen");
    candidate(&sentence, "学习中文");
    candidate(&sentence, "学习");
    candidate(&query(&mut focused, "xuexi"), "学");
    candidate(&query(&mut focused, "zhongguo"), "中");
    candidate(&query(&mut focused, "gaitaikuang"), "钙");
    candidate(&query(&mut focused, "lue"), "略");
}

#[test]
fn filtered_partial_and_english_choices_commit_through_the_real_engine() {
    let mut focused = adapter(true);
    for english in [false, true] {
        let frame = query(&mut focused, "kaifazhe");
        let c = candidate(&frame, "开发");
        let mut request = json!({"action":"commit","input":"kaifazhe","candidate":"开发","syllables":c["syllables"]});
        if english {
            request["english"] = json!("develop");
        }
        let reply = focused.request_json(&request.to_string());
        assert_eq!(reply["committed"], if english { "develop" } else { "开发" });
        assert_eq!(reply["input"], "zhe");
    }
    let c = candidate(&query(&mut focused, "xuexi"), "学习");
    let valid = json!({"action":"commit","input":"xuexi","candidate":"学习","syllables":c["syllables"],"english":"learning"});
    assert_eq!(
        focused.request_json(&valid.to_string())["committed"],
        "learning"
    );
    let mut invalid = valid;
    invalid["english"] = json!("fabricated translation");
    assert!(focused.request_json(&invalid.to_string())["error"].is_string());
}

#[test]
fn personal_word_keeps_priority_across_focused_engine_restart() {
    let dir = std::env::temp_dir().join(format!(
        "focused-memory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("words.json");
    let mut focused = adapter(true);
    focused.set_memory_path(Some(path.clone()));
    let c = candidate(&query(&mut focused, "shi"), "示");
    let commit = focused.request_json(&json!({"action":"commit","input":"shi","candidate":"示","syllables":c["syllables"],"context":"focused-test"}).to_string());
    assert_eq!(commit["committed"], "示");
    let confirm = focused.request_json(&json!({"action":"confirm","input":"","context":"focused-test","learning_token":commit["learningToken"],"chinese_output":true}).to_string());
    assert!(confirm.get("error").is_none());
    drop(focused);
    let mut restarted = adapter(true);
    restarted.set_memory_path(Some(path));
    assert_eq!(query(&mut restarted, "shi")["candidates"][0]["text"], "示");
    let segmented = query(&mut restarted, "shijie");
    assert_eq!(segmented["candidates"][0]["text"], "世界");
    let first_single = segmented["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["text"].as_str().unwrap().chars().count() == 1)
        .unwrap();
    assert_eq!(first_single["text"], "示");
    drop(restarted);
    std::fs::remove_dir_all(dir).unwrap();
}
