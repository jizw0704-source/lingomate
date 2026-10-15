//! Bounded FIFO engine work; only the background thread owns live composition and receipts.
use crate::bridge::{Bridge, Confirmation, EngineFailure, Frame};
use crate::input::{Input, Key};
use crate::learning::Word;
use crate::punctuation::{Action, Mode, Punctuation};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::{Duration, Instant};

pub const QUEUE_CAPACITY: usize = 128;

/// A click describes the displayed frame, never the most recently queued keystroke.
#[derive(Clone, Debug)]
pub struct Selection {
    pub revision: u64,
    pub raw: String,
    pub generation: u64,
    pub selected: usize,
    pub sense: usize,
    pub expanded: bool,
}
impl Selection {
    pub fn from_input(revision: u64, input: &Input) -> Self {
        Self {
            revision,
            raw: input.raw.clone(),
            generation: input.frame.generation,
            selected: input.selected,
            sense: input.sense,
            expanded: input.expanded,
        }
    }
    fn apply(&self, revision: u64, input: &mut Input) -> bool {
        if self.revision != revision
            || self.raw != input.raw
            || self.generation == 0
            || self.generation != input.frame.generation
            || input.error
        {
            return false;
        }
        let Some(candidate) = input.frame.candidates.get(self.selected) else {
            return false;
        };
        if self.sense >= candidate.translations.len().max(1) {
            return false;
        }
        input.selected = self.selected;
        input.sense = self.sense;
        input.expanded = self.expanded;
        true
    }
}

#[derive(Clone)]
pub enum Command {
    Keyboard {
        id: u64,
        idle: Option<Key>,
        composing: Option<Key>,
    },
    Key {
        id: u64,
        key: Key,
        selection: Option<Selection>,
    },
    Finish {
        id: u64,
    },
    Reset {
        id: u64,
    },
    CancelMemory,
    BreakContext,
    Remember(bool),
    PunctuationMode(Mode),
    EnglishMode(bool),
    Configure {
        id: u64,
        setting: crate::settings::Setting,
    },
    Sentence {
        id: u64,
        selection: Selection,
        source: String,
        english: String,
    },
    Confirm {
        receipt: Confirmation,
        successful: bool,
        output: Option<String>,
    },
    Barrier {
        id: u64,
    },
    Stop,
}

pub enum Reply {
    Update {
        id: u64,
        input: Box<Input>,
        commit: Option<String>,
        study: Option<Word>,
        memory: Option<Confirmation>,
        failure: Option<EngineFailure>,
        memory_warning: bool,
    },
    Warning {
        failure: Option<EngineFailure>,
        memory_warning: bool,
    },
    Configured {
        id: u64,
        english: bool,
        bilingual: bool,
        remember: bool,
        punctuation: Mode,
    },
    Barrier {
        id: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueError {
    Full,
    Stopped,
}

pub struct Worker {
    commands: Option<SyncSender<Command>>,
    replies: Option<Receiver<Reply>>,
    stop: Arc<AtomicBool>,
}
impl Worker {
    pub fn start_with(
        factory: impl FnMut() -> Result<Bridge, EngineFailure> + Send + 'static,
    ) -> Self {
        let (commands, pending) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (updates, replies) = mpsc::sync_channel(QUEUE_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        // A detached worker may be completing native initialization during Drop.
        // The embedding DLL must pin its native module until that thread exits.
        std::thread::spawn(move || {
            let mut state = EngineState::default();
            let mut factory = factory;
            while let Ok(command) = pending.recv() {
                if stopping.load(Ordering::Acquire) || matches!(command, Command::Stop) {
                    break;
                }
                let reply = state.handle(command, &mut factory);
                if stopping.load(Ordering::Acquire) {
                    break;
                }
                if let Some(reply) = reply
                    && updates.send(reply).is_err()
                {
                    break;
                }
            }
        });
        Self {
            commands: Some(commands),
            replies: Some(replies),
            stop,
        }
    }
    pub fn enqueue(&self, command: Command) -> Result<(), EnqueueError> {
        if self.stop.load(Ordering::Acquire) {
            return Err(EnqueueError::Stopped);
        }
        self.commands
            .as_ref()
            .ok_or(EnqueueError::Stopped)?
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => EnqueueError::Full,
                TrySendError::Disconnected(_) => EnqueueError::Stopped,
            })
    }
    pub fn try_recv(&self) -> Result<Reply, TryRecvError> {
        self.replies
            .as_ref()
            .ok_or(TryRecvError::Disconnected)?
            .try_recv()
    }
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Reply, mpsc::RecvTimeoutError> {
        self.replies
            .as_ref()
            .ok_or(mpsc::RecvTimeoutError::Disconnected)?
            .recv_timeout(timeout)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // Disconnect a full reply channel before requesting stop. No initialization,
        // engine cleanup or blocked channel send is awaited on the host UI thread.
        self.replies.take();
        self.stop.store(true, Ordering::Release);
        self.commands.take();
    }
}

#[derive(Default)]
struct EngineState {
    input: Input,
    engine: Option<Bridge>,
    retry_after: Option<Instant>,
    failure: Option<EngineFailure>,
    memory_warning: bool,
    revision: u64,
}
impl EngineState {
    fn handle(
        &mut self,
        command: Command,
        factory: &mut impl FnMut() -> Result<Bridge, EngineFailure>,
    ) -> Option<Reply> {
        match command {
            Command::Keyboard {
                id,
                idle,
                composing,
            } => {
                let key = if self.input.raw.is_empty() {
                    idle
                } else {
                    composing
                };
                if let Some(key) = key {
                    self.handle(
                        Command::Key {
                            id,
                            key,
                            selection: None,
                        },
                        factory,
                    )
                } else {
                    Some(self.update(id, None))
                }
            }
            Command::Key { id, key, selection } => {
                if let Some(selection) = selection
                    && !selection.apply(self.revision, &mut self.input)
                {
                    return Some(self.update(id, None));
                }
                let commit = if matches!(key, Key::Return) {
                    self.finish_memory();
                    let raw = self.input.raw_commit();
                    (!raw.is_empty()).then_some(raw)
                } else if let Key::Literal(ch) = key {
                    self.finish_memory();
                    Some(self.input.literal_commit(ch))
                } else if self.input.raw.is_empty()
                    && match key {
                        Key::Punctuation(ch) => matches!(
                            Punctuation::action(ch, "", false, self.input.punctuation_mode),
                            Some(Action::Symbol)
                        ),
                        Key::Letter(_) => self.input.punctuation.is_latin(),
                        _ => false,
                    }
                {
                    // Direct characters need neither candidates nor a cold core.
                    // Existing engines still receive the FIFO memory boundary.
                    self.unavailable_key(key)
                } else {
                    self.ensure_engine(factory);
                    if let Some(engine) = self.engine.as_mut() {
                        engine.failure = None;
                        let result = self.input.handle(key, engine);
                        let failure = engine.failure;
                        match result {
                            Ok(commit) => {
                                self.failure = None;
                                if let Some(failure) = failure {
                                    self.disconnect(failure);
                                }
                                commit
                            }
                            Err(_) => {
                                self.disconnect(failure.unwrap_or(EngineFailure::InvalidReply));
                                None
                            }
                        }
                    } else {
                        let commit = self.unavailable_key(key);
                        // Escape/Return clear the previous error. Composition typed
                        // during cooldown still needs a query on the next connection.
                        if !self.input.raw.is_empty() {
                            self.input.error = true;
                        }
                        commit
                    }
                };
                Some(self.update(id, commit))
            }
            Command::Finish { id } => {
                self.finish_memory();
                let raw = self.input.raw_commit();
                Some(self.update(id, (!raw.is_empty()).then_some(raw)))
            }
            Command::Reset { id } => {
                self.cancel_memory();
                self.input.reset();
                Some(self.update(id, None))
            }
            Command::CancelMemory => {
                self.cancel_memory();
                None
            }
            Command::BreakContext => {
                self.cancel_memory();
                self.input.punctuation.reset_context();
                None
            }
            Command::Remember(remember) => {
                self.cancel_memory();
                self.input.remember = remember;
                None
            }
            Command::PunctuationMode(mode) => {
                self.input.punctuation_mode = mode;
                self.input.punctuation.reset_context();
                None
            }
            Command::EnglishMode(english) => {
                self.input.english = english;
                self.input.last_english = false;
                self.input.punctuation.reset_context();
                None
            }
            Command::Configure { id, setting } => {
                match setting {
                    crate::settings::Setting::English(value) => {
                        self.input.english = value;
                        self.input.last_english = false;
                        self.input.punctuation.reset_context();
                    }
                    crate::settings::Setting::Bilingual(value) => {
                        self.input.bilingual = value;
                        if !value {
                            self.input.expanded = false;
                        }
                    }
                    crate::settings::Setting::Remember(value) => {
                        self.cancel_memory();
                        self.input.remember = value;
                    }
                    crate::settings::Setting::Punctuation(value) => {
                        self.input.punctuation_mode = Mode::ALL[value as usize];
                        self.input.punctuation.reset_context();
                    }
                    _ => {}
                }
                Some(Reply::Configured {
                    id,
                    english: self.input.english,
                    bilingual: self.input.bilingual,
                    remember: self.input.remember,
                    punctuation: self.input.punctuation_mode,
                })
            }
            Command::Sentence {
                id,
                selection,
                source,
                english,
            } => {
                if !selection.apply(self.revision, &mut self.input) {
                    return Some(self.update(id, None));
                }
                let commit = if let Some(engine) = self.engine.as_mut() {
                    engine.failure = None;
                    let result = self.input.commit_sentence(engine, &source, &english);
                    let failure = engine.failure;
                    match result {
                        Ok(commit) => commit,
                        Err(_) => {
                            self.disconnect(failure.unwrap_or(EngineFailure::InvalidReply));
                            None
                        }
                    }
                } else {
                    None
                };
                Some(self.update(id, commit))
            }
            Command::Confirm {
                receipt,
                successful,
                output,
            } => {
                if !self.input.remember {
                    return None;
                }
                if let Some(engine) = self.engine.as_mut() {
                    engine.failure = None;
                    if (!successful || output.as_deref() != Some(receipt.output.as_str()))
                        && receipt.generation == engine.generation
                        && receipt.context == self.input.memory_context
                    {
                        self.input.reset_memory_context();
                    }
                    let result = engine.confirm_output(&receipt, successful, output.as_deref());
                    let failure = engine.failure;
                    if result.is_err() {
                        self.memory_warning = true;
                    }
                    if let Some(failure) = failure {
                        self.disconnect(failure);
                    }
                    Some(Reply::Warning {
                        failure: self.failure,
                        memory_warning: self.memory_warning,
                    })
                } else {
                    None
                }
            }
            Command::Barrier { id } => Some(Reply::Barrier { id }),
            Command::Stop => None,
        }
    }
    fn update(&mut self, id: u64, commit: Option<String>) -> Reply {
        self.revision = id;
        self.memory_warning |= self.input.frame.memory_warning.is_some();
        let study = self.input.pending_study.take();
        let memory = self.input.pending_memory.take();
        Reply::Update {
            id,
            input: Box::new(self.input.clone()),
            commit,
            study,
            memory,
            failure: self.failure,
            memory_warning: self.memory_warning,
        }
    }
    fn ensure_engine(&mut self, factory: &mut impl FnMut() -> Result<Bridge, EngineFailure>) {
        if self.engine.is_some() || self.retry_after.is_some_and(|at| Instant::now() < at) {
            return;
        }
        match factory() {
            Ok(engine) => {
                self.engine = Some(engine);
                self.failure = None;
                self.retry_after = None;
            }
            Err(failure) => self.disconnect(failure),
        }
    }
    fn disconnect(&mut self, failure: EngineFailure) {
        self.failure = Some(failure);
        if !matches!(failure, EngineFailure::Rejected) {
            self.engine = None;
            self.retry_after = Some(Instant::now() + Duration::from_secs(2));
        }
        self.input.frame = Frame::default();
        self.input.selected = 0;
        self.input.sense = 0;
        self.input.expanded = false;
        self.input.pending_study = None;
        self.input.pending_memory = None;
        self.input.error = true;
    }
    fn cancel_memory(&mut self) {
        let context = self.input.reset_memory_context();
        if let Some(engine) = self.engine.as_mut() {
            engine.failure = None;
            engine.cancel_memory(&context);
            if let Some(failure) = engine.failure {
                self.disconnect(failure);
            }
        }
    }
    fn finish_memory(&mut self) {
        let context = self.input.reset_memory_context();
        if let Some(engine) = self.engine.as_mut() {
            engine.failure = None;
            engine.retire_memory(&context);
            if let Some(failure) = engine.failure {
                self.disconnect(failure);
            }
        }
    }
    fn unavailable_key(&mut self, key: Key) -> Option<String> {
        self.input.pending_study = None;
        self.input.pending_memory = None;
        match key {
            Key::ToggleBilingual => {
                self.input.bilingual = !self.input.bilingual;
                self.input.expanded = false;
            }
            Key::CyclePunctuation => {
                self.finish_memory();
                let raw = self.input.raw_commit();
                let index = Mode::ALL
                    .iter()
                    .position(|m| *m == self.input.punctuation_mode)
                    .unwrap_or(0);
                self.input.punctuation_mode = Mode::ALL[(index + 1) % 3];
                return (!raw.is_empty()).then_some(raw);
            }
            Key::Letter(ch) => {
                if self.input.punctuation.is_latin() && self.input.raw.is_empty() {
                    self.finish_memory();
                    self.input.punctuation.observe_literal(&ch.to_string());
                    return Some(ch.to_string());
                }
                if self.input.raw.len() < 240 && (ch.is_ascii_alphabetic() || ch == '\'') {
                    self.input.raw.push(ch);
                }
            }
            Key::Literal(ch) => {
                self.finish_memory();
                return Some(self.input.literal_commit(ch));
            }
            Key::Backspace => {
                self.cancel_memory();
                self.input.punctuation.reset_literal();
                self.input.raw.pop();
            }
            Key::Escape => {
                if self.input.expanded {
                    self.input.expanded = false;
                } else {
                    self.cancel_memory();
                    self.input.reset();
                }
            }
            Key::Punctuation(ch) => {
                let Some(action) = Punctuation::action(
                    ch,
                    &self.input.normalized(),
                    false,
                    self.input.punctuation_mode,
                ) else {
                    return self.unavailable_key(Key::Letter(ch));
                };
                let raw = match action {
                    Action::Raw => {
                        let raw = self.input.raw_commit();
                        self.input.punctuation.observe_literal(&raw);
                        raw
                    }
                    Action::Symbol => String::new(),
                    Action::CommitChinese => return None,
                };
                let symbol = self
                    .input
                    .punctuation
                    .render(ch, self.input.punctuation_mode)?;
                self.finish_memory();
                return Some(format!("{raw}{symbol}"));
            }
            _ => {}
        }
        None
    }
}
