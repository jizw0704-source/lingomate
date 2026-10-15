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
fn sentence_output_is_engine_validated_without_word_learning() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "jintianxiawukaihui");
    input.selected = input
        .frame
        .candidates
        .iter()
        .position(|c| c.text == "今天下午开会" && c.translations.is_empty())
        .expect("real sentence candidate");
    let selected = input.selected;
    let raw = input.raw.clone();
    assert!(
        input
            .commit_sentence(&mut engine, "错误的候选", "An obsolete result.")
            .unwrap()
            .is_none()
    );
    assert_eq!(input.raw, raw);
    assert_eq!(input.selected, selected);
    assert_eq!(
        input
            .commit_sentence(
                &mut engine,
                "今天下午开会",
                "There is a meeting this afternoon."
            )
            .unwrap()
            .as_deref(),
        Some("There is a meeting this afternoon.")
    );
    assert!(input.raw.is_empty());
    assert!(input.pending_study.is_none());
    assert!(input.last_english);
}
#[test]
fn translated_partial_engine_commit_retains_original_suffix_case() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "kaiFaZhe");
    input.selected = input
        .frame
        .candidates
        .iter()
        .position(|c| c.text == "开发")
        .unwrap();
    // Isolate the engine's partial-consumption contract. Production eligibility
    // uses a candidate without offline senses; this fixture leaves the engine's
    // real Chinese candidate and syllable count unchanged.
    input.frame.candidates[input.selected].translations.clear();
    assert_eq!(
        input
            .commit_sentence(&mut engine, "开发", "Develop.")
            .unwrap()
            .as_deref(),
        Some("Develop.")
    );
    assert_eq!(input.raw, "Zhe");
    assert!(input.pending_study.is_none());
    assert!(input.last_english);
    assert_eq!(
        input.handle(Key::Return, &mut engine).unwrap().as_deref(),
        Some("Zhe")
    );
}
#[test]
fn bilingual_toggle_retains_candidate_and_chinese_only_rejects_english_output() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "xuexi");
    let selected = input.selected;
    let chinese = input.frame.candidates[selected].text.clone();
    input.handle(Key::ToggleBilingual, &mut engine).unwrap();
    assert!(!input.bilingual);
    assert_eq!(input.raw, "xuexi");
    assert_eq!(input.frame.candidates[selected].text, chinese);
    assert!(
        input
            .handle(Key::Space(true), &mut engine)
            .unwrap()
            .is_none()
    );
    assert_eq!(input.raw, "xuexi");
    assert_eq!(
        input
            .handle(Key::Space(false), &mut engine)
            .unwrap()
            .as_deref(),
        Some(chinese.as_str())
    );
    assert!(input.raw.is_empty());
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
fn literal_commits_remaining_raw_before_the_character() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "Wo");
    assert_eq!(
        input
            .handle(Key::Literal('6'), &mut engine)
            .unwrap()
            .as_deref(),
        Some("Wo6")
    );
    assert!(input.raw.is_empty());
    assert!(input.pending_memory.is_none());
    assert!(input.pending_study.is_none());
    assert_eq!(
        input
            .handle(Key::Punctuation('.'), &mut engine)
            .unwrap()
            .as_deref(),
        Some(".")
    );
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
fn correcting_pinyin_preserves_quotes_around_confirmed_chinese() {
    let mut engine = engine();
    for (key, opening, closing) in [('"', "“", "”"), ('\'', "‘", "’")] {
        let mut input = Input::default();
        assert_eq!(
            input
                .handle(Key::Punctuation(key), &mut engine)
                .unwrap()
                .as_deref(),
            Some(opening)
        );
        type_letters(&mut input, &mut engine, "nihaoo");
        input.handle(Key::Backspace, &mut engine).unwrap();
        assert_eq!(input.raw, "nihao");
        assert_eq!(
            input
                .handle(Key::Space(false), &mut engine)
                .unwrap()
                .as_deref(),
            Some("你好")
        );
        assert_eq!(
            input
                .handle(Key::Punctuation(key), &mut engine)
                .unwrap()
                .as_deref(),
            Some(closing)
        );
    }
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

#[test]
fn slow_first_reply_has_a_startup_budget_but_warm_requests_remain_bounded() {
    let fixture = PathBuf::from(env!("CARGO_BIN_EXE_lingomate-fixture"));
    for action in ["query", "cancel"] {
        let mut bridge = Bridge::start(&fixture, &PathBuf::from("slow-start-and-reply")).unwrap();
        let first = Instant::now();
        assert!(bridge.call(json!({"action":action, "input":""})).is_ok());
        assert!(first.elapsed() >= Duration::from_millis(700));
        let warm = Instant::now();
        assert!(bridge.call(json!({"action":"query", "input":""})).is_err());
        assert!(warm.elapsed() < Duration::from_secs(1));
        assert!(bridge.call(json!({"action":"query", "input":""})).is_err());
    }
}

#[test]
fn replacement_engine_recovers_retained_composition_before_candidate_actions() {
    for (key, expected) in [(Key::Space(false), "学习"), (Key::Space(true), "study")] {
        let mut engine = engine();
        let mut input = Input {
            raw: "XueXI".into(),
            error: true,
            ..Input::default()
        };
        assert_eq!(
            input.handle(key, &mut engine).unwrap().as_deref(),
            Some(expected)
        );
        assert!(input.raw.is_empty());
        assert!(!input.error);
    }
    for key in [Key::Tab(false), Key::Next] {
        let mut engine = engine();
        let mut input = Input {
            raw: "xuexi".into(),
            error: true,
            ..Input::default()
        };
        assert!(input.handle(key, &mut engine).unwrap().is_none());
        assert!(!input.frame.candidates.is_empty());
        assert!(!input.error);
        assert_eq!(input.raw, "xuexi");
        if matches!(key, Key::Tab(_)) {
            assert!(input.expanded);
        } else {
            assert_eq!(input.page(), 1);
        }
    }
}

#[test]
fn learning_metadata_only_follows_validated_english_candidate_commit() {
    let mut engine = engine();
    let mut input = Input::default();
    type_letters(&mut input, &mut engine, "xuexi");
    assert!(input.pending_study.is_none());
    input.handle(Key::Escape, &mut engine).unwrap();
    assert!(input.pending_study.is_none());
    type_letters(&mut input, &mut engine, "AI");
    input.handle(Key::Return, &mut engine).unwrap();
    assert!(input.pending_study.is_none());
    type_letters(&mut input, &mut engine, "xuexi");
    input.handle(Key::Space(false), &mut engine).unwrap();
    assert!(input.pending_study.is_none());
    type_letters(&mut input, &mut engine, "xuexi");
    input.selected = input
        .frame
        .candidates
        .iter()
        .position(|c| c.text == "学习")
        .unwrap();
    let output = input
        .handle(Key::Space(true), &mut engine)
        .unwrap()
        .unwrap();
    let word = input.pending_study.take().unwrap();
    assert_eq!(word.english, output);
    assert_eq!(word.chinese, "学习");
    assert!(!word.mastered);
    type_letters(&mut input, &mut engine, "xuexi");
    input.frame.candidates[input.selected].translations[0].word = "fabricated".into();
    assert!(input.handle(Key::Space(true), &mut engine).is_err());
    assert!(input.pending_study.is_none());
}

#[test]
fn extended_punctuation_raw_addresses_quotes_and_numbers_use_real_engine() {
    use lingomate_tsf::punctuation::Mode;
    let mut engine = engine();
    let mut input = Input::default();
    assert_eq!(
        input
            .handle(Key::Punctuation('"'), &mut engine)
            .unwrap()
            .as_deref(),
        Some("“")
    );
    type_letters(&mut input, &mut engine, "xuexi");
    assert_eq!(
        input
            .handle(Key::Space(false), &mut engine)
            .unwrap()
            .as_deref(),
        Some("学习")
    );
    assert_eq!(
        input
            .handle(Key::Punctuation('"'), &mut engine)
            .unwrap()
            .as_deref(),
        Some("”")
    );
    for (key, expected) in [
        ('(', "（"),
        (')', "）"),
        ('[', "【"),
        (']', "】"),
        ('<', "《"),
        ('>', "》"),
        ('/', "、"),
        ('\\', "、"),
    ] {
        assert_eq!(
            input
                .handle(Key::Punctuation(key), &mut engine)
                .unwrap()
                .as_deref(),
            Some(expected)
        );
    }
    input.handle(Key::Literal('3'), &mut engine).unwrap();
    assert_eq!(
        input
            .handle(Key::Punctuation('.'), &mut engine)
            .unwrap()
            .as_deref(),
        Some(".")
    );
    input.handle(Key::Literal(' '), &mut engine).unwrap();
    type_letters(&mut input, &mut engine, "https");
    let mut url = input
        .handle(Key::Punctuation(':'), &mut engine)
        .unwrap()
        .unwrap();
    for key in [
        Key::Punctuation('/'),
        Key::Punctuation('/'),
        Key::Letter('A'),
        Key::Punctuation('.'),
        Key::Letter('c'),
        Key::Punctuation('/'),
    ] {
        url += &input.handle(key, &mut engine).unwrap().unwrap();
    }
    assert_eq!(url, "https://A.c/");
    input.handle(Key::Literal(' '), &mut engine).unwrap();
    type_letters(&mut input, &mut engine, "xuexi");
    assert_eq!(
        input
            .handle(Key::Space(false), &mut engine)
            .unwrap()
            .as_deref(),
        Some("学习")
    );
    assert_eq!(
        input
            .handle(Key::Punctuation(','), &mut engine)
            .unwrap()
            .as_deref(),
        Some("，")
    );
    type_letters(&mut input, &mut engine, "test");
    assert_eq!(
        input
            .handle(Key::Punctuation('@'), &mut engine)
            .unwrap()
            .as_deref(),
        Some("test@")
    );
    input.handle(Key::Literal(' '), &mut engine).unwrap();
    input.punctuation_mode = Mode::English;
    type_letters(&mut input, &mut engine, "nihao");
    assert_eq!(
        input
            .handle(Key::Punctuation(','), &mut engine)
            .unwrap()
            .as_deref(),
        Some("nihao,")
    );
    input.handle(Key::Literal(' '), &mut engine).unwrap();
    input.punctuation_mode = Mode::Chinese;
    type_letters(&mut input, &mut engine, "AI");
    assert_eq!(
        input.handle(Key::Return, &mut engine).unwrap().as_deref(),
        Some("AI")
    );
    assert_eq!(
        input
            .handle(Key::Punctuation(','), &mut engine)
            .unwrap()
            .as_deref(),
        Some("，")
    );
    type_letters(&mut input, &mut engine, "xi'an");
    assert_eq!(input.raw, "xi'an");
}
#[test]
fn punctuation_does_not_drop_partial_composition_or_add_learning() {
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
            .handle(Key::Punctuation(','), &mut engine)
            .unwrap()
            .as_deref(),
        Some("开发")
    );
    assert_eq!(input.raw, "Zhe");
    assert!(input.pending_study.is_none());
    input.handle(Key::Escape, &mut engine).unwrap();
    type_letters(&mut input, &mut engine, "test");
    input.handle(Key::Punctuation('@'), &mut engine).unwrap();
    assert!(input.pending_study.is_none());
    assert!(input.pending_memory.is_none());
}
