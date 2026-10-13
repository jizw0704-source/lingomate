use super::{Guard, Key, MODULE, State, down, updates};
use crate::{
    appearance::{self, px},
    candidate_layout::{BoxRect, Layout},
};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, Result, w};
pub(super) struct Panel {
    window: HWND,
    class_name: HSTRING,
    owner: *mut Weak<State>,
    caret: Cell<POINT>,
    pressed: Cell<Option<(i32, i32)>>,
    pressed_revision: Cell<u64>,
    limit: Cell<Option<i32>>,
    hovered: Cell<Option<(i32, i32)>>,
    tracking: Cell<bool>,
    palette: Cell<appearance::Palette>,
    bounds: Cell<Option<[i32; 4]>>,
    visible: Cell<bool>,
    scroll: Cell<i32>,
    selection: Cell<Option<(u64, usize, usize, bool)>>,
    tooltip: Option<HWND>,
    tips: RefCell<Vec<(BoxRect, HSTRING)>>,
    _guard: Guard,
}
fn work_area(x: i32, y: i32) -> RECT {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        info.rcWork
    } else {
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        }
    }
}
fn layout(window: HWND, state: &State) -> Layout {
    let input = state.input.borrow();
    let empty = input.error || input.frame.candidates.is_empty();
    let count = input
        .frame
        .candidates
        .len()
        .saturating_sub(input.page() * 5)
        .min(5);
    let extra = if input.expanded && !empty {
        input
            .frame
            .candidates
            .get(input.selected)
            .map_or(0, |c| c.translations.len())
    } else {
        0
    };
    let point = state
        .panel
        .borrow()
        .as_ref()
        .map_or(POINT { x: 40, y: 40 }, |p| p.caret.get());
    let work = work_area(point.x, point.y);
    let lengths: Vec<_> = input
        .frame
        .candidates
        .iter()
        .skip(input.page() * 5)
        .take(if empty { 0 } else { count })
        .map(|c| c.text.chars().count())
        .collect();
    let layout = Layout::with_lengths(
        &lengths,
        extra,
        state.display.get().font,
        appearance::dpi(window),
        state
            .panel
            .borrow()
            .as_ref()
            .and_then(|p| p.limit.get())
            .unwrap_or(work.right - work.left - 8),
    );
    layout.with_viewport(
        work.bottom - work.top - px(16, appearance::dpi(window)),
        state
            .panel
            .borrow()
            .as_ref()
            .map_or(0, |panel| panel.scroll.get()),
    )
}
impl Panel {
    pub fn window(&self) -> HWND {
        self.window
    }
    pub fn new(state: Weak<State>) -> Result<Self> {
        let instance = HINSTANCE(MODULE.load(Ordering::SeqCst));
        // A pinned previous DLL can still be resident after an upgrade. Its
        // window procedure must never interpret this module's Weak<State>.
        let class_name = HSTRING::from(format!(
            "LingoMate.Candidates.{}.{:X}",
            env!("CARGO_PKG_VERSION"),
            instance.0 as usize
        ));
        let theme = state
            .upgrade()
            .map_or(crate::preferences::Theme::Light, |state| {
                state.display.get().theme
            });
        let palette = appearance::palette(theme);
        let class = WNDCLASSW {
            style: CS_DROPSHADOW,
            hInstance: instance,
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(procedure),
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
            ..Default::default()
        };
        unsafe {
            RegisterClassW(&class);
        }
        let window = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                class.lpszClassName,
                w!("灵果候选"),
                WS_POPUP,
                40,
                40,
                640,
                132,
                None,
                None,
                Some(instance),
                None,
            )?
        };
        let live = state.upgrade().is_some_and(|state| !state.preview);
        let owner = Box::into_raw(Box::new(state));
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, owner as isize);
            if live {
                SetTimer(Some(window), 2, 20, None);
            }
        }
        let tooltip = unsafe {
            let controls = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_WIN95_CLASSES,
            };
            let _ = InitCommonControlsEx(&controls);
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                TOOLTIPS_CLASSW,
                None,
                WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX | TTS_NOANIMATE | TTS_NOFADE),
                0,
                0,
                0,
                0,
                Some(window),
                None,
                Some(instance),
                None,
            )
            .ok()
        };
        Ok(Self {
            window,
            class_name,
            owner,
            caret: Cell::new(POINT { x: 40, y: 40 }),
            pressed: Cell::new(None),
            pressed_revision: Cell::new(0),
            limit: Cell::new(None),
            hovered: Cell::new(None),
            tracking: Cell::new(false),
            palette: Cell::new(palette),
            bounds: Cell::new(None),
            visible: Cell::new(false),
            scroll: Cell::new(0),
            selection: Cell::new(None),
            tooltip,
            tips: RefCell::new(Vec::new()),
            _guard: Guard::new(),
        })
    }
    pub fn set_palette(&self, palette: appearance::Palette) {
        self.palette.set(palette);
    }
    pub fn update(&self) {
        let Some(state) = unsafe { &*self.owner }.upgrade() else {
            self.hide();
            return;
        };
        if state.input.borrow().raw.is_empty() {
            self.hide();
            return;
        }
        let key = {
            let input = state.input.borrow();
            (
                input.frame.generation,
                input.selected,
                input.sense,
                input.expanded,
            )
        };
        let previous = self.selection.replace(Some(key));
        if previous != Some(key) {
            if previous.is_none_or(|old| old.0 != key.0) || !key.3 {
                self.scroll.set(0);
            }
            let l = layout(self.window, &state);
            if key.3 && l.extra > 0 {
                let r = l.sense_rect(key.2);
                let bottom = l.height() - l.padding;
                let adjustment = if r.y < l.body_rect().y {
                    r.y - l.body_rect().y
                } else {
                    (r.y + r.height - bottom).max(0)
                };
                self.scroll
                    .set((l.scroll_offset() + adjustment).clamp(0, l.scroll_max()));
            }
        }
        let l = layout(self.window, &state);
        self.scroll.set(l.scroll_offset());
        self.set_tips(tooltip_texts(&state, l));
        let point = self.caret.get();
        let work = work_area(point.x, point.y);
        let x = point
            .x
            .clamp(work.left, (work.right - l.width() - 2).max(work.left));
        let below = point.y + px(8, appearance::dpi(self.window));
        let y = if below + l.height() + 2 <= work.bottom {
            below
        } else {
            point.y - l.height() - px(8, appearance::dpi(self.window))
        }
        .clamp(work.top, (work.bottom - l.height() - 2).max(work.top));
        unsafe {
            let bounds = [x, y, l.width(), l.height()];
            if self.bounds.get() != Some(bounds) {
                let _ = SetWindowPos(
                    self.window,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    l.width(),
                    l.height(),
                    SWP_NOACTIVATE,
                );
                self.bounds.set(Some(bounds));
                // The owned region is replaced only when geometry changes.
                let radius = px(32, appearance::dpi(self.window));
                let region =
                    CreateRoundRectRgn(0, 0, l.width() + 1, l.height() + 1, radius, radius);
                if SetWindowRgn(self.window, Some(region), true) == 0 {
                    let _ = DeleteObject(region.into());
                }
            }
            let _ = InvalidateRect(Some(self.window), None, false);
            if !self.visible.replace(true) {
                let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE);
            }
        }
    }
    pub fn position(&self, x: i32, y: i32) {
        self.caret.set(POINT { x, y });
        self.update();
    }
    pub fn hide(&self) {
        self.scroll.set(0);
        self.selection.set(None);
        if let Some(tooltip) = self.tooltip {
            unsafe {
                SendMessageW(tooltip, TTM_POP, None, None);
            }
        }
        self.pressed.set(None);
        self.hovered.set(None);
        if self.tracking.replace(false) {
            let mut track = windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<
                    windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT,
                >() as u32,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::TME_CANCEL
                    | windows::Win32::UI::Input::KeyboardAndMouse::TME_LEAVE,
                hwndTrack: self.window,
                ..Default::default()
            };
            let _ =
                unsafe { windows::Win32::UI::Input::KeyboardAndMouse::TrackMouseEvent(&mut track) };
        }
        if unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetCapture() } == self.window {
            let _ = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture() };
        }
        unsafe {
            if self.visible.replace(false) {
                let _ = ShowWindow(self.window, SW_HIDE);
            }
        }
    }
    fn set_tips(&self, items: Vec<(BoxRect, HSTRING)>) {
        let Some(window) = self.tooltip else {
            return;
        };
        if *self.tips.borrow() == items {
            return;
        }
        let mut tips = self.tips.borrow_mut();
        unsafe {
            SendMessageW(window, TTM_POP, None, None);
            for id in 1..=tips.len() {
                let tool = TTTOOLINFOW {
                    cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
                    hwnd: self.window,
                    uId: id,
                    ..Default::default()
                };
                SendMessageW(
                    window,
                    TTM_DELTOOLW,
                    None,
                    Some(LPARAM((&tool as *const TTTOOLINFOW) as isize)),
                );
            }
            *tips = items;
            for (i, (r, content)) in tips.iter().enumerate() {
                let tool = TTTOOLINFOW {
                    cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
                    hwnd: self.window,
                    uId: i + 1,
                    rect: rect(*r),
                    lpszText: windows::core::PWSTR(content.as_ptr().cast_mut()),
                    ..Default::default()
                };
                SendMessageW(
                    window,
                    TTM_ADDTOOLW,
                    None,
                    Some(LPARAM((&tool as *const TTTOOLINFOW) as isize)),
                );
            }
            SendMessageW(
                window,
                TTM_SETMAXTIPWIDTH,
                None,
                Some(LPARAM(px(480, appearance::dpi(self.window)) as isize)),
            );
        }
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        unsafe {
            if let Some(tooltip) = self.tooltip {
                let _ = DestroyWindow(tooltip);
            }
            let _ = KillTimer(Some(self.window), 2);
            SetWindowLongPtrW(self.window, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.window);
            drop(Box::from_raw(self.owner));
            let _ = UnregisterClassW(
                &self.class_name,
                Some(HINSTANCE(MODULE.load(Ordering::SeqCst))),
            );
        }
    }
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
    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const Weak<State>;
    let state = if pointer.is_null() {
        None
    } else {
        unsafe { &*pointer }.upgrade()
    };
    if let Some(state) = state {
        // Explicit relay avoids subclassing the candidate window or installing
        // callbacks in the host. Literal tip buffers live until each tool is removed.
        if matches!(
            message,
            WM_MOUSEMOVE
                | WM_LBUTTONDOWN
                | WM_LBUTTONUP
                | WM_RBUTTONDOWN
                | WM_RBUTTONUP
                | WM_MBUTTONDOWN
                | WM_MBUTTONUP
        ) {
            let tooltip = state
                .panel
                .borrow()
                .as_ref()
                .and_then(|panel| panel.tooltip);
            if let Some(tooltip) = tooltip {
                let mut point = POINT {
                    x: (lparam.0 as u16) as i16 as i32,
                    y: ((lparam.0 >> 16) as u16) as i16 as i32,
                };
                unsafe {
                    let _ = ClientToScreen(window, &mut point);
                    let event = MSG {
                        hwnd: window,
                        message,
                        wParam: wparam,
                        lParam: lparam,
                        time: GetMessageTime() as u32,
                        pt: point,
                    };
                    SendMessageW(
                        tooltip,
                        TTM_RELAYEVENT,
                        None,
                        Some(LPARAM((&event as *const MSG) as isize)),
                    );
                }
            }
        }
        if message == WM_TIMER && wparam.0 == 2 {
            if state.poll() {
                State::reap();
            }
            return LRESULT(0);
        }
        if matches!(
            message,
            WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED | WM_DPICHANGED
        ) {
            state.refresh_palette();
            state.refresh();
            return LRESULT(0);
        }
        if message == WM_PAINT {
            paint(window, &state, None);
            return LRESULT(0);
        }
        if message == WM_PRINTCLIENT {
            paint(window, &state, Some(HDC(wparam.0 as *mut _)));
            return LRESULT(0);
        }
        if message == WM_ERASEBKGND {
            return LRESULT(1);
        }
        if message == WM_MOUSEMOVE || message == windows::Win32::UI::Controls::WM_MOUSELEAVE {
            let l = layout(window, &state);
            if let Some(panel) = state.panel.borrow().as_ref() {
                let point = if message == windows::Win32::UI::Controls::WM_MOUSELEAVE {
                    if let Some(tooltip) = panel.tooltip {
                        unsafe {
                            SendMessageW(tooltip, TTM_POP, None, None);
                        }
                    }
                    panel.tracking.set(false);
                    None
                } else {
                    Some((
                        (lparam.0 as u16) as i16 as i32,
                        ((lparam.0 >> 16) as u16) as i16 as i32,
                    ))
                };
                let old = panel.hovered.replace(point);
                if message == WM_MOUSEMOVE && !panel.tracking.get() {
                    let mut track = windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<
                            windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT,
                        >() as u32,
                        dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::TME_LEAVE,
                        hwndTrack: window,
                        dwHoverTime: 0,
                    };
                    if unsafe {
                        windows::Win32::UI::Input::KeyboardAndMouse::TrackMouseEvent(&mut track)
                    }
                    .is_ok()
                    {
                        panel.tracking.set(true);
                    }
                }
                if (
                    old.and_then(|(x, y)| l.hit(x, y)),
                    old.and_then(|(x, y)| header_hit(l, x, y)),
                    old.and_then(|(x, y)| l.sense_hit(x, y)),
                ) != (
                    point.and_then(|(x, y)| l.hit(x, y)),
                    point.and_then(|(x, y)| header_hit(l, x, y)),
                    point.and_then(|(x, y)| l.sense_hit(x, y)),
                ) {
                    let _ = unsafe { InvalidateRect(Some(window), None, false) };
                }
            }
            return LRESULT(0);
        }
        if message == WM_MOUSEWHEEL {
            let l = layout(window, &state);
            if l.scroll_max() > 0 {
                if let Some(panel) = state.panel.borrow().as_ref() {
                    let delta = (wparam.0 >> 16) as u16 as i16 as i32;
                    panel.pressed.set(None);
                    panel.scroll.set(
                        (l.scroll_offset() - delta * px(88, appearance::dpi(window)) / 120)
                            .clamp(0, l.scroll_max()),
                    );
                    panel.update();
                }
                return LRESULT(0);
            }
            return LRESULT(0);
        }
        if message == WM_CAPTURECHANGED {
            if let Some(panel) = state.panel.borrow().as_ref() {
                panel.pressed.set(None);
            }
            return LRESULT(0);
        }
        if message == WM_LBUTTONDOWN {
            unsafe {
                windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(window);
            }
            let _ = unsafe { InvalidateRect(Some(window), None, false) };
            if let Some(panel) = state.panel.borrow().as_ref() {
                panel.pressed_revision.set(state.displayed.get());
                panel.pressed.set(Some((
                    (lparam.0 as u16) as i16 as i32,
                    ((lparam.0 >> 16) as u16) as i16 as i32,
                )));
            }
            return LRESULT(0);
        }
        if message == WM_LBUTTONUP {
            let x = (lparam.0 as u16) as i16 as i32;
            let y = ((lparam.0 >> 16) as u16) as i16 as i32;
            let pressed = state.panel.borrow().as_ref().and_then(|p| p.pressed.take());
            let same_revision = state
                .panel
                .borrow()
                .as_ref()
                .is_some_and(|p| p.pressed_revision.get() == state.displayed.get());
            if unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetCapture() } == window {
                let _ = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture() };
            }
            let _ = unsafe { InvalidateRect(Some(window), None, false) };
            if state.preview {
                return LRESULT(0);
            }
            let l = layout(window, &state);
            if pressed.is_none() {
                return LRESULT(0);
            }
            if (!same_revision || state.busy.get()) && header_hit(l, x, y) != Some(4) {
                return LRESULT(0);
            }
            if let Some(item) = header_hit(l, x, y) {
                if pressed.and_then(|(px, py)| header_hit(l, px, py)) == Some(item) {
                    match item {
                        0 => state.click(Key::Previous),
                        2 => state.click(Key::Next),
                        3 => details(window, &state),
                        4 => settings(window, &state),
                        _ => {}
                    }
                }
            } else if let Some((slot, english)) = l.hit(x, y) {
                if pressed.and_then(|(x, y)| l.hit(x, y)) == Some((slot, english)) {
                    let mut input = state.input.borrow_mut();
                    input.selected = input.page() * 5 + slot;
                    input.sense = 0;
                    drop(input);
                    state.click(Key::Space(english));
                }
            } else if let Some(row) = l.sense_hit(x, y)
                && pressed.is_some_and(|(px, py)| l.visible(l.sense_rect(row)).contains(px, py))
            {
                let mut input = state.input.borrow_mut();
                if input.expanded {
                    input.sense = row;
                    drop(input);
                    state.click(Key::Space(true));
                }
            }
            return LRESULT(0);
        }
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
fn font(size: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -size,
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
            ANTIALIASED_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Microsoft YaHei UI"),
        )
    }
}
fn fill(dc: HDC, rect: &RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(dc, rect, brush);
        let _ = DeleteObject(brush.into());
    }
}
struct PaintSession {
    window: HWND,
    dc: HDC,
    paint: PAINTSTRUCT,
    owned: bool,
}
impl PaintSession {
    fn begin(window: HWND, print: Option<HDC>) -> Self {
        let mut paint = PAINTSTRUCT::default();
        let dc = print.unwrap_or_else(|| unsafe { BeginPaint(window, &mut paint) });
        if print.is_some() {
            let _ = unsafe { GetClientRect(window, &mut paint.rcPaint) };
        }
        Self {
            window,
            dc,
            paint,
            owned: print.is_none(),
        }
    }
}
impl Drop for PaintSession {
    fn drop(&mut self) {
        if self.owned {
            let _ = unsafe { EndPaint(self.window, &self.paint) };
        }
    }
}
fn rect(r: BoxRect) -> RECT {
    RECT {
        left: r.x,
        top: r.y,
        right: r.x + r.width,
        bottom: r.y + r.height,
    }
}
fn rounded(dc: HDC, r: BoxRect, radius: i32, surface: COLORREF, border: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(surface);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(dc, brush.into());
        let old_pen = SelectObject(dc, pen.into());
        let _ = RoundRect(dc, r.x, r.y, r.x + r.width, r.y + r.height, radius, radius);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        let _ = DeleteObject(pen.into());
        let _ = DeleteObject(brush.into());
    }
}
fn insets(r: BoxRect, amount: i32) -> BoxRect {
    BoxRect {
        x: r.x + amount,
        width: (r.width - amount * 2).max(1),
        ..r
    }
}
fn label(dc: HDC, content: &str, r: BoxRect, color: COLORREF, wrap: bool) {
    if content.is_empty() || r.width <= 0 || r.height <= 0 {
        return;
    }
    let mut r = rect(r);
    let mut content: Vec<_> = content.encode_utf16().collect();
    unsafe {
        SetTextColor(dc, color);
        let flags = if wrap {
            DT_WORDBREAK | DT_EDITCONTROL
        } else {
            DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS
        };
        DrawTextW(dc, &mut content, &mut r, DT_LEFT | DT_NOPREFIX | flags);
    }
}
fn centered(dc: HDC, content: &str, r: BoxRect, color: COLORREF) {
    if content.is_empty() || r.width <= 0 || r.height <= 0 {
        return;
    }
    let mut r = rect(r);
    let mut content: Vec<_> = content.encode_utf16().collect();
    unsafe {
        SetTextColor(dc, color);
        DrawTextW(
            dc,
            &mut content,
            &mut r,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }
}
struct Fonts {
    dc: HDC,
    original: HGDIOBJ,
    main: HFONT,
    english: HFONT,
    header: HFONT,
    small: HFONT,
    detail: HFONT,
    spacing: i32,
}
impl Fonts {
    fn new(dc: HDC, l: Layout, dpi: i32) -> Self {
        let main = font(l.chinese_font());
        let original = unsafe { SelectObject(dc, main.into()) };
        Self {
            dc,
            original,
            main,
            english: font(l.english_font()),
            header: font(px(14, dpi)),
            small: font(px(11, dpi)),
            detail: font(px(16, dpi)),
            spacing: px(6, dpi),
        }
    }
    fn select(&self, value: HFONT) {
        unsafe {
            SelectObject(self.dc, value.into());
        }
    }
}
impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.original);
            for value in [
                self.main,
                self.english,
                self.header,
                self.small,
                self.detail,
            ] {
                let _ = DeleteObject(value.into());
            }
        }
    }
}
fn translation(
    dc: HDC,
    fonts: &Fonts,
    word: &str,
    pos: &str,
    r: BoxRect,
    palette: appearance::Palette,
    detail: bool,
) {
    if word.is_empty() {
        return;
    }
    let face = if detail { fonts.detail } else { fonts.english };
    fonts.select(fonts.small);
    let suffix: Vec<_> = pos.encode_utf16().collect();
    let mut suffix_size = SIZE::default();
    if !suffix.is_empty() {
        unsafe {
            let _ = GetTextExtentPoint32W(dc, &suffix, &mut suffix_size);
        }
    }
    fonts.select(face);
    let word_text: Vec<_> = word.encode_utf16().collect();
    let mut word_size = SIZE::default();
    unsafe {
        let _ = GetTextExtentPoint32W(dc, &word_text, &mut word_size);
    }
    let reserve = if pos.is_empty() {
        0
    } else {
        suffix_size.cx + fonts.spacing
    };
    let word_width = (r.width - reserve).max(1);
    label(
        dc,
        word,
        BoxRect {
            width: word_width,
            ..r
        },
        palette.accent,
        false,
    );
    if !pos.is_empty() {
        fonts.select(fonts.small);
        let x = r.x + word_size.cx.min(word_width) + fonts.spacing;
        label(
            dc,
            pos,
            BoxRect {
                x,
                width: (r.x + r.width - x).max(1),
                ..r
            },
            palette.secondary,
            false,
        );
    }
}
fn paint(window: HWND, state: &Rc<State>, print: Option<HDC>) {
    let session = PaintSession::begin(window, print);
    let dc = session.dc;
    let dpi = appearance::dpi(window);
    let palette = state.panel.borrow().as_ref().map_or_else(
        || appearance::palette(crate::preferences::Theme::Light),
        |panel| panel.palette.get(),
    );
    let l = layout(window, state);
    let fonts = Fonts::new(dc, l, dpi);
    fill(dc, &session.paint.rcPaint, palette.background);
    rounded(
        dc,
        BoxRect {
            x: 0,
            y: 0,
            width: l.width(),
            height: l.height(),
        },
        px(32, dpi),
        palette.background,
        palette.divider,
    );
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        SetBkColor(dc, palette.background);
    }
    let hovered = state.panel.borrow().as_ref().and_then(|p| p.hovered.get());
    let pressed = state.panel.borrow().as_ref().and_then(|p| p.pressed.get());
    let input = state.input.borrow();
    let empty = input.error || input.frame.candidates.is_empty();
    fonts.select(fonts.header);
    label(dc, &input.raw, l.title_rect(), palette.foreground, false);
    fonts.select(fonts.small);
    centered(
        dc,
        if input.english { "英文" } else { "中英" },
        l.mode_rect(),
        palette.secondary,
    );
    for (i, value) in [
        "‹".to_owned(),
        format!("{}/{}", input.page() + 1, input.page_count().max(1)),
        "›".into(),
        if input.expanded { "收起" } else { "译法" }.into(),
        "⚙".into(),
    ]
    .iter()
    .enumerate()
    {
        let r = l.header_item(i);
        let enabled = match i {
            0 => !empty && input.page() > 0,
            2 => !empty && input.page() + 1 < input.page_count(),
            3 => !empty,
            _ => true,
        };
        if i != 1 && enabled && hovered.is_some_and(|(x, y)| r.contains(x, y)) {
            rounded(
                dc,
                r,
                px(20, dpi),
                if pressed.is_some_and(|(x, y)| r.contains(x, y)) {
                    palette.divider
                } else {
                    palette.selected
                },
                palette.selected,
            );
        }
        fonts.select(if i == 1 { fonts.small } else { fonts.header });
        centered(
            dc,
            value,
            r,
            if enabled {
                palette.foreground
            } else {
                palette.secondary
            },
        );
    }
    let saved_body = unsafe { SaveDC(dc) };
    unsafe {
        let _ = IntersectClipRect(dc, 0, l.body_rect().y, l.width(), l.height() - l.padding);
    }
    if empty {
        fonts.select(fonts.main);
        let message = if state.busy.get() {
            "正在加载候选…".to_owned()
        } else if input.error {
            state
                .failure
                .get()
                .unwrap_or(crate::bridge::EngineFailure::Unavailable)
                .description()
        } else {
            "未找到候选".to_owned()
        };
        label(
            dc,
            &message,
            BoxRect {
                x: l.padding,
                y: l.body_rect().y,
                width: l.width() - 2 * l.padding,
                height: l.row,
            },
            palette.foreground,
            false,
        );
        fonts.select(fonts.english);
        label(
            dc,
            "回车可提交原样字母",
            BoxRect {
                x: l.padding,
                y: l.body_rect().y + l.row,
                width: l.width() - 2 * l.padding,
                height: l.row,
            },
            palette.secondary,
            false,
        );
    }
    for (slot, candidate) in input
        .frame
        .candidates
        .iter()
        .skip(input.page() * 5)
        .take(if empty { 0 } else { 5 })
        .enumerate()
    {
        for english in [false, true] {
            let r = l.candidate_rect(slot, english);
            let hot = hovered.and_then(|(x, y)| l.hit(x, y)) == Some((slot, english));
            let down = pressed.and_then(|(x, y)| l.hit(x, y)) == Some((slot, english));
            let selected = !english && input.selected == input.page() * 5 + slot;
            if selected || hot || down {
                rounded(
                    dc,
                    r,
                    px(20, dpi),
                    if down {
                        palette.divider
                    } else {
                        palette.selected
                    },
                    palette.selected,
                );
            }
            let inner = insets(r, px(8, dpi));
            if !english {
                fonts.select(fonts.main);
                let foreground = if selected {
                    palette.selected_text
                } else {
                    palette.foreground
                };
                let title = format!("{} {}", slot + 1, candidate.text);
                if l.compact && r.height == l.row {
                    label(dc, &title, inner, foreground, false);
                } else {
                    label(
                        dc,
                        &title,
                        BoxRect {
                            y: inner.y + px(10, dpi),
                            height: inner.height - px(10, dpi),
                            ..inner
                        },
                        foreground,
                        true,
                    );
                }
            } else if let Some(sense) = candidate.translations.first() {
                translation(
                    dc,
                    &fonts,
                    &sense.word,
                    sense.visible_pos(state.show_pos.get()),
                    inner,
                    palette,
                    false,
                );
            } else {
                fonts.select(fonts.english);
                label(dc, "—", inner, palette.secondary, false);
            }
        }
    }
    if !empty
        && input.expanded
        && let Some(candidate) = input.frame.candidates.get(input.selected)
    {
        let title = l.details_title_rect();
        fill(
            dc,
            &RECT {
                left: l.padding,
                top: title.y,
                right: l.width() - l.padding,
                bottom: title.y + px(1, dpi),
            },
            palette.divider,
        );
        fonts.select(fonts.detail);
        label(
            dc,
            &format!("{} · 更多译法", candidate.text),
            insets(title, px(4, dpi)),
            palette.foreground,
            false,
        );
        for (row, sense) in candidate
            .translations
            .iter()
            .enumerate()
            .skip(l.visible_senses().start)
            .take(l.visible_senses().len())
        {
            let r = l.sense_rect(row);
            if row == input.sense || hovered.is_some_and(|(x, y)| r.contains(x, y)) {
                rounded(dc, r, px(20, dpi), palette.selected, palette.selected);
            }
            let inner = insets(r, px(8, dpi));
            let line = px(28, dpi);
            translation(
                dc,
                &fonts,
                &sense.word,
                sense.visible_pos(state.show_pos.get()),
                BoxRect {
                    height: line,
                    ..inner
                },
                palette,
                true,
            );
            fonts.select(fonts.english);
            label(
                dc,
                &sense.note,
                BoxRect {
                    y: inner.y + line,
                    height: line,
                    ..inner
                },
                palette.secondary,
                false,
            );
            fonts.select(fonts.small);
            label(
                dc,
                &sense.example,
                BoxRect {
                    y: inner.y + 2 * line,
                    height: line,
                    ..inner
                },
                palette.secondary,
                false,
            );
        }
        fonts.select(fonts.small);
        label(
            dc,
            &format!(
                "Tab 选择译法 · 滚轮浏览 · Shift+空格确认   {}/{}",
                input.sense + 1,
                candidate.translations.len()
            ),
            BoxRect {
                x: l.padding,
                y: l.height() - l.padding - px(24, dpi),
                width: l.width() - 2 * l.padding,
                height: px(24, dpi),
            },
            palette.secondary,
            false,
        );
    }
    if saved_body != 0 {
        unsafe {
            let _ = RestoreDC(dc, saved_body);
        }
    }
}

fn header_hit(l: Layout, x: i32, y: i32) -> Option<usize> {
    (0..5).find(|i| *i != 1 && l.header_item(*i).contains(x, y))
}
fn tooltip_texts(state: &State, l: Layout) -> Vec<(BoxRect, HSTRING)> {
    let input = state.input.borrow();
    let mut items = vec![(l.title_rect(), HSTRING::from(&input.raw))];
    for (i, title) in ["上一页", "", "下一页", "更多译法 / 收起", "设置"]
        .iter()
        .enumerate()
    {
        if !title.is_empty() {
            items.push((l.header_item(i), HSTRING::from(*title)));
        }
    }
    if input.error || input.frame.candidates.is_empty() {
        return items;
    }
    for (slot, candidate) in input
        .frame
        .candidates
        .iter()
        .skip(input.page() * 5)
        .take(5)
        .enumerate()
    {
        items.push((
            l.visible(l.candidate_rect(slot, false)),
            HSTRING::from(&candidate.text),
        ));
        if let Some(sense) = candidate.translations.first() {
            items.push((
                l.visible(l.candidate_rect(slot, true)),
                HSTRING::from(format!(
                    "{}\n{}\n{}",
                    sense.display_word(state.show_pos.get()),
                    sense.note,
                    sense.example
                )),
            ));
        }
    }
    if input.expanded
        && let Some(candidate) = input.frame.candidates.get(input.selected)
    {
        items.push((
            l.visible(l.details_title_rect()),
            HSTRING::from(format!("{} · 更多译法", candidate.text)),
        ));
        for (row, sense) in candidate
            .translations
            .iter()
            .enumerate()
            .skip(l.visible_senses().start)
            .take(l.visible_senses().len())
        {
            items.push((
                l.visible(l.sense_rect(row)),
                HSTRING::from(format!(
                    "{}\n{}\n{}",
                    sense.display_word(state.show_pos.get()),
                    sense.note,
                    sense.example
                )),
            ));
        }
    }
    items.retain(|(r, _)| r.width > 0 && r.height > 0);
    items
}
fn details(window: HWND, state: &Rc<State>) {
    if state.busy.get() {
        return;
    }
    if state.input.borrow().expanded {
        state.click(Key::Escape);
        return;
    }
    let choices: Vec<_> = {
        let input = state.input.borrow();
        input
            .frame
            .candidates
            .iter()
            .enumerate()
            .skip(input.page() * 5)
            .take(5)
            .filter(|(_, c)| !c.translations.is_empty())
            .map(|(i, c)| (i, format!("{} · 更多译法", c.text)))
            .collect()
    };
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
        for (index, title) in choices {
            let _ = AppendMenuW(menu, MF_STRING, index + 1, &HSTRING::from(title));
        }
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY,
            point.x,
            point.y,
            None,
            window,
            None,
        )
        .0;
        let _ = DestroyMenu(menu);
        if selected > 0 {
            let mut input = state.input.borrow_mut();
            input.selected = selected as usize - 1;
            input.sense = 0;
            input.expanded = false;
            drop(input);
            state.click(Key::Tab(false));
        }
    }
}

pub(super) fn settings_shortcut(key: usize) -> bool {
    key == 0x50 && down(0x11) && down(0x10) && !down(0x12) && !down(0x5B) && !down(0x5C)
}
pub(super) fn settings(window: HWND, state: &Rc<State>) {
    if state.preview {
        return;
    }
    // Native checked menu; no activation or host text access. Cancel makes no change.
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    let current = crate::preferences::load();
    state.display.set(crate::preferences::display());
    let display = state.display.get();
    let punctuation = state.input.borrow().punctuation_mode;
    let remember = state.input.borrow().remember;
    state.show_pos.set(current);
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
        let flags = MF_STRING | if current { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, flags, 1, w!("显示词性（n. / v. / adj. / adv.）"));
        let _ = AppendMenuW(menu, MF_STRING, 2, w!("本地学习记录…"));
        let memory_flags = MF_STRING | if remember { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, memory_flags, 4, w!("中文选词记忆（当前应用）"));
        if state.memory_warning.get() {
            let _ = AppendMenuW(
                menu,
                MF_STRING | MF_GRAYED,
                5,
                w!("中文选词记忆未保存，请检查本机存储"),
            );
        }
        if crate::learning::SAVE_WARNING.load(Ordering::Relaxed) {
            let _ = AppendMenuW(
                menu,
                MF_STRING | MF_GRAYED,
                3,
                w!("本地记录未保存，请检查存储权限"),
            );
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, 6, w!("检查版本更新…"));
        let theme = CreatePopupMenu().ok();
        let font = CreatePopupMenu().ok();
        let marks = CreatePopupMenu().ok();
        for (submenu, title, choices) in [
            (
                theme,
                "外观",
                crate::preferences::Theme::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (10 + i, t.title().to_owned(), *t == display.theme))
                    .collect::<Vec<_>>(),
            ),
            (
                font,
                "候选字号",
                crate::preferences::Display::SIZES
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        (
                            20 + i,
                            ["小", "标准", "大", "特大"][i].to_owned(),
                            *v == display.font,
                        )
                    })
                    .collect(),
            ),
            (
                marks,
                "标点规则（当前应用）",
                crate::punctuation::Mode::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, m)| (30 + i, m.title().to_owned(), *m == punctuation))
                    .collect(),
            ),
        ] {
            if let Some(submenu) = submenu {
                for (id, label, checked) in choices {
                    let _ = AppendMenuW(
                        submenu,
                        MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED },
                        id,
                        &windows::core::HSTRING::from(label),
                    );
                }
                if AppendMenuW(
                    menu,
                    MF_POPUP,
                    submenu.0 as usize,
                    &windows::core::HSTRING::from(title),
                )
                .is_err()
                {
                    let _ = DestroyMenu(submenu);
                }
            }
        }
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY,
            point.x,
            point.y,
            None,
            window,
            None,
        )
        .0;
        let _ = DestroyMenu(menu);
        if selected == 6 {
            let _ = updates::launch(false);
        }
        if selected == 4 {
            state.remember(!remember);
        }
        if selected == 2
            && super::module_path()
                .ok()
                .and_then(|dll| crate::learning_window::launch(dll).ok())
                .is_none()
        {
            let _ = MessageBoxW(
                Some(window),
                w!("无法打开本地学习窗口，请重试。"),
                w!("灵果"),
                MB_OK | MB_ICONERROR,
            );
        }
        if selected == 1 {
            if crate::preferences::save(!current).is_ok() {
                state.show_pos.set(!current);
            } else {
                let _ = MessageBoxW(
                    Some(window),
                    w!("未能保存词性显示设置，请重试。"),
                    w!("灵果"),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        let saved = match selected {
            10..=12 => crate::preferences::save_theme(
                crate::preferences::Theme::ALL[selected as usize - 10],
            ),
            20..=23 => crate::preferences::save_font(
                crate::preferences::Display::SIZES[selected as usize - 20],
            ),
            30..=32 => {
                state.punctuation_mode(crate::punctuation::Mode::ALL[selected as usize - 30]);
                Ok(())
            }
            _ => Ok(()),
        };
        if saved.is_err() {
            let _ = MessageBoxW(
                Some(window),
                w!("未能保存显示设置，请重试。"),
                w!("灵果"),
                MB_OK | MB_ICONERROR,
            );
        }
        state.display.set(crate::preferences::display());
        state.refresh_palette();
        state.refresh();
        let _ = InvalidateRect(Some(window), None, false);
    }
}

// Isolated render-only callbacks: synthetic frame, no engine, registry writes, TSF registration,
// host text or personal records. Fixed entry points deliberately ignore command-line payload.
fn preview(
    instance: HINSTANCE,
    theme: crate::preferences::Theme,
    size: u32,
    narrow: bool,
) -> Result<()> {
    let state = preview_state(theme, size);
    let panel = Panel::new(Rc::downgrade(&state))?;
    let window = panel.window;
    if narrow {
        panel.limit.set(Some(320));
    }
    unsafe {
        let _ = SetWindowTextW(window, w!("灵果候选 · 隔离样例"));
    }
    *state.panel.borrow_mut() = Some(panel);
    state.panel.borrow().as_ref().unwrap().update();
    let mut message = MSG::default();
    unsafe {
        // Preview lifetime is bounded without foreground activation or a real input session.
        SetTimer(Some(window), 1, 15000, None);
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            if message.message == WM_TIMER {
                break;
            }
            if message.message == WM_CLOSE {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = KillTimer(Some(window), 1);
    }
    state.panel.borrow_mut().take();
    let _ = instance;
    Ok(())
}
#[unsafe(no_mangle)]
pub extern "system" fn LingoMateCandidatePreviewW(
    _owner: HWND,
    instance: HINSTANCE,
    _command: windows::core::PCWSTR,
    _show: i32,
) {
    let _ =
        std::panic::catch_unwind(|| preview(instance, crate::preferences::Theme::Light, 18, false));
}
#[unsafe(no_mangle)]
pub extern "system" fn LingoMateCandidateDarkPreviewW(
    _owner: HWND,
    instance: HINSTANCE,
    _command: windows::core::PCWSTR,
    _show: i32,
) {
    let _ =
        std::panic::catch_unwind(|| preview(instance, crate::preferences::Theme::Dark, 18, false));
}
#[unsafe(no_mangle)]
pub extern "system" fn LingoMateCandidateLargePreviewW(
    _owner: HWND,
    instance: HINSTANCE,
    _command: windows::core::PCWSTR,
    _show: i32,
) {
    let _ =
        std::panic::catch_unwind(|| preview(instance, crate::preferences::Theme::Dark, 22, false));
}

#[unsafe(no_mangle)]
pub extern "system" fn LingoMateCandidateNarrowPreviewW(
    _owner: HWND,
    instance: HINSTANCE,
    _command: windows::core::PCWSTR,
    _show: i32,
) {
    let _ =
        std::panic::catch_unwind(|| preview(instance, crate::preferences::Theme::Dark, 22, true));
}

fn preview_state(theme: crate::preferences::Theme, size: u32) -> Rc<State> {
    use crate::bridge::{Candidate, Translation};
    let state = Rc::new(State {
        input: std::cell::RefCell::new(crate::input::Input::default()),
        worker: std::cell::RefCell::new(None),
        outbox: std::cell::RefCell::new(Default::default()),
        deferred_boundary: std::cell::RefCell::new(None),
        deferred_mode: Cell::new(None),
        deferred_context: Cell::new(false),
        sequence: Cell::new(0),
        displayed: Cell::new(0),
        latest: Cell::new(0),
        pending: std::cell::RefCell::new(Default::default()),
        edited: std::cell::RefCell::new(Vec::new()),
        busy: Cell::new(false),
        polling: Cell::new(false),
        refreshing_palette: Cell::new(false),
        composing: Cell::new(false),
        failure: Cell::new(None),
        retiring: Cell::new(false),
        barrier: Cell::new(None),
        drained: Cell::new(false),
        document: std::cell::RefCell::new(None),
        panel: std::cell::RefCell::new(None),
        mode: std::cell::RefCell::new(None),
        client: Cell::new(0),
        show_pos: Cell::new(true),
        display: Cell::new(crate::preferences::Display { theme, font: size }),
        memory_warning: Cell::new(false),
        preview: true,
    });
    {
        let mut input = state.input.borrow_mut();
        input.raw = "xuexi".into();
        input.expanded = true;
        input.frame.candidates = ["学习", "学系", "学戏", "雪溪", "雪西"]
            .iter()
            .map(|s| Candidate {
                text: (*s).into(),
                syllables: vec!["xue".into(), "xi".into()],
                translations: ["study", "learn", "learning"]
                    .iter()
                    .map(|word| Translation {
                        word: (*word).into(),
                        pos: "v.".into(),
                        note: "学习；研究".into(),
                        example: String::new(),
                    })
                    .collect(),
            })
            .collect();
    }
    state
}

#[cfg(test)]
#[test]
fn render_preview_never_opens_engine_or_commits_words() {
    let state = preview_state(crate::preferences::Theme::Dark, 22);
    let raw = state.input.borrow().raw.clone();
    for key in [
        Key::Space(false),
        Key::Space(true),
        Key::Letter('a'),
        Key::Punctuation(','),
        Key::Return,
    ] {
        state.key(key);
    }
    assert_eq!(state.input.borrow().raw, raw);
    assert!(state.worker.borrow().is_none());
    assert!(state.input.borrow().pending_memory.is_none());
    assert!(state.input.borrow().pending_study.is_none());
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
