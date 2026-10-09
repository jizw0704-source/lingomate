use lingomate_tsf::bridge::Bridge;
use lingomate_tsf::input::{Input, Key};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};
fn engine() -> Bridge {
    let executable = PathBuf::from(
        std::env::var("LINGOMATE_TEST_BRIDGE")
            .expect("Set LINGOMATE_TEST_BRIDGE to the real bridge executable"),
    );
    let root = PathBuf::from(
        std::env::var("LINGOMATE_TEST_RESOURCES")
            .expect("Set LINGOMATE_TEST_RESOURCES to the fixture resources"),
    );
    Bridge::start(&executable, &root).unwrap()
}
fn type_letters(input: &mut Input, engine: &mut Bridge, text: &str) {
    for ch in text.chars() {
        assert!(input.handle(Key::Letter(ch), engine).unwrap().is_none());
    }
}
#[test]
fn chinese_english_and_third_sense_use_real_engine_validation() {
    let mut engine = engine();
    for (sense, expected) in [(None, "学习"), (Some(0), "study"), (Some(2), "learning")] {
        let mut input = Input::default();
        type_letters(&mut input, &mut engine, "xuexi");
        input.selected = input
            .frame
            .candidates
            .iter()
            .position(|c| c.text == "学习")
            .unwrap();
        input.sense = sense.unwrap_or(0);
        assert_eq!(
            input
                .handle(Key::Space(sense.is_some()), &mut engine)
                .unwrap()
                .as_deref(),
            Some(expected)
        );
        assert!(input.raw.is_empty());
    }
}
#[test]
fn mixed_input_preserves_case_without_a_mode_switch() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "wo");
    let mut output = input
        .handle(Key::Space(false), &mut engine)
        .unwrap()
        .unwrap();
    type_letters(&mut input, &mut engine, "AI");
    output += &input.handle(Key::Return, &mut engine).unwrap().unwrap();
    type_letters(&mut input, &mut engine, "xuexi");
    output += &input
        .handle(Key::Space(false), &mut engine)
        .unwrap()
        .unwrap();
    assert_eq!(output, "我AI学习");
    assert!(!input.english);
}
#[test]
fn partial_commit_preserves_original_case_suffix() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "kaiFaZhe");
    input.selected = input
        .frame
        .candidates
        .iter()
        .position(|c| c.text == "开发")
        .unwrap();
    assert_eq!(
        input
            .handle(Key::Space(false), &mut engine)
            .unwrap()
            .as_deref(),
        Some("开发")
    );
    assert_eq!(input.raw, "Zhe");
    assert_eq!(
        input.handle(Key::Return, &mut engine).unwrap().as_deref(),
        Some("Zhe")
    );
}
#[test]
fn page_digits_select_the_displayed_page_and_ignore_empty_slots() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "shi");
    input.move_page(true);
    let expected = input.frame.candidates[5].text.clone();
    assert_eq!(
        input.handle(Key::Digit(0), &mut engine).unwrap().as_deref(),
        Some(expected.as_str())
    );
    type_letters(&mut input, &mut engine, "shi");
    while input.page() + 1 < input.page_count() {
        input.move_page(true);
    }
    assert!(input.handle(Key::Digit(4), &mut engine).unwrap().is_none());
    assert_eq!(input.raw, "shi");
}
#[test]
fn punctuation_follows_confirmed_language() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "nihao");
    assert_eq!(
        input
            .handle(Key::Punctuation(','), &mut engine)
            .unwrap()
            .as_deref(),
        Some("你好，")
    );
    type_letters(&mut input, &mut engine, "xuexi");
    input.handle(Key::Space(true), &mut engine).unwrap();
    assert_eq!(
        input
            .handle(Key::Punctuation(','), &mut engine)
            .unwrap()
            .as_deref(),
        Some(",")
    );
}
#[test]
fn fabricated_translation_is_rejected() {
    let mut engine = engine();
    let frame = engine
        .call(json!({"action":"query", "input":"xuexi"}))
        .unwrap();
    let candidate = frame.candidates.iter().find(|c| c.text == "学习").unwrap();
    assert!(engine.call(json!({"action":"commit", "input":"xuexi", "candidate":"学习", "syllables":candidate.syllables, "english":"fabricated"})).is_err());
}
#[test]
fn hung_invalid_and_oversized_engine_responses_are_bounded() {
    let fixture = PathBuf::from(env!("CARGO_BIN_EXE_lingomate-fixture"));
    for mode in ["hang", "bad", "huge"] {
        let mut bridge = Bridge::start(&fixture, &PathBuf::from(mode)).unwrap();
        let start = Instant::now();
        assert!(
            bridge
                .call(json!({"action":"query", "input":"xuexi"}))
                .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(2));
        drop(bridge);
    }
    assert!(
        !engine()
            .call(json!({"action":"query", "input":"xuexi"}))
            .unwrap()
            .candidates
            .is_empty()
    );
}
