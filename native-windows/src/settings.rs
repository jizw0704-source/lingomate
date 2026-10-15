//! Enum-only settings IPC and session-scoped shared preferences. Never carries input or credentials.
use crate::preferences::{Display, Theme};

const MAGIC: u32 = 0x4c4d0000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub english: bool,
    pub bilingual: bool,
    pub remember: bool,
    pub punctuation: u32,
    pub show_pos: bool,
    pub display: Display,
    pub pending: bool,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            english: false,
            bilingual: true,
            remember: true,
            punctuation: 0,
            show_pos: true,
            display: Display::default(),
            pending: false,
        }
    }
}
impl Snapshot {
    pub fn encode(self) -> usize {
        MAGIC as usize
            | 1
            | (usize::from(self.english) << 1)
            | (usize::from(self.bilingual) << 2)
            | (usize::from(self.remember) << 3)
            | ((self.punctuation as usize) << 4)
            | (usize::from(self.show_pos) << 6)
            | ((self.display.theme.value() as usize) << 7)
            | (Display::SIZES
                .iter()
                .position(|v| *v == self.display.font)
                .unwrap_or(1)
                << 9)
            | (usize::from(self.pending) << 11)
    }
    pub fn decode(value: usize) -> Option<Self> {
        if value & !0xfff != MAGIC as usize
            || value & 1 == 0
            || (value >> 4) & 3 > 2
            || (value >> 7) & 3 > 2
        {
            return None;
        }
        Some(Self {
            english: value & 2 != 0,
            bilingual: value & 4 != 0,
            remember: value & 8 != 0,
            punctuation: ((value >> 4) & 3) as u32,
            show_pos: value & 64 != 0,
            display: Display {
                theme: Theme::from_value(((value >> 7) & 3) as u32),
                font: Display::SIZES[(value >> 9) & 3],
            },
            pending: value & 2048 != 0,
        })
    }
    fn runtime(self) -> u64 {
        u64::from(self.english)
            | (u64::from(self.bilingual) << 1)
            | (u64::from(self.remember) << 2)
            | ((self.punctuation as u64) << 3)
    }
    fn display_bits(self) -> u64 {
        u64::from(self.show_pos)
            | ((self.display.theme.value() as u64) << 1)
            | ((self.display.font as u64) << 3)
    }
    fn from_parts(runtime: u64, display: u64) -> Option<Self> {
        let font = (display >> 3) as u32;
        if runtime & !31 != 0
            || (runtime >> 3) > 2
            || (display >> 1) & 3 > 2
            || !Display::SIZES.contains(&font)
        {
            return None;
        }
        Some(Self {
            english: runtime & 1 != 0,
            bilingual: runtime & 2 != 0,
            remember: runtime & 4 != 0,
            punctuation: (runtime >> 3) as u32,
            show_pos: display & 1 != 0,
            display: Display {
                theme: Theme::from_value(((display >> 1) & 3) as u32),
                font,
            },
            pending: false,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    English(bool),
    Bilingual(bool),
    Remember(bool),
    Punctuation(u32),
    ShowPOS(bool),
    Font(u32),
    Theme(Theme),
}
impl Setting {
    pub fn packet(self) -> (usize, isize) {
        match self {
            Self::English(v) => (1, v.into()),
            Self::Bilingual(v) => (2, v.into()),
            Self::Remember(v) => (3, v.into()),
            Self::Punctuation(v) => (4, v as isize),
            Self::ShowPOS(v) => (5, v.into()),
            Self::Font(v) => (6, v as isize),
            Self::Theme(v) => (7, v.value() as isize),
        }
    }
    pub fn decode(code: usize, value: isize) -> Option<Self> {
        match (code, value) {
            (1, 0..=1) => Some(Self::English(value != 0)),
            (2, 0..=1) => Some(Self::Bilingual(value != 0)),
            (3, 0..=1) => Some(Self::Remember(value != 0)),
            (4, 0..=2) => Some(Self::Punctuation(value as u32)),
            (5, 0..=1) => Some(Self::ShowPOS(value != 0)),
            (6, 16 | 18 | 20 | 22) => Some(Self::Font(value as u32)),
            (7, 0..=2) => Some(Self::Theme(Theme::from_value(value as u32))),
            _ => None,
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use windows::Win32::Foundation::*;
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::System::Threading::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::{HSTRING, PWSTR, w};
    const CHANNEL_MAGIC: u64 = 0x4c494e474f313701;
    #[repr(C)]
    struct Channel {
        magic: AtomicU64,
        runtime: AtomicU64,
        display: AtomicU64,
        revision: AtomicU64,
        configuration: AtomicU64,
        window: AtomicU64,
        reserved: AtomicU64,
        reserved2: AtomicU64,
    }
    pub struct Shared {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
    }
    // Every shared field is an aligned atomic; the mapping remains owned until the last reference drops.
    unsafe impl Send for Shared {}
    unsafe impl Sync for Shared {}
    impl Drop for Shared {
        fn drop(&mut self) {
            unsafe {
                let _ = UnmapViewOfFile(self.view);
                let _ = CloseHandle(self.handle);
            }
        }
    }
    fn name() -> Option<HSTRING> {
        let mut token = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.ok()?;
        let mut size = 0;
        let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut size) };
        let mut storage = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        let result = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                Some(storage.as_mut_ptr().cast()),
                size,
                &mut size,
            )
        };
        unsafe {
            let _ = CloseHandle(token);
        }
        result.ok()?;
        let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
        let mut value = PWSTR::null();
        unsafe { ConvertSidToStringSidW(user.User.Sid, &mut value) }.ok()?;
        let sid = unsafe { value.to_string() }.ok();
        unsafe {
            let _ = LocalFree(Some(HLOCAL(value.0.cast())));
        }
        let test = std::env::var("LINGOMATE_TEST_SESSION")
            .ok()
            .filter(|v| v.len() <= 64 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
            .map(|v| format!(".Test.{v}"))
            .unwrap_or_default();
        Some(HSTRING::from(format!(
            "Local\\LingoMate.Settings.{}{test}",
            sid?
        )))
    }
    impl Shared {
        pub fn open() -> Option<Self> {
            Self::open_named(&name()?)
        }
        fn open_named(name: &HSTRING) -> Option<Self> {
            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    None,
                    PAGE_READWRITE,
                    0,
                    std::mem::size_of::<Channel>() as u32,
                    name,
                )
            }
            .ok()?;
            let fresh = unsafe { GetLastError() } != ERROR_ALREADY_EXISTS;
            let view = unsafe {
                MapViewOfFile(
                    handle,
                    FILE_MAP_ALL_ACCESS,
                    0,
                    0,
                    std::mem::size_of::<Channel>(),
                )
            };
            if view.Value.is_null() {
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return None;
            }
            let shared = Self { handle, view };
            let data = shared.data();
            if fresh {
                let default = Snapshot::default();
                data.runtime.store(default.runtime(), Ordering::Relaxed);
                data.display
                    .store(default.display_bits(), Ordering::Relaxed);
                data.revision.store(1, Ordering::Relaxed);
                data.magic.store(CHANNEL_MAGIC, Ordering::Release);
            }
            if data.magic.load(Ordering::Acquire) != CHANNEL_MAGIC {
                return None;
            }
            Some(shared)
        }
        fn data(&self) -> &Channel {
            unsafe { &*self.view.Value.cast::<Channel>() }
        }
        pub fn snapshot(&self) -> Option<Snapshot> {
            let d = self.data();
            if d.magic.load(Ordering::Acquire) != CHANNEL_MAGIC {
                return None;
            }
            Snapshot::from_parts(
                d.runtime.load(Ordering::Acquire),
                d.display.load(Ordering::Acquire),
            )
        }
        pub fn revision(&self) -> u64 {
            self.data().revision.load(Ordering::Acquire)
        }
        pub fn set_runtime(&self, snapshot: Snapshot) {
            self.data()
                .runtime
                .store(snapshot.runtime(), Ordering::Release);
            self.data().revision.fetch_add(1, Ordering::AcqRel);
        }
        pub fn set_english(&self, english: bool) {
            if self
                .data()
                .runtime
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    let desired = (current & !1) | u64::from(english);
                    (desired != current).then_some(desired)
                })
                .is_ok()
            {
                self.data().revision.fetch_add(1, Ordering::AcqRel);
            }
        }
        pub fn set_display(&self, snapshot: Snapshot) {
            self.data()
                .display
                .store(snapshot.display_bits(), Ordering::Release);
            self.data().revision.fetch_add(1, Ordering::AcqRel);
        }
        pub fn configuration(&self) -> u64 {
            self.data().configuration.load(Ordering::Acquire)
        }
        pub fn set_configuration(&self, value: u64) {
            self.data().configuration.store(value, Ordering::Release);
        }
        pub fn account(&self) -> u64 {
            self.data().reserved.load(Ordering::Acquire)
        }
        pub fn set_account(&self, value: u64) {
            self.data().reserved.store(value, Ordering::Release);
        }
        pub fn focused(&self, window: HWND) {
            self.data()
                .window
                .store(window.0 as usize as u64, Ordering::Release);
        }
        pub fn service_window(&self) -> HWND {
            HWND(self.data().window.load(Ordering::Acquire) as usize as *mut core::ffi::c_void)
        }
    }
    pub fn message() -> u32 {
        static MESSAGE: OnceLock<u32> = OnceLock::new();
        *MESSAGE
            .get_or_init(|| unsafe { RegisterWindowMessageW(w!("LingoMate.Settings.078A5202.1")) })
    }
    pub fn request(window: HWND, setting: Option<Setting>) -> Option<Snapshot> {
        let mut class = [0u16; 256];
        let count = unsafe { GetClassNameW(window, &mut class) };
        if count <= 0
            || !String::from_utf16_lossy(&class[..count as usize])
                .starts_with("LingoMate.Candidates.")
        {
            return None;
        }
        let (code, value) = setting.map_or((0, 0), Setting::packet);
        let mut result = 0usize;
        if unsafe {
            SendMessageTimeoutW(
                window,
                message(),
                WPARAM(code),
                LPARAM(value),
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                100,
                Some(&mut result),
            )
        }
        .0 == 0
        {
            return None;
        }
        Snapshot::decode(result)
    }
    #[cfg(test)]
    #[test]
    fn shared_snapshot_contains_only_valid_enums_and_session_values() {
        let name = HSTRING::from(format!(
            "Local\\LingoMate.Settings.Test.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = Shared::open_named(&name).unwrap();
        let second = Shared::open_named(&name).unwrap();
        let mut value = first.snapshot().unwrap();
        assert!(value.bilingual && value.remember);
        value.bilingual = false;
        value.english = true;
        first.set_runtime(value);
        assert!(second.snapshot().unwrap().english);
        assert!(!second.snapshot().unwrap().bilingual);
        value.display.font = 22;
        value.display.theme = Theme::Dark;
        second.set_display(value);
        assert_eq!(first.snapshot().unwrap().display, value.display);
    }
}
#[cfg(windows)]
pub use native::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_protocol_rejects_out_of_range_and_carries_no_strings() {
        for setting in [
            Setting::English(true),
            Setting::Bilingual(false),
            Setting::Remember(false),
            Setting::Punctuation(2),
            Setting::ShowPOS(false),
            Setting::Font(22),
            Setting::Theme(Theme::System),
        ] {
            let (code, value) = setting.packet();
            assert_eq!(Setting::decode(code, value), Some(setting));
        }
        for (code, value) in [(1, 2), (4, 3), (6, 19), (7, -1), (999, 1)] {
            assert!(Setting::decode(code, value).is_none());
        }
        assert_eq!(
            Snapshot::decode(Snapshot::default().encode()),
            Some(Snapshot::default())
        );
        assert!(Snapshot::decode(0).is_none());
        assert!(Snapshot::decode(Snapshot::default().encode() | 0x1000).is_none());
    }
}
