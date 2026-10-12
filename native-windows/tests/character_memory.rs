use lingomate_tsf::bridge::{Bridge, Confirmation};
use lingomate_tsf::input::{Input, Key};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    engine: Bridge,
    directory: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let directory = std::env::temp_dir().join(format!(
            "lingomate-cn-memory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self {
            engine: Self::start(directory.join("words.json")),
            directory,
        }
    }
    fn start(path: PathBuf) -> Bridge {
        Bridge::start_with_memory(
            &PathBuf::from(std::env::var("LINGOMATE_TEST_BRIDGE").unwrap()),
            &PathBuf::from(std::env::var("LINGOMATE_TEST_RESOURCES").unwrap()),
            Some(&path),
        )
        .unwrap()
    }
    fn restart(&mut self) {
        // A new owned bridge generation opens the same temporary store.
        self.engine = Self::start(self.directory.join("words.json"));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn choose(
    input: &mut Input,
    engine: &mut Bridge,
    pinyin: &str,
    word: &str,
) -> (String, Confirmation) {
    for ch in pinyin.chars() {
        input.handle(Key::Letter(ch), engine).unwrap();
    }
    input.selected = input
        .frame
        .candidates
        .iter()
        .position(|c| c.text == word)
        .unwrap();
    let output = input.handle(Key::Space(false), engine).unwrap().unwrap();
    let receipt = input.pending_memory.take().unwrap();
    assert_eq!(output, word);
    assert!(input.raw.is_empty());
    (output, receipt)
}
fn query(engine: &mut Bridge, pinyin: &str) -> Vec<String> {
    engine
        .call(serde_json::json!({"action":"query","input":pinyin}))
        .unwrap()
        .candidates
        .into_iter()
        .map(|c| c.text)
        .collect()
}

#[test]
fn queued_host_successes_learn_separate_characters_and_survive_restart() {
    let mut fixture = Fixture::new();
    let mut input = Input::default();
    let first = choose(&mut input, &mut fixture.engine, "gai", "钙");
    let second = choose(&mut input, &mut fixture.engine, "tai", "钛");
    let third = choose(&mut input, &mut fixture.engine, "kuang", "矿");
    assert!(!fixture.directory.join("words.json").exists());
    // Edit sessions may be batched after subsequent keyboard queries; receipts remain FIFO.
    for (output, receipt) in [first, second, third] {
        assert!(
            fixture
                .engine
                .confirm_output(&receipt, true, Some(&output))
                .unwrap()
        );
    }
    fixture.restart();
    assert_eq!(query(&mut fixture.engine, "gaitaikuang")[0], "钙钛矿");
    input.break_memory(&mut fixture.engine);
    for (pinyin, text) in [("gai", "钙"), ("tai", "钛"), ("nie", "镍")] {
        let (output, receipt) = choose(&mut input, &mut fixture.engine, pinyin, text);
        assert!(
            fixture
                .engine
                .confirm_output(&receipt, true, Some(&output))
                .unwrap()
        );
    }
    fixture.restart();
    assert_eq!(query(&mut fixture.engine, "gaitainie")[0], "钙钛镍");
}

#[test]
fn failed_or_mismatched_host_edits_and_stale_generations_never_learn() {
    let mut fixture = Fixture::new();
    let mut input = Input::default();
    let (output, receipt) = choose(&mut input, &mut fixture.engine, "gai", "钙");
    assert!(
        !fixture
            .engine
            .confirm_output(&receipt, false, Some(&output))
            .unwrap()
    );
    assert!(!fixture.directory.join("words.json").exists());
    input.break_memory(&mut fixture.engine);
    let (_, receipt) = choose(&mut input, &mut fixture.engine, "tai", "钛");
    assert!(
        !fixture
            .engine
            .confirm_output(&receipt, true, Some("another output"))
            .unwrap()
    );
    assert!(!fixture.directory.join("words.json").exists());
    input.break_memory(&mut fixture.engine);
    let (output, receipt) = choose(&mut input, &mut fixture.engine, "nie", "镍");
    fixture.restart();
    assert!(
        !fixture
            .engine
            .confirm_output(&receipt, true, Some(&output))
            .unwrap()
    );
    assert!(!fixture.directory.join("words.json").exists());
}

#[test]
fn punctuation_raw_return_and_pause_end_the_character_run() {
    let mut fixture = Fixture::new();
    let mut input = Input::default();
    let (output, receipt) = choose(&mut input, &mut fixture.engine, "nie", "镍");
    fixture
        .engine
        .confirm_output(&receipt, true, Some(&output))
        .unwrap();
    input
        .handle(Key::Punctuation(','), &mut fixture.engine)
        .unwrap();
    let (output, receipt) = choose(&mut input, &mut fixture.engine, "gai", "钙");
    fixture
        .engine
        .confirm_output(&receipt, true, Some(&output))
        .unwrap();
    assert!(
        !fixture
            .engine
            .call(serde_json::json!({"action":"query", "input":"niegai"}))
            .unwrap()
            .candidates
            .iter()
            .any(|c| c.text == "镍钙")
    );
    for ch in "AI".chars() {
        input.handle(Key::Letter(ch), &mut fixture.engine).unwrap();
    }
    assert_eq!(
        input
            .handle(Key::Return, &mut fixture.engine)
            .unwrap()
            .as_deref(),
        Some("AI")
    );
    input.break_memory(&mut fixture.engine);
    input.remember = false;
    for ch in "tai".chars() {
        input.handle(Key::Letter(ch), &mut fixture.engine).unwrap();
    }
    input
        .handle(Key::Space(false), &mut fixture.engine)
        .unwrap();
    assert!(input.pending_memory.is_none());
}

#[test]
fn already_queued_commits_can_finish_before_a_raw_boundary_without_crossing_it() {
    let mut fixture = Fixture::new();
    let mut input = Input::default();
    let receipts = [
        choose(&mut input, &mut fixture.engine, "gai", "钙"),
        choose(&mut input, &mut fixture.engine, "tai", "钛"),
        choose(&mut input, &mut fixture.engine, "kuang", "矿"),
    ];
    // A raw Return/space or focus boundary may be delivered before the asynchronous edit.
    input.finish_memory(&mut fixture.engine);
    for (output, receipt) in receipts {
        assert!(
            fixture
                .engine
                .confirm_output(&receipt, true, Some(&output))
                .unwrap()
        );
    }
    let (output, receipt) = choose(&mut input, &mut fixture.engine, "nie", "镍");
    fixture
        .engine
        .confirm_output(&receipt, true, Some(&output))
        .unwrap();
    fixture.restart();
    assert_eq!(query(&mut fixture.engine, "gaitaikuang")[0], "钙钛矿");
    let data = std::fs::read_to_string(fixture.directory.join("words.json")).unwrap();
    assert!(!data.contains("钙钛矿镍"));
}
