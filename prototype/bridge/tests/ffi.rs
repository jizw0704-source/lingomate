//! Real CLI/FFI parity with explicit synthetic vocabulary stores only.
use lingomate_engine::ffi::*;
use serde_json::{Value, json};
use std::ffi::c_void;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

fn resources() -> Option<PathBuf> {
    let root = std::env::var_os("LINGOMATE_TEST_RESOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    if root.join("dict.tsv").is_file() || root.join("data/generated/dict.tsv").is_file() {
        Some(root)
    } else {
        eprintln!("Real FFI parity needs prepared data or LINGOMATE_TEST_RESOURCES.");
        None
    }
}

struct Core(*mut c_void);
impl Core {
    fn create(root: &Path, memory: Option<&Path>) -> Self {
        let root = root.to_str().unwrap();
        let memory = memory.map_or("", |path| path.to_str().unwrap());
        let mut handle = ptr::null_mut();
        let status = unsafe {
            LingoEngineCreate(
                root.as_ptr(),
                root.len(),
                memory.as_ptr(),
                memory.len(),
                &mut handle,
            )
        };
        assert_eq!(
            status, OK,
            "Core initialization failed with static code {status}"
        );
        assert!(!handle.is_null());
        Self(handle)
    }
    fn request_bytes(&self, request: &[u8]) -> Value {
        let mut bytes = ptr::null_mut();
        let mut length = 0;
        assert_eq!(
            unsafe {
                LingoEngineRequest(
                    self.0,
                    request.as_ptr(),
                    request.len(),
                    &mut bytes,
                    &mut length,
                )
            },
            OK
        );
        struct Reply(*mut u8, usize);
        impl Drop for Reply {
            fn drop(&mut self) {
                unsafe { LingoEngineFreeReply(self.0, self.1) };
            }
        }
        let reply = Reply(bytes, length);
        assert!(!reply.0.is_null());
        assert!(reply.1 <= MAX_REPLY_BYTES);
        serde_json::from_slice(unsafe { std::slice::from_raw_parts(reply.0, reply.1) }).unwrap()
    }
    fn request(&self, request: &Value) -> Value {
        self.request_bytes(&serde_json::to_vec(request).unwrap())
    }
}
impl Drop for Core {
    fn drop(&mut self) {
        unsafe { LingoEngineDestroy(self.0) };
    }
}

struct Cli {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Cli {
    fn start(root: &Path, memory: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_bilingual-ime-bridge"))
            .arg(root)
            .arg("--memory")
            .arg(memory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }
    fn request(&mut self, request: &Value) -> Value {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{request}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        assert_ne!(self.output.read_line(&mut line).unwrap(), 0);
        serde_json::from_str(&line).unwrap()
    }
}
impl Drop for Cli {
    fn drop(&mut self) {
        self.input.take();
        if self.child.wait().is_err() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "lingomate-core-ffi-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        // This directory was exclusively created above, directly under temp_dir.
        if self.0.parent() == Some(std::env::temp_dir().as_path()) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

struct Pair {
    core: Core,
    cli: Cli,
}
impl Pair {
    fn request(&mut self, request: Value) -> Value {
        let actual = self.core.request(&request);
        let expected = self.cli.request(&request);
        assert_eq!(
            actual, expected,
            "CLI and FFI disagree on a synthetic request"
        );
        actual
    }
    fn commit(
        &mut self,
        input: &str,
        chinese: &str,
        context: &str,
        english: Option<&str>,
    ) -> Value {
        let query = self.request(json!({"action":"query", "input":input, "context":context}));
        let candidate = query["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| candidate["text"] == chinese)
            .unwrap();
        self.request(json!({"action":"commit", "input":input,
            "candidate":chinese, "syllables":candidate["syllables"],
            "english":english, "context":context, "deferred_output":true}))
    }
    fn confirm(&mut self, frame: &Value, context: &str, chinese: bool) -> Value {
        self.request(json!({"action":"confirm", "input":"", "context":context,
            "learning_token":frame["learningToken"], "chinese_output":chinese}))
    }
}

#[test]
fn ffi_matches_cli_for_partial_english_deferred_and_restart_memory() {
    let Some(root) = resources() else { return };
    let scratch = Scratch::new();
    let core_memory = scratch.0.join("ffi/words.json");
    let cli_memory = scratch.0.join("cli/words.json");
    let mut pair = Pair {
        core: Core::create(&root, Some(&core_memory)),
        cli: Cli::start(&root, &cli_memory),
    };
    for input in ["wo", "xuexi", "gongshi", "nihao", "", "中"] {
        pair.request(json!({"action":"query", "input":input}));
    }
    let english = pair.commit("xuexi", "学习", "english", Some("learning"));
    assert_eq!(english["committed"], "learning");
    pair.confirm(&english, "english", false);
    let partial = pair.commit("kaifazhe", "开发", "partial", Some("develop"));
    assert_eq!(partial["input"], "zhe");
    pair.request(json!({"action":"cancel", "input":"", "context":"partial"}));
    assert!(
        pair.commit("xuexi", "学习", "invalid", Some("fabricated"))
            .get("error")
            .is_some()
    );
    let first = pair.commit("gai", "钙", "characters", None);
    let second = pair.commit("tai", "钛", "characters", None);
    assert!(
        pair.confirm(&second, "characters", true)
            .get("error")
            .is_some()
    );
    pair.confirm(&first, "characters", true);
    assert!(
        pair.confirm(&first, "characters", true)
            .get("error")
            .is_some()
    );
    pair.confirm(&second, "characters", true);
    let third = pair.commit("nie", "镍", "characters", None);
    pair.request(
        json!({"action":"cancel", "input":"", "context":"characters",
        "deferred_output":true}),
    );
    assert!(
        pair.request(json!({"action":"query", "input":"wo", "context":"characters"}))
            .get("error")
            .is_some()
    );
    pair.confirm(&third, "characters", true);
    drop(pair);
    let saved_core: Value = serde_json::from_slice(&std::fs::read(&core_memory).unwrap()).unwrap();
    let saved_cli: Value = serde_json::from_slice(&std::fs::read(&cli_memory).unwrap()).unwrap();
    assert_eq!(saved_core, saved_cli);
    let mut pair = Pair {
        core: Core::create(&root, Some(&core_memory)),
        cli: Cli::start(&root, &cli_memory),
    };
    let frame = pair.request(json!({"action":"query", "input":"gaitainie"}));
    assert_eq!(frame["candidates"][0]["text"], "钙钛镍");
    assert_eq!(frame["candidates"][0]["personal"], true);
}

#[test]
fn ffi_errors_reset_outputs_and_valid_requests_survive_rejected_buffers() {
    let Some(root) = resources() else { return };
    let core = Core::create(&root, None);
    let mut bytes = ptr::dangling_mut::<u8>();
    let mut length = usize::MAX;
    unsafe {
        assert_eq!(
            LingoEngineRequest(
                core.0,
                ptr::null(),
                MAX_REQUEST_BYTES + 1,
                &mut bytes,
                &mut length
            ),
            REQUEST_TOO_LARGE
        );
        assert!(bytes.is_null());
        assert_eq!(length, 0);
        let invalid = [0xff];
        assert_eq!(
            LingoEngineRequest(
                core.0,
                invalid.as_ptr(),
                invalid.len(),
                &mut bytes,
                &mut length
            ),
            INVALID_UTF8
        );
        assert!(bytes.is_null());
        assert_eq!(length, 0);
        assert_eq!(
            LingoEngineRequest(ptr::null_mut(), ptr::null(), 0, &mut bytes, &mut length),
            INVALID_ARGUMENT
        );
        LingoEngineFreeReply(ptr::null_mut(), 0);
        LingoEngineDestroy(ptr::null_mut());
    }
    assert_eq!(
        core.request_bytes(b"not JSON"),
        json!({"error":"请求格式错误"})
    );
    for _ in 0..100 {
        assert!(
            !core.request(&json!({"action":"query", "input":"wo"}))["candidates"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}
