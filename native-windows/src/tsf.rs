#![allow(non_snake_case)]
mod diagnostics;
mod document;
mod mode;
mod panel;
mod registration;
mod updates;
use crate::bridge::{Bridge, Confirmation, EngineFailure};
use crate::input::{Input, Key};
use crate::worker::{Command, Reply, Selection, Worker};
use diagnostics::Stage;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::ffi::c_void;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, AtomicPtr, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleFileNameW,
    GetModuleHandleExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
use windows::Win32::UI::TextServices::*;
use windows::core::{
    BOOL, GUID, HRESULT, IUnknown, IUnknownImpl, Interface, Ref, Result, implement,
};

const CLASS: GUID = GUID::from_u128(0x078a5202_3f7a_4dd5_89cc_322a9011df81);
const PROFILE: GUID = GUID::from_u128(0x5c46b182_7e32_4b6e_ab5d_2ee4f17d7695);
const CLASS_TEXT: &str = "{078A5202-3F7A-4DD5-89CC-322A9011DF81}";
static MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static REFERENCES: AtomicIsize = AtomicIsize::new(0);
pub(crate) struct Guard;
impl Guard {
    pub(crate) fn new() -> Self {
        REFERENCES.fetch_add(1, Ordering::SeqCst);
        Self
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        REFERENCES.fetch_sub(1, Ordering::SeqCst);
    }
}
fn module_path() -> Result<PathBuf> {
    let mut buffer = [0u16; 32768];
    let size =
        unsafe { GetModuleFileNameW(Some(HMODULE(MODULE.load(Ordering::SeqCst))), &mut buffer) }
            as usize;
    if size == 0 || size >= buffer.len() {
        return Err(E_FAIL.into());
    }
    Ok(PathBuf::from(String::from_utf16_lossy(&buffer[..size])))
}
fn guarded<T>(body: impl FnOnce() -> Result<T>) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
}
fn down(key: i32) -> bool {
    (unsafe { GetKeyState(key) }) < 0
}
fn command_modifier() -> bool {
    down(0x11) || down(0x12) || down(0x5B) || down(0x5C)
}
fn blocked(context: &ITfContext) -> bool {
    let Ok(manager) = context.cast::<ITfCompartmentMgr>() else {
        return true;
    };
    [
        GUID_COMPARTMENT_KEYBOARD_DISABLED,
        GUID_COMPARTMENT_EMPTYCONTEXT,
    ]
    .iter()
    .any(|guid| {
        unsafe { manager.GetCompartment(guid) }
            .and_then(|c| unsafe { c.GetValue() })
            .ok()
            .and_then(|v| i32::try_from(&v).ok())
            .is_some_and(|v| v != 0)
    })
}
fn decode(vk: usize, composing: bool) -> Option<Key> {
    let shift = down(0x10);
    match vk {
        0x41..=0x5A => {
            let uppercase = shift ^ (unsafe { GetKeyState(0x14) } & 1 != 0);
            Some(Key::Letter(if uppercase {
                vk as u8 as char
            } else {
                (vk as u8 + 32) as char
            }))
        }
        0xDE if composing && !shift => Some(Key::Letter('\'')),
        0x20 if composing => Some(Key::Space(shift)),
        0x0D if composing => Some(Key::Return),
        0x08 if composing => Some(Key::Backspace),
        0x1B if composing => Some(Key::Escape),
        0x09 if composing => Some(Key::Tab(shift)),
        0x26 if composing => Some(Key::Up),
        0x28 if composing => Some(Key::Down),
        0x21 | 0xBD if composing && !shift => Some(Key::Previous),
        0x22 | 0xBB if composing && !shift => Some(Key::Next),
        0x31..=0x35 if composing && !shift => Some(Key::Digit(vk - 0x31)),
        0x30 if shift => Some(Key::Punctuation(')')),
        0x39 if shift => Some(Key::Punctuation('(')),
        0x32 if shift => Some(Key::Punctuation('@')),
        0xDB if !shift => Some(Key::Punctuation('[')),
        0xDD if !shift => Some(Key::Punctuation(']')),
        0xDE => Some(Key::Punctuation(if shift { '"' } else { '\'' })),
        0xDC if !shift => Some(Key::Punctuation('\\')),
        0xBD if shift => Some(Key::Punctuation('_')),
        0xBC => Some(Key::Punctuation(if shift { '<' } else { ',' })),
        0xBE => Some(Key::Punctuation(if shift { '>' } else { '.' })),
        0xBF => Some(Key::Punctuation(if shift { '?' } else { '/' })),
        0x31 if shift => Some(Key::Punctuation('!')),
        0xBA => Some(Key::Punctuation(if shift { ':' } else { ';' })),
        0x30..=0x39 if !composing && !shift => Some(Key::Literal(vk as u8 as char)),
        0x60..=0x69 if !composing => Some(Key::Literal((vk as u8 - 0x60 + b'0') as char)),
        0x6E if !composing => Some(Key::Punctuation('.')),
        0x20 if !composing => Some(Key::Literal(' ')),
        0xBD if !composing => Some(Key::Literal('-')),
        0xBB if !composing => Some(Key::Literal(if shift { '+' } else { '=' })),
        _ => None,
    }
}
struct State {
    input: RefCell<Input>,
    worker: RefCell<Option<Worker>>,
    outbox: RefCell<VecDeque<Command>>,
    deferred_boundary: RefCell<Option<(Command, Binding)>>,
    deferred_mode: Cell<Option<bool>>,
    deferred_context: Cell<bool>,
    sequence: Cell<u64>,
    displayed: Cell<u64>,
    latest: Cell<u64>,
    pending: RefCell<BTreeMap<u64, Binding>>,
    edited: RefCell<Vec<Rc<document::Document>>>,
    busy: Cell<bool>,
    polling: Cell<bool>,
    refreshing_palette: Cell<bool>,
    composing: Cell<bool>,
    failure: Cell<Option<EngineFailure>>,
    retiring: Cell<bool>,
    barrier: Cell<Option<u64>>,
    drained: Cell<bool>,
    document: RefCell<Option<Rc<document::Document>>>,
    panel: RefCell<Option<panel::Panel>>,
    mode: RefCell<Option<Rc<mode::ModeController>>>,
    client: Cell<u32>,
    show_pos: Cell<bool>,
    display: Cell<crate::preferences::Display>,
    memory_warning: Cell<bool>,
    preview: bool,
}
#[derive(Clone)]
struct Binding {
    document: Option<Rc<document::Document>>,
    epoch: u64,
    text: bool,
}
thread_local! {
    // TSF can deactivate while an engine query is in flight. Keep the originating
    // document and hidden reply window alive until edits and confirmations drain.
    static RETIRED: RefCell<Vec<Rc<State>>> = const { RefCell::new(Vec::new()) };
}
impl State {
    fn new() -> Rc<Self> {
        let state = Self::with_display_preferences(crate::preferences::load());
        state.display.set(crate::preferences::display());
        state
    }
    fn with_display_preferences(show_pos: bool) -> Rc<Self> {
        Rc::new(Self {
            input: RefCell::new(Input::default()),
            worker: RefCell::new(None),
            outbox: RefCell::new(VecDeque::new()),
            deferred_boundary: RefCell::new(None),
            deferred_mode: Cell::new(None),
            deferred_context: Cell::new(false),
            sequence: Cell::new(0),
            displayed: Cell::new(0),
            latest: Cell::new(0),
            pending: RefCell::new(BTreeMap::new()),
            edited: RefCell::new(Vec::new()),
            busy: Cell::new(false),
            polling: Cell::new(false),
            refreshing_palette: Cell::new(false),
            composing: Cell::new(false),
            failure: Cell::new(None),
            retiring: Cell::new(false),
            barrier: Cell::new(None),
            drained: Cell::new(false),
            document: RefCell::new(None),
            panel: RefCell::new(None),
            mode: RefCell::new(None),
            client: Cell::new(0),
            show_pos: Cell::new(show_pos),
            display: Cell::new(crate::preferences::Display::default()),
            memory_warning: Cell::new(false),
            preview: false,
        })
    }
    fn initialize(&self) -> Result<()> {
        if self.preview || self.worker.borrow().is_some() {
            return Ok(());
        }
        // A detached query must never execute after this TSF DLL was unloaded.
        // Pinning is process-local; it does not select a profile or restart a host.
        let mut pinned = HMODULE::default();
        unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                windows::core::PCWSTR(DllGetClassObject as *const () as *const u16),
                &mut pinned,
            )?;
        }
        let root = module_path()?.parent().ok_or(E_FAIL)?.to_path_buf();
        let memory = std::env::var_os("LOCALAPPDATA")
            .map(|path| PathBuf::from(path).join("LingoMate/PersonalVocabulary/words.json"));
        *self.worker.borrow_mut() = Some(Worker::start_with(move || {
            Bridge::start_embedded(&root.join("lingomate_engine.dll"), &root, memory.as_deref())
        }));
        let input = self.input.borrow();
        self.send(Command::Remember(input.remember));
        self.send(Command::PunctuationMode(input.punctuation_mode));
        self.send(Command::EnglishMode(input.english));
        Ok(())
    }
    fn send(&self, command: Command) -> bool {
        let data = matches!(&command, Command::Key { .. } | Command::Keyboard { .. });
        let limit = if data {
            32
        } else {
            crate::worker::QUEUE_CAPACITY * 2
        };
        if self.worker.borrow().is_none() || self.outbox.borrow().len() >= limit {
            return false;
        }
        if self.outbox.borrow().is_empty() {
            let result = self
                .worker
                .borrow()
                .as_ref()
                .unwrap()
                .enqueue(command.clone());
            match result {
                Ok(()) => return true,
                Err(crate::worker::EnqueueError::Stopped) => return false,
                Err(crate::worker::EnqueueError::Full) => {}
            }
        }
        // Retain FIFO boundaries and confirmations when the worker is occupied.
        // Spare control capacity keeps a new document behind the old Finish.
        self.outbox.borrow_mut().push_back(command);
        true
    }
    fn accepts_keys(&self) -> bool {
        self.worker.borrow().is_some()
            && self.deferred_boundary.borrow().is_none()
            && self.deferred_mode.get().is_none()
            && !self.deferred_context.get()
            && self.outbox.borrow().len() < 32
    }
    fn flush(&self) {
        loop {
            if self.outbox.borrow().len() < crate::worker::QUEUE_CAPACITY * 2 {
                let boundary = self.deferred_boundary.borrow_mut().take();
                if let Some((command, binding)) = boundary {
                    let id = match &command {
                        Command::Finish { id } | Command::Reset { id } => *id,
                        _ => unreachable!(),
                    };
                    self.outbox.borrow_mut().push_back(command);
                    self.bind_to(id, binding);
                    self.input.borrow_mut().reset();
                    self.composing.set(false);
                }
            }
            if self.deferred_boundary.borrow().is_none()
                && self.outbox.borrow().len() < crate::worker::QUEUE_CAPACITY * 2
                && let Some(english) = self.deferred_mode.take()
            {
                // Keep the mode after its Finish even when both bounded queues
                // are full. New keys cannot overtake this control boundary.
                self.outbox
                    .borrow_mut()
                    .push_back(Command::EnglishMode(english));
            }
            if self.deferred_boundary.borrow().is_none()
                && self.outbox.borrow().len() < crate::worker::QUEUE_CAPACITY * 2
                && self.deferred_context.replace(false)
            {
                self.outbox.borrow_mut().push_back(Command::BreakContext);
            }
            let command = self.outbox.borrow().front().cloned();
            let Some(command) = command else { break };
            let sent = self
                .worker
                .borrow()
                .as_ref()
                .is_some_and(|worker| worker.enqueue(command).is_ok());
            if !sent {
                break;
            }
            self.outbox.borrow_mut().pop_front();
        }
    }
    fn next(&self) -> u64 {
        let id = self.sequence.get().wrapping_add(1).max(1);
        self.sequence.set(id);
        id
    }
    fn bind(&self, id: u64, text: bool) {
        let document = self.document.borrow().clone();
        let epoch = document.as_ref().map_or(0, |doc| doc.epoch.get());
        self.bind_to(
            id,
            Binding {
                document,
                epoch,
                text,
            },
        );
    }
    fn bind_to(&self, id: u64, binding: Binding) {
        self.pending.borrow_mut().insert(id, binding);
        self.latest.set(id);
        self.busy.set(true);
    }
    fn cancel_memory(&self) {
        self.send(Command::CancelMemory);
    }
    fn break_input_context(&self) {
        self.deferred_context.set(true);
        self.flush();
        self.input.borrow_mut().punctuation.reset_context();
    }
    fn cancel_composition(&self) {
        let id = self.next();
        self.boundary(id, true);
        self.input.borrow_mut().reset();
        self.composing.set(false);
        self.refresh();
    }
    fn boundary(&self, id: u64, reset: bool) -> bool {
        let command = if reset {
            Command::Reset { id }
        } else {
            Command::Finish { id }
        };
        if self.deferred_boundary.borrow().is_none() && self.send(command.clone()) {
            self.bind(id, false);
            return true;
        }
        if reset || self.deferred_boundary.borrow().is_none() {
            let document = self.document.borrow().clone();
            let epoch = document.as_ref().map_or(0, |doc| doc.epoch.get());
            *self.deferred_boundary.borrow_mut() = Some((
                command,
                Binding {
                    document,
                    epoch,
                    text: false,
                },
            ));
        }
        false
    }
    fn finish_memory(&self) {
        // A boundary belongs after all earlier queued keys, including a pending commit.
        self.finish();
    }
    fn confirm_memory(&self, receipt: &Confirmation, successful: bool, output: Option<&str>) {
        if self.input.borrow().remember
            && !self.send(Command::Confirm {
                receipt: receipt.clone(),
                successful,
                output: output.map(str::to_owned),
            })
        {
            self.memory_warning.set(true);
        }
    }
    fn remember(&self, remember: bool) {
        if self.send(Command::Remember(remember)) {
            self.input.borrow_mut().remember = remember;
        }
    }
    fn punctuation_mode(&self, mode: crate::punctuation::Mode) {
        if self.send(Command::PunctuationMode(mode)) {
            let mut input = self.input.borrow_mut();
            input.punctuation_mode = mode;
            input.punctuation.reset_context();
        }
    }
    fn settings(self: &Rc<Self>) {
        let window = self.panel.borrow().as_ref().map(panel::Panel::window);
        if let Some(window) = window {
            panel::settings(window, self);
        }
    }
    fn english(&self, english: bool) {
        self.english_from_system(english);
        let mode = self.mode.borrow().clone();
        if let Some(mode) = mode {
            let _ = mode.set_english(english);
        }
    }
    fn english_from_system(&self, english: bool) {
        if self.input.borrow().english == english {
            return;
        }
        self.finish();
        diagnostics::stage(Stage::ModeChange, S_OK);
        diagnostics::mode(english);
        {
            let mut input = self.input.borrow_mut();
            input.english = english;
            input.last_english = english;
            input.punctuation.reset_context();
        }
        self.deferred_mode.set(Some(english));
        self.flush();
    }
    #[cfg(test)]
    fn key(self: &Rc<Self>, key: Key) {
        let _ = self.submit_key(key, None, None);
    }
    fn click(self: &Rc<Self>, key: Key) {
        if !self.busy.get() {
            let selection = Selection::from_input(self.displayed.get(), &self.input.borrow());
            let _ = self.submit_key(key, Some(selection), None);
        }
    }
    fn keyboard(self: &Rc<Self>, vk: usize) -> bool {
        let idle = decode(vk, false);
        let composing = decode(vk, true);
        let shadow = if self.input.borrow().raw.is_empty() {
            idle
        } else {
            composing
        };
        if let Some(key) = shadow.or(composing) {
            return self.submit_key(key, None, Some((idle, composing)));
        }
        false
    }
    fn submit_key(
        self: &Rc<Self>,
        key: Key,
        selection: Option<Selection>,
        keyboard: Option<(Option<Key>, Option<Key>)>,
    ) -> bool {
        if self.preview || self.retiring.get() || !self.accepts_keys() {
            return false;
        }
        let id = self.next();
        let command = if let Some((idle, composing)) = keyboard {
            Command::Keyboard {
                id,
                idle,
                composing,
            }
        } else {
            Command::Key { id, key, selection }
        };
        if !self.send(command) {
            self.failure.set(Some(EngineFailure::Unavailable));
            self.input.borrow_mut().error = true;
            self.refresh();
            return false;
        }
        let document = self.document.borrow().clone();
        let own_pending = self.pending.borrow().values().any(|binding| {
            binding.text
                && binding
                    .document
                    .as_ref()
                    .zip(document.as_ref())
                    .is_some_and(|(old, current)| Rc::ptr_eq(old, current))
        });
        let anchor = !own_pending && self.input.borrow().raw.is_empty();
        self.bind(id, true);
        {
            let mut input = self.input.borrow_mut();
            let navigation = matches!(
                key,
                Key::Tab(_) | Key::Up | Key::Down | Key::Previous | Key::Next
            ) || (matches!(key, Key::Escape) && input.expanded);
            if !input.raw.is_empty() || matches!(key, Key::Letter(_)) {
                self.composing.set(true);
            }
            match key {
                Key::Letter(ch) if input.raw.len() < 240 => input.raw.push(ch),
                Key::Backspace => {
                    input.raw.pop();
                }
                Key::Escape if input.expanded => input.expanded = false,
                Key::Escape | Key::Return => input.reset(),
                _ => {}
            }
            // The displayed frame stays until the FIFO worker replies, so typing
            // never flashes an empty, resized panel. Clicks remain disabled until
            // that reply arrives; a failed frame is never shown again.
            if !navigation && input.error {
                input.frame = Default::default();
            }
            input.error = false;
        }
        // Anchor the first synthetic preedit immediately, before a slow query can
        // observe a later selection. No host text is read.
        if anchor && let Some(doc) = document {
            let preedit = match key {
                Key::Letter(ch) | Key::Literal(ch) | Key::Punctuation(ch) => Some(ch.to_string()),
                _ => None,
            };
            if let Some(preedit) = preedit {
                let _ = doc.enqueue(None, preedit, None, None);
            }
            if !self.edited.borrow().iter().any(|old| Rc::ptr_eq(old, &doc)) {
                self.edited.borrow_mut().push(doc);
            }
        }
        self.refresh();
        true
    }
    fn refresh(&self) {
        if let Some(panel) = self.panel.borrow().as_ref() {
            if self.retiring.get() {
                panel.hide();
            } else {
                panel.update();
            }
        }
    }
    fn refresh_palette(&self) {
        if self.retiring.get() || self.refreshing_palette.replace(true) {
            return;
        }
        struct Refreshing<'a>(&'a Cell<bool>);
        impl Drop for Refreshing<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _refreshing = Refreshing(&self.refreshing_palette);
        // Keep palette reads outside hover repaint and without an input borrow.
        let palette = crate::appearance::palette(self.display.get().theme);
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.set_palette(palette);
        }
    }
    fn poll(self: &Rc<Self>) -> bool {
        if self.polling.replace(true) {
            return false;
        }
        struct Polling<'a>(&'a Cell<bool>);
        impl Drop for Polling<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _polling = Polling(&self.polling);
        let started = Instant::now();
        self.flush();
        let mut changed = false;
        for _ in 0..16 {
            if started.elapsed() >= Duration::from_millis(4) {
                break;
            }
            // Release this borrow before edits can synchronously confirm a receipt.
            let reply = self
                .worker
                .borrow()
                .as_ref()
                .and_then(|worker| worker.try_recv().ok());
            let Some(reply) = reply else { break };
            changed = true;
            match reply {
                Reply::Update {
                    id,
                    input,
                    commit,
                    study,
                    memory,
                    failure,
                    memory_warning,
                } => {
                    let binding = self.pending.borrow_mut().remove(&id);
                    if let Some(binding) = binding {
                        let valid = binding.document.as_ref().is_none_or(|doc| {
                            !blocked(&doc.context)
                                && doc.epoch.get() == binding.epoch
                                && !doc.terminated.get()
                        });
                        if valid {
                            if let Some(doc) = binding.document {
                                // Queries for old prefixes need no intermediate rewrite.
                                // Commits remain FIFO and always use their original document.
                                if commit.is_some() || id == self.latest.get() {
                                    let result = doc.enqueue(
                                        commit,
                                        input.raw.clone(),
                                        study,
                                        memory.clone(),
                                    );
                                    if result.is_err()
                                        && let Some(receipt) = &memory
                                    {
                                        self.confirm_memory(receipt, false, None);
                                    }
                                    if !self.edited.borrow().iter().any(|old| Rc::ptr_eq(old, &doc))
                                    {
                                        self.edited.borrow_mut().push(doc);
                                    }
                                }
                            }
                        } else if let Some(receipt) = &memory {
                            self.confirm_memory(receipt, false, None);
                        }
                        if id == self.latest.get() && valid {
                            let mut cached = self.input.borrow_mut();
                            let remember = cached.remember;
                            let english = cached.english;
                            let mode = cached.punctuation_mode;
                            *cached = *input;
                            cached.remember = remember;
                            cached.english = english;
                            cached.punctuation_mode = mode;
                            self.displayed.set(id);
                            self.failure.set(failure);
                            self.busy.set(false);
                            self.composing.set(!cached.raw.is_empty());
                        } else if id == self.latest.get() {
                            self.input.borrow_mut().reset();
                            self.busy.set(false);
                            self.composing.set(false);
                        }
                        self.memory_warning
                            .set(memory_warning || self.memory_warning.get());
                    }
                }
                Reply::Warning {
                    failure,
                    memory_warning,
                } => {
                    if failure.is_some() {
                        self.failure.set(failure);
                    }
                    self.memory_warning
                        .set(memory_warning || self.memory_warning.get());
                }
                Reply::Barrier { id } => {
                    if self.barrier.get() == Some(id) {
                        self.drained.set(true);
                    }
                }
            }
            self.flush();
        }
        self.edited.borrow_mut().retain(|doc| !doc.is_idle());
        if self.retiring.get()
            && self.pending.borrow().is_empty()
            && self.edited.borrow().is_empty()
            && self.deferred_boundary.borrow().is_none()
            && self.deferred_mode.get().is_none()
            && !self.deferred_context.get()
            && self.barrier.get().is_none()
        {
            let id = self.next();
            if self.send(Command::Barrier { id }) {
                self.barrier.set(Some(id));
            }
        }
        if changed {
            self.refresh();
        }
        true
    }
    fn finish(&self) -> bool {
        if self.preview {
            return true;
        }
        let id = self.next();
        let queued = self.boundary(id, false);
        self.input.borrow_mut().reset();
        self.composing.set(false);
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.hide();
        }
        queued
    }
    fn retire(self: &Rc<Self>) {
        if self.retiring.replace(true) {
            return;
        }
        if self.worker.borrow().is_some() {
            RETIRED.with(|retired| retired.borrow_mut().push(self.clone()));
        } else {
            self.panel.borrow_mut().take();
        }
    }
    fn reap() {
        RETIRED.with(|retired| {
            retired.borrow_mut().retain(|state| {
                if state.drained.get() {
                    state.worker.borrow_mut().take();
                    state.panel.borrow_mut().take();
                    state.document.borrow_mut().take();
                    false
                } else {
                    true
                }
            });
        });
    }
    fn attach(self: &Rc<Self>, context: &ITfContext) {
        let same = self
            .document
            .borrow()
            .as_ref()
            .is_some_and(|d| d.context.as_raw() == context.as_raw());
        if !same {
            if !self.finish() {
                return;
            }
            *self.document.borrow_mut() = Some(document::Document::new(
                context.clone(),
                self.client.get(),
                Rc::downgrade(self),
            ));
        }
        if self
            .document
            .borrow()
            .as_ref()
            .is_some_and(|d| d.terminated.replace(false))
        {
            self.input.borrow_mut().reset();
        }
    }
}

#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfThreadMgrEventSink,
    ITfThreadFocusSink
)]
struct Service {
    manager: RefCell<Option<ITfThreadMgr>>,
    state: RefCell<Rc<State>>,
    shift: Cell<Option<Instant>>,
    advice: Cell<Option<u32>>,
    thread_advice: Cell<Option<u32>>,
    thread_focused: Cell<bool>,
    keys_advised: Cell<bool>,
    activating: Cell<bool>,
    _guard: Guard,
}
impl Service_Impl {
    fn attach_focus(&self) {
        let manager = self.manager.borrow().clone();
        if let Some(manager) = manager
            && let Ok(document) = unsafe { manager.GetFocus() }
            && let Ok(context) = unsafe { document.GetTop() }
        {
            let state = self.state.borrow().clone();
            state.attach(&context);
        }
    }
    fn disconnect(&self, state: &State) {
        let mode = state.mode.borrow_mut().take();
        if let Some(mode) = mode {
            let _ = mode.shutdown();
        }
        let manager = self.manager.borrow_mut().take();
        if let Some(manager) = manager {
            if let Ok(source) = manager.cast::<ITfSource>() {
                for cookie in [self.thread_advice.take(), self.advice.take()]
                    .into_iter()
                    .flatten()
                {
                    let _ = unsafe { source.UnadviseSink(cookie) };
                }
            }
            if self.keys_advised.replace(false)
                && let Ok(keys) = manager.cast::<ITfKeystrokeMgr>()
            {
                let _ = unsafe { keys.UnadviseKeyEventSink(state.client.get()) };
            }
        }
        self.thread_focused.set(false);
        self.shift.set(None);
    }
}
impl ITfTextInputProcessor_Impl for Service_Impl {
    fn Activate(&self, manager: Ref<ITfThreadMgr>, client: u32) -> Result<()> {
        diagnostics::stage(Stage::ActivateStart, S_OK);
        let result = guarded(|| {
            if self.activating.replace(true) {
                return Err(E_UNEXPECTED.into());
            }
            struct Activating<'a>(&'a Cell<bool>);
            impl Drop for Activating<'_> {
                fn drop(&mut self) {
                    self.0.set(false);
                }
            }
            let _activating = Activating(&self.activating);
            if self.manager.borrow().is_some() {
                return Err(E_UNEXPECTED.into());
            }
            let state = self.state.borrow().clone();
            let state = if state.retiring.get() {
                let fresh = State::new();
                {
                    let previous = state.input.borrow();
                    let mut input = fresh.input.borrow_mut();
                    input.remember = previous.remember;
                    input.english = previous.english;
                    input.last_english = previous.english;
                    input.punctuation_mode = previous.punctuation_mode;
                }
                *self.state.borrow_mut() = fresh.clone();
                fresh
            } else {
                state
            };
            let manager = manager.ok()?.clone();
            state.client.set(client);
            let source: ITfSource = manager.cast()?;
            diagnostics::stage(Stage::ActivateWorker, S_OK);
            state.initialize()?;
            let result = (|| {
                diagnostics::stage(Stage::ActivatePanel, S_OK);
                let panel = panel::Panel::new(Rc::downgrade(&state))?;
                *state.panel.borrow_mut() = Some(panel);
                *self.manager.borrow_mut() = Some(manager.clone());
                diagnostics::stage(Stage::ActivateManager, S_OK);
                self.thread_focused
                    .set(unsafe { manager.IsThreadFocus()? }.as_bool());
                diagnostics::state(state.input.borrow().english, self.thread_focused.get());
                let keys: ITfKeystrokeMgr = manager.cast()?;
                let sink: ITfKeyEventSink = self.to_interface();
                diagnostics::stage(Stage::ActivateKeySink, S_OK);
                unsafe { keys.AdviseKeyEventSink(client, &sink, true)? };
                self.keys_advised.set(true);
                let focus: ITfThreadMgrEventSink = self.to_interface();
                diagnostics::stage(Stage::ActivateDocumentSink, S_OK);
                self.advice.set(Some(unsafe {
                    source.AdviseSink(&ITfThreadMgrEventSink::IID, &focus)?
                }));
                let thread: ITfThreadFocusSink = self.to_interface();
                diagnostics::stage(Stage::ActivateThreadSink, S_OK);
                self.thread_advice.set(Some(unsafe {
                    source.AdviseSink(&ITfThreadFocusSink::IID, &thread)?
                }));
                diagnostics::stage(Stage::ActivateMode, S_OK);
                let mode = mode::ModeController::new(&manager, client, Rc::downgrade(&state))?;
                *state.mode.borrow_mut() = Some(mode);
                diagnostics::stage(Stage::ActivateFocus, S_OK);
                self.attach_focus();
                Ok(())
            })();
            if let Err(error) = result {
                self.disconnect(&state);
                state.panel.borrow_mut().take();
                state.document.borrow_mut().take();
                state.outbox.borrow_mut().clear();
                state.deferred_mode.set(None);
                state.deferred_context.set(false);
                state.deferred_boundary.borrow_mut().take();
                state.pending.borrow_mut().clear();
                state.input.borrow_mut().reset();
                state.busy.set(false);
                state.composing.set(false);
                state.worker.borrow_mut().take();
                return Err(error);
            }
            let _ = updates::launch(true);
            diagnostics::state(state.input.borrow().english, self.thread_focused.get());
            diagnostics::stage(Stage::ActivateReady, S_OK);
            Ok(())
        });
        if let Err(error) = &result {
            diagnostics::status(error.code());
        }
        result
    }
    fn Deactivate(&self) -> Result<()> {
        diagnostics::stage(Stage::DeactivateStart, S_OK);
        guarded(|| {
            let state = self.state.borrow().clone();
            self.shift.set(None);
            state.finish();
            self.disconnect(&state);
            state.retire();
            diagnostics::state(state.input.borrow().english, false);
            diagnostics::stage(Stage::DeactivateDone, S_OK);
            Ok(())
        })
    }
}
impl ITfTextInputProcessorEx_Impl for Service_Impl {
    fn ActivateEx(&self, manager: Ref<ITfThreadMgr>, client: u32, flags: u32) -> Result<()> {
        // No COM is initialized on the engine worker. The TSF thread may be STA,
        // MTA or COM-less; it keeps all TSF interfaces and edit sessions locally.
        diagnostics::stage(Stage::ActivateExStart, S_OK);
        let active_flags = manager
            .ok()
            .ok()
            .and_then(|manager| manager.cast::<ITfThreadMgrEx>().ok())
            .and_then(|manager| unsafe { manager.GetActiveFlags() }.ok());
        diagnostics::flags(flags, active_flags);
        self.Activate(manager, client)
    }
}
impl ITfKeyEventSink_Impl for Service_Impl {
    fn OnSetFocus(&self, foreground: BOOL) -> Result<()> {
        guarded(|| {
            let state = self.state.borrow().clone();
            if foreground.as_bool() {
                state.show_pos.set(crate::preferences::load());
                state.display.set(crate::preferences::display());
                state.refresh_palette();
            }
            self.shift.set(None);
            if !foreground.as_bool() {
                state.finish();
            }
            Ok(())
        })
    }
    fn OnTestKeyDown(&self, context: Ref<ITfContext>, key: WPARAM, _flags: LPARAM) -> Result<BOOL> {
        guarded(|| {
            if !self.thread_focused.get() {
                return Ok(FALSE);
            }
            let state = self.state.borrow().clone();
            if key.0 == 0x10 && !command_modifier() && down(0xA0) != down(0xA1) {
                if self.shift.get().is_none() {
                    self.shift.set(Some(Instant::now()));
                }
            } else {
                self.shift.set(None);
            }
            let Ok(context) = context.ok() else {
                return Ok(FALSE);
            };
            if blocked(context) {
                state.cancel_composition();
                return Ok(FALSE);
            }
            if state.input.borrow().english || command_modifier() {
                state.break_input_context();
                if !blocked(context)
                    && (updates::shortcut(key.0) || panel::settings_shortcut(key.0))
                {
                    return Ok(TRUE);
                }
                return Ok(FALSE);
            }
            if !state.accepts_keys() {
                return Ok(FALSE);
            }
            let decoded = decode(
                key.0,
                state.composing.get() || !state.input.borrow().raw.is_empty(),
            );
            if decoded.is_none() {
                if key.0 == 0x20 || key.0 == 0x0D {
                    state.finish_memory();
                } else if matches!(key.0, 0x10 | 0xA0 | 0xA1) {
                    state.cancel_memory();
                } else {
                    state.break_input_context();
                }
            }
            Ok(decoded.is_some().into())
        })
    }
    fn OnKeyDown(&self, context: Ref<ITfContext>, key: WPARAM, _flags: LPARAM) -> Result<BOOL> {
        guarded(|| {
            if !self.thread_focused.get() {
                return Ok(FALSE);
            }
            let state = self.state.borrow().clone();
            let context = context.ok()?;
            if !blocked(context) && panel::settings_shortcut(key.0) {
                state.settings();
                return Ok(TRUE);
            }
            if !blocked(context) && updates::shortcut(key.0) {
                let _ = updates::launch(false);
                return Ok(TRUE);
            }
            if blocked(context) || command_modifier() || state.input.borrow().english {
                return Ok(FALSE);
            }
            state.attach(context);
            let composing = state.composing.get() || !state.input.borrow().raw.is_empty();
            if decode(key.0, composing).is_some() {
                Ok(state.keyboard(key.0).into())
            } else {
                state.finish();
                Ok(FALSE)
            }
        })
    }
    fn OnTestKeyUp(&self, context: Ref<ITfContext>, key: WPARAM, _flags: LPARAM) -> Result<BOOL> {
        guarded(|| {
            let state = self.state.borrow().clone();
            if key.0 == 0x10
                && self.thread_focused.get()
                && let Some(at) = self.shift.take()
                && at.elapsed() <= Duration::from_millis(700)
                && !command_modifier()
                && context.ok().is_ok_and(|c| !blocked(c))
            {
                let english = !state.input.borrow().english;
                state.english(english);
            }
            Ok(FALSE)
        })
    }
    fn OnKeyUp(&self, _context: Ref<ITfContext>, _key: WPARAM, _flags: LPARAM) -> Result<BOOL> {
        guarded(|| Ok(FALSE))
    }
    fn OnPreservedKey(&self, _context: Ref<ITfContext>, _guid: *const GUID) -> Result<BOOL> {
        guarded(|| Ok(FALSE))
    }
}
impl ITfThreadFocusSink_Impl for Service_Impl {
    fn OnSetThreadFocus(&self) -> Result<()> {
        guarded(|| {
            self.thread_focused.set(true);
            self.shift.set(None);
            let state = self.state.borrow().clone();
            diagnostics::state(state.input.borrow().english, true);
            diagnostics::stage(Stage::ThreadFocus, S_OK);
            state.show_pos.set(crate::preferences::load());
            state.display.set(crate::preferences::display());
            state.refresh_palette();
            self.attach_focus();
            Ok(())
        })
    }
    fn OnKillThreadFocus(&self) -> Result<()> {
        guarded(|| {
            self.thread_focused.set(false);
            self.shift.set(None);
            let state = self.state.borrow().clone();
            diagnostics::state(state.input.borrow().english, false);
            diagnostics::stage(Stage::ThreadFocus, S_OK);
            state.finish();
            Ok(())
        })
    }
}
impl ITfThreadMgrEventSink_Impl for Service_Impl {
    fn OnInitDocumentMgr(&self, _manager: Ref<ITfDocumentMgr>) -> Result<()> {
        guarded(|| Ok(()))
    }
    fn OnUninitDocumentMgr(&self, _manager: Ref<ITfDocumentMgr>) -> Result<()> {
        guarded(|| Ok(()))
    }
    fn OnSetFocus(
        &self,
        manager: Ref<ITfDocumentMgr>,
        _previous: Ref<ITfDocumentMgr>,
    ) -> Result<()> {
        guarded(|| {
            let state = self.state.borrow().clone();
            state.show_pos.set(crate::preferences::load());
            state.display.set(crate::preferences::display());
            state.refresh_palette();
            self.shift.set(None);
            state.finish();
            if let Ok(manager) = manager.ok()
                && let Ok(context) = unsafe { manager.GetTop() }
            {
                state.attach(&context);
            }
            Ok(())
        })
    }
    fn OnPushContext(&self, context: Ref<ITfContext>) -> Result<()> {
        guarded(|| {
            let state = self.state.borrow().clone();
            self.shift.set(None);
            if let Ok(context) = context.ok() {
                state.attach(context);
            }
            Ok(())
        })
    }
    fn OnPopContext(&self, context: Ref<ITfContext>) -> Result<()> {
        guarded(|| {
            let state = self.state.borrow().clone();
            self.shift.set(None);
            let own = context.ok().is_ok_and(|c| {
                state
                    .document
                    .borrow()
                    .as_ref()
                    .is_some_and(|d| d.context.as_raw() == c.as_raw())
            });
            if own {
                state.finish();
                state.document.borrow_mut().take();
            }
            Ok(())
        })
    }
}
#[implement(IClassFactory)]
struct Factory {
    _guard: Guard,
}
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        result: *mut *mut c_void,
    ) -> Result<()> {
        diagnostics::stage(Stage::FactoryCreate, S_OK);
        if !outer.is_null() {
            return CLASS_E_NOAGGREGATION.ok();
        }
        if iid.is_null() || result.is_null() {
            return E_POINTER.ok();
        }
        unsafe {
            *result = std::ptr::null_mut();
        }
        let object: IUnknown = Service {
            _guard: Guard::new(),
            manager: RefCell::new(None),
            state: RefCell::new(State::new()),
            shift: Cell::new(None),
            advice: Cell::new(None),
            thread_advice: Cell::new(None),
            thread_focused: Cell::new(false),
            keys_advised: Cell::new(false),
            activating: Cell::new(false),
        }
        .into();
        unsafe { object.query(iid, result).ok() }
    }
    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            REFERENCES.fetch_add(1, Ordering::SeqCst);
        } else {
            REFERENCES.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }
}
#[unsafe(no_mangle)]
extern "system" fn DllMain(instance: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == 1 {
        MODULE.store(instance.0, Ordering::SeqCst);
    }
    TRUE
}
#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(
    class: *const GUID,
    iid: *const GUID,
    result: *mut *mut c_void,
) -> HRESULT {
    if class.is_null() || iid.is_null() || result.is_null() {
        return E_POINTER;
    }
    unsafe {
        *result = std::ptr::null_mut();
    }
    if unsafe { *class } != CLASS {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    diagnostics::stage(Stage::ClassObject, S_OK);
    let factory: IClassFactory = Factory {
        _guard: Guard::new(),
    }
    .into();
    unsafe { factory.query(iid, result) }
}
#[unsafe(no_mangle)]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    if REFERENCES.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}
#[unsafe(no_mangle)]
extern "system" fn DllRegisterServer() -> HRESULT {
    registration::register().err().map_or(S_OK, |e| e.code())
}
#[unsafe(no_mangle)]
extern "system" fn DllUnregisterServer() -> HRESULT {
    registration::unregister().err().map_or(S_OK, |e| e.code())
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn nested_timer_does_not_reenter_the_dispatcher() {
        let state = State::with_display_preferences(false);
        state.polling.set(true);
        let held = state.input.borrow_mut();
        assert!(!state.poll());
        assert!(state.polling.get());
        drop(held);
        state.polling.set(false);
        assert!(state.poll());
        assert!(!state.polling.get());
    }

    fn drain(state: &Rc<State>) {
        let until = Instant::now() + Duration::from_secs(5);
        while !state.pending.borrow().is_empty()
            || !state.outbox.borrow().is_empty()
            || state.deferred_mode.get().is_some()
            || state.deferred_context.get()
            || state.deferred_boundary.borrow().is_some()
        {
            assert!(Instant::now() < until, "owned dispatcher did not drain");
            state.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn system_mode_boundary_survives_both_full_queues_before_keys_resume() {
        let (started, initialized) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let state = State::with_display_preferences(false);
        *state.worker.borrow_mut() = Some(Worker::start_with(move || {
            started.send(()).unwrap();
            gate.recv().unwrap();
            Err(EngineFailure::Unavailable)
        }));
        assert!(state.submit_key(Key::Letter('a'), None, None));
        initialized.recv_timeout(Duration::from_secs(1)).unwrap();
        while state.send(Command::Remember(false)) {}
        assert_eq!(
            state.outbox.borrow().len(),
            crate::worker::QUEUE_CAPACITY * 2
        );
        state.english_from_system(true);
        assert!(state.input.borrow().english);
        assert_eq!(state.deferred_mode.get(), Some(true));
        assert!(!state.accepts_keys());
        state.english_from_system(false);
        assert_eq!(state.deferred_mode.get(), Some(false));
        assert!(!state.accepts_keys());
        state.english_from_system(true);
        assert_eq!(state.deferred_mode.get(), Some(true));
        state.break_input_context();
        assert!(state.deferred_context.get());
        release.send(()).unwrap();
        drain(&state);
        assert!(state.accepts_keys());
        assert!(state.input.borrow().english);
        fn worker_mode(state: &State, expected: bool) {
            // Inspect the worker's own mode in its next reply; poll deliberately
            // preserves the UI flag, so checking only that cache misses a drop.
            assert!(state.send(Command::Finish { id: u64::MAX }));
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                state.flush();
                assert!(Instant::now() < until);
                let reply = state
                    .worker
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .recv_timeout(Duration::from_millis(1));
                if let Ok(Reply::Update {
                    id: u64::MAX,
                    input,
                    ..
                }) = reply
                {
                    assert_eq!(input.english, expected);
                    break;
                }
            }
        }
        worker_mode(&state, true);
        state.english_from_system(false);
        drain(&state);
        assert!(!state.input.borrow().english);
        worker_mode(&state, false);
        assert!(state.input.borrow().raw.is_empty());
    }

    #[test]
    fn slow_initialization_does_not_block_ui_and_boundaries_survive_full_queue() {
        let (started, initialized) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let state = State::with_display_preferences(false);
        *state.worker.borrow_mut() = Some(Worker::start_with(move || {
            started.send(()).unwrap();
            gate.recv().unwrap();
            Err(EngineFailure::Unavailable)
        }));
        let at = Instant::now();
        assert!(state.submit_key(Key::Letter('a'), None, None));
        assert!(at.elapsed() < Duration::from_millis(50));
        initialized.recv_timeout(Duration::from_secs(1)).unwrap();
        while state.accepts_keys() {
            assert!(state.submit_key(Key::Letter('a'), None, None));
        }
        assert!(!state.submit_key(Key::Letter('x'), None, None));
        assert!(
            state.finish(),
            "control capacity must preserve the old boundary"
        );
        release.send(()).unwrap();
        drain(&state);
        assert!(state.input.borrow().raw.is_empty());
        assert!(!state.composing.get());
        assert!(state.submit_key(Key::Letter('b'), None, None));
        drain(&state);
        assert_eq!(state.input.borrow().raw, "b");
    }

    #[test]
    fn idle_work_is_distinct_from_pending_composition_and_return_is_single_use() {
        let state = State::with_display_preferences(false);
        *state.worker.borrow_mut() = Some(Worker::start_with(|| Err(EngineFailure::Unavailable)));
        state.finish();
        assert!(state.busy.get());
        assert!(!state.composing.get());
        assert!(state.submit_key(Key::Literal('1'), None, None));
        assert!(!state.composing.get());
        drain(&state);
        assert!(state.submit_key(Key::Letter('A'), None, None));
        assert!(state.submit_key(Key::Letter('I'), None, None));
        assert!(state.submit_key(Key::Return, None, None));
        assert!(state.input.borrow().raw.is_empty());
        assert!(
            state.composing.get(),
            "a second Return must wait for the original commit"
        );
        assert!(state.submit_key(Key::Return, None, None));
        drain(&state);
        assert!(state.input.borrow().raw.is_empty());
        assert!(!state.composing.get());
    }

    #[test]
    fn pending_keys_keep_the_displayed_frame_and_collapse_keeps_pinyin() {
        let (release, gate) = mpsc::channel();
        let state = State::with_display_preferences(false);
        *state.worker.borrow_mut() = Some(Worker::start_with(move || {
            gate.recv().unwrap();
            Err(EngineFailure::Unavailable)
        }));
        {
            let mut input = state.input.borrow_mut();
            input.raw = "wode".into();
            input.expanded = true;
            input.frame.generation = 42;
            input.frame.candidates = vec![crate::bridge::Candidate {
                text: "我的".into(),
                ..Default::default()
            }];
        }
        assert!(state.submit_key(Key::Tab(false), None, None));
        assert!(state.busy.get());
        assert_eq!(state.input.borrow().frame.candidates[0].text, "我的");
        assert_eq!(state.input.borrow().frame.generation, 42);
        assert!(state.submit_key(Key::Escape, None, None));
        assert_eq!(state.input.borrow().raw, "wode");
        assert!(!state.input.borrow().expanded);
        assert_eq!(state.input.borrow().frame.candidates.len(), 1);
        assert!(state.submit_key(Key::Letter('a'), None, None));
        assert_eq!(state.input.borrow().raw, "wodea");
        assert_eq!(state.input.borrow().frame.generation, 42);
        state.input.borrow_mut().error = true;
        assert!(state.submit_key(Key::Letter('b'), None, None));
        assert!(!state.input.borrow().error);
        assert!(state.input.borrow().frame.candidates.is_empty());
        release.send(()).unwrap();
        drain(&state);
    }
}
