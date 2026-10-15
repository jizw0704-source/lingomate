use super::{account, translation};
use crate::appearance::{self, Palette, px};
use crate::credentials::SystemVault;
use crate::http::{NativeHTTP, Transport};
use crate::learning::Word;
use crate::preferences::{Display, Theme};
use crate::remote::{AIConfiguration, RemoteError};
use crate::settings::{Setting, Snapshot};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, IsWindowEnabled, SetFocus,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, w};

#[allow(non_snake_case)]
unsafe fn SetInputScope(
    window: HWND,
    scope: windows::Win32::UI::TextServices::InputScope,
) -> windows::core::HRESULT {
    type SetScope = unsafe extern "system" fn(
        HWND,
        windows::Win32::UI::TextServices::InputScope,
    ) -> windows::core::HRESULT;
    static FUNCTION: std::sync::OnceLock<Option<SetScope>> = std::sync::OnceLock::new();
    let function = FUNCTION.get_or_init(|| {
        use windows::Win32::System::LibraryLoader::*;
        // System DLL only; keep its reference until this standalone helper exits.
        let module =
            unsafe { LoadLibraryExW(w!("msctf.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.ok()?;
        let function = unsafe { GetProcAddress(module, windows::core::s!("SetInputScope")) }?;
        Some(unsafe {
            std::mem::transmute::<unsafe extern "system" fn() -> isize, SetScope>(function)
        })
    });
    function.map_or(windows::Win32::Foundation::E_NOTIMPL, |function| unsafe {
        function(window, scope)
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Account,
    Translation,
    Input,
    Appearance,
    Learning,
    Local,
}
impl Section {
    fn packet(self) -> usize {
        if self == Self::Local { 5 } else { self.index() }
    }
    fn from_packet(value: usize) -> Option<Self> {
        if value == 5 {
            Some(Self::Local)
        } else {
            SECTIONS.get(value).copied()
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Account => "账号",
            Self::Translation => "翻译",
            Self::Input => "输入",
            Self::Appearance => "外观",
            Self::Learning => "学习",
            Self::Local => "准备本地整句翻译",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Account => 0,
            Self::Translation | Self::Local => 1,
            Self::Input => 2,
            Self::Appearance => 3,
            Self::Learning => 4,
        }
    }
}
const SECTIONS: [Section; 5] = [
    Section::Account,
    Section::Translation,
    Section::Input,
    Section::Appearance,
    Section::Learning,
];
const ACCOUNT_URL: u16 = 200;
const EMAIL: u16 = 201;
const CODE: u16 = 202;
const ENDPOINT: u16 = 210;
const MODEL: u16 = 211;
const SECRET: u16 = 212;
const LOCAL_URL: u16 = 213;
const LOCAL_MODEL: u16 = 214;
const SEARCH: u16 = 220;
const FILTER: u16 = 221;
const LIST: u16 = 222;
const SAVE_URL: u16 = 300;
const SEND_CODE: u16 = 301;
const VERIFY: u16 = 302;
const LOGOUT: u16 = 303;
const SYNC: u16 = 304;
const DOMESTIC: u16 = 310;
const INTERNATIONAL: u16 = 311;
const SAVE_AI: u16 = 312;
const DISABLE_AI: u16 = 313;
const PREPARE_LOCAL: u16 = 314;
const SAVE_LOCAL: u16 = 315;
const TEST_AI: u16 = 316;
const KEY_STATUS: u16 = 504;
const MASTER: u16 = 320;
const DELETE: u16 = 321;
const REFRESH: u16 = 322;
const EXPORT: u16 = 323;
const MORE: u16 = 324;
const UPDATE: u16 = 340;
const REOPEN: u32 = WM_APP + 37;
const REOPENED: isize = 0x4c475549;

struct Apartment;
impl Apartment {
    fn open() -> Result<Self, RemoteError> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .map_err(|_| RemoteError::Unavailable)?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct Instance {
    handle: HANDLE,
    owner: bool,
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            if self.owner {
                let _ = windows::Win32::System::Threading::ReleaseMutex(self.handle);
            }
            let _ = CloseHandle(self.handle);
        }
    }
}
fn user_identity() -> Result<String, RemoteError> {
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::*;
    use windows::Win32::System::Threading::*;
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(|_| RemoteError::Unavailable)?;
    let mut bytes = 0;
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut bytes) };
    let mut storage = vec![0usize; (bytes as usize).div_ceil(std::mem::size_of::<usize>())];
    let result = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            bytes,
            &mut bytes,
        )
    };
    unsafe {
        let _ = CloseHandle(token);
    }
    result.map_err(|_| RemoteError::Unavailable)?;
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut sid = windows::core::PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) }
        .map_err(|_| RemoteError::Unavailable)?;
    let identity = unsafe { sid.to_string() };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(sid.0.cast())));
    }
    identity.map_err(|_| RemoteError::Unavailable)
}
fn instance(
    class: &HSTRING,
    section: Section,
    service: HWND,
) -> Result<(Instance, bool), RemoteError> {
    use windows::Win32::System::Threading::*;
    // Local objects are session-scoped; the class includes the current user's SID and version.
    let handle = unsafe { CreateMutexW(None, true, &HSTRING::from(format!("Local\\{class}"))) }
        .map_err(|_| RemoteError::Unavailable)?;
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let mut guard = Instance {
        handle,
        owner: !exists,
    };
    if !exists {
        return Ok((guard, false));
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Ok(window) = unsafe { FindWindowW(class, None) } {
            let mut pid = 0;
            unsafe {
                GetWindowThreadProcessId(window, Some(&mut pid));
                let _ = AllowSetForegroundWindow(pid);
            }
            let mut reply = 0usize;
            unsafe {
                SendMessageTimeoutW(
                    window,
                    REOPEN,
                    WPARAM(section.packet()),
                    LPARAM(service.0 as isize),
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    500,
                    Some(&mut reply),
                );
            }
            if reply == REOPENED as usize {
                return Ok((guard, true));
            }
        }
        let waited = unsafe { WaitForSingleObject(handle, 0) };
        if waited == WAIT_OBJECT_0 || waited == WAIT_ABANDONED {
            guard.owner = true;
            return Ok((guard, false));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(RemoteError::Unavailable)
}
#[derive(Clone, Copy)]
enum Role {
    Text,
    Secondary,
    Button { selected: bool, outlined: bool },
    Edit,
    List,
}
struct Font(HFONT, bool);
impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            if self.1 {
                let _ = DeleteObject(self.0.into());
            }
        }
    }
}
fn font(size: i32, dpi: i32) -> Font {
    // Use the system message font and its normal Chinese font linking. If system
    // metrics cannot be read, GDI maps YaHei UI to an available native fallback.
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    let handle = if unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok()
    {
        metrics.lfMessageFont.lfHeight = -px(size, dpi);
        metrics.lfMessageFont.lfQuality = CLEARTYPE_QUALITY;
        unsafe { CreateFontIndirectW(&metrics.lfMessageFont) }
    } else {
        unsafe {
            CreateFontW(
                -px(size, dpi),
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32,
                w!("Microsoft YaHei UI"),
            )
        }
    };
    if handle.is_invalid() {
        Font(HFONT(unsafe { GetStockObject(DEFAULT_GUI_FONT) }.0), false)
    } else {
        Font(handle, true)
    }
}
#[derive(Clone, Copy)]
struct Focus {
    id: u16,
    selection: Option<(u32, u32)>,
}
#[derive(Default)]
struct ListState {
    filter: isize,
    selected: Option<String>,
    top: isize,
}
enum Reply {
    Done {
        message: String,
        clear_secret: bool,
        clear_code: bool,
    },
    Tested {
        generation: u64,
        result: Result<(), RemoteError>,
        elapsed: Duration,
    },
    Words(Result<Vec<Word>, RemoteError>),
    Account(Result<account::Data, RemoteError>),
}
type Placement = (HWND, i32, i32, i32, i32);
struct Window {
    hwnd: Cell<HWND>,
    body: Cell<HWND>,
    preview: bool,
    fixture_dpi: Option<i32>,
    root: Option<PathBuf>,
    service: Cell<HWND>,
    shared: Option<crate::settings::Shared>,
    section: Cell<Section>,
    snapshot: Cell<Option<Snapshot>>,
    theme: Cell<Theme>,
    display: Cell<Display>,
    show_pos: Cell<bool>,
    palette: Cell<Palette>,
    brush: Cell<HBRUSH>,
    fonts: RefCell<Vec<Font>>,
    tabs: RefCell<Vec<HWND>>,
    controls: RefCell<BTreeMap<u16, HWND>>,
    roles: RefCell<BTreeMap<u16, Role>>,
    placements: RefCell<Vec<Placement>>,
    drafts: RefCell<BTreeMap<u16, String>>,
    words: RefCell<Vec<Word>>,
    visible: RefCell<Vec<Word>>,
    cloud: RefCell<Option<account::Data>>,
    feedback: RefCell<String>,
    connection_feedback: Cell<bool>,
    translation_generation: Cell<u64>,
    fixture_saved_key: bool,
    busy: Cell<bool>,
    cooldown: Cell<Option<Instant>>,
    scroll: Cell<i32>,
    height: Cell<i32>,
    width: Cell<i32>,
    limit: Cell<usize>,
    rebuilding: Cell<bool>,
    focus: Cell<Option<Focus>>,
    list_state: RefCell<ListState>,
    account_seen: Cell<u64>,
    rx: Receiver<Reply>,
    tx: Sender<Reply>,
    deadline: Option<Instant>,
}
impl Drop for Window {
    fn drop(&mut self) {
        self.clear_input_scopes();
        unsafe {
            SetWindowLongPtrW(self.hwnd.get(), GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.hwnd.get());
            let _ = DeleteObject(self.brush.get().into());
        }
    }
}
impl Window {
    fn effective_snapshot(&self) -> Snapshot {
        effective_snapshot(self.snapshot.get(), self.display.get(), self.show_pos.get())
    }
    fn clear_input_scopes(&self) {
        let controls: Vec<_> = self
            .controls
            .borrow()
            .values()
            .copied()
            .chain(self.tabs.borrow().iter().copied())
            .chain([self.body.get()])
            .collect();
        for control in controls {
            if unsafe { IsWindow(Some(control)) }.as_bool() {
                let _ =
                    unsafe { SetInputScope(control, windows::Win32::UI::TextServices::IS_DEFAULT) };
            }
        }
    }
    fn close(&self) {
        self.clear_input_scopes();
        unsafe {
            let _ = DestroyWindow(self.hwnd.get());
        }
    }
    fn reopen(&self, section: Section, service: HWND) {
        self.capture();
        self.translation_edited();
        self.service.set(service);
        self.snapshot.set(crate::settings::request(service, None));
        let display = crate::preferences::display();
        self.display.set(display);
        self.show_pos.set(crate::preferences::load());
        self.theme.set(display.theme);
        self.update_palette();
        self.section.set(section);
        self.feedback.borrow_mut().clear();
        self.limit.set(50);
        self.render(false);
        unsafe {
            let _ = ShowWindow(
                self.hwnd.get(),
                if IsIconic(self.hwnd.get()).as_bool() {
                    SW_RESTORE
                } else {
                    SW_SHOW
                },
            );
            let _ = SetForegroundWindow(self.hwnd.get());
        }
        if let Some(tab) = self.tabs.borrow().get(section.index()).copied() {
            let _ = unsafe { SetFocus(Some(tab)) };
        }
    }
    fn remember_focus(&self) -> Option<Focus> {
        let focused = unsafe { GetFocus() };
        if let Some((&id, _)) = self
            .controls
            .borrow()
            .iter()
            .find(|(_, window)| **window == focused)
        {
            let selection = if matches!(
                id,
                ACCOUNT_URL
                    | EMAIL
                    | CODE
                    | ENDPOINT
                    | MODEL
                    | SECRET
                    | LOCAL_URL
                    | LOCAL_MODEL
                    | SEARCH
            ) {
                let (mut start, mut end) = (0u32, 0u32);
                unsafe {
                    SendMessageW(
                        focused,
                        EM_GETSEL,
                        Some(WPARAM((&mut start as *mut u32) as usize)),
                        Some(LPARAM((&mut end as *mut u32) as isize)),
                    );
                }
                Some((start, end))
            } else {
                None
            };
            let value = Focus { id, selection };
            self.focus.set(Some(value));
            Some(value)
        } else if unsafe { GetForegroundWindow() } == self.hwnd.get()
            && (focused.is_invalid() || focused == self.hwnd.get() || focused == self.body.get())
        {
            self.focus.get()
        } else {
            None
        }
    }
    fn restore_focus(&self, focus: Option<Focus>) {
        if unsafe { GetForegroundWindow() } != self.hwnd.get() {
            return;
        }
        let Some(focus) = focus else {
            return;
        };
        let Some(control) = self.controls.borrow().get(&focus.id).copied() else {
            return;
        };
        if !unsafe { IsWindowEnabled(control) }.as_bool() {
            return;
        }
        if let Some((start, end)) = focus.selection {
            unsafe {
                SendMessageW(
                    control,
                    EM_SETSEL,
                    Some(WPARAM(start as usize)),
                    Some(LPARAM(end as isize)),
                );
            }
        }
        let _ = unsafe { SetFocus(Some(control)) };
    }
    fn ensure_focus_visible(&self) {
        let focused = unsafe { GetFocus() };
        let placement = self
            .placements
            .borrow()
            .iter()
            .copied()
            .find(|(control, ..)| *control == focused);
        let Some((_, _, y, _, height)) = placement else {
            return;
        };
        let mut rect = RECT::default();
        if unsafe { GetClientRect(self.body.get(), &mut rect) }.is_err() {
            return;
        }
        let viewport = (rect.bottom - rect.top) * 96 / self.dpi();
        let height = if unsafe { GetDlgCtrlID(focused) } == i32::from(FILTER) {
            44
        } else {
            height
        };
        let scroll = if y < self.scroll.get() || height > viewport {
            y
        } else if y + height > self.scroll.get() + viewport {
            y + height - viewport
        } else {
            self.scroll.get()
        }
        .clamp(0, self.scroll_max());
        if scroll != self.scroll.replace(scroll) {
            self.position_controls();
        }
    }
    fn dpi(&self) -> i32 {
        self.fixture_dpi
            .unwrap_or_else(|| appearance::dpi(self.hwnd.get()))
    }
    fn font(&self, size: usize) -> HFONT {
        self.fonts.borrow()[size].0
    }
    fn capture(&self) {
        let controls = self.controls.borrow();
        let mut draft = self.drafts.borrow_mut();
        for id in [
            ACCOUNT_URL,
            EMAIL,
            CODE,
            ENDPOINT,
            MODEL,
            SECRET,
            LOCAL_URL,
            LOCAL_MODEL,
            SEARCH,
        ] {
            if let Some(window) = controls.get(&id) {
                draft.insert(id, text(*window));
            }
        }
    }
    fn draft(&self, id: u16) -> String {
        self.drafts.borrow().get(&id).cloned().unwrap_or_default()
    }
    fn palette(&self) -> Palette {
        self.palette.get()
    }
    fn update_palette(&self) {
        let palette = appearance::palette(self.theme.get());
        self.palette.set(palette);
        let brush = unsafe { CreateSolidBrush(palette.background) };
        let old = self.brush.replace(brush);
        unsafe {
            let _ = DeleteObject(old.into());
        }
        appearance::title(self.hwnd.get(), palette);
    }
    #[allow(clippy::too_many_arguments)]
    fn child(
        &self,
        class: PCWSTR,
        title: &str,
        style: WINDOW_STYLE,
        id: u16,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        role: Role,
        font: usize,
    ) -> HWND {
        let dpi = self.dpi();
        let value = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                &HSTRING::from(title),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | style,
                px(x, dpi),
                px(y - self.scroll.get(), dpi),
                px(width, dpi),
                px(height, dpi),
                Some(self.body.get()),
                Some(HMENU(id as usize as *mut _)),
                GetModuleHandleW(None).ok().map(|m| HINSTANCE(m.0)),
                None,
            )
        }
        .unwrap_or_default();
        unsafe {
            SendMessageW(
                value,
                WM_SETFONT,
                Some(WPARAM(self.font(font).0 as usize)),
                Some(LPARAM(1)),
            );
        }
        self.controls.borrow_mut().insert(id, value);
        self.roles.borrow_mut().insert(id, role);
        self.placements
            .borrow_mut()
            .push((value, x, y, width, height));
        if self.busy.get() {
            unsafe {
                let _ = EnableWindow(value, false);
            }
        }
        value
    }
    fn label(&self, id: u16, title: &str, y: i32, height: i32, secondary: bool, font: usize) {
        self.child(
            w!("STATIC"),
            title,
            WINDOW_STYLE(0),
            id,
            0,
            y,
            self.width.get(),
            height,
            if secondary {
                Role::Secondary
            } else {
                Role::Text
            },
            font,
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn button(
        &self,
        id: u16,
        title: &str,
        x: i32,
        y: i32,
        width: i32,
        selected: bool,
        outlined: bool,
        enabled: bool,
    ) {
        let child = self.child(
            w!("BUTTON"),
            title,
            WINDOW_STYLE(BS_OWNERDRAW as u32) | WS_TABSTOP,
            id,
            x,
            y,
            width,
            44,
            Role::Button { selected, outlined },
            0,
        );
        unsafe {
            let _ = EnableWindow(child, enabled && !self.busy.get());
        }
    }
    fn field(&self, id: u16, label: &str, placeholder: &str, y: i32, password: bool, limit: usize) {
        self.label(id + 1000, label, y, 20, true, 2);
        let child = self.child(
            w!("EDIT"),
            &self.draft(id),
            WINDOW_STYLE(ES_AUTOHSCROLL as u32 | if password { ES_PASSWORD as u32 } else { 0 })
                | WS_TABSTOP
                | WS_BORDER,
            id,
            0,
            y + 24,
            self.width.get(),
            44,
            Role::Edit,
            0,
        );
        unsafe {
            SendMessageW(child, EM_SETLIMITTEXT, Some(WPARAM(limit)), None);
            let hint = HSTRING::from(placeholder);
            SendMessageW(
                child,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(hint.as_ptr() as isize)),
            );
        }
        let scope = match id {
            SECRET => windows::Win32::UI::TextServices::IS_PASSWORD,
            ENDPOINT | LOCAL_URL | ACCOUNT_URL => windows::Win32::UI::TextServices::IS_URL,
            EMAIL => windows::Win32::UI::TextServices::IS_EMAIL_SMTPEMAILADDRESS,
            CODE => windows::Win32::UI::TextServices::IS_DIGITS,
            _ => windows::Win32::UI::TextServices::IS_DEFAULT,
        };
        let _ = unsafe { SetInputScope(child, scope) };
        unsafe {
            let _ = EnableWindow(child, !self.busy.get());
        }
    }
    fn choices(
        &self,
        label_id: u16,
        title: &str,
        y: i32,
        items: &[(u16, &str, bool)],
        enabled: bool,
    ) -> i32 {
        self.label(label_id, title, y, 22, false, 0);
        let count = items.len() as i32;
        let width = (self.width.get() - 8 * (count - 1)) / count;
        for (i, (id, text, selected)) in items.iter().enumerate() {
            self.button(
                *id,
                text,
                i as i32 * (width + 8),
                y + 28,
                width,
                *selected,
                false,
                enabled,
            );
        }
        y + 84
    }
    fn set_height(&self, height: i32) {
        self.height.set(height);
        self.scroll
            .set(self.scroll.get().clamp(0, self.scroll_max()));
        self.position_controls();
    }
    fn scroll_max(&self) -> i32 {
        let mut rect = RECT::default();
        let _ = unsafe { GetClientRect(self.body.get(), &mut rect) };
        self.height
            .get()
            .saturating_sub((rect.bottom - rect.top) * 96 / self.dpi())
            .max(0)
    }
    fn position_controls(&self) {
        let dpi = self.dpi();
        position_children(
            self.body.get(),
            &self.placements.borrow(),
            self.scroll.get(),
            dpi,
        );
        let info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: (self.height.get() - 1).max(0),
            nPage: (self.height.get() - self.scroll_max()).max(1) as u32,
            nPos: self.scroll.get(),
            ..Default::default()
        };
        unsafe {
            SetScrollInfo(self.hwnd.get(), SB_VERT, &info, true);
        }
    }
    fn render(&self, preserve: bool) {
        if self.rebuilding.replace(true) {
            return;
        }
        unsafe {
            SendMessageW(self.body.get(), WM_SETREDRAW, Some(WPARAM(0)), None);
        }
        let focus = if preserve {
            self.remember_focus()
        } else {
            self.focus.set(None);
            None
        };
        if preserve {
            let controls = self.controls.borrow();
            if let (Some(filter), Some(list)) = (controls.get(&FILTER), controls.get(&LIST)) {
                *self.list_state.borrow_mut() = ListState {
                    filter: unsafe { SendMessageW(*filter, CB_GETCURSEL, None, None) }.0,
                    selected: self.selected_word().map(|word| word.id()),
                    top: unsafe { SendMessageW(*list, LB_GETTOPINDEX, None, None) }.0,
                };
            }
        } else {
            *self.list_state.borrow_mut() = ListState::default();
        }
        let windows: Vec<_> = self.controls.borrow_mut().values().copied().collect();
        self.controls.borrow_mut().clear();
        self.roles.borrow_mut().clear();
        self.placements.borrow_mut().clear();
        for window in windows {
            unsafe {
                let _ = SetInputScope(window, windows::Win32::UI::TextServices::IS_DEFAULT);
                let _ = DestroyWindow(window);
            }
        }
        if !preserve {
            self.scroll.set(0);
        }
        for (index, tab) in self.tabs.borrow().iter().enumerate() {
            unsafe {
                let _ = InvalidateRect(Some(*tab), None, false);
            }
            let _ = index;
        }
        match self.section.get() {
            Section::Account => self.account_page(),
            Section::Translation => self.translation_page(),
            Section::Input => self.input_page(),
            Section::Appearance => self.appearance_page(),
            Section::Learning => self.learning_page(),
            Section::Local => self.local_page(),
        }
        self.rebuilding.set(false);
        unsafe {
            SendMessageW(self.body.get(), WM_SETREDRAW, Some(WPARAM(1)), None);
        }
        self.restore_focus(focus);
        unsafe {
            let _ = RedrawWindow(
                Some(self.hwnd.get()),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
        }
    }
    fn feedback(&self, id: u16, y: i32) {
        let value = self.feedback.borrow().clone();
        if !value.is_empty() {
            self.label(id, &value, y, 64, true, 0);
        }
    }
    fn translation_page(&self) {
        self.label(500, "翻译", 0, 30, false, 1);
        let status = if self.preview {
            "界面样例：设置只保存在临时内存，不联网。".into()
        } else {
            self.root
                .as_ref()
                .and_then(|root| translation::Store::new(root.clone()).load().ok())
                .and_then(|v| v.ai)
                .map_or(
                    "默认使用本机翻译，未开启在线请求。".into(),
                    |c| {
                        if c.enabled {
                            format!("AI 已启用 · {}", c.model)
                        } else {
                            "当前使用本机翻译，不发送在线请求。".into()
                        }
                    },
                )
        };
        self.label(501, &status, 40, 44, true, 0);
        let width = (self.width.get() - 8) / 2;
        self.button(DOMESTIC, "MiniMax 国内", 0, 92, width, false, false, true);
        self.button(
            INTERNATIONAL,
            "MiniMax 国际",
            width + 8,
            92,
            width,
            false,
            false,
            true,
        );
        self.field(
            ENDPOINT,
            "接口地址",
            "https://服务域名/v1/chat/completions",
            150,
            false,
            1024,
        );
        self.field(MODEL, "模型名", "填写服务商提供的模型名", 232, false, 128);
        self.field(
            SECRET,
            "API 密钥",
            "输入新密钥可替换；留空保留已保存的密钥",
            314,
            true,
            512,
        );
        self.label(KEY_STATUS, &self.key_status(), 390, 36, true, 2);
        let feedback = self.feedback.borrow();
        self.label(
            503,
            if feedback.is_empty() {
                "保存只保存设置；点击“测试连接”发送固定示例句，验证密钥及模型，会使用少量额度。"
            } else {
                &feedback
            },
            434,
            64,
            true,
            0,
        );
        self.button(SAVE_AI, "保存并启用 AI", 0, 510, width, false, true, true);
        self.button(
            TEST_AI,
            "测试连接",
            width + 8,
            510,
            width,
            false,
            false,
            true,
        );
        self.button(
            DISABLE_AI,
            "使用本机翻译",
            0,
            568,
            width,
            false,
            false,
            true,
        );
        self.button(
            PREPARE_LOCAL,
            "本机翻译设置",
            width + 8,
            568,
            width,
            false,
            false,
            true,
        );
        self.label(505, "测试仅向当前接口发送固定示例“今天下午开会。”，会使用少量额度；测试不会保存设置或启用 AI。", 622, 48, true, 2);
        self.label(502,"启用后，仅当前较长中文候选在停顿后发送到上述接口。拼音、输入历史和个人词库不上传；词语释义仍在本机。",680,64,true,2);
        self.set_height(758);
    }
    fn key_status(&self) -> String {
        let saved = if self.preview {
            Ok(self.fixture_saved_key)
        } else if crate::remote::Endpoint::parse(self.draft(ENDPOINT).trim(), false).is_err() {
            Ok(false)
        } else {
            translation::Store::new(self.root.as_ref().unwrap().clone())
                .has_saved_key(self.draft(ENDPOINT).trim(), &SystemVault)
        };
        key_status_text(saved, !self.draft(SECRET).is_empty()).into()
    }
    fn translation_edited(&self) {
        self.translation_generation
            .set(self.translation_generation.get().wrapping_add(1));
        if self.connection_feedback.replace(false) {
            self.feedback.borrow_mut().clear();
            if let Some(window) = self.controls.borrow().get(&503) {
                let _ = unsafe { SetWindowTextW(*window, w!("")) };
            }
        }
        if let Some(window) = self.controls.borrow().get(&KEY_STATUS) {
            let _ = unsafe { SetWindowTextW(*window, &HSTRING::from(self.key_status())) };
        }
    }
    fn local_page(&self) {
        self.label(500, "本机整句翻译设置", 0, 30, false, 1);
        self.label(501,"Windows 使用 Ollama 已下载的本地模型。请先准备中英文模型，并关闭 Ollama 云功能后启动服务。",42,76,true,0);
        self.field(
            LOCAL_URL,
            "本机接口地址",
            "http://127.0.0.1:11434/v1/chat/completions",
            132,
            false,
            1024,
        );
        self.field(
            LOCAL_MODEL,
            "中英文模型名",
            "填写已在本机准备好的模型名",
            214,
            false,
            128,
        );
        self.label(502,"只接受回环地址，并验证云功能已关闭、模型已下载。验证使用固定测试句；在线 AI 在翻译页单独关闭。",298,80,true,2);
        self.button(
            SAVE_LOCAL,
            "验证并准备中英文翻译",
            0,
            392,
            self.width.get(),
            false,
            true,
            !self.preview,
        );
        self.feedback(503, 450);
        self.set_height(530);
    }
    fn input_page(&self) {
        self.label(500, "输入", 0, 30, false, 1);
        let snapshot = self.snapshot.get();
        let ready = snapshot.is_some_and(|s| !s.pending);
        self.label(
            501,
            if self.preview {
                "界面样例：只改变此预览。"
            } else if ready {
                "输入设置已从当前服务回读。"
            } else if snapshot.is_some() {
                "正在应用输入设置…"
            } else {
                "输入服务未响应。请从正在使用灵果的应用打开设置。"
            },
            40,
            56,
            true,
            0,
        );
        let value = self.effective_snapshot();
        let mut y = 110;
        y = self.choices(
            510,
            "输入语言 · 单按 Shift 也能切换",
            y,
            &[
                (400, "中文拼音", !value.english),
                (401, "英文直输", value.english),
            ],
            ready || self.preview,
        );
        y = self.choices(
            511,
            "中文候选 · F6 切换",
            y,
            &[
                (402, "同时显示英文译词", value.bilingual),
                (403, "只显示中文", !value.bilingual),
            ],
            ready || self.preview,
        );
        y = self.choices(
            512,
            "英文词性 · 保存此选择",
            y,
            &[
                (404, "显示词性", value.show_pos),
                (405, "隐藏词性", !value.show_pos),
            ],
            true,
        );
        y = self.choices(
            513,
            "标点 · Ctrl＋Shift＋P 切换",
            y,
            &[
                (406, "自动", value.punctuation == 0),
                (407, "中文", value.punctuation == 1),
                (408, "英文", value.punctuation == 2),
            ],
            ready || self.preview,
        );
        y = self.choices(
            514,
            "选词记忆",
            y,
            &[
                (409, "开启记忆", value.remember),
                (410, "暂停新增记忆", !value.remember),
            ],
            ready || self.preview,
        );
        self.label(515, "暂停只停止新增，已有词继续参与排序。", y, 44, true, 2);
        self.feedback(516, y + 50);
        self.set_height(y + 130);
    }
    fn appearance_page(&self) {
        self.label(500, "外观", 0, 30, false, 1);
        self.label(501, "选择输入法和设置窗口的外观。", 42, 44, true, 0);
        let mut y = self.choices(
            510,
            "外观",
            98,
            &[
                (420, "白色", self.theme.get() == Theme::Light),
                (421, "黑色", self.theme.get() == Theme::Dark),
                (422, "跟随系统", self.theme.get() == Theme::System),
            ],
            true,
        );
        let size = self.display.get().font;
        y = self.choices(
            511,
            "候选栏字体",
            y,
            &[
                (423, "小", size == 16),
                (424, "标准", size == 18),
                (425, "大", size == 20),
                (426, "特大", size == 22),
            ],
            true,
        );
        self.label(
            512,
            "小 16 · 标准 18 · 大 20 · 特大 22，英文、编号和词性按比例调整。字号选择会保存。",
            y,
            64,
            true,
            2,
        );
        self.button(
            UPDATE,
            "检查版本更新…",
            0,
            y + 82,
            self.width.get(),
            false,
            false,
            !self.preview,
        );
        self.feedback(513, y + 142);
        self.set_height(y + 222);
    }
    fn list_controls(&self, y: i32, cloud: bool) {
        self.label(510, "搜索英文或中文", y, 22, true, 2);
        self.child(
            w!("EDIT"),
            &self.draft(SEARCH),
            WS_BORDER | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            SEARCH,
            0,
            y + 26,
            (self.width.get() - 12) * 2 / 3,
            44,
            Role::Edit,
            0,
        );
        let filter = self.child(
            w!("COMBOBOX"),
            "",
            WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0),
            FILTER,
            (self.width.get() - 12) * 2 / 3 + 12,
            y + 26,
            (self.width.get() - 12) / 3,
            180,
            Role::Edit,
            0,
        );
        for title in if cloud {
            ["全部", "待掌握", "已掌握"]
        } else {
            ["全部", "已学习", "已掌握"]
        } {
            let value = HSTRING::from(title);
            unsafe {
                SendMessageW(
                    filter,
                    CB_ADDSTRING,
                    None,
                    Some(LPARAM(value.as_ptr() as isize)),
                );
            }
        }
        unsafe {
            SendMessageW(
                filter,
                CB_SETCURSEL,
                Some(WPARAM(self.list_state.borrow().filter.max(0) as usize)),
                None,
            );
        }
        let list = self.child(
            w!("LISTBOX"),
            "",
            WS_BORDER
                | WS_TABSTOP
                | WS_VSCROLL
                | WINDOW_STYLE(
                    LBS_OWNERDRAWFIXED as u32
                        | LBS_HASSTRINGS as u32
                        | LBS_NOINTEGRALHEIGHT as u32
                        | LBS_NOTIFY as u32,
                ),
            LIST,
            0,
            y + 86,
            self.width.get(),
            220,
            Role::List,
            0,
        );
        unsafe {
            SendMessageW(
                list,
                LB_SETITEMHEIGHT,
                None,
                Some(LPARAM(px(44, self.dpi()) as isize)),
            );
        }
        let width = (self.width.get() - 12) / 2;
        self.button(MASTER, "标记掌握", 0, y + 358, width, false, false, false);
        if cloud {
            self.button(
                MORE,
                "继续加载",
                width + 12,
                y + 358,
                width,
                false,
                false,
                true,
            );
        } else {
            self.button(
                DELETE,
                "删除词条",
                width + 12,
                y + 358,
                width,
                false,
                false,
                false,
            );
        }
        if !cloud {
            self.button(REFRESH, "刷新", 0, y + 412, width, false, false, true);
            self.button(
                EXPORT,
                "导出全部…",
                width + 12,
                y + 412,
                width,
                false,
                false,
                !self.preview,
            );
        }
        self.filter_words();
    }
    fn learning_page(&self) {
        self.label(500, "本地单词学习", 0, 30, false, 1);
        self.label(
            501,
            "英文候选确认后自动记录；掌握状态由你标记。仅保存在本机，不自动上传。",
            40,
            64,
            true,
            0,
        );
        self.list_controls(114, false);
        self.feedback(503, 596);
        self.set_height(680);
    }
    fn account_page(&self) {
        let data = self.cloud.borrow().clone().unwrap_or_default();
        if let Some(account) = &data.active {
            self.label(500, "我的学习", 0, 30, false, 1);
            self.label(
                501,
                &format!(
                    "{} · {} 条待同步",
                    account.email,
                    data.pending.get(&account.scope()).map_or(0, Vec::len)
                ),
                40,
                44,
                true,
                0,
            );
            let width = (self.width.get() - 12) / 2;
            self.button(SYNC, "同步", 0, 96, width, false, true, !self.preview);
            self.button(
                LOGOUT,
                "退出账号",
                width + 12,
                96,
                width,
                false,
                false,
                !self.preview,
            );
            self.list_controls(156, true);
            self.feedback(503, 630);
            self.set_height(710);
        } else {
            self.label(500, "登录账号", 0, 30, false, 1);
            self.label(
                501,
                "使用自建邮箱验证码服务。账号服务未配置时，本地单词学习仍可使用。",
                40,
                64,
                true,
                0,
            );
            self.field(EMAIL, "邮箱", "填写邮箱地址", 116, false, 254);
            self.field(CODE, "验证码", "六位邮件验证码", 198, false, 6);
            let width = (self.width.get() - 12) / 2;
            let cooled = self.cooldown.get().is_none_or(|v| Instant::now() >= v);
            let label = self.cooldown.get().filter(|v| Instant::now() < *v).map_or(
                "发送验证码".into(),
                |v| {
                    format!(
                        "{} 秒后重发",
                        v.saturating_duration_since(Instant::now()).as_secs() + 1
                    )
                },
            );
            self.button(
                SEND_CODE,
                &label,
                0,
                280,
                width,
                false,
                false,
                !self.preview && cooled && !self.draft(ACCOUNT_URL).is_empty(),
            );
            self.button(
                VERIFY,
                "验证并登录",
                width + 12,
                280,
                width,
                false,
                true,
                !self.preview,
            );
            self.field(
                ACCOUNT_URL,
                "高级设置 · 服务地址",
                "填写已部署账号服务的 HTTPS 地址",
                348,
                false,
                1024,
            );
            self.button(
                SAVE_URL,
                "保存服务地址",
                0,
                430,
                self.width.get(),
                false,
                false,
                !self.preview,
            );
            self.feedback(503, 490);
            self.set_height(580);
        }
    }
    fn selected_word(&self) -> Option<Word> {
        let list = *self.controls.borrow().get(&LIST)?;
        let index = unsafe { SendMessageW(list, LB_GETCURSEL, None, None) }.0;
        usize::try_from(index)
            .ok()
            .and_then(|i| self.visible.borrow().get(i).cloned())
    }
    fn selection_changed(&self) {
        let selected = self.selected_word();
        let controls = self.controls.borrow();
        if let Some(button) = controls.get(&MASTER) {
            unsafe {
                let _ = EnableWindow(
                    *button,
                    selected.is_some() && !self.busy.get() && !self.preview,
                );
                let _ = SetWindowTextW(
                    *button,
                    &HSTRING::from(if selected.as_ref().is_some_and(|w| w.mastered) {
                        "重新学习"
                    } else {
                        "标记掌握"
                    }),
                );
            }
        }
        if let Some(button) = controls.get(&DELETE) {
            unsafe {
                let _ = EnableWindow(
                    *button,
                    selected.is_some() && !self.busy.get() && !self.preview,
                );
            }
        }
    }
    fn filter_words(&self) {
        let selected = self
            .selected_word()
            .map(|word| word.id())
            .or_else(|| self.list_state.borrow().selected.clone());
        let controls = self.controls.borrow();
        let Some(list) = controls.get(&LIST).copied() else {
            return;
        };
        let query = controls
            .get(&SEARCH)
            .map_or(String::new(), |w| text(*w))
            .to_ascii_lowercase();
        let filter = controls.get(&FILTER).map_or(
            0,
            |w| unsafe { SendMessageW(*w, CB_GETCURSEL, None, None) }.0,
        );
        let cloud = self.section.get() == Section::Account;
        let words = if cloud {
            self.cloud
                .borrow()
                .as_ref()
                .and_then(|d| d.active.as_ref().map(|a| d.visible(a)))
                .unwrap_or_default()
        } else {
            self.words.borrow().clone()
        };
        let visible: Vec<_> = words
            .iter()
            .filter(|w| {
                (query.is_empty()
                    || w.english.to_ascii_lowercase().contains(&query)
                    || w.chinese.contains(&query))
                    && (filter == 0 || w.mastered == (filter == 2))
            })
            .take(if cloud { self.limit.get() } else { 5000 })
            .cloned()
            .collect();
        let count = visible.len();
        *self.visible.borrow_mut() = visible;
        drop(controls);
        unsafe {
            SendMessageW(list, LB_RESETCONTENT, None, None);
            for word in self.visible.borrow().iter() {
                let label = HSTRING::from(format!(
                    "{} {} · {} · {} 次 · {}",
                    word.english,
                    word.pos,
                    word.chinese,
                    word.uses,
                    if word.mastered {
                        "已掌握"
                    } else {
                        "已学习"
                    }
                ));
                SendMessageW(
                    list,
                    LB_ADDSTRING,
                    None,
                    Some(LPARAM(label.as_ptr() as isize)),
                );
            }
            if let Some(index) = selected.and_then(|selected| {
                self.visible
                    .borrow()
                    .iter()
                    .position(|word| word.id() == selected)
            }) {
                SendMessageW(list, LB_SETCURSEL, Some(WPARAM(index)), None);
            }
            SendMessageW(
                list,
                LB_SETTOPINDEX,
                Some(WPARAM(self.list_state.borrow().top.max(0) as usize)),
                None,
            );
        }
        let count_text = if words.is_empty() {
            "暂无记录。确认输出一个英文候选即可开始。".into()
        } else {
            format!(
                "共 {} 个词条 · 已掌握 {} · 当前显示 {count}",
                words.len(),
                words.iter().filter(|w| w.mastered).count()
            )
        };
        let existing = self.controls.borrow().get(&530).copied();
        if let Some(window) = existing {
            unsafe {
                let _ = SetWindowTextW(window, &HSTRING::from(count_text));
            }
        } else {
            let y = if cloud { 478 } else { 436 };
            self.label(530, &count_text, y, 44, true, 2);
        }
        self.selection_changed();
    }
}

#[derive(Clone)]
enum Action {
    SaveURL(String),
    SendCode(String),
    Verify(String, String),
    Logout,
    Sync,
    SaveAI(AIConfiguration, String),
    TestAI(AIConfiguration, String),
    DisableAI,
    Local(AIConfiguration),
    RefreshWords,
    Master(Word, bool, Option<u64>),
    Delete(Word),
}
#[derive(Clone, Copy)]
enum ErrorContext {
    Account,
    Translation,
    Learning,
    Update,
}
fn key_status_text(saved: Result<bool, RemoteError>, pending: bool) -> &'static str {
    if pending {
        "新密钥待保存 · 输入内容已隐藏"
    } else {
        match saved {
            Ok(true) => "已保存 · ••••••••（留空保留，输入新密钥可替换）",
            Ok(false) => "尚未保存密钥 · 请填写后保存",
            Err(_) => "密钥保存状态暂不可读 · 可点击测试连接验证",
        }
    }
}
fn connection_message(result: Result<(), RemoteError>, elapsed: Duration) -> String {
    match result {
        Ok(()) => format!(
            "连接成功 · {:.1} 秒。认证和模型调用已通过。",
            elapsed.as_secs_f64()
        ),
        Err(error) => format!("连接失败：{}", error.description()),
    }
}
impl Action {
    fn context(&self) -> ErrorContext {
        match self {
            Self::SaveURL(_)
            | Self::SendCode(_)
            | Self::Verify(_, _)
            | Self::Logout
            | Self::Sync
            | Self::Master(_, _, Some(_)) => ErrorContext::Account,
            Self::SaveAI(_, _) | Self::TestAI(_, _) | Self::DisableAI | Self::Local(_) => {
                ErrorContext::Translation
            }
            Self::RefreshWords | Self::Master(_, _, None) | Self::Delete(_) => {
                ErrorContext::Learning
            }
        }
    }
}
fn error_message(context: ErrorContext, error: RemoteError) -> &'static str {
    match context {
        ErrorContext::Translation => error.description(),
        ErrorContext::Learning => "本地学习记录暂不可读或保存，原文件已保留。",
        ErrorContext::Update => "无法打开版本更新窗口，请检查安装文件后重试。",
        ErrorContext::Account => match error {
            RemoteError::Configuration => "请检查账号服务的 HTTPS 地址、邮箱或六位验证码。",
            RemoteError::MissingKey => "本机登录凭据不可用，请重新验证邮箱登录。",
            RemoteError::Denied => "验证码或登录凭据无效，请重新获取验证码并登录。",
            RemoteError::Quota => "账号请求过于频繁或待同步词条已满，请稍后重试。",
            RemoteError::Network => "账号服务暂时无法连接，请检查服务地址和网络后重试。",
            RemoteError::InvalidResponse => "账号服务返回的数据无法读取，请检查服务版本后重试。",
            RemoteError::Changed => "账号或服务地址已变化，本次操作未提交。请重新确认账号后重试。",
            RemoteError::Storage => "无法保存账号设置或登录凭据，原数据已保留。",
            RemoteError::Unavailable => "账号服务尚未配置，请保存已部署服务的 HTTPS 地址。",
            RemoteError::HelperUnavailable => "账号辅助进程暂不可用，请重新打开设置后重试。",
            RemoteError::Packaged => "账号辅助进程仍有应用包身份，已停止访问设置或凭据。",
            RemoteError::LocalSafety => "账号请求未通过安全校验，请检查服务地址。",
        },
    }
}
impl Window {
    fn begin(&self, action: Action) {
        if self.preview {
            *self.feedback.borrow_mut() = "界面样例已更新到临时内存，未联网或访问凭据。".into();
            self.capture();
            self.render(true);
            return;
        }
        if self.busy.replace(true) {
            return;
        }
        self.capture();
        let testing = matches!(action, Action::TestAI(_, _));
        self.connection_feedback.set(testing);
        *self.feedback.borrow_mut() = if testing {
            "正在测试连接…"
        } else {
            "正在处理…"
        }
        .into();
        self.render(true);
        let root = self.root.as_ref().unwrap().clone();
        let tx = self.tx.clone();
        let context = action.context();
        let generation = self.translation_generation.get();
        std::thread::spawn(move || {
            if let Action::TestAI(configuration, key) = &action {
                let start = Instant::now();
                let result = translation::Store::new(root).test_ai(
                    configuration.clone(),
                    key.trim(),
                    &SystemVault,
                    &NativeHTTP,
                );
                let _ = tx.send(Reply::Tested {
                    generation,
                    result,
                    elapsed: start.elapsed(),
                });
                return;
            }
            let clear_secret = matches!(action, Action::SaveAI(_, _));
            let clear_code = matches!(action, Action::Verify(_, _));
            let store = account::Store::new(root.clone());
            let service = account::Service {
                store: &store,
                vault: &SystemVault,
                http: &NativeHTTP,
            };
            let result = (|| -> Result<&'static str, RemoteError> {
                match action {
                    Action::SaveURL(url) => {
                        store.save_url(url.trim())?;
                        Ok("服务地址已保存。")
                    }
                    Action::SendCode(email) => {
                        service.send_code(email.trim())?;
                        Ok("验证码已发送，请检查邮箱。")
                    }
                    Action::Verify(email, code) => {
                        service.verify(email.trim(), code.trim())?;
                        let _ = service.sync();
                        Ok("已登录。")
                    }
                    Action::Logout => {
                        service.logout()?;
                        Ok("已退出本机账号，云端词条保留。")
                    }
                    Action::Sync => {
                        service.sync()?;
                        Ok("已同步。")
                    }
                    Action::SaveAI(configuration, key) => {
                        translation::Store::new(root.clone()).save_ai(
                            configuration,
                            key.trim(),
                            &SystemVault,
                        )?;
                        Ok(
                            "已保存并启用 AI。密钥保存在 Windows 凭据管理器；可点击“测试连接”验证接口。",
                        )
                    }
                    Action::DisableAI => {
                        translation::Store::new(root.clone()).disable_ai()?;
                        Ok("已关闭 AI 在线翻译；已保存的密钥保留。")
                    }
                    Action::TestAI(_, _) => unreachable!(),
                    Action::Local(configuration) => {
                        super::local::verify(&configuration, &NativeHTTP)?;
                        let body = crate::remote::request_body(
                            "这是本机翻译准备测试。",
                            &configuration,
                            true,
                        )?;
                        let response = NativeHTTP.request(
                            &configuration.endpoint,
                            "POST",
                            None,
                            Some(&body),
                            true,
                            64_000,
                        )?;
                        crate::remote::response(response.status, &response.bytes)?;
                        translation::Store::new(root.clone()).save_local(configuration)?;
                        Ok("本机中英文翻译调用已通过。关闭在线 AI 后，重新选择候选即可使用。")
                    }
                    Action::RefreshWords => Ok("已刷新。"),
                    Action::Master(word, mastered, expected) => {
                        if let Some(expected) = expected {
                            store.append(&word, "mastered", mastered, expected)?;
                            let _ = service.sync();
                        } else {
                            crate::learning::Store::new(root.join("LocalWords"))
                                .mastered(&word.id(), mastered)
                                .map_err(|_| RemoteError::Storage)?;
                        }
                        Ok("掌握状态已保存。")
                    }
                    Action::Delete(word) => {
                        crate::learning::Store::new(root.join("LocalWords"))
                            .delete(&word.id())
                            .map_err(|_| RemoteError::Storage)?;
                        Ok("本地词条已删除。再次选用时将重新建立记录。")
                    }
                }
            })();
            let success = result.is_ok();
            let _ = tx.send(Reply::Done {
                message: result.map_or_else(|e| error_message(context, e).into(), String::from),
                clear_secret: clear_secret && success,
                clear_code: clear_code && success,
            });
            let _ = tx.send(Reply::Words(
                crate::learning::Store::new(root.join("LocalWords"))
                    .snapshot()
                    .map_err(|_| RemoteError::Storage),
            ));
            let _ = tx.send(Reply::Account(store.snapshot()));
        });
    }
    fn setting(&self, setting: Setting) {
        if self.preview {
            let mut snapshot = self.snapshot.get().unwrap_or_default();
            apply_snapshot(&mut snapshot, setting);
            self.snapshot.set(Some(snapshot));
            self.display.set(snapshot.display);
            self.show_pos.set(snapshot.show_pos);
            self.theme.set(snapshot.display.theme);
            self.update_palette();
            self.render(true);
            return;
        }
        if matches!(
            setting,
            Setting::ShowPOS(_) | Setting::Font(_) | Setting::Theme(_)
        ) {
            let saved = match setting {
                Setting::ShowPOS(v) => crate::preferences::save(v),
                Setting::Font(v) => crate::preferences::save_font(v),
                Setting::Theme(v) => crate::preferences::save_theme(v),
                _ => unreachable!(),
            };
            if saved.is_ok() {
                let mut desired = self.effective_snapshot();
                apply_snapshot(&mut desired, setting);
                self.display.set(desired.display);
                self.show_pos.set(desired.show_pos);
                if let Some(shared) = &self.shared {
                    shared.set_display(desired);
                }
                if let Some(mut value) = self.snapshot.get() {
                    apply_snapshot(&mut value, setting);
                    self.snapshot.set(Some(value));
                }
                let _ = crate::settings::request(self.service.get(), Some(setting));
                if let Setting::Theme(theme) = setting {
                    self.theme.set(theme);
                    self.update_palette();
                }
                *self.feedback.borrow_mut() = "显示偏好已保存。".into();
            } else {
                *self.feedback.borrow_mut() = "本机偏好未保存，请检查权限后重试。".into();
            }
            self.render(true);
            return;
        }
        let response = crate::settings::request(self.service.get(), Some(setting));
        if let Some(snapshot) = response {
            self.snapshot.set(Some(snapshot));
            let saved = match setting {
                Setting::ShowPOS(value) => crate::preferences::save(value),
                Setting::Font(value) => crate::preferences::save_font(value),
                Setting::Theme(value) => crate::preferences::save_theme(value),
                _ => Ok(()),
            };
            if let Some(shared) = &self.shared {
                let mut desired = shared.snapshot().unwrap_or(snapshot);
                apply_snapshot(&mut desired, setting);
                if matches!(
                    setting,
                    Setting::Font(_) | Setting::ShowPOS(_) | Setting::Theme(_)
                ) {
                    shared.set_display(desired);
                } else if !snapshot.pending {
                    shared.set_runtime(desired);
                }
            }
            if let Setting::Theme(theme) = setting {
                self.theme.set(theme);
                self.update_palette();
            }
            *self.feedback.borrow_mut() = if saved.is_err() {
                "输入设置已请求，本机偏好暂未保存。".into()
            } else if snapshot.pending {
                "正在等待输入服务确认…".into()
            } else {
                "已应用并回读确认。".into()
            };
        } else {
            *self.feedback.borrow_mut() = "输入服务未响应，设置尚未确认。".into();
            self.snapshot.set(None);
        }
        self.render(true);
    }
    fn command(&self, id: u16, notification: u16) {
        if self.rebuilding.get() {
            return;
        }
        if (100..105).contains(&id) {
            self.capture();
            self.translation_edited();
            self.section.set(SECTIONS[(id - 100) as usize]);
            self.limit.set(50);
            self.feedback.borrow_mut().clear();
            self.render(false);
            return;
        }
        if [ENDPOINT, MODEL, SECRET].contains(&id) && notification == EN_CHANGE as u16 {
            self.capture();
            self.translation_edited();
            return;
        }
        if id == SEARCH && notification == EN_CHANGE as u16 {
            self.filter_words();
            return;
        }
        if id == FILTER && notification == CBN_SELCHANGE as u16 {
            self.filter_words();
            return;
        }
        if id == LIST && notification == LBN_SELCHANGE as u16 {
            self.selection_changed();
            return;
        }
        if notification != BN_CLICKED as u16 {
            return;
        }
        self.capture();
        let configuration = || AIConfiguration {
            version: 1,
            endpoint: self.draft(ENDPOINT).trim().into(),
            model: self.draft(MODEL).trim().into(),
            enabled: true,
            revision: String::new(),
        };
        match id {
            DOMESTIC | INTERNATIONAL => {
                let value = AIConfiguration::minimax(id == INTERNATIONAL);
                self.drafts.borrow_mut().insert(ENDPOINT, value.endpoint);
                self.drafts.borrow_mut().insert(MODEL, value.model);
                self.drafts.borrow_mut().insert(SECRET, String::new());
                self.translation_edited();
                *self.feedback.borrow_mut() = "已填入预设，尚未保存。".into();
                self.render(true);
            }
            SAVE_AI => self.begin(Action::SaveAI(configuration(), self.draft(SECRET))),
            TEST_AI => self.begin(Action::TestAI(configuration(), self.draft(SECRET))),
            DISABLE_AI => self.begin(Action::DisableAI),
            PREPARE_LOCAL => {
                self.translation_edited();
                self.section.set(Section::Local);
                self.feedback.borrow_mut().clear();
                self.render(false);
            }
            SAVE_LOCAL => self.begin(Action::Local(AIConfiguration {
                version: 1,
                endpoint: self.draft(LOCAL_URL).trim().into(),
                model: self.draft(LOCAL_MODEL).trim().into(),
                enabled: true,
                revision: String::new(),
            })),
            SAVE_URL => self.begin(Action::SaveURL(self.draft(ACCOUNT_URL))),
            SEND_CODE => {
                self.cooldown
                    .set(Some(Instant::now() + Duration::from_secs(60)));
                self.begin(Action::SendCode(self.draft(EMAIL)));
            }
            VERIFY => self.begin(Action::Verify(self.draft(EMAIL), self.draft(CODE))),
            LOGOUT => self.begin(Action::Logout),
            SYNC => self.begin(Action::Sync),
            REFRESH => self.begin(Action::RefreshWords),
            MASTER => {
                if let Some(word) = self.selected_word() {
                    let mastered = !word.mastered;
                    // Bind the queued operation to the account shown at click time;
                    // the store rejects it if another process switches accounts.
                    let expected = if self.section.get() == Section::Account {
                        let Some(expected) = self
                            .cloud
                            .borrow()
                            .as_ref()
                            .and_then(|data| data.active.as_ref())
                            .map(account::Identity::fingerprint)
                        else {
                            return;
                        };
                        Some(expected)
                    } else {
                        None
                    };
                    self.begin(Action::Master(word, mastered, expected));
                }
            }
            DELETE => {
                if let Some(word) = self.selected_word() {
                    let prompt = HSTRING::from(format!(
                        "删除“{}”的本地记录？再次确认输出该词时，会重新建立记录。",
                        word.english
                    ));
                    if unsafe {
                        MessageBoxW(
                            Some(self.hwnd.get()),
                            &prompt,
                            w!("删除词条"),
                            MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2,
                        )
                    } == IDYES
                    {
                        self.begin(Action::Delete(word));
                    }
                }
            }
            EXPORT => self.export(),
            MORE => {
                self.limit.set((self.limit.get() + 50).min(5000));
                self.filter_words();
            }
            UPDATE => {
                if let Err(error) = launch_update() {
                    *self.feedback.borrow_mut() = error_message(ErrorContext::Update, error).into();
                    self.render(true);
                }
            }
            400 => self.setting(Setting::English(false)),
            401 => self.setting(Setting::English(true)),
            402 => self.setting(Setting::Bilingual(true)),
            403 => self.setting(Setting::Bilingual(false)),
            404 => self.setting(Setting::ShowPOS(true)),
            405 => self.setting(Setting::ShowPOS(false)),
            406..=408 => self.setting(Setting::Punctuation(u32::from(id - 406))),
            409 => self.setting(Setting::Remember(true)),
            410 => self.setting(Setting::Remember(false)),
            420..=422 => self.setting(Setting::Theme(Theme::from_value(u32::from(id - 420)))),
            423..=426 => self.setting(Setting::Font(Display::SIZES[(id - 423) as usize])),
            _ => {}
        }
    }
    fn export(&self) {
        if self.preview {
            return;
        }
        let mut path = [0u16; 32768];
        let initial: Vec<u16> = "lingomate-local-words.csv".encode_utf16().collect();
        path[..initial.len()].copy_from_slice(&initial);
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: self.hwnd.get(),
            lpstrFilter: w!("CSV 文件\0*.csv\0\0"),
            lpstrFile: windows::core::PWSTR(path.as_mut_ptr()),
            nMaxFile: path.len() as u32,
            lpstrDefExt: w!("csv"),
            Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        if !unsafe { GetSaveFileNameW(&mut dialog) }.as_bool() {
            return;
        }
        let length = path.iter().position(|v| *v == 0).unwrap_or(path.len());
        let path = PathBuf::from(String::from_utf16_lossy(&path[..length]));
        let result = crate::learning::Store::new(self.root.as_ref().unwrap().join("LocalWords"))
            .snapshot()
            .map(|words| crate::learning::csv(&words))
            .and_then(|bytes| {
                std::fs::write(path, bytes).map_err(|_| "导出未完成，请检查保存位置。".into())
            });
        *self.feedback.borrow_mut() =
            result.map_or_else(|error| error, |_| "已导出全部本地词条。".into());
        self.render(true);
    }
    fn timer(&self) {
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            self.close();
            return;
        }
        // Capture before replies clear secrets/code; capturing after the reply
        // would repopulate those drafts from the old edit controls.
        self.capture();
        let mut redraw = false;
        for reply in self.rx.try_iter() {
            match reply {
                Reply::Done {
                    message,
                    clear_secret,
                    clear_code,
                } => {
                    self.busy.set(false);
                    *self.feedback.borrow_mut() = message;
                    if clear_secret {
                        self.drafts.borrow_mut().insert(SECRET, String::new());
                    }
                    if clear_code {
                        self.drafts.borrow_mut().insert(CODE, String::new());
                    }
                    redraw = true;
                }
                Reply::Tested {
                    generation,
                    result,
                    elapsed,
                } => {
                    self.busy.set(false);
                    if generation == self.translation_generation.get() {
                        *self.feedback.borrow_mut() = connection_message(result, elapsed);
                        self.connection_feedback.set(true);
                    }
                    redraw = true;
                }
                Reply::Words(words) => match words {
                    Ok(words) => {
                        *self.words.borrow_mut() = words;
                        redraw = true;
                    }
                    Err(error) => {
                        *self.feedback.borrow_mut() =
                            error_message(ErrorContext::Learning, error).into();
                        redraw = true;
                    }
                },
                Reply::Account(data) => match data {
                    Ok(data) => {
                        *self.cloud.borrow_mut() = Some(data);
                        redraw = true;
                    }
                    Err(error) => {
                        *self.feedback.borrow_mut() =
                            error_message(ErrorContext::Account, error).into();
                        redraw = true;
                    }
                },
            }
        }
        if !self.preview {
            let snapshot = crate::settings::request(self.service.get(), None);
            if snapshot != self.snapshot.get() {
                if self.snapshot.get().is_some_and(|old| old.pending)
                    && snapshot.is_some_and(|value| !value.pending)
                {
                    *self.feedback.borrow_mut() = "已应用并回读确认。".into();
                }
                self.snapshot.set(snapshot);
                if matches!(self.section.get(), Section::Input | Section::Appearance) {
                    redraw = true;
                }
            }
            if let Some(shared) = &self.shared {
                if let Some(value) = shared.snapshot()
                    && (value.display != self.display.get()
                        || value.show_pos != self.show_pos.get())
                {
                    let theme_changed = value.display.theme != self.theme.get();
                    self.display.set(value.display);
                    self.show_pos.set(value.show_pos);
                    self.theme.set(value.display.theme);
                    if theme_changed {
                        self.update_palette();
                    }
                    redraw |= theme_changed
                        || matches!(self.section.get(), Section::Input | Section::Appearance);
                }
                let account = shared.account();
                if account != self.account_seen.replace(account) {
                    match account::Store::new(self.root.as_ref().unwrap().clone()).snapshot() {
                        Ok(data) => *self.cloud.borrow_mut() = Some(data),
                        Err(error) => {
                            *self.cloud.borrow_mut() = None;
                            *self.feedback.borrow_mut() =
                                error_message(ErrorContext::Account, error).into();
                        }
                    }
                    self.limit.set(50);
                    redraw |= self.section.get() == Section::Account;
                }
            }
        }
        if let Some(button) = self.controls.borrow().get(&SEND_CODE).copied() {
            let cooled = self.cooldown.get().is_none_or(|v| Instant::now() >= v);
            let label = self.cooldown.get().filter(|v| Instant::now() < *v).map_or(
                "发送验证码".into(),
                |v| {
                    format!(
                        "{} 秒后重发",
                        v.saturating_duration_since(Instant::now()).as_secs() + 1
                    )
                },
            );
            unsafe {
                let _ = SetWindowTextW(button, &HSTRING::from(label));
                let _ = EnableWindow(
                    button,
                    !self.preview
                        && !self.busy.get()
                        && cooled
                        && !self.draft(ACCOUNT_URL).is_empty(),
                );
            }
        }
        if redraw {
            self.render(true);
        }
    }
    fn resize(&self) {
        if self.hwnd.get().is_invalid() || self.body.get().is_invalid() {
            return;
        }
        self.capture();
        let mut rect = RECT::default();
        let _ = unsafe { GetClientRect(self.hwnd.get(), &mut rect) };
        let dpi = self.dpi();
        let logical_width = (rect.right - rect.left) * 96 / dpi;
        self.width.set((logical_width - 32).max(240));
        unsafe {
            let _ = MoveWindow(
                self.body.get(),
                px(16, dpi),
                px(108, dpi),
                (rect.right - rect.left) - px(32, dpi),
                ((rect.bottom - rect.top) - px(124, dpi)).max(1),
                true,
            );
        }
        let width = (logical_width - 32 - 4 * 4) / 5;
        for (index, tab) in self.tabs.borrow().iter().enumerate() {
            let _ = unsafe {
                MoveWindow(
                    *tab,
                    px(16 + index as i32 * (width + 4), dpi),
                    px(52, dpi),
                    px(width, dpi),
                    px(44, dpi),
                    true,
                )
            };
        }
        self.render(true);
    }
    fn draw_item(&self, item: &DRAWITEMSTRUCT) {
        let dc = item.hDC;
        let palette = self.palette();
        let saved = unsafe { SaveDC(dc) };
        unsafe {
            let _ = SetBkMode(dc, TRANSPARENT);
            SelectObject(dc, self.font(0).into());
        }
        if item.CtlID == u32::from(LIST) {
            let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
            let color = if selected {
                palette.selected
            } else {
                palette.background
            };
            fill(dc, item.rcItem, color);
            if let Some(word) = self.visible.borrow().get(item.itemID as usize) {
                let text = format!(
                    "{} {}    {}    {} 次    {}",
                    word.english,
                    word.pos,
                    word.chinese,
                    word.uses,
                    if word.mastered {
                        "已掌握"
                    } else {
                        "已学习"
                    }
                );
                draw(
                    dc,
                    &text,
                    item.rcItem,
                    palette.foreground,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                    px(8, self.dpi()),
                );
            }
        } else {
            let role = if (100..105).contains(&item.CtlID) {
                Role::Button {
                    selected: (item.CtlID - 100) as usize == self.section.get().index(),
                    outlined: false,
                }
            } else {
                self.roles
                    .borrow()
                    .get(&(item.CtlID as u16))
                    .copied()
                    .unwrap_or(Role::Button {
                        selected: false,
                        outlined: false,
                    })
            };
            let (selected, outlined) = match role {
                Role::Button { selected, outlined } => (selected, outlined),
                _ => (false, false),
            };
            let pressed = item.itemState.0 & ODS_SELECTED.0 != 0;
            let disabled = item.itemState.0 & ODS_DISABLED.0 != 0;
            fill(dc, item.rcItem, palette.background);
            rounded(
                dc,
                item.rcItem,
                if selected || pressed {
                    palette.selected
                } else {
                    palette.background
                },
                if outlined {
                    palette.divider
                } else {
                    palette.background
                },
                px(24, self.dpi()),
            );
            draw(
                dc,
                &text(item.hwndItem),
                item.rcItem,
                if disabled {
                    palette.secondary
                } else {
                    palette.foreground
                },
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                px(6, self.dpi()),
            );
            if item.itemState.0 & ODS_FOCUS.0 != 0 {
                let mut rect = item.rcItem;
                rect.left += 4;
                rect.right -= 4;
                rect.top += 4;
                rect.bottom -= 4;
                unsafe {
                    let _ = DrawFocusRect(dc, &rect);
                }
            }
        }
        unsafe {
            let _ = RestoreDC(dc, saved);
        }
    }
    fn paint(&self, dc: HDC) {
        let saved = unsafe { SaveDC(dc) };
        let mut rect = RECT::default();
        let _ = unsafe { GetClientRect(self.hwnd.get(), &mut rect) };
        fill(dc, rect, self.palette().background);
        unsafe {
            SelectObject(dc, self.font(3).into());
            SetBkMode(dc, TRANSPARENT);
        }
        let dpi = self.dpi();
        let title = RECT {
            left: px(16, dpi),
            top: px(10, dpi),
            right: rect.right - px(16, dpi),
            bottom: px(44, dpi),
        };
        draw(
            dc,
            "设置",
            title,
            self.palette().foreground,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            0,
        );
        unsafe {
            SelectObject(dc, self.font(2).into());
        }
        draw(
            dc,
            "灵果",
            title,
            self.palette().secondary,
            DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
            0,
        );
        unsafe {
            let _ = RestoreDC(dc, saved);
        }
    }
    fn render_bitmap(&self, path: &std::path::Path) -> Result<(), RemoteError> {
        if !self.preview || self.fixture_dpi.is_none() || !path.is_absolute() {
            return Err(RemoteError::Configuration);
        }
        // Expand this offscreen owned window to show the entire existing page.
        // The controls, fonts, owner draw callbacks and placements are the real UI.
        let mut outer = RECT::default();
        let mut client = RECT::default();
        unsafe {
            GetWindowRect(self.hwnd.get(), &mut outer).map_err(|_| RemoteError::Unavailable)?;
            GetClientRect(self.hwnd.get(), &mut client).map_err(|_| RemoteError::Unavailable)?;
            SetWindowPos(
                self.hwnd.get(),
                None,
                0,
                0,
                px(560, self.dpi()) + (outer.right - outer.left) - client.right,
                px(self.height.get() + 124, self.dpi()) + (outer.bottom - outer.top)
                    - client.bottom,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .map_err(|_| RemoteError::Unavailable)?;
        }
        self.resize();
        unsafe { GetClientRect(self.hwnd.get(), &mut client) }
            .map_err(|_| RemoteError::Unavailable)?;
        let width = client.right;
        let height = client.bottom;
        if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return Err(RemoteError::Configuration);
        }
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return Err(RemoteError::Unavailable);
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap =
            unsafe { CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) };
        let bitmap = match bitmap {
            Ok(bitmap) => bitmap,
            Err(_) => {
                unsafe {
                    let _ = DeleteDC(dc);
                }
                return Err(RemoteError::Unavailable);
            }
        };
        let old = unsafe { SelectObject(dc, bitmap.into()) };
        unsafe {
            SendMessageW(
                self.hwnd.get(),
                WM_PRINT,
                Some(WPARAM(dc.0 as usize)),
                Some(LPARAM(
                    (PRF_CLIENT | PRF_CHILDREN | PRF_ERASEBKGND) as isize,
                )),
            );
        }
        unsafe {
            let _ = GdiFlush();
        }
        let length = width as usize * height as usize * 4;
        let pixels = if bits.is_null() {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(bits.cast::<u8>(), length) }.to_vec())
        };
        unsafe {
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
        }
        let pixels = pixels.ok_or(RemoteError::Unavailable)?;
        let mut output = Vec::with_capacity(54 + pixels.len());
        output.extend_from_slice(b"BM");
        output.extend_from_slice(&((54 + pixels.len()) as u32).to_le_bytes());
        output.extend_from_slice(&[0; 4]);
        output.extend_from_slice(&54u32.to_le_bytes());
        output.extend_from_slice(&40u32.to_le_bytes());
        output.extend_from_slice(&width.to_le_bytes());
        output.extend_from_slice(&(-height).to_le_bytes());
        output.extend_from_slice(&1u16.to_le_bytes());
        output.extend_from_slice(&32u16.to_le_bytes());
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
        let density = (self.dpi() * 10_000 + 127) / 254;
        output.extend_from_slice(&density.to_le_bytes());
        output.extend_from_slice(&density.to_le_bytes());
        output.extend_from_slice(&[0; 8]);
        output.extend_from_slice(&pixels);
        std::fs::write(path, output).map_err(|_| RemoteError::Storage)
    }
}
fn position_children(body: HWND, placements: &[(HWND, i32, i32, i32, i32)], scroll: i32, dpi: i32) {
    // Do not copy old sibling pixels or paint an intermediate layout. A wheel
    // step can move a button through an edit's former rectangle.
    let flags = SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_NOREDRAW;
    let batch = unsafe { BeginDeferWindowPos(placements.len() as i32) }
        .and_then(|batch| {
            placements
                .iter()
                .try_fold(batch, |batch, (window, x, y, width, height)| unsafe {
                    DeferWindowPos(
                        batch,
                        *window,
                        None,
                        px(*x, dpi),
                        px(*y - scroll, dpi),
                        px(*width, dpi),
                        px(*height, dpi),
                        flags,
                    )
                })
        })
        .and_then(|batch| unsafe { EndDeferWindowPos(batch) });
    if batch.is_err() {
        // DeferWindowPos frees its failed batch. Finish every position without
        // repainting before restoring the complete body and child surfaces.
        for (window, x, y, width, height) in placements {
            let _ = unsafe {
                SetWindowPos(
                    *window,
                    None,
                    px(*x, dpi),
                    px(*y - scroll, dpi),
                    px(*width, dpi),
                    px(*height, dpi),
                    flags,
                )
            };
        }
    }
    unsafe {
        let _ = RedrawWindow(
            Some(body),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );
    }
}
fn apply_snapshot(snapshot: &mut Snapshot, setting: Setting) {
    match setting {
        Setting::English(v) => snapshot.english = v,
        Setting::Bilingual(v) => snapshot.bilingual = v,
        Setting::Remember(v) => snapshot.remember = v,
        Setting::Punctuation(v) => snapshot.punctuation = v,
        Setting::ShowPOS(v) => snapshot.show_pos = v,
        Setting::Font(v) => snapshot.display.font = v,
        Setting::Theme(v) => snapshot.display.theme = v,
    }
}
fn effective_snapshot(snapshot: Option<Snapshot>, display: Display, show_pos: bool) -> Snapshot {
    Snapshot {
        display,
        show_pos,
        ..snapshot.unwrap_or_default()
    }
}
fn text(window: HWND) -> String {
    let size = unsafe { GetWindowTextLengthW(window) }.clamp(0, 2048) as usize;
    let mut buffer = vec![0u16; size + 1];
    let count = unsafe { GetWindowTextW(window, &mut buffer) }.max(0) as usize;
    String::from_utf16_lossy(&buffer[..count]).trim().into()
}
fn fill(dc: HDC, rect: RECT, color: COLORREF) {
    let brush = unsafe { CreateSolidBrush(color) };
    unsafe {
        FillRect(dc, &rect, brush);
        let _ = DeleteObject(brush.into());
    }
}
fn rounded(dc: HDC, rect: RECT, color: COLORREF, border: COLORREF, radius: i32) {
    let brush = unsafe { CreateSolidBrush(color) };
    let pen = unsafe { CreatePen(PS_SOLID, 1, border) };
    unsafe {
        let old = SelectObject(dc, brush.into());
        let line = SelectObject(dc, pen.into());
        let _ = RoundRect(
            dc,
            rect.left + 1,
            rect.top + 1,
            rect.right - 1,
            rect.bottom - 1,
            radius,
            radius,
        );
        SelectObject(dc, old);
        SelectObject(dc, line);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(pen.into());
    }
}
fn draw(
    dc: HDC,
    text: &str,
    mut rect: RECT,
    color: COLORREF,
    flags: DRAW_TEXT_FORMAT,
    padding: i32,
) {
    rect.left += padding;
    rect.right -= padding;
    let mut value: Vec<u16> = text.encode_utf16().collect();
    if !value.is_empty() {
        unsafe {
            SetTextColor(dc, color);
            DrawTextW(dc, &mut value, &mut rect, flags | DT_NOPREFIX);
        }
    }
}
fn launch_update() -> Result<(), RemoteError> {
    let root = std::env::current_exe()
        .map_err(|_| RemoteError::Unavailable)?
        .parent()
        .ok_or(RemoteError::Unavailable)?
        .to_path_buf();
    let script = root.join("update.ps1");
    if !script.is_file() {
        return Err(RemoteError::Unavailable);
    }
    let windows = std::env::var_os("WINDIR").ok_or(RemoteError::Unavailable)?;
    use std::os::windows::process::CommandExt;
    std::process::Command::new(
        PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    )
    .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
    .arg(script)
    .creation_flags(0x08000000)
    .spawn()
    .map(|_| ())
    .map_err(|_| RemoteError::Unavailable)
}
unsafe extern "system" fn body_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if matches!(
        message,
        WM_COMMAND
            | WM_DRAWITEM
            | WM_MEASUREITEM
            | WM_CTLCOLORSTATIC
            | WM_CTLCOLOREDIT
            | WM_CTLCOLORLISTBOX
            | WM_CTLCOLORBTN
            | WM_MOUSEWHEEL
    ) {
        return unsafe {
            SendMessageW(
                GetParent(window).unwrap_or_default(),
                message,
                Some(wparam),
                Some(lparam),
            )
        };
    }
    if message == WM_ERASEBKGND || message == WM_PRINTCLIENT {
        let parent = unsafe { GetParent(window) }.unwrap_or_default();
        let pointer = unsafe { GetWindowLongPtrW(parent, GWLP_USERDATA) } as *const Window;
        if !pointer.is_null() {
            let state = unsafe { &*pointer };
            let mut rect = RECT::default();
            let _ = unsafe { GetClientRect(window, &mut rect) };
            fill(HDC(wparam.0 as *mut _), rect, state.palette().background);
            return LRESULT(1);
        }
    }
    if message == WM_PAINT {
        let parent = unsafe { GetParent(window) }.unwrap_or_default();
        let pointer = unsafe { GetWindowLongPtrW(parent, GWLP_USERDATA) } as *const Window;
        if !pointer.is_null() {
            let state = unsafe { &*pointer };
            let mut paint = PAINTSTRUCT::default();
            let dc = unsafe { BeginPaint(window, &mut paint) };
            fill(dc, paint.rcPaint, state.palette().background);
            let _ = unsafe { EndPaint(window, &paint) };
            return LRESULT(0);
        }
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        procedure_body(window, message, wparam, lparam)
    }))
    .unwrap_or(LRESULT(0))
}
fn procedure_body(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const Window;
    if !pointer.is_null() {
        let state = unsafe { &*pointer };
        match message {
            REOPEN if !state.preview => {
                if let Some(section) = Section::from_packet(wparam.0) {
                    state.reopen(section, HWND(lparam.0 as *mut _));
                    return LRESULT(REOPENED);
                }
                return LRESULT(0);
            }
            WM_CLOSE => {
                state.close();
                return LRESULT(0);
            }
            WM_COMMAND => {
                state.command(wparam.0 as u16, ((wparam.0 >> 16) & 0xffff) as u16);
                return LRESULT(0);
            }
            WM_TIMER => {
                state.timer();
                return LRESULT(0);
            }
            WM_SIZE => {
                state.resize();
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xffff) as i32;
                state.capture();
                *state.fonts.borrow_mut() =
                    vec![font(13, dpi), font(20, dpi), font(12, dpi), font(22, dpi)];
                for tab in state.tabs.borrow().iter() {
                    unsafe {
                        SendMessageW(
                            *tab,
                            WM_SETFONT,
                            Some(WPARAM(state.font(0).0 as usize)),
                            Some(LPARAM(1)),
                        );
                    }
                }
                let rect = unsafe { &*(lparam.0 as *const RECT) };
                unsafe {
                    let _ = SetWindowPos(
                        window,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                state.resize();
                return LRESULT(0);
            }
            WM_GETMINMAXINFO => {
                let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
                info.ptMinTrackSize = POINT {
                    x: px(440, state.dpi()),
                    y: px(440, state.dpi()),
                };
                return LRESULT(0);
            }
            WM_VSCROLL | WM_MOUSEWHEEL => {
                let step = if message == WM_MOUSEWHEEL {
                    -(((wparam.0 >> 16) as u16 as i16) as i32) / 120 * 88
                } else {
                    match (wparam.0 & 0xffff) as u32 {
                        0 => -44,
                        1 => 44,
                        2 => -220,
                        3 => 220,
                        4 | 5 => ((wparam.0 >> 16) & 0xffff) as i32 - state.scroll.get(),
                        _ => 0,
                    }
                };
                state
                    .scroll
                    .set((state.scroll.get() + step).clamp(0, state.scroll_max()));
                state.position_controls();
                return LRESULT(0);
            }
            WM_DRAWITEM => {
                state.draw_item(unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) });
                return LRESULT(1);
            }
            WM_MEASUREITEM => {
                let item = unsafe { &mut *(lparam.0 as *mut MEASUREITEMSTRUCT) };
                if item.CtlID == u32::from(LIST) {
                    item.itemHeight = px(44, state.dpi()) as u32;
                    return LRESULT(1);
                }
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
                let dc = HDC(wparam.0 as *mut _);
                let id = unsafe { GetDlgCtrlID(HWND(lparam.0 as *mut _)) };
                let role = state.roles.borrow().get(&(id as u16)).copied();
                let palette = state.palette();
                unsafe {
                    SetTextColor(
                        dc,
                        if matches!(role, Some(Role::Secondary)) {
                            palette.secondary
                        } else {
                            palette.foreground
                        },
                    );
                    SetBkColor(dc, palette.background);
                }
                return LRESULT(state.brush.get().0 as isize);
            }
            WM_ERASEBKGND => {
                let mut rect = RECT::default();
                let _ = unsafe { GetClientRect(window, &mut rect) };
                fill(HDC(wparam.0 as *mut _), rect, state.palette().background);
                return LRESULT(1);
            }
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = unsafe { BeginPaint(window, &mut paint) };
                state.paint(dc);
                unsafe {
                    let _ = EndPaint(window, &paint);
                }
                return LRESULT(0);
            }
            WM_PRINTCLIENT => {
                state.paint(HDC(wparam.0 as *mut _));
                return LRESULT(0);
            }
            WM_SETTINGCHANGE | WM_THEMECHANGED => {
                state.update_palette();
                state.resize();
                return LRESULT(0);
            }
            WM_DESTROY => {
                state.clear_input_scopes();
                unsafe {
                    let _ = KillTimer(Some(window), 1);
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                    PostQuitMessage(0);
                }
                return LRESULT(0);
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

pub fn run(args: &[String]) -> Result<(), RemoteError> {
    let preview = args.iter().any(|v| v == "--preview");
    let argument = |name: &str| {
        args.iter()
            .position(|v| v == name)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    let render_path = argument("--preview-render").map(PathBuf::from);
    if render_path
        .as_ref()
        .is_some_and(|path| !preview || !path.is_absolute())
    {
        return Err(RemoteError::Configuration);
    }
    let fixture_dpi = if render_path.is_some() {
        Some(match argument("--preview-dpi") {
            None | Some("96") => 96,
            Some("144") => 144,
            _ => return Err(RemoteError::Configuration),
        })
    } else {
        None
    };
    let section = match argument("--section") {
        Some("translation") => Section::Translation,
        Some("input") => Section::Input,
        Some("appearance") => Section::Appearance,
        Some("learning") => Section::Learning,
        Some("local") => Section::Local,
        _ => Section::Account,
    };
    let _apartment = Apartment::open()?;
    let shared = if preview {
        None
    } else {
        crate::settings::Shared::open()
    };
    let service = argument("--service")
        .and_then(|s| usize::from_str_radix(s, 16).ok())
        .map_or_else(
            || {
                shared
                    .as_ref()
                    .map_or(HWND::default(), crate::settings::Shared::service_window)
            },
            |value| HWND(value as *mut _),
        );
    let class = HSTRING::from(format!(
        "LingoMate.Settings.{}.{}",
        env!("CARGO_PKG_VERSION"),
        if preview {
            format!("Preview.{}", std::process::id())
        } else {
            user_identity()?
        }
    ));
    let _instance = if preview {
        None
    } else {
        let (guard, reused) = instance(&class, section, service)?;
        if reused {
            return Ok(());
        }
        Some(guard)
    };
    let root = if preview {
        None
    } else {
        Some(super::data_root()?)
    };
    let snapshot = if preview {
        Some(Snapshot::default())
    } else {
        crate::settings::request(service, None)
    };
    let display = if preview {
        Display {
            theme: match argument("--theme") {
                Some("dark") => Theme::Dark,
                Some("system") => Theme::System,
                _ => Theme::Light,
            },
            ..Display::default()
        }
    } else {
        crate::preferences::display()
    };
    let theme = display.theme;
    let show_pos = preview || crate::preferences::load();
    let snapshot = snapshot.map(|value| effective_snapshot(Some(value), display, show_pos));
    let mut drafts = BTreeMap::new();
    drafts.insert(
        LOCAL_URL,
        "http://127.0.0.1:11434/v1/chat/completions".into(),
    );
    let mut feedback = String::new();
    let mut cloud = None;
    let mut words = Vec::new();
    if let Some(root) = &root {
        match translation::Store::new(root.clone()).load() {
            Ok(configuration) => {
                if let Some(ai) = configuration.ai {
                    drafts.insert(ENDPOINT, ai.endpoint);
                    drafts.insert(MODEL, ai.model);
                }
                if let Some(local) = configuration.local {
                    drafts.insert(LOCAL_URL, local.endpoint);
                    drafts.insert(LOCAL_MODEL, local.model);
                }
            }
            Err(error) => feedback = error_message(ErrorContext::Translation, error).into(),
        }
        let store = account::Store::new(root.clone());
        if let Ok(Some(url)) = store.url() {
            drafts.insert(ACCOUNT_URL, url);
        }
        match store.snapshot() {
            Ok(data) => cloud = Some(data),
            Err(error) => feedback = error_message(ErrorContext::Account, error).into(),
        }
        match crate::learning::Store::new(root.join("LocalWords")).snapshot() {
            Ok(data) => words = data,
            Err(_) => feedback = "本地学习记录暂不可读，原文件已保留。".into(),
        }
        if let Some(shared) = &shared {
            let mut value = shared.snapshot().unwrap_or_default();
            value.display = display;
            value.show_pos = show_pos;
            shared.set_display(value);
        }
    } else {
        let sample = AIConfiguration::minimax(false);
        drafts.insert(ENDPOINT, sample.endpoint);
        drafts.insert(MODEL, sample.model);
        drafts.insert(EMAIL, "fixture@example.com".into());
        if let Some(result) = argument("--preview-test-result") {
            feedback = connection_message(
                if result == "success" {
                    Ok(())
                } else {
                    Err(RemoteError::Denied)
                },
                Duration::from_millis(2900),
            );
        }
        words = vec![
            Word::new("study".into(), "学习".into(), "n./v.".into()),
            Word {
                english: "friend".into(),
                chinese: "朋友".into(),
                pos: "n.".into(),
                uses: 2,
                mastered: true,
            },
        ];
        if args.iter().any(|value| value == "--preview-signed-in") {
            let identity = account::Identity {
                id: "00000000-0000-4000-8000-000000000001".into(),
                session: "00000000-0000-4000-8000-000000000002".into(),
                email: "fixture@example.com".into(),
                project: "https://account.example.invalid".into(),
            };
            let mut data = account::Data::default();
            data.words.insert(identity.scope(), words.clone());
            data.active = Some(identity);
            cloud = Some(data);
        }
    }
    if render_path.is_none() {
        let _ =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    }
    let instance = unsafe { GetModuleHandleW(None) }.map_err(|_| RemoteError::Unavailable)?;
    let controls = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_WIN95_CLASSES,
    };
    let _ = unsafe { InitCommonControlsEx(&controls) };
    let definition = WNDCLASSW {
        hInstance: HINSTANCE(instance.0),
        lpszClassName: PCWSTR(class.as_ptr()),
        lpfnWndProc: Some(procedure),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.map_err(|_| RemoteError::Unavailable)?,
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&definition);
    }
    let body_class = HSTRING::from(format!("{class}.Body"));
    let body_definition = WNDCLASSW {
        hInstance: HINSTANCE(instance.0),
        lpszClassName: PCWSTR(body_class.as_ptr()),
        lpfnWndProc: Some(body_procedure),
        hCursor: definition.hCursor,
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&body_definition);
    }
    let hwnd = unsafe {
        CreateWindowExW(
            if render_path.is_some() {
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
            } else {
                WINDOW_EX_STYLE(0)
            },
            &class,
            &HSTRING::from(if preview {
                "灵果 · 设置 · 隔离界面样例"
            } else {
                "灵果 · 设置"
            }),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_VSCROLL,
            if render_path.is_some() {
                -32000
            } else {
                CW_USEDEFAULT
            },
            if render_path.is_some() {
                -32000
            } else {
                CW_USEDEFAULT
            },
            560,
            660,
            None,
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|_| RemoteError::Unavailable)?;
    let dpi = fixture_dpi.unwrap_or_else(|| appearance::dpi(hwnd));
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            px(560, dpi),
            px(660, dpi),
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let palette = appearance::palette(theme);
    let account_seen = cloud
        .as_ref()
        .and_then(|data| data.active.as_ref())
        .map_or(0, account::Identity::fingerprint);
    let window = Box::new(Window {
        hwnd: Cell::new(hwnd),
        body: Cell::new(HWND::default()),
        preview,
        fixture_dpi,
        root,
        service: Cell::new(service),
        shared,
        section: Cell::new(section),
        snapshot: Cell::new(snapshot),
        theme: Cell::new(theme),
        display: Cell::new(display),
        show_pos: Cell::new(show_pos),
        palette: Cell::new(palette),
        brush: Cell::new(unsafe { CreateSolidBrush(palette.background) }),
        fonts: RefCell::new(vec![
            font(13, dpi),
            font(20, dpi),
            font(12, dpi),
            font(22, dpi),
        ]),
        tabs: Default::default(),
        controls: Default::default(),
        roles: Default::default(),
        placements: Default::default(),
        drafts: RefCell::new(drafts),
        words: RefCell::new(words),
        visible: Default::default(),
        cloud: RefCell::new(cloud),
        feedback: RefCell::new(feedback),
        connection_feedback: Cell::new(preview && argument("--preview-test-result").is_some()),
        translation_generation: Cell::new(0),
        fixture_saved_key: preview && args.iter().any(|value| value == "--preview-key-saved"),
        busy: Cell::new(false),
        cooldown: Cell::new(None),
        scroll: Cell::new(0),
        height: Cell::new(600),
        width: Cell::new(512),
        limit: Cell::new(50),
        rebuilding: Cell::new(false),
        focus: Cell::new(None),
        list_state: RefCell::new(ListState::default()),
        account_seen: Cell::new(account_seen),
        rx,
        tx,
        deadline: preview.then(|| {
            Instant::now()
                + Duration::from_secs(
                    argument("--preview-seconds")
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(300)
                        .clamp(1, 900),
                )
        }),
    });
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&*window as *const Window) as isize);
    }
    let body = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            &body_class,
            None,
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
            16,
            108,
            512,
            480,
            Some(hwnd),
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|_| RemoteError::Unavailable)?;
    window.body.set(body);
    for (index, section) in SECTIONS.iter().enumerate() {
        let tab = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                &HSTRING::from(section.title()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
                16 + index as i32 * 104,
                52,
                100,
                44,
                Some(hwnd),
                Some(HMENU((100 + index) as *mut _)),
                Some(HINSTANCE(instance.0)),
                None,
            )
        }
        .map_err(|_| RemoteError::Unavailable)?;
        unsafe {
            SendMessageW(
                tab,
                WM_SETFONT,
                Some(WPARAM(window.font(0).0 as usize)),
                Some(LPARAM(1)),
            );
        }
        window.tabs.borrow_mut().push(tab);
    }
    // Keep the five section buttons before the body in dialog tab traversal.
    unsafe {
        let _ = SetWindowPos(
            body,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
    window.update_palette();
    window.resize();
    if preview && args.iter().any(|value| value == "--preview-scroll-bottom") {
        window.scroll.set(window.scroll_max());
        window.position_controls();
    }
    let result = if let Some(path) = &render_path {
        let foreground = unsafe { GetForegroundWindow() };
        let mut rect = RECT::default();
        let positioned = unsafe { GetWindowRect(hwnd, &mut rect) };
        if positioned.is_err() || rect.right >= 0 || rect.bottom >= 0 {
            Err(RemoteError::Unavailable)
        } else {
            // EDIT/COMBOBOX require logically visible ancestors to run their
            // complete native paint paths. This owned tool window stays far
            // outside the screen and cannot activate or receive keyboard focus.
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            let result = window.render_bitmap(path);
            if unsafe { GetForegroundWindow() } != foreground {
                Err(RemoteError::Changed)
            } else {
                result
            }
        }
    } else {
        unsafe {
            SetTimer(Some(hwnd), 1, 250, None);
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = UpdateWindow(hwnd);
            if let Some(tab) = window.tabs.borrow().get(section.index()).copied() {
                let _ = SetFocus(Some(tab));
            }
        }
        let mut message = MSG::default();
        let mut focused = unsafe { GetFocus() };
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
            if !unsafe { IsDialogMessageW(hwnd, &message) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            let current = unsafe { GetFocus() };
            if current != focused {
                // Scroll only when focus changes, so wheel scrolling stays under
                // the user's control while an edit field retains its focus.
                window.ensure_focus_visible();
                focused = current;
            }
        }
        Ok(())
    };
    window.clear_input_scopes();
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let _ = DestroyWindow(hwnd);
        window.hwnd.set(HWND::default());
        window.body.set(HWND::default());
        window.controls.borrow_mut().clear();
        window.tabs.borrow_mut().clear();
        let _ = UnregisterClassW(&class, Some(HINSTANCE(instance.0)));
        let _ = UnregisterClassW(&body_class, Some(HINSTANCE(instance.0)));
    }
    drop(window);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_repaints_only_after_all_siblings_reach_their_new_positions() {
        struct Probe {
            children: [HWND; 2],
            expected_y: [i32; 2],
            paints: Cell<u32>,
            partial_layout: Cell<bool>,
        }
        unsafe extern "system" fn observe(
            window: HWND,
            message: u32,
            w: WPARAM,
            l: LPARAM,
        ) -> LRESULT {
            if message == WM_PAINT {
                if let Ok(parent) = unsafe { GetParent(window) } {
                    let pointer =
                        unsafe { GetWindowLongPtrW(parent, GWLP_USERDATA) } as *const Probe;
                    if !pointer.is_null() {
                        let probe = unsafe { &*pointer };
                        probe.paints.set(probe.paints.get() + 1);
                        for (child, expected_y) in probe.children.iter().zip(probe.expected_y) {
                            let mut rect = RECT::default();
                            let _ = unsafe { GetWindowRect(*child, &mut rect) };
                            let mut point = POINT {
                                x: rect.left,
                                y: rect.top,
                            };
                            let _ = unsafe { ScreenToClient(parent, &mut point) };
                            if point.y != expected_y {
                                probe.partial_layout.set(true);
                            }
                        }
                    }
                }
                let mut paint = PAINTSTRUCT::default();
                let dc = unsafe { BeginPaint(window, &mut paint) };
                fill(dc, paint.rcPaint, COLORREF(0xffffff));
                let _ = unsafe { EndPaint(window, &paint) };
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(window, message, w, l) }
        }
        let foreground = unsafe { GetForegroundWindow() };
        let _apartment = Apartment::open().unwrap();
        let class = HSTRING::from(format!(
            "LingoMate.Settings.ScrollTest.{}",
            crate::storage::nonce()
        ));
        let module = unsafe { GetModuleHandleW(None) }.unwrap();
        let definition = WNDCLASSW {
            hInstance: HINSTANCE(module.0),
            lpszClassName: PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(observe),
            ..Default::default()
        };
        assert_ne!(unsafe { RegisterClassW(&definition) }, 0);
        let parent = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                &class,
                None,
                WS_POPUP | WS_CLIPCHILDREN,
                -32000,
                -32000,
                400,
                400,
                None,
                None,
                Some(HINSTANCE(module.0)),
                None,
            )
        }
        .unwrap();
        let mut checks = Vec::new();
        for dpi in [96, 144] {
            let children: [HWND; 2] = [100, 200].map(|y| unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    &class,
                    None,
                    WS_CHILD | WS_VISIBLE,
                    20,
                    px(y, dpi),
                    200,
                    px(44, dpi),
                    Some(parent),
                    None,
                    Some(HINSTANCE(module.0)),
                    None,
                )
                .unwrap()
            });
            let probe = Box::new(Probe {
                children,
                expected_y: [px(48, dpi), px(148, dpi)],
                paints: Cell::new(0),
                partial_layout: Cell::new(false),
            });
            unsafe {
                let _ = ShowWindow(parent, SW_SHOWNOACTIVATE);
                let _ = RedrawWindow(
                    Some(parent),
                    None,
                    None,
                    RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW,
                );
                SetWindowLongPtrW(parent, GWLP_USERDATA, (&*probe as *const Probe) as isize);
            }
            position_children(
                parent,
                &[
                    (children[0], 20, 100, 200, 44),
                    (children[1], 20, 200, 200, 44),
                ],
                52,
                dpi,
            );
            checks.push((dpi, probe.paints.get(), probe.partial_layout.get()));
            unsafe {
                SetWindowLongPtrW(parent, GWLP_USERDATA, 0);
                for child in children {
                    let _ = DestroyWindow(child);
                }
            }
        }
        unsafe {
            let _ = DestroyWindow(parent);
            let _ = UnregisterClassW(&class, Some(HINSTANCE(module.0)));
        }
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
        for (dpi, paints, partial_layout) in checks {
            assert!(paints > 0, "scrolling did not repaint at {dpi} DPI");
            assert!(
                !partial_layout,
                "scrolling painted a mixture of old and new sibling positions at {dpi} DPI"
            );
        }
    }

    #[test]
    fn export_offscreen_settings_preview_artifacts_when_requested() {
        let Some(directory) = std::env::var_os("LINGOMATE_SETTINGS_PREVIEW_DIR") else {
            return;
        };
        let directory = PathBuf::from(directory);
        assert!(
            directory.is_absolute(),
            "preview artifact directory must be absolute"
        );
        std::fs::create_dir_all(&directory).unwrap();
        for dpi in [96, 144] {
            for theme in ["light", "dark"] {
                for section in [
                    "account",
                    "translation",
                    "input",
                    "appearance",
                    "learning",
                    "local",
                    "account-signed-in",
                    "translation-saved",
                    "translation-tested",
                    "translation-failed",
                ] {
                    let path = directory.join(format!("{section}-{theme}-{dpi}.bmp"));
                    let mut args = vec![
                        "--preview".into(),
                        "--section".into(),
                        if section.starts_with("translation-") {
                            "translation"
                        } else if section == "account-signed-in" {
                            "account"
                        } else {
                            section
                        }
                        .into(),
                        "--theme".into(),
                        theme.into(),
                        "--preview-dpi".into(),
                        dpi.to_string(),
                        "--preview-render".into(),
                        path.to_str().unwrap().into(),
                    ];
                    if section == "account-signed-in" {
                        args.push("--preview-signed-in".into());
                    }
                    if section.starts_with("translation-") {
                        args.push("--preview-key-saved".into());
                    }
                    if section == "translation-tested" || section == "translation-failed" {
                        args.push("--preview-scroll-bottom".into());
                        args.extend([
                            "--preview-test-result".into(),
                            if section == "translation-tested" {
                                "success"
                            } else {
                                "failure"
                            }
                            .into(),
                        ]);
                    }
                    // A fresh offscreen owned thread/apartment each time; no global DPI,
                    // source selection, production settings or foreign HWNDs.
                    std::thread::spawn(move || run(&args))
                        .join()
                        .unwrap()
                        .unwrap();
                    let bytes = std::fs::read(path).unwrap();
                    assert_eq!(&bytes[..2], b"BM");
                    assert!(bytes.len() > 54);
                    let pixels = &bytes[54..];
                    assert!(
                        pixels
                            .chunks_exact(4)
                            .any(|pixel| pixel[..3] != pixels[..3]),
                        "native settings preview painted only a flat background"
                    );
                }
            }
        }
    }

    #[test]
    fn reopen_accepts_only_known_sections_and_keeps_local_distinct() {
        for section in SECTIONS.into_iter().chain([Section::Local]) {
            assert!(Section::from_packet(section.packet()) == Some(section));
        }
        assert_eq!(Section::Local.index(), Section::Translation.index());
        assert!(Section::from_packet(6).is_none());
        assert!(Section::from_packet(usize::MAX).is_none());
    }
    #[test]
    fn saved_key_badge_is_separate_from_the_edit_value_and_test_status_is_explicit() {
        assert!(key_status_text(Ok(true), false).contains("已保存 · ••••••••"));
        assert!(key_status_text(Ok(true), true).contains("待保存"));
        assert!(key_status_text(Ok(false), false).contains("尚未保存"));
        assert!(key_status_text(Err(RemoteError::Storage), false).contains("暂不可读"));
        assert!(
            connection_message(Ok(()), Duration::from_millis(2900)).contains("连接成功 · 2.9 秒")
        );
        assert!(
            connection_message(Err(RemoteError::Denied), Duration::ZERO).starts_with("连接失败：")
        );
    }

    #[test]
    fn saved_display_choices_survive_an_absent_or_stale_input_service() {
        let display = Display {
            theme: Theme::Dark,
            font: 22,
        };
        for runtime in [
            None,
            Some(Snapshot {
                english: true,
                remember: false,
                ..Snapshot::default()
            }),
        ] {
            let value = effective_snapshot(runtime, display, false);
            assert_eq!(value.display, display);
            assert!(!value.show_pos);
            if let Some(runtime) = runtime {
                assert_eq!(value.english, runtime.english);
                assert_eq!(value.remember, runtime.remember);
                assert_eq!(value.pending, runtime.pending);
            }
        }
    }

    #[test]
    fn account_errors_name_the_account_flow_and_preserve_retry_guidance() {
        assert!(error_message(ErrorContext::Account, RemoteError::Network).contains("账号服务"));
        assert!(error_message(ErrorContext::Account, RemoteError::Denied).contains("验证码"));
        assert!(error_message(ErrorContext::Account, RemoteError::MissingKey).contains("登录凭据"));
        assert!(
            error_message(ErrorContext::Account, RemoteError::Changed).contains("本次操作未提交")
        );
        for error in [
            RemoteError::Configuration,
            RemoteError::MissingKey,
            RemoteError::Denied,
            RemoteError::Quota,
            RemoteError::Network,
            RemoteError::InvalidResponse,
            RemoteError::Changed,
            RemoteError::Storage,
            RemoteError::Unavailable,
            RemoteError::Packaged,
            RemoteError::LocalSafety,
        ] {
            let message = error_message(ErrorContext::Account, error);
            assert!(
                !message.contains("译文") && !message.contains("翻译") && !message.contains("API")
            );
        }
    }

    #[test]
    fn synthetic_settings_instance_forwards_section_and_origin_without_opening_another_window() {
        use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
        unsafe extern "system" fn receive(
            window: HWND,
            message: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            if message == REOPEN {
                let receipt = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) }
                    as *const (AtomicUsize, AtomicIsize);
                if !receipt.is_null() {
                    let receipt = unsafe { &*receipt };
                    receipt.0.store(wparam.0, Ordering::SeqCst);
                    receipt.1.store(lparam.0, Ordering::SeqCst);
                    return LRESULT(REOPENED);
                }
            }
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
        let _apartment = Apartment::open().unwrap();
        let class = HSTRING::from(format!(
            "LingoMate.Settings.Test.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (_owner, reused) = instance(&class, Section::Account, HWND::default()).unwrap();
        assert!(!reused);
        let module = unsafe { GetModuleHandleW(None) }.unwrap();
        let definition = WNDCLASSW {
            hInstance: HINSTANCE(module.0),
            lpszClassName: PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(receive),
            ..Default::default()
        };
        assert_ne!(unsafe { RegisterClassW(&definition) }, 0);
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                &class,
                None,
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(HINSTANCE(module.0)),
                None,
            )
        }
        .unwrap();
        let receipt = (AtomicUsize::new(usize::MAX), AtomicIsize::new(0));
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, (&receipt as *const _) as isize);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let other_class = class.clone();
        let other = std::thread::spawn(move || {
            let result = instance(&other_class, Section::Local, HWND(0x1234usize as *mut _))
                .map(|(_instance, reused)| reused);
            let _ = tx.send(result);
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Ok(result) = rx.try_recv() {
                break result;
            }
            assert!(
                Instant::now() < deadline,
                "synthetic settings reopen timed out"
            );
            let mut message = MSG::default();
            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    DispatchMessageW(&message);
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        other.join().unwrap();
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            DestroyWindow(window).unwrap();
            UnregisterClassW(&class, Some(HINSTANCE(module.0))).unwrap();
        }
        assert!(result.unwrap());
        assert_eq!(receipt.0.load(Ordering::SeqCst), Section::Local.packet());
        assert_eq!(receipt.1.load(Ordering::SeqCst), 0x1234);
    }
}
