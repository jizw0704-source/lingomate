//! Opt-in, finite TSF lifecycle snapshots. Never call this module from DllMain
//! or record keyboard events; there are deliberately no key/input event stages.
//!
//! Only factory/activation entry stages may check the registry opt-in on the
//! calling thread. All registry writes (and subsequent opt-in checks) run on a
//! separate worker, including mode changes caused indirectly by a key callback.
//! A missing snapshot can mean disabled diagnostics, a dropped publication or
//! registry failure; it is not evidence that the service never activated.
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleHandleExW,
};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::core::{HRESULT, HSTRING, PCWSTR, w};

const ROOT: &str = "Software\\LingoMate\\Diagnostics";
const MAX_THREADS: usize = 32;
static CHECKED: AtomicBool = AtomicBool::new(false);
static BUS: OnceLock<Arc<Bus>> = OnceLock::new();

/// Fixed lifecycle markers only. Do not add keyboard/input-content stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage {
    ClassObject,
    FactoryCreate,
    ActivateStart,
    ActivateExStart,
    ActivateWorker,
    ActivatePanel,
    ActivateManager,
    ActivateKeySink,
    ActivateDocumentSink,
    ActivateThreadSink,
    ActivateMode,
    ActivateFocus,
    ActivateReady,
    ModeChange,
    ThreadFocus,
    DeactivateStart,
    DeactivateDone,
}
impl Stage {
    const fn name(self) -> &'static str {
        match self {
            Self::ClassObject => "ClassObject",
            Self::FactoryCreate => "FactoryCreate",
            Self::ActivateStart => "ActivateStart",
            Self::ActivateExStart => "ActivateExStart",
            Self::ActivateWorker => "ActivateWorker",
            Self::ActivatePanel => "ActivatePanel",
            Self::ActivateManager => "ActivateManager",
            Self::ActivateKeySink => "ActivateKeySink",
            Self::ActivateDocumentSink => "ActivateDocumentSink",
            Self::ActivateThreadSink => "ActivateThreadSink",
            Self::ActivateMode => "ActivateMode",
            Self::ActivateFocus => "ActivateFocus",
            Self::ActivateReady => "ActivateReady",
            Self::ModeChange => "ModeChange",
            Self::ThreadFocus => "ThreadFocus",
            Self::DeactivateStart => "DeactivateStart",
            Self::DeactivateDone => "DeactivateDone",
        }
    }
    const fn entry(self) -> bool {
        matches!(
            self,
            Self::ClassObject | Self::FactoryCreate | Self::ActivateStart | Self::ActivateExStart
        )
    }
}

#[derive(Clone, Copy)]
struct Snapshot {
    stage: Stage,
    code: HRESULT,
    activation: Option<u32>,
    thread: Option<u32>,
    english: Option<bool>,
    focus: Option<bool>,
}
impl Snapshot {
    const INITIAL: Self = Self {
        stage: Stage::ClassObject,
        code: HRESULT(0),
        activation: None,
        thread: None,
        english: None,
        focus: None,
    };
    fn with_mode(self, english: bool) -> Self {
        Self {
            english: Some(english),
            ..self
        }
    }
}
thread_local! {
    static SNAPSHOT: Cell<Snapshot> = const { Cell::new(Snapshot::INITIAL) };
}

/// Record a fixed phase/result. Enable before restarting the target application:
/// the per-process opt-in is checked once, at the first factory/activation entry.
pub(super) fn stage(stage: Stage, code: HRESULT) {
    if stage.entry() {
        initialize();
    }
    SNAPSHOT.with(|cell| {
        let mut snapshot = cell.get();
        snapshot.stage = stage;
        snapshot.code = code;
        cell.set(snapshot);
        publish(snapshot);
    });
}

/// Update the result without replacing the failed operation's fixed stage.
pub(super) fn status(code: HRESULT) {
    SNAPSHOT.with(|cell| {
        let mut snapshot = cell.get();
        snapshot.code = code;
        cell.set(snapshot);
        publish(snapshot);
    });
}

/// `None` means GetActiveFlags was unavailable; retain no stale thread flags.
pub(super) fn flags(activation: u32, thread: Option<u32>) {
    SNAPSHOT.with(|cell| {
        let mut snapshot = cell.get();
        snapshot.activation = Some(activation);
        snapshot.thread = thread;
        cell.set(snapshot);
        publish(snapshot);
    });
}

/// Technical mode/focus metadata only; never pass application text or key data.
pub(super) fn state(english: bool, thread_focus: bool) {
    SNAPSHOT.with(|cell| {
        let mut snapshot = cell.get();
        snapshot.english = Some(english);
        snapshot.focus = Some(thread_focus);
        cell.set(snapshot);
        publish(snapshot);
    });
}

/// Update mode without inventing or replacing the service's thread-focus state.
/// This never checks the registry opt-in or initializes the diagnostic worker.
pub(super) fn mode(english: bool) {
    SNAPSHOT.with(|cell| {
        let snapshot = cell.get().with_mode(english);
        cell.set(snapshot);
        publish(snapshot);
    });
}

#[derive(Clone, Copy)]
struct Record {
    process: u32,
    thread: u32,
    snapshot: Snapshot,
}
impl Record {
    const fn valid(self) -> bool {
        self.process != 0 && self.thread != 0
    }
    fn path(self) -> Option<String> {
        self.valid().then(|| {
            format!(
                "{ROOT}\\Processes\\{}\\Threads\\{}",
                self.process, self.thread
            )
        })
    }
}

// Fixed slots coalesce publications by caller thread; there is no event history
// and no allocation in mode/focus publication. Busy/full queues drop metadata.
struct Pending {
    slots: [Option<Record>; MAX_THREADS],
}
impl Pending {
    const fn new() -> Self {
        Self {
            slots: [None; MAX_THREADS],
        }
    }
    fn insert(&mut self, record: Record) -> bool {
        if !record.valid() {
            return false;
        }
        let index = self
            .slots
            .iter()
            .position(|slot| {
                slot.is_some_and(|old| old.process == record.process && old.thread == record.thread)
            })
            .or_else(|| self.slots.iter().position(Option::is_none));
        if let Some(index) = index {
            self.slots[index] = Some(record);
            true
        } else {
            false
        }
    }
    fn take(&mut self) -> [Option<Record>; MAX_THREADS] {
        std::mem::replace(&mut self.slots, [None; MAX_THREADS])
    }
}
struct Bus {
    pending: Mutex<Pending>,
    wake: SyncSender<()>,
}
impl Bus {
    fn queue(&self, record: Record) {
        let queued = self
            .pending
            .try_lock()
            .ok()
            .is_some_and(|mut pending| pending.insert(record));
        if queued {
            let _ = self.wake.try_send(());
        }
    }
}

fn publish(snapshot: Snapshot) {
    let Some(bus) = BUS.get() else {
        return;
    };
    bus.queue(Record {
        process: unsafe { GetCurrentProcessId() },
        thread: unsafe { GetCurrentThreadId() },
        snapshot,
    });
}

fn initialize() {
    // Concurrent entry callbacks never wait for another thread's opt-in check.
    if CHECKED.swap(true, Ordering::Relaxed) || !enabled() {
        return;
    }
    // The worker can outlive a factory object. Keep its code loaded even if
    // activation fails, without changing the COM object/reference count.
    let mut pinned = HMODULE::default();
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            PCWSTR(registry_worker as *const () as *const u16),
            &mut pinned,
        )
    }
    .is_err()
    {
        return;
    }
    let (sender, receiver) = sync_channel(1);
    let bus = Arc::new(Bus {
        pending: Mutex::new(Pending::new()),
        wake: sender,
    });
    let worker_bus = Arc::clone(&bus);
    if std::thread::Builder::new()
        .name("LingoMate diagnostics".into())
        .spawn(move || registry_worker(worker_bus, receiver))
        .is_ok()
    {
        let _ = BUS.set(bus);
    }
}

fn enabled_value(kind: REG_VALUE_TYPE, size: u32, value: u32) -> bool {
    kind == REG_DWORD && size == 4 && value == 1
}
fn enabled() -> bool {
    let mut kind = REG_VALUE_TYPE::default();
    let mut value = 0u32;
    let mut size = std::mem::size_of_val(&value) as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\LingoMate\\Diagnostics"),
            w!("Enabled"),
            RRF_RT_REG_DWORD,
            Some(&mut kind),
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    }
    .is_ok()
        && enabled_value(kind, size, value)
}

fn registry_worker(bus: Arc<Bus>, receiver: Receiver<()>) {
    while receiver.recv().is_ok() {
        let records = match bus.pending.lock() {
            Ok(mut pending) => pending.take(),
            Err(_) => return,
        };
        // No shared queue lock is held while accessing the registry.
        for record in records.into_iter().flatten() {
            if !enabled() {
                continue;
            }
            let Some(path) = record.path() else {
                continue;
            };
            let Some(mut key) = RegistrySink::open(&path) else {
                continue;
            };
            write_fields(true, record, &mut key);
        }
    }
}

#[derive(Clone, Copy)]
enum Value {
    Dword(u32),
    Stage(&'static str),
}
trait SnapshotSink {
    fn set(&mut self, name: &'static str, value: Value);
    fn remove(&mut self, name: &'static str);
}
fn write_fields(enabled: bool, record: Record, sink: &mut impl SnapshotSink) {
    if !enabled || !record.valid() {
        return;
    }
    let snapshot = record.snapshot;
    sink.set("Stage", Value::Stage(snapshot.stage.name()));
    sink.set("HRESULT", Value::Dword(snapshot.code.0 as u32));
    match snapshot.activation {
        Some(flags) => sink.set("ActivationFlags", Value::Dword(flags)),
        None => sink.remove("ActivationFlags"),
    }
    match snapshot.thread {
        Some(flags) => sink.set("ThreadFlags", Value::Dword(flags)),
        None => sink.remove("ThreadFlags"),
    }
    match snapshot.english {
        Some(english) => sink.set("English", Value::Dword(u32::from(english))),
        None => sink.remove("English"),
    }
    match snapshot.focus {
        Some(focus) => sink.set("ThreadFocus", Value::Dword(u32::from(focus))),
        None => sink.remove("ThreadFocus"),
    }
}

struct RegistrySink(HKEY);
impl RegistrySink {
    fn open(path: &str) -> Option<Self> {
        let mut key = HKEY::default();
        let created = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                Some(0),
                None,
                REG_OPTION_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            )
        }
        .is_ok();
        if created { Some(Self(key)) } else { None }
    }
}
impl SnapshotSink for RegistrySink {
    fn set(&mut self, name: &'static str, value: Value) {
        let (kind, bytes) = match value {
            Value::Dword(value) => (REG_DWORD, value.to_le_bytes().to_vec()),
            Value::Stage(stage) => (
                REG_SZ,
                stage
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            ),
        };
        let _ =
            unsafe { RegSetValueExW(self.0, &HSTRING::from(name), Some(0), kind, Some(&bytes)) };
    }
    fn remove(&mut self, name: &'static str) {
        let _ = unsafe { RegDeleteValueW(self.0, &HSTRING::from(name)) };
    }
}
impl Drop for RegistrySink {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MockSink {
        fields: BTreeMap<&'static str, Value>,
        calls: usize,
        fail: bool,
    }
    impl SnapshotSink for MockSink {
        fn set(&mut self, name: &'static str, value: Value) {
            self.calls += 1;
            if !self.fail {
                self.fields.insert(name, value);
            }
        }
        fn remove(&mut self, name: &'static str) {
            self.calls += 1;
            if !self.fail {
                self.fields.remove(name);
            }
        }
    }
    fn record() -> Record {
        Record {
            process: 123,
            thread: 456,
            snapshot: Snapshot {
                stage: Stage::ActivateReady,
                code: HRESULT(-2147467259),
                activation: Some(8),
                thread: Some(2),
                english: Some(false),
                focus: Some(true),
            },
        }
    }
    fn dword(sink: &MockSink, name: &'static str) -> Option<u32> {
        match sink.fields.get(name) {
            Some(Value::Dword(value)) => Some(*value),
            _ => None,
        }
    }

    #[test]
    fn opt_in_requires_exact_enabled_dword() {
        assert!(enabled_value(REG_DWORD, 4, 1));
        for (kind, size, value) in [
            (REG_SZ, 4, 1),
            (REG_QWORD, 8, 1),
            (REG_DWORD, 0, 1),
            (REG_DWORD, 8, 1),
            (REG_DWORD, 4, 0),
            (REG_DWORD, 4, 2),
            (REG_DWORD, 4, u32::MAX),
        ] {
            assert!(!enabled_value(kind, size, value));
        }
    }

    #[test]
    fn disabled_and_invalid_identifiers_never_publish_fields() {
        let mut sink = MockSink::default();
        write_fields(false, record(), &mut sink);
        for invalid in [
            Record {
                process: 0,
                ..record()
            },
            Record {
                thread: 0,
                ..record()
            },
        ] {
            assert!(invalid.path().is_none());
            write_fields(true, invalid, &mut sink);
        }
        assert_eq!(sink.calls, 0);
        assert!(sink.fields.is_empty());
        assert_eq!(
            record().path().as_deref(),
            Some("Software\\LingoMate\\Diagnostics\\Processes\\123\\Threads\\456")
        );
    }

    #[test]
    fn snapshot_contains_only_fixed_metadata_and_clears_unknown_values() {
        let mut sink = MockSink::default();
        write_fields(true, record(), &mut sink);
        assert_eq!(sink.calls, 6);
        assert_eq!(sink.fields.len(), 6);
        assert!(matches!(
            sink.fields.get("Stage"),
            Some(Value::Stage("ActivateReady"))
        ));
        assert_eq!(dword(&sink, "HRESULT"), Some(0x80004005));
        assert_eq!(dword(&sink, "ActivationFlags"), Some(8));
        assert_eq!(dword(&sink, "ThreadFlags"), Some(2));
        assert_eq!(dword(&sink, "English"), Some(0));
        assert_eq!(dword(&sink, "ThreadFocus"), Some(1));
        let unknown = Record {
            snapshot: Snapshot {
                stage: Stage::ActivateMode,
                ..Snapshot::INITIAL
            },
            ..record()
        };
        write_fields(true, unknown, &mut sink);
        assert_eq!(sink.fields.len(), 2);
        assert!(sink.fields.contains_key("Stage"));
        assert!(sink.fields.contains_key("HRESULT"));
    }

    #[test]
    fn fixed_pending_slots_coalesce_and_drop_excess_threads() {
        let mut pending = Pending::new();
        assert!(!pending.insert(Record {
            thread: 0,
            ..record()
        }));
        for thread in 1..=MAX_THREADS as u32 {
            assert!(pending.insert(Record { thread, ..record() }));
        }
        assert!(!pending.insert(Record {
            thread: MAX_THREADS as u32 + 1,
            ..record()
        }));
        let latest = Record {
            thread: 1,
            snapshot: Snapshot {
                stage: Stage::DeactivateDone,
                ..record().snapshot
            },
            ..record()
        };
        assert!(pending.insert(latest));
        let records = pending.take();
        assert_eq!(records.into_iter().flatten().count(), MAX_THREADS);
        assert_eq!(records[0].unwrap().snapshot.stage, Stage::DeactivateDone);
        assert!(pending.take().into_iter().all(|slot| slot.is_none()));
    }

    #[test]
    fn mode_only_changes_english_and_preserves_known_or_unknown_focus() {
        let mut sink = MockSink::default();
        let before = record();
        let after = Record {
            snapshot: before.snapshot.with_mode(true),
            ..before
        };
        write_fields(true, after, &mut sink);
        assert_eq!(dword(&sink, "English"), Some(1));
        assert_eq!(dword(&sink, "ThreadFocus"), Some(1));
        assert_eq!(after.snapshot.stage, before.snapshot.stage);
        assert_eq!(after.snapshot.code, before.snapshot.code);
        assert_eq!(after.snapshot.activation, before.snapshot.activation);
        assert_eq!(after.snapshot.thread, before.snapshot.thread);
        let unknown_focus = Record {
            snapshot: Snapshot::INITIAL.with_mode(false),
            ..before
        };
        write_fields(true, unknown_focus, &mut sink);
        assert_eq!(dword(&sink, "English"), Some(0));
        assert_eq!(dword(&sink, "ThreadFocus"), None);
    }

    #[test]
    fn busy_queue_and_write_failure_do_not_wait_or_fail_the_caller() {
        let (sender, receiver) = sync_channel(1);
        let bus = Bus {
            pending: Mutex::new(Pending::new()),
            wake: sender,
        };
        let locked = bus.pending.lock().unwrap();
        bus.queue(record());
        assert!(receiver.try_recv().is_err());
        drop(locked);
        bus.queue(record());
        bus.queue(record());
        assert!(receiver.try_recv().is_ok());
        assert!(receiver.try_recv().is_err());
        let mut failed = MockSink {
            fail: true,
            ..MockSink::default()
        };
        write_fields(true, record(), &mut failed);
        assert_eq!(failed.calls, 6);
        assert!(failed.fields.is_empty());
    }
}
