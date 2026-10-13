use serde::Deserialize;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Translation {
    pub word: String,
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub example: String,
}
impl Translation {
    pub fn visible_pos(&self, show: bool) -> &str {
        if show
            && !self.pos.is_empty()
            && self.pos.split('/').all(|p| {
                [
                    "n.", "v.", "adj.", "adv.", "pron.", "prep.", "conj.", "det.", "num.", "int.",
                    "phr.",
                ]
                .contains(&p)
            })
        {
            &self.pos
        } else {
            ""
        }
    }
    pub fn display_word(&self, show: bool) -> String {
        let pos = self.visible_pos(show);
        if pos.is_empty() {
            self.word.clone()
        } else {
            format!("{} {pos}", self.word)
        }
    }
}

#[cfg(test)]
mod pos_tests {
    use super::*;
    #[test]
    fn output_word_never_contains_display_metadata() {
        let mut word = Translation {
            word: "study".into(),
            pos: "n./v.".into(),
            ..Default::default()
        };
        assert_eq!(word.display_word(true), "study n./v.");
        assert_eq!(word.display_word(false), "study");
        assert_eq!(word.word, "study");
        word.pos = "名词".into();
        assert_eq!(word.display_word(true), "study");
        word.pos.clear();
        assert_eq!(word.display_word(true), "study");
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Candidate {
    pub text: String,
    pub syllables: Vec<String>,
    pub translations: Vec<Translation>,
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Frame {
    pub input: String,
    pub candidates: Vec<Candidate>,
    pub committed: Option<String>,
    #[serde(default, rename = "learningToken")]
    pub learning_token: Option<u64>,
    #[serde(default, rename = "memoryWarning")]
    pub memory_warning: Option<String>,
    #[serde(skip)]
    pub generation: u64,
}

static GENERATION: AtomicU64 = AtomicU64::new(1);

/// Static failure metadata only. Never contains a request, reply or vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum EngineFailure {
    Module { code: u32 },
    NativeLoad { code: u32 },
    CoreInit { code: u32 },
    CoreReply { code: u32 },
    Spawn { code: Option<i32> },
    StartupPipe,
    FirstReplyTimeout,
    ReplyTimeout,
    PipeClosed { exit_code: Option<i32> },
    InvalidReply,
    Rejected,
    Unavailable,
}
impl EngineFailure {
    pub fn description(self) -> String {
        match self {
            Self::Module { code } => format!("引擎路径定位失败（0x{code:08X}）"),
            Self::NativeLoad { code } => format!("引擎库加载失败（0x{code:08X}）"),
            Self::CoreInit { code } => format!("引擎初始化失败（代码 {code}）"),
            Self::CoreReply { code } => format!("引擎查询失败（代码 {code}）"),
            Self::Spawn { code: Some(code) } => format!("引擎启动失败（系统错误 {code}）"),
            Self::Spawn { code: None } => "引擎启动失败".into(),
            Self::StartupPipe => "引擎管道创建失败".into(),
            Self::FirstReplyTimeout => "引擎首次响应超时".into(),
            Self::ReplyTimeout => "引擎响应超时".into(),
            Self::PipeClosed {
                exit_code: Some(code),
            } => format!("引擎已退出（代码 {code}）"),
            Self::PipeClosed { exit_code: None } => "引擎连接已关闭".into(),
            Self::InvalidReply => "引擎响应格式无效".into(),
            Self::Rejected => "引擎请求被拒绝".into(),
            Self::Unavailable => "引擎暂不可用".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Confirmation {
    pub context: String,
    pub generation: u64,
    token: u64,
    pub output: String,
    chinese: bool,
    pub boundary_after: bool,
}
impl Confirmation {
    pub fn new(frame: &Frame, context: String, chinese: bool) -> Option<Self> {
        Some(Self {
            context,
            generation: frame.generation,
            token: frame.learning_token?,
            output: frame.committed.clone()?,
            chinese,
            boundary_after: false,
        })
    }
}

pub struct Bridge {
    child: Option<Child>,
    input: Option<ChildStdin>,
    #[cfg(windows)]
    embedded: Option<crate::native_core::NativeCore>,
    replies: Option<Receiver<Result<String, String>>>,
    reader: Option<std::thread::JoinHandle<()>>,
    pub generation: u64,
    live: bool,
    startup_pending: bool,
    pub failure: Option<EngineFailure>,
}
impl Bridge {
    pub fn start(executable: &Path, resources: &Path) -> Result<Self, String> {
        Self::start_with_memory(executable, resources, None)
    }
    pub fn start_with_memory(
        executable: &Path,
        resources: &Path,
        memory: Option<&Path>,
    ) -> Result<Self, String> {
        Self::start_diagnosed(executable, resources, memory).map_err(EngineFailure::description)
    }
    pub fn start_diagnosed(
        executable: &Path,
        resources: &Path,
        memory: Option<&Path>,
    ) -> Result<Self, EngineFailure> {
        let mut command = Command::new(executable);
        command
            .arg(resources)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(path) = memory {
            command.arg("--memory").arg(path);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // No console in the host editor.
        }
        let mut child = command.spawn().map_err(|e| EngineFailure::Spawn {
            code: e.raw_os_error(),
        })?;
        let input = child.stdin.take().ok_or(EngineFailure::StartupPipe)?;
        let output = child.stdout.take().ok_or(EngineFailure::StartupPipe)?;
        let (sender, replies) = mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut bytes = Vec::new();
                // Cap each frame independently; malformed engines cannot allocate unbounded text.
                let result = std::io::Read::take(&mut reader, 4 * 1024 * 1024 + 1)
                    .read_until(b'\n', &mut bytes);
                let reply = match result {
                    Ok(0) => break,
                    Ok(size) if size > 4 * 1024 * 1024 || !bytes.ends_with(b"\n") => {
                        Err("Oversized or incomplete engine frame".into())
                    }
                    Ok(_) => String::from_utf8(bytes).map_err(|_| "Invalid engine encoding".into()),
                    Err(_) => Err("Engine pipe closed".into()),
                };
                let failed = reply.is_err();
                if sender.send(reply).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child: Some(child),
            input: Some(input),
            #[cfg(windows)]
            embedded: None,
            replies: Some(replies),
            reader: Some(reader),
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            live: true,
            startup_pending: true,
            failure: None,
        })
    }

    #[cfg(windows)]
    pub fn start_embedded(
        core_dll: &Path,
        resources: &Path,
        memory: Option<&Path>,
    ) -> Result<Self, EngineFailure> {
        let core = crate::native_core::NativeCore::load(core_dll, resources, memory)?;
        Ok(Self {
            child: None,
            input: None,
            embedded: Some(core),
            replies: None,
            reader: None,
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            live: true,
            startup_pending: false,
            failure: None,
        })
    }

    pub fn cancel_memory(&mut self, context: &str) {
        let _ = self.call(serde_json::json!({"action":"cancel", "input":"", "context":context}));
    }
    pub fn retire_memory(&mut self, context: &str) {
        let _ = self.call(serde_json::json!({"action":"cancel", "input":"", "context":context, "deferred_output":true}));
    }

    pub fn confirm_output(
        &mut self,
        receipt: &Confirmation,
        successful: bool,
        output: Option<&str>,
    ) -> Result<bool, String> {
        if receipt.generation != self.generation {
            return Ok(false);
        }
        if !successful || output != Some(receipt.output.as_str()) {
            self.cancel_memory(&receipt.context);
            return Ok(false);
        }
        let frame = self.call(
            serde_json::json!({"action":"confirm", "input":"", "context":receipt.context,
            "learning_token":receipt.token, "chinese_output":receipt.chinese}),
        )?;
        if receipt.boundary_after {
            self.cancel_memory(&receipt.context);
        }
        if frame.memory_warning.is_some() {
            return Err("中文选词记忆未保存".into());
        }
        Ok(true)
    }
    fn exchange(&mut self, request: Value) -> Result<Value, String> {
        #[cfg(windows)]
        if let Some(core) = self.embedded.as_mut() {
            return core.request(&request).map_err(|failure| {
                self.failure = Some(failure);
                failure.description()
            });
        }
        // Every first reply, including cancel after Backspace/Return, waits for
        // loading the pinned tables. Warm calls keep their shorter deadline.
        let timeout_ms = if self.startup_pending { 1500 } else { 500 };
        let input = self.input.as_mut().ok_or("Engine connection closed")?;
        serde_json::to_writer(&mut *input, &request).map_err(|e| e.to_string())?;
        input
            .write_all(b"\n")
            .and_then(|_| input.flush())
            .map_err(|e| e.to_string())?;
        let reply = self
            .replies
            .as_ref()
            .ok_or("Engine connection closed")?
            .recv_timeout(Duration::from_millis(timeout_ms))
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => {
                    format!("Engine did not respond within {timeout_ms} ms")
                }
                RecvTimeoutError::Disconnected => "Engine pipe closed".into(),
            })??;
        self.startup_pending = false;
        serde_json::from_str(&reply).map_err(|_| "Invalid engine frame".into())
    }
    fn poison(&mut self) {
        self.live = false;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.input.take();
        self.replies.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        #[cfg(windows)]
        self.embedded.take();
    }
    pub fn call(&mut self, request: Value) -> Result<Frame, String> {
        if !self.live {
            return Err("Engine connection closed".into());
        }
        self.failure = None;
        let value = match self.exchange(request) {
            Ok(value) => value,
            Err(error) => {
                self.failure = self.failure.or(Some(
                    if error.starts_with("Engine did not respond within") {
                        if self.startup_pending {
                            EngineFailure::FirstReplyTimeout
                        } else {
                            EngineFailure::ReplyTimeout
                        }
                    } else if matches!(
                        error.as_str(),
                        "Oversized or incomplete engine frame"
                            | "Invalid engine encoding"
                            | "Invalid engine frame"
                    ) {
                        EngineFailure::InvalidReply
                    } else {
                        EngineFailure::PipeClosed {
                            exit_code: self
                                .child
                                .as_mut()
                                .and_then(|child| child.try_wait().ok().flatten())
                                .and_then(|status| status.code()),
                        }
                    },
                ));
                self.poison();
                return Err(error);
            }
        };
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            self.failure = Some(EngineFailure::Rejected);
            return Err(error.into());
        }
        let mut frame: Frame = match serde_json::from_value(value) {
            Ok(frame) => frame,
            Err(_) => {
                self.failure = Some(EngineFailure::InvalidReply);
                self.poison();
                return Err("Invalid engine frame".into());
            }
        };
        frame.generation = self.generation;
        if frame.candidates.len() > 64 || frame.input.len() > 240 || frame.learning_token == Some(0)
        {
            self.failure = Some(EngineFailure::InvalidReply);
            self.poison();
            return Err("Invalid engine limits".into());
        }
        self.failure = None;
        Ok(frame)
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.poison();
    }
}
