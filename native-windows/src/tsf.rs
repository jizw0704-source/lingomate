#![allow(non_snake_case)]
mod document;
mod panel;
mod registration;
mod updates;
use crate::bridge::{Bridge, Confirmation};
use crate::input::{Input, Key};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, AtomicPtr, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
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
        0xDB => Some(Key::Punctuation('[')),
        0xDD => Some(Key::Punctuation(']')),
        0xDE => Some(Key::Punctuation(if shift { '"' } else { '\'' })),
        0xDC => Some(Key::Punctuation('\\')),
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
    engine: RefCell<Option<Bridge>>,
    retry_after: Cell<Option<Instant>>,
    document: RefCell<Option<Rc<document::Document>>>,
    panel: RefCell<Option<panel::Panel>>,
    client: Cell<u32>,
    show_pos: Cell<bool>,
    display: Cell<crate::preferences::Display>,
    memory_warning: Cell<bool>,
}
impl State {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            input: RefCell::new(Input::default()),
            engine: RefCell::new(None),
            retry_after: Cell::new(None),
            document: RefCell::new(None),
            panel: RefCell::new(None),
            client: Cell::new(0),
            show_pos: Cell::new(crate::preferences::load()),
            display: Cell::new(crate::preferences::display()),
            memory_warning: Cell::new(false),
        })
    }
    fn cancel_memory(&self) {
        let context = self.input.borrow_mut().reset_memory_context();
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            engine.cancel_memory(&context);
        }
    }
    fn finish_memory(&self) {
        let context = self.input.borrow_mut().reset_memory_context();
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            engine.retire_memory(&context);
        }
    }
    fn confirm_memory(&self, receipt: &Confirmation, successful: bool, output: Option<&str>) {
        let active = {
            let input = self.input.borrow();
            input.remember
        };
        if !active {
            return;
        }
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            self.memory_warning
                .set(engine.confirm_output(receipt, successful, output).is_err());
        }
    }
    fn key(self: &Rc<Self>, key: Key) {
        let commit = {
            let mut input = self.input.borrow_mut();
            let mut engine = self.engine.borrow_mut();
            if engine.is_none() && self.retry_after.get().is_none_or(|at| Instant::now() >= at) {
                *engine = module_path()
                    .ok()
                    .and_then(|path| path.parent().map(PathBuf::from))
                    .and_then(|root| {
                        let memory = std::env::var_os("LOCALAPPDATA").map(|path| {
                            PathBuf::from(path).join("LingoMate/PersonalVocabulary/words.json")
                        });
                        Bridge::start_with_memory(
                            &root.join("bilingual-ime-bridge.exe"),
                            &root,
                            memory.as_deref(),
                        )
                        .ok()
                    });
            }
            let result = if matches!(key, Key::Return) {
                if let Some(engine) = engine.as_mut() {
                    input.finish_memory(engine);
                }
                Ok(Some(input.raw_commit()))
            } else if let Some(engine) = engine.as_mut() {
                input.handle(key, engine)
            } else {
                match key {
                    Key::Literal(ch) if input.raw.is_empty() => {
                        input.punctuation.observe_literal(&ch.to_string());
                        Ok(Some(ch.to_string()))
                    }
                    Key::Punctuation(ch) if input.raw.is_empty() => {
                        let mode = input.punctuation_mode;
                        Ok(input.punctuation.render(ch, mode))
                    }
                    Key::Letter(ch) if input.raw.is_empty() && input.punctuation.is_latin() => {
                        input.punctuation.observe_literal(&ch.to_string());
                        Ok(Some(ch.to_string()))
                    }
                    _ => {
                        match key {
                            Key::Letter(ch) if input.raw.len() < 240 => input.raw.push(ch),
                            Key::Backspace => {
                                input.raw.pop();
                                input.punctuation.reset_context();
                            }
                            Key::Escape => input.reset(),
                            _ => {}
                        }
                        Err("Engine unavailable".into())
                    }
                }
            };
            match result {
                Ok(text) => text,
                Err(_) => {
                    *engine = None;
                    self.retry_after
                        .set(Some(Instant::now() + Duration::from_secs(2)));
                    input.frame = Default::default();
                    input.error = true;
                    None
                }
            }
        };
        if self.input.borrow().frame.memory_warning.is_some() {
            self.memory_warning.set(true);
        }
        self.update(commit);
    }
    fn update(self: &Rc<Self>, commit: Option<String>) {
        if let Some(doc) = self.document.borrow().clone() {
            let raw = self.input.borrow().raw.clone();
            let study = self.input.borrow_mut().pending_study.take();
            let memory = self.input.borrow_mut().pending_memory.take();
            if doc.enqueue(commit, raw, study, memory).is_err() {
                self.cancel_memory();
                self.input.borrow_mut().reset();
            }
        }
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.update();
        }
    }
    fn finish(self: &Rc<Self>) {
        self.finish_memory();
        self.input.borrow_mut().punctuation.reset_context();
        let terminated = self
            .document
            .borrow()
            .as_ref()
            .is_some_and(|d| d.terminated.get());
        if terminated {
            self.input.borrow_mut().reset();
        }
        let raw = self.input.borrow_mut().raw_commit();
        if !raw.is_empty() {
            self.update(Some(raw));
        }
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.hide();
        }
    }
    fn attach(self: &Rc<Self>, context: &ITfContext) {
        let same = self
            .document
            .borrow()
            .as_ref()
            .is_some_and(|d| d.context.as_raw() == context.as_raw());
        if !same {
            self.finish();
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
#[implement(ITfTextInputProcessor, ITfKeyEventSink, ITfThreadMgrEventSink)]
struct Service {
    manager: RefCell<Option<ITfThreadMgr>>,
    state: Rc<State>,
    shift: Cell<Option<Instant>>,
    advice: Cell<Option<u32>>,
    _guard: Guard,
}
impl ITfTextInputProcessor_Impl for Service_Impl {
    fn Activate(&self, manager: Ref<ITfThreadMgr>, client: u32) -> Result<()> {
        guarded(|| {
            let manager = manager.ok()?.clone();
            self.state.client.set(client);
            let source: ITfSource = manager.cast()?;
            let panel = panel::Panel::new(Rc::downgrade(&self.state))?;
            let keys: ITfKeystrokeMgr = manager.cast()?;
            let sink: ITfKeyEventSink = self.to_interface();
            unsafe {
                keys.AdviseKeyEventSink(client, &sink, true)?;
            }
            let focus: ITfThreadMgrEventSink = self.to_interface();
            match unsafe { source.AdviseSink(&ITfThreadMgrEventSink::IID, &focus) } {
                Ok(cookie) => self.advice.set(Some(cookie)),
                Err(error) => {
                    let _ = unsafe { keys.UnadviseKeyEventSink(client) };
                    return Err(error);
                }
            }
            *self.manager.borrow_mut() = Some(manager);
            *self.state.panel.borrow_mut() = Some(panel);
            let _ = updates::launch(true);
            Ok(())
        })
    }
    fn Deactivate(&self) -> Result<()> {
        guarded(|| {
            self.shift.set(None);
            self.state.finish();
            let manager = self.manager.borrow_mut().take();
            if let Some(manager) = manager {
                if let Some(cookie) = self.advice.take()
                    && let Ok(source) = manager.cast::<ITfSource>()
                {
                    let _ = unsafe { source.UnadviseSink(cookie) };
                }
                if let Ok(keys) = manager.cast::<ITfKeystrokeMgr>() {
                    let _ = unsafe { keys.UnadviseKeyEventSink(self.state.client.get()) };
                }
            }
            self.state.panel.borrow_mut().take();
            self.state.engine.borrow_mut().take();
            Ok(())
        })
    }
}
impl ITfKeyEventSink_Impl for Service_Impl {
    fn OnSetFocus(&self, foreground: BOOL) -> Result<()> {
        guarded(|| {
            if foreground.as_bool() {
                self.state.show_pos.set(crate::preferences::load());
                self.state.display.set(crate::preferences::display());
            }
            self.shift.set(None);
            if !foreground.as_bool() {
                self.state.finish();
            }
            Ok(())
        })
    }
    fn OnTestKeyDown(&self, context: Ref<ITfContext>, key: WPARAM, _flags: LPARAM) -> Result<BOOL> {
        guarded(|| {
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
            if blocked(context) || self.state.input.borrow().english || command_modifier() {
                self.state.cancel_memory();
                self.state.input.borrow_mut().punctuation.reset_context();
                if !blocked(context)
                    && (updates::shortcut(key.0) || panel::settings_shortcut(key.0))
                {
                    return Ok(TRUE);
                }
                return Ok(FALSE);
            }
            let decoded = decode(key.0, !self.state.input.borrow().raw.is_empty());
            if decoded.is_none() {
                if !matches!(key.0, 0x10 | 0xA0 | 0xA1) {
                    self.state.input.borrow_mut().punctuation.reset_context();
                }
                if key.0 == 0x20 || key.0 == 0x0D {
                    self.state.finish_memory();
                } else {
                    self.state.cancel_memory();
                }
            }
            Ok(decoded.is_some().into())
        })
    }
    fn OnKeyDown(&self, context: Ref<ITfContext>, key: WPARAM, _flags: LPARAM) -> Result<BOOL> {
        guarded(|| {
            let context = context.ok()?;
            if !blocked(context) && panel::settings_shortcut(key.0) {
                if let Some(panel) = self.state.panel.borrow().as_ref() {
                    panel.settings();
                }
                return Ok(TRUE);
            }
            if !blocked(context) && updates::shortcut(key.0) {
                let _ = updates::launch(false);
                return Ok(TRUE);
            }
            if blocked(context) || command_modifier() || self.state.input.borrow().english {
                return Ok(FALSE);
            }
            self.state.attach(context);
            let composing = !self.state.input.borrow().raw.is_empty();
            if let Some(key) = decode(key.0, composing) {
                self.state.key(key);
                Ok(TRUE)
            } else {
                self.state.finish();
                Ok(FALSE)
            }
        })
    }
    fn OnTestKeyUp(&self, context: Ref<ITfContext>, key: WPARAM, _flags: LPARAM) -> Result<BOOL> {
        guarded(|| {
            if key.0 == 0x10
                && let Some(at) = self.shift.take()
                && at.elapsed() <= Duration::from_millis(700)
                && !command_modifier()
                && context.ok().is_ok_and(|c| !blocked(c))
            {
                self.state.finish();
                let mut input = self.state.input.borrow_mut();
                input.english = !input.english;
                input.last_english = input.english;
                let language = if input.english {
                    crate::punctuation::Language::English
                } else {
                    crate::punctuation::Language::Chinese
                };
                input.punctuation.confirm(language);
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
            self.state.show_pos.set(crate::preferences::load());
            self.state.display.set(crate::preferences::display());
            self.shift.set(None);
            self.state.finish();
            if let Ok(manager) = manager.ok()
                && let Ok(context) = unsafe { manager.GetTop() }
            {
                self.state.attach(&context);
            }
            Ok(())
        })
    }
    fn OnPushContext(&self, context: Ref<ITfContext>) -> Result<()> {
        guarded(|| {
            self.shift.set(None);
            if let Ok(context) = context.ok() {
                self.state.attach(context);
            }
            Ok(())
        })
    }
    fn OnPopContext(&self, context: Ref<ITfContext>) -> Result<()> {
        guarded(|| {
            self.shift.set(None);
            let own = context.ok().is_ok_and(|c| {
                self.state
                    .document
                    .borrow()
                    .as_ref()
                    .is_some_and(|d| d.context.as_raw() == c.as_raw())
            });
            if own {
                self.state.finish();
                self.state.document.borrow_mut().take();
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
            state: State::new(),
            shift: Cell::new(None),
            advice: Cell::new(None),
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
