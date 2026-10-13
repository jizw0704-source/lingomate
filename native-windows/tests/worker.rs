//! Owned temporary fixtures verify worker ordering and host-thread responsiveness.
use lingomate_tsf::bridge::{Bridge, Confirmation, EngineFailure};
use lingomate_tsf::input::{Input, Key};
use lingomate_tsf::punctuation::Mode;
use lingomate_tsf::worker::{Command, EnqueueError, QUEUE_CAPACITY, Reply, Selection, Worker};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

fn real_worker(memory: Option<PathBuf>) -> Worker {
    let executable = PathBuf::from(std::env::var("LINGOMATE_TEST_BRIDGE").unwrap());
    let resources = PathBuf::from(std::env::var("LINGOMATE_TEST_RESOURCES").unwrap());
    Worker::start_with(move || start_engine(&executable, &resources, memory.as_deref()))
}
fn start_engine(
    executable: &Path,
    resources: &Path,
    memory: Option<&Path>,
) -> Result<Bridge, EngineFailure> {
    #[cfg(windows)]
    if let Some(core) = std::env::var_os("LINGOMATE_TEST_CORE") {
        return Bridge::start_embedded(&PathBuf::from(core), resources, memory);
    }
    Bridge::start_with_memory(executable, resources, memory).map_err(|_| EngineFailure::Unavailable)
}
fn send(worker: &Worker, id: u64, key: Key) {
    assert_eq!(
        worker.enqueue(Command::Key {
            id,
            key,
            selection: None
        }),
        Ok(())
    );
}
fn update(worker: &Worker) -> (u64, Input, Option<String>, Option<Confirmation>) {
    match worker.recv_timeout(Duration::from_secs(5)).unwrap() {
        Reply::Update {
            id,
            input,
            commit,
            memory,
            failure,
            ..
        } => {
            assert!(
                failure.is_none(),
                "unexpected worker engine failure: {failure:?}"
            );
            (id, *input, commit, memory)
        }
        _ => panic!("expected a keyboard update"),
    }
}
fn query(worker: &Worker, id: &mut u64, text: &str) -> (u64, Input) {
    let mut result = None;
    for ch in text.chars() {
        *id += 1;
        send(worker, *id, Key::Letter(ch));
        let (revision, input, commit, _) = update(worker);
        assert!(commit.is_none());
        result = Some((revision, input));
    }
    result.unwrap()
}
fn choose(worker: &Worker, id: &mut u64, pinyin: &str, text: &str) -> (String, Confirmation) {
    let (revision, input) = query(worker, id, pinyin);
    let mut selection = Selection::from_input(revision, &input);
    selection.selected = input
        .frame
        .candidates
        .iter()
        .position(|c| c.text == text)
        .unwrap();
    *id += 1;
    worker
        .enqueue(Command::Key {
            id: *id,
            key: Key::Space(false),
            selection: Some(selection),
        })
        .unwrap();
    let (_, input, commit, memory) = update(worker);
    assert!(input.raw.is_empty());
    assert_eq!(commit.as_deref(), Some(text));
    (commit.unwrap(), memory.unwrap())
}
fn confirm(worker: &Worker, receipt: Confirmation, successful: bool, output: &str) {
    worker
        .enqueue(Command::Confirm {
            receipt,
            successful,
            output: Some(output.into()),
        })
        .unwrap();
    assert!(matches!(
        worker.recv_timeout(Duration::from_secs(5)).unwrap(),
        Reply::Warning { .. }
    ));
}

#[test]
fn slow_initialization_never_blocks_enqueue_or_drop_and_commands_are_bounded() {
    let (started, initialized) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let (finished, exited) = mpsc::channel();
    let worker = Worker::start_with(move || {
        started.send(()).unwrap();
        blocked.recv().unwrap();
        finished.send(()).unwrap();
        Err(EngineFailure::Unavailable)
    });
    send(&worker, 1, Key::Letter('w'));
    initialized.recv_timeout(Duration::from_secs(2)).unwrap();
    let started = Instant::now();
    for id in 2..QUEUE_CAPACITY as u64 + 2 {
        send(&worker, id, Key::Letter('o'));
    }
    assert!(started.elapsed() < Duration::from_millis(50));
    assert_eq!(
        worker.enqueue(Command::Finish { id: 300 }),
        Err(EnqueueError::Full)
    );
    let started = Instant::now();
    drop(worker);
    assert!(started.elapsed() < Duration::from_millis(50));
    release.send(()).unwrap();
    exited.recv_timeout(Duration::from_secs(2)).unwrap();
}

#[test]
fn failed_engine_keeps_case_raw_return_and_literal_punctuation_without_postponing_retry() {
    let attempts = Arc::new(AtomicU64::new(0));
    let starts = attempts.clone();
    let worker = Worker::start_with(move || {
        starts.fetch_add(1, Ordering::SeqCst);
        Err(EngineFailure::Unavailable)
    });
    for (id, key) in [
        (1, Key::Letter('A')),
        (2, Key::Letter('I')),
        (3, Key::Return),
    ] {
        send(&worker, id, key);
        let Reply::Update { input, commit, .. } =
            worker.recv_timeout(Duration::from_secs(2)).unwrap()
        else {
            panic!()
        };
        if id == 2 {
            assert_eq!(input.raw, "AI");
        }
        if id == 3 {
            assert_eq!(commit.as_deref(), Some("AI"));
        }
    }
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    worker
        .enqueue(Command::PunctuationMode(Mode::Automatic))
        .unwrap();
    send(&worker, 4, Key::Literal('3'));
    send(&worker, 5, Key::Punctuation('.'));
    for (id, expected) in [(4, "3"), (5, ".")] {
        let Reply::Update {
            id: actual, commit, ..
        } = worker.recv_timeout(Duration::from_secs(2)).unwrap()
        else {
            panic!()
        };
        assert_eq!(actual, id);
        assert_eq!(commit.as_deref(), Some(expected));
    }
    std::thread::sleep(Duration::from_millis(2100));
    send(&worker, 6, Key::Letter('w'));
    worker.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[test]
fn queued_mixed_input_commits_fifo_and_preserves_uppercase() {
    let worker = real_worker(None);
    let mut keys = Vec::new();
    for text in ["wo", "AI", "xuexi"] {
        keys.extend(text.chars().map(Key::Letter));
        keys.push(if text == "AI" {
            Key::Return
        } else {
            Key::Space(false)
        });
    }
    for (index, key) in keys.iter().enumerate() {
        send(&worker, index as u64 + 1, *key);
    }
    let mut output = String::new();
    for index in 0..keys.len() {
        let (id, _, commit, _) = update(&worker);
        assert_eq!(id, index as u64 + 1);
        if let Some(commit) = commit {
            output.push_str(&commit);
        }
    }
    assert_eq!(output, "我AI学习");
}

#[test]
fn partial_click_then_queued_return_preserves_original_case_suffix() {
    let worker = real_worker(None);
    let mut id = 0;
    let (revision, input) = query(&worker, &mut id, "kaiFaZhe");
    let mut selection = Selection::from_input(revision, &input);
    selection.selected = input
        .frame
        .candidates
        .iter()
        .position(|c| c.text == "开发")
        .unwrap();
    worker
        .enqueue(Command::Key {
            id: id + 1,
            key: Key::Space(false),
            selection: Some(selection),
        })
        .unwrap();
    worker
        .enqueue(Command::Keyboard {
            id: id + 2,
            idle: None,
            composing: Some(Key::Return),
        })
        .unwrap();
    let (_, input, commit, _) = update(&worker);
    assert_eq!(commit.as_deref(), Some("开发"));
    assert_eq!(input.raw, "Zhe");
    let (_, input, commit, _) = update(&worker);
    assert_eq!(commit.as_deref(), Some("Zhe"));
    assert!(input.raw.is_empty());
}

#[test]
fn queued_returns_and_focus_finish_commit_raw_only_once() {
    let worker = real_worker(None);
    let mut id = 0;
    query(&worker, &mut id, "AI");
    send(&worker, id + 1, Key::Return);
    worker
        .enqueue(Command::Keyboard {
            id: id + 2,
            idle: None,
            composing: Some(Key::Return),
        })
        .unwrap();
    worker.enqueue(Command::Finish { id: id + 3 }).unwrap();
    let (_, _, first, _) = update(&worker);
    assert_eq!(first.as_deref(), Some("AI"));
    for _ in 0..2 {
        let (_, input, commit, memory) = update(&worker);
        assert!(input.raw.is_empty());
        assert!(commit.is_none());
        assert!(memory.is_none());
    }
}

#[test]
fn rapid_commit_then_digit_uses_worker_composition_instead_of_late_ui_state() {
    let worker = real_worker(None);
    send(&worker, 1, Key::Letter('w'));
    send(&worker, 2, Key::Letter('o'));
    worker
        .enqueue(Command::Keyboard {
            id: 3,
            idle: Some(Key::Literal(' ')),
            composing: Some(Key::Space(false)),
        })
        .unwrap();
    worker
        .enqueue(Command::Keyboard {
            id: 4,
            idle: Some(Key::Literal('1')),
            composing: Some(Key::Digit(0)),
        })
        .unwrap();
    let mut output = String::new();
    for id in 1..=4 {
        let (actual, _, commit, _) = update(&worker);
        assert_eq!(actual, id);
        if let Some(commit) = commit {
            output.push_str(&commit);
        }
    }
    assert_eq!(output, "我1");
}

#[test]
fn cooldown_typing_after_escape_recovers_candidates_before_space() {
    let executable = PathBuf::from(std::env::var("LINGOMATE_TEST_BRIDGE").unwrap());
    let resources = PathBuf::from(std::env::var("LINGOMATE_TEST_RESOURCES").unwrap());
    let mut first = true;
    let worker = Worker::start_with(move || {
        if first {
            first = false;
            Err(EngineFailure::Unavailable)
        } else {
            start_engine(&executable, &resources, None)
        }
    });
    for (id, key) in [
        (1, Key::Letter('x')),
        (2, Key::Escape),
        (3, Key::Letter('w')),
        (4, Key::Letter('o')),
    ] {
        send(&worker, id, key);
        let Reply::Update { input, commit, .. } =
            worker.recv_timeout(Duration::from_secs(2)).unwrap()
        else {
            panic!()
        };
        assert!(commit.is_none());
        if id == 2 {
            assert!(input.raw.is_empty());
        }
        if id == 4 {
            assert_eq!(input.raw, "wo");
            assert!(input.error);
        }
    }
    std::thread::sleep(Duration::from_millis(2100));
    send(&worker, 5, Key::Space(false));
    let (_, input, commit, _) = update(&worker);
    assert_eq!(commit.as_deref(), Some("我"));
    assert!(input.raw.is_empty());
    assert!(!input.error);
}

#[test]
fn stale_clicked_frame_is_rejected_and_keyboard_navigation_keeps_latest_selection() {
    let worker = real_worker(None);
    let mut id = 0;
    let (revision, input) = query(&worker, &mut id, "shi");
    let stale = Selection::from_input(revision, &input);
    let expected = input.frame.candidates[1].text.clone();
    send(&worker, id + 1, Key::Down);
    send(&worker, id + 2, Key::Space(false));
    update(&worker);
    let (_, _, commit, _) = update(&worker);
    assert_eq!(commit.as_deref(), Some(expected.as_str()));
    worker
        .enqueue(Command::Key {
            id: id + 3,
            key: Key::Space(false),
            selection: Some(stale),
        })
        .unwrap();
    assert!(update(&worker).2.is_none());
    id += 3;
    let (revision, input) = query(&worker, &mut id, "xuexi");
    let mut wrong_generation = Selection::from_input(revision, &input);
    wrong_generation.generation += 1;
    worker
        .enqueue(Command::Key {
            id: id + 1,
            key: Key::Space(true),
            selection: Some(wrong_generation),
        })
        .unwrap();
    let (_, input, commit, _) = update(&worker);
    assert!(commit.is_none());
    assert_eq!(input.raw, "xuexi");
}

#[test]
fn receipts_require_matching_generation_host_success_and_output_before_learning() {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let directory = std::env::temp_dir().join(format!(
        "lingomate-worker-memory-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let path = directory.join("words.json");
    let worker = real_worker(Some(path.clone()));
    let mut id = 0;
    let (output, receipt) = choose(&worker, &mut id, "gai", "钙");
    assert!(!path.exists());
    let mut stale = receipt.clone();
    stale.generation += 1;
    confirm(&worker, stale, true, &output);
    assert!(!path.exists());
    confirm(&worker, receipt, false, &output);
    assert!(!path.exists());
    let (_, receipt) = choose(&worker, &mut id, "tai", "钛");
    confirm(&worker, receipt, true, "mismatched output");
    assert!(!path.exists());
    let (output, receipt) = choose(&worker, &mut id, "kuang", "矿");
    confirm(&worker, receipt, true, &output);
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let entries = data["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["text"], "矿");
    assert_eq!(entries[0]["count"], 1);
    drop(worker);
    std::fs::remove_file(path).unwrap();
    let _ = std::fs::remove_file(directory.join("words.lock"));
    let _ = std::fs::remove_dir(directory);
}

#[test]
fn barrier_does_not_initialize_an_unused_engine() {
    let worker = Worker::start_with(|| panic!("barrier must not initialize an engine"));
    worker.enqueue(Command::Barrier { id: 99 }).unwrap();
    assert!(matches!(
        worker.recv_timeout(Duration::from_secs(2)).unwrap(),
        Reply::Barrier { id: 99 }
    ));
}

#[test]
fn idle_literals_symbols_and_latin_url_never_initialize_candidates() {
    let worker = Worker::start_with(|| panic!("direct characters must not initialize candidates"));
    let keys = [
        Key::Literal('1'),
        Key::Literal(' '),
        Key::Punctuation('!'),
        Key::Literal('h'),
        Key::Letter('t'),
        Key::Letter('t'),
        Key::Letter('p'),
        Key::Letter('s'),
        Key::Punctuation(':'),
        Key::Punctuation('/'),
        Key::Punctuation('/'),
        Key::Letter('A'),
        Key::Punctuation('.'),
        Key::Letter('c'),
        Key::Punctuation('/'),
    ];
    for (index, key) in keys.iter().enumerate() {
        send(&worker, index as u64 + 1, *key);
    }
    let mut output = String::new();
    for _ in keys {
        let (_, input, commit, _) = update(&worker);
        assert!(input.raw.is_empty());
        output.push_str(&commit.unwrap());
    }
    assert_eq!(output, "1 ！https://A.c/");
}

fn enter_url(worker: &Worker, id: &mut u64) {
    query(worker, id, "https");
    for (key, expected) in [
        (Key::Punctuation(':'), "https:"),
        (Key::Punctuation('/'), "/"),
        (Key::Punctuation('/'), "/"),
    ] {
        *id += 1;
        send(worker, *id, key);
        let (_, input, commit, _) = update(worker);
        assert_eq!(commit.as_deref(), Some(expected));
        assert!(input.raw.is_empty());
        assert!(input.punctuation.is_latin());
    }
}

#[test]
fn url_context_break_restores_pinyin_candidates_and_chinese_submission() {
    let worker = real_worker(None);
    let mut id = 0;
    enter_url(&worker, &mut id);
    worker.enqueue(Command::BreakContext).unwrap();
    let (_, input) = query(&worker, &mut id, "wo");
    assert_eq!(input.raw, "wo");
    assert!(!input.punctuation.is_latin());
    assert_eq!(input.frame.candidates.first().unwrap().text, "我");
    id += 1;
    send(&worker, id, Key::Space(false));
    let (_, input, commit, _) = update(&worker);
    assert_eq!(commit.as_deref(), Some("我"));
    assert!(input.raw.is_empty());
}

#[test]
fn memory_cancellation_alone_keeps_url_letters_literal() {
    let worker = real_worker(None);
    let mut id = 0;
    enter_url(&worker, &mut id);
    worker.enqueue(Command::CancelMemory).unwrap();
    for ch in ['W', 'o'] {
        id += 1;
        send(&worker, id, Key::Letter(ch));
        let (_, input, commit, _) = update(&worker);
        assert_eq!(commit, Some(ch.to_string()));
        assert!(input.raw.is_empty());
        assert!(input.punctuation.is_latin());
    }
}

#[test]
fn confirm_then_barrier_flushes_fifo_before_retirement_and_preserves_frame_revision() {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let directory = std::env::temp_dir().join(format!(
        "lingomate-worker-barrier-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let path = directory.join("words.json");
    let worker = real_worker(Some(path.clone()));
    let mut id = 0;
    let (revision, input) = query(&worker, &mut id, "gai");
    let mut selection = Selection::from_input(revision, &input);
    selection.selected = input
        .frame
        .candidates
        .iter()
        .position(|c| c.text == "钙")
        .unwrap();
    worker.enqueue(Command::Barrier { id: 900 }).unwrap();
    assert!(matches!(
        worker.recv_timeout(Duration::from_secs(2)).unwrap(),
        Reply::Barrier { id: 900 }
    ));
    worker
        .enqueue(Command::Key {
            id: id + 1,
            key: Key::Space(false),
            selection: Some(selection),
        })
        .unwrap();
    let (_, _, commit, receipt) = update(&worker);
    assert_eq!(commit.as_deref(), Some("钙"));
    worker
        .enqueue(Command::Confirm {
            receipt: receipt.unwrap(),
            successful: true,
            output: commit,
        })
        .unwrap();
    worker.enqueue(Command::Barrier { id: 901 }).unwrap();
    assert!(matches!(
        worker.recv_timeout(Duration::from_secs(5)).unwrap(),
        Reply::Warning {
            failure: None,
            memory_warning: false
        }
    ));
    assert!(matches!(
        worker.recv_timeout(Duration::from_secs(5)).unwrap(),
        Reply::Barrier { id: 901 }
    ));
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(data["entries"][0]["text"], "钙");
    assert_eq!(data["entries"][0]["count"], 1);
    drop(worker);
    std::fs::remove_file(path).unwrap();
    let _ = std::fs::remove_file(directory.join("words.lock"));
    let _ = std::fs::remove_dir(directory);
}

#[test]
fn reset_discards_old_composition_and_unconfirmed_receipts_before_next_scope() {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let directory = std::env::temp_dir().join(format!(
        "lingomate-worker-reset-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let path = directory.join("words.json");
    let worker = real_worker(Some(path.clone()));
    let mut id = 0;
    query(&worker, &mut id, "wo");
    id += 1;
    worker.enqueue(Command::Reset { id }).unwrap();
    let (_, input, commit, memory) = update(&worker);
    assert!(input.raw.is_empty());
    assert!(commit.is_none());
    assert!(memory.is_none());
    let (output, cancelled) = choose(&worker, &mut id, "xuexi", "学习");
    id += 1;
    worker.enqueue(Command::Reset { id }).unwrap();
    update(&worker);
    worker
        .enqueue(Command::Confirm {
            receipt: cancelled,
            successful: true,
            output: Some(output),
        })
        .unwrap();
    assert!(matches!(
        worker.recv_timeout(Duration::from_secs(5)).unwrap(),
        Reply::Warning {
            failure: Some(EngineFailure::Rejected),
            ..
        }
    ));
    assert!(!path.exists());
    let (output, receipt) = choose(&worker, &mut id, "xuexi", "学习");
    confirm(&worker, receipt, true, &output);
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(data["entries"].as_array().unwrap().len(), 1);
    assert_eq!(data["entries"][0]["text"], "学习");
    assert_eq!(data["entries"][0]["count"], 1);
    drop(worker);
    std::fs::remove_file(path).unwrap();
    let _ = std::fs::remove_file(directory.join("words.lock"));
    let _ = std::fs::remove_dir(directory);
}
