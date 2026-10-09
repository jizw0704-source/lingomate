use serde::Deserialize;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
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
}

pub struct Bridge {
    child: Child,
    input: ChildStdin,
    replies: Receiver<Result<String, String>>,
}
impl Bridge {
    pub fn start(executable: &Path, resources: &Path) -> Result<Self, String> {
        let mut command = Command::new(executable);
        command
            .arg(resources)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // No console in the host editor.
        }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let input = child.stdin.take().ok_or("No engine input pipe")?;
        let output = child.stdout.take().ok_or("No engine output pipe")?;
        let (sender, replies) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
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
            child,
            input,
            replies,
        })
    }
    pub fn call(&mut self, request: Value) -> Result<Frame, String> {
        serde_json::to_writer(&mut self.input, &request).map_err(|e| e.to_string())?;
        self.input
            .write_all(b"\n")
            .and_then(|_| self.input.flush())
            .map_err(|e| e.to_string())?;
        let reply = self
            .replies
            .recv_timeout(Duration::from_millis(500))
            .map_err(|_| "Engine did not respond within 500 ms".to_string())??;
        let value: Value = serde_json::from_str(&reply).map_err(|_| "Invalid engine frame")?;
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return Err(error.into());
        }
        let frame: Frame = serde_json::from_value(value).map_err(|_| "Invalid engine frame")?;
        if frame.candidates.len() > 64 || frame.input.len() > 240 {
            return Err("Invalid engine limits".into());
        }
        Ok(frame)
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
