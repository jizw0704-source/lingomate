use super::{Guard, Key, MODULE, State, down, updates};
use crate::{
    appearance::{self, px},
    candidate_layout::Layout,
};
use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};
pub(super) struct Panel {
    window: HWND,
    owner: *mut Weak<State>,
    caret: Cell<POINT>,
    pressed: Cell<Option<(i32, i32)>>,
    limit: Cell<Option<i32>>,
    hovered: Cell<Option<(i32, i32)>>,
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
            .map_or(0, |c| c.translations.len().min(6))
    } else {
        0
    };
    let point = state
        .panel
        .borrow()
        .as_ref()
        .map_or(POINT { x: 40, y: 40 }, |p| p.caret.get());
    let work = work_area(point.x, point.y);
    Layout::new(
        if empty { 0 } else { count },
        extra,
        state.display.get().font,
        appearance::dpi(window),
        state
            .panel
            .borrow()
            .as_ref()
            .and_then(|p| p.limit.get())
            .unwrap_or(work.right - work.left - 8),
    )
}
impl Panel {
    pub fn settings(&self) {
        if let Some(state) = unsafe { &*self.owner }.upgrade() {
            settings(self.window, &state);
        }
    }
    pub fn new(state: Weak<State>) -> Result<Self> {
        let instance = HINSTANCE(MODULE.load(Ordering::SeqCst));
        let class = WNDCLASSW {
            hInstance: instance,
            lpszClassName: w!("LingoMate.Candidates.v1"),
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
                WS_POPUP | WS_BORDER,
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
        let owner = Box::into_raw(Box::new(state));
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, owner as isize);
        }
        Ok(Self {
            window,
            owner,
            caret: Cell::new(POINT { x: 40, y: 40 }),
            pressed: Cell::new(None),
            limit: Cell::new(None),
            hovered: Cell::new(None),
            _guard: Guard::new(),
        })
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
        let l = layout(self.window, &state);
        let point = self.caret.get();
        let work = work_area(point.x, point.y);
        let x = point
            .x
            .clamp(work.left, (work.right - l.width() - 2).max(work.left));
        let below = point.y + 4;
        let y = if below + l.height() + 2 <= work.bottom {
            below
        } else {
            point.y - l.height() - 4
        }
        .clamp(work.top, (work.bottom - l.height() - 2).max(work.top));
        unsafe {
            let _ = SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                x,
                y,
                l.width() + 2,
                l.height() + 2,
                SWP_NOACTIVATE,
            );
            let _ = InvalidateRect(Some(self.window), None, false);
            let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE);
        }
    }
    pub fn position(&self, x: i32, y: i32) {
        self.caret.set(POINT { x, y });
        self.update();
    }
    pub fn hide(&self) {
        self.pressed.set(None);
        if unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetCapture() } == self.window {
            let _ = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture() };
        }
        unsafe {
            let _ = ShowWindow(self.window, SW_HIDE);
        }
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.window, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.window);
            drop(Box::from_raw(self.owner));
            let _ = UnregisterClassW(
                w!("LingoMate.Candidates.v1"),
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
        if matches!(
            message,
            WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED | WM_DPICHANGED
        ) {
            if let Some(panel) = state.panel.borrow().as_ref() {
                panel.update();
            }
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
            if let Some(panel) = state.panel.borrow().as_ref() {
                panel
                    .hovered
                    .set(if message == windows::Win32::UI::Controls::WM_MOUSELEAVE {
                        None
                    } else {
                        Some((
                            (lparam.0 as u16) as i16 as i32,
                            ((lparam.0 >> 16) as u16) as i16 as i32,
                        ))
                    });
                let mut track = windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<
                        windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT,
                    >() as u32,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::TME_LEAVE,
                    hwndTrack: window,
                    dwHoverTime: 0,
                };
                let _ = unsafe {
                    windows::Win32::UI::Input::KeyboardAndMouse::TrackMouseEvent(&mut track)
                };
                let _ = unsafe { InvalidateRect(Some(window), None, false) };
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
            let same = |a: i32, b: i32| a / l.row == b / l.row;
            if y >= 0
                && y < l.row
                && pressed.is_some_and(|(px, py)| py >= 0 && py < l.row && same(px, x))
            {
                match x / l.row {
                    0 => state.key(Key::Previous),
                    1 => state.key(Key::Next),
                    2 => state.key(Key::Tab(false)),
                    3 => {
                        let _ = updates::launch(false);
                    }
                    4 => settings(window, &state),
                    _ => {}
                }
            } else if let Some((slot, english)) = l.hit(x, y) {
                if pressed.and_then(|(x, y)| l.hit(x, y)) == Some((slot, english)) {
                    let mut input = state.input.borrow_mut();
                    input.selected = input.page() * 5 + slot;
                    input.sense = 0;
                    drop(input);
                    state.key(Key::Space(english));
                }
            } else if y >= l.senses_y()
                && y < l.height()
                && x >= 0
                && x < l.width()
                && pressed.is_some_and(|(_, py)| same(py - l.senses_y(), y - l.senses_y()))
            {
                let mut input = state.input.borrow_mut();
                if input.expanded {
                    input.sense = input.sense / 6 * 6 + ((y - l.senses_y()) / l.row) as usize;
                    drop(input);
                    state.key(Key::Space(true));
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
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Microsoft YaHei UI"),
        )
    }
}
fn text(dc: HDC, content: &str, x: i32, y: i32, width: i32, height: i32, color: COLORREF) {
    let mut rect = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    let mut text: Vec<_> = content.encode_utf16().collect();
    unsafe {
        SetTextColor(dc, color);
        DrawTextW(
            dc,
            &mut text,
            &mut rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
}
fn fill(dc: HDC, rect: &RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(dc, rect, brush);
        let _ = DeleteObject(brush.into());
    }
}
fn paint(window: HWND, state: &Rc<State>, print: Option<HDC>) {
    let mut ps = PAINTSTRUCT::default();
    let dc = print.unwrap_or_else(|| unsafe { BeginPaint(window, &mut ps) });
    if print.is_some() {
        let _ = unsafe { GetClientRect(window, &mut ps.rcPaint) };
    }
    let dpi = appearance::dpi(window);
    let display = state.display.get();
    let palette = appearance::palette(display.theme);
    let l = layout(window, state);
    let main = font(px(display.font as i32, dpi));
    let original = unsafe { SelectObject(dc, main.into()) };
    fill(dc, &ps.rcPaint, palette.background);
    unsafe {
        SetBkMode(dc, TRANSPARENT);
    }
    let header = font(px(14, dpi));
    unsafe {
        SelectObject(dc, header.into());
    }
    for (i, label) in ["‹", "›", "译法", "更新", "设置"].iter().enumerate() {
        text(
            dc,
            label,
            i as i32 * l.row + px(6, dpi),
            0,
            l.row - px(6, dpi),
            l.row,
            palette.foreground,
        );
    }
    let hovered = state.panel.borrow().as_ref().and_then(|p| p.hovered.get());
    let pressed = state.panel.borrow().as_ref().and_then(|p| p.pressed.get());
    let input = state.input.borrow();
    let empty = input.error || input.frame.candidates.is_empty();
    let title = if empty {
        input.raw.clone()
    } else {
        format!(
            "{}  {}/{}",
            input.raw,
            input.page() + 1,
            input.page_count().max(1)
        )
    };
    text(
        dc,
        &title,
        5 * l.row + px(8, dpi),
        0,
        l.width() - 5 * l.row - px(12, dpi),
        l.row,
        palette.secondary,
    );
    unsafe {
        SelectObject(dc, main.into());
        let _ = DeleteObject(header.into());
    }
    if empty {
        text(
            dc,
            if input.error {
                "引擎暂不可用"
            } else {
                "未找到候选"
            },
            px(12, dpi),
            l.row,
            l.width() - px(24, dpi),
            l.row,
            palette.foreground,
        );
        text(
            dc,
            "回车可提交原样字母",
            px(12, dpi),
            l.row * 2,
            l.width() - px(24, dpi),
            l.row,
            palette.secondary,
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
        let (x, y) = l.position(slot);
        let selected = input.page() * 5 + slot == input.selected;
        if selected
            || hovered
                .and_then(|(x, y)| l.hit(x, y))
                .is_some_and(|(hover, _)| hover == slot)
        {
            fill(
                dc,
                &RECT {
                    left: x,
                    top: y,
                    right: x + l.column,
                    bottom: y + l.row * 2,
                },
                palette.selected,
            );
        }
        if selected
            || pressed
                .and_then(|(x, y)| l.hit(x, y))
                .is_some_and(|(clicked, _)| clicked == slot)
        {
            let brush = unsafe { CreateSolidBrush(palette.foreground) };
            unsafe {
                FrameRect(
                    dc,
                    &RECT {
                        left: x + px(1, dpi),
                        top: y + px(1, dpi),
                        right: x + l.column - px(1, dpi),
                        bottom: y + l.row * 2 - px(1, dpi),
                    },
                    brush,
                );
                let _ = DeleteObject(brush.into());
            }
        }
        let foreground = if selected {
            palette.selected_text
        } else {
            palette.foreground
        };
        text(
            dc,
            &format!("{} {}", slot + 1, candidate.text),
            x + px(8, dpi),
            y,
            l.column - px(16, dpi),
            l.row,
            foreground,
        );
        if let Some(translation) = candidate.translations.first() {
            let pos = translation.visible_pos(state.show_pos.get());
            if pos.is_empty() {
                text(
                    dc,
                    &translation.word,
                    x + px(8, dpi),
                    y + l.row,
                    l.column - px(16, dpi),
                    l.row,
                    foreground,
                );
            } else {
                let top = px(display.font as i32 + 3, dpi);
                text(
                    dc,
                    &translation.word,
                    x + px(8, dpi),
                    y + l.row,
                    l.column - px(16, dpi),
                    top,
                    foreground,
                );
                let small = font(px((display.font as i32 * 2 / 3).max(11), dpi));
                let old = unsafe { SelectObject(dc, small.into()) };
                text(
                    dc,
                    pos,
                    x + px(8, dpi),
                    y + l.row + top,
                    l.column - px(16, dpi),
                    l.row - top,
                    palette.secondary,
                );
                unsafe {
                    SelectObject(dc, old);
                    let _ = DeleteObject(small.into());
                }
            }
        }
    }
    if !empty
        && input.expanded
        && let Some(candidate) = input.frame.candidates.get(input.selected)
    {
        for (row, sense) in candidate
            .translations
            .iter()
            .enumerate()
            .skip(input.sense / 6 * 6)
            .take(6)
        {
            let y = l.senses_y() + (row % 6) as i32 * l.row;
            if row == input.sense {
                fill(
                    dc,
                    &RECT {
                        left: 0,
                        top: y,
                        right: l.width(),
                        bottom: y + l.row,
                    },
                    palette.selected,
                );
            }
            text(
                dc,
                &format!(
                    "{}{} · {}",
                    if row == input.sense { "› " } else { "  " },
                    sense.display_word(state.show_pos.get()),
                    sense.note
                ),
                px(12, dpi),
                y,
                l.width() - px(24, dpi),
                l.row,
                if row == input.sense {
                    palette.selected_text
                } else {
                    palette.foreground
                },
            );
        }
    }
    unsafe {
        SelectObject(dc, original);
        let _ = DeleteObject(main.into());
        if print.is_none() {
            let _ = EndPaint(window, &ps);
        }
    }
}

pub(super) fn settings_shortcut(key: usize) -> bool {
    key == 0x50 && down(0x11) && down(0x10) && !down(0x12) && !down(0x5B) && !down(0x5C)
}
fn settings(window: HWND, state: &Rc<State>) {
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
                    .map(|(i, v)| (20 + i, format!("{v}"), *v == display.font))
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
        if selected == 4 {
            state.cancel_memory();
            state.input.borrow_mut().remember = !remember;
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
                let mut input = state.input.borrow_mut();
                input.punctuation_mode = crate::punctuation::Mode::ALL[selected as usize - 30];
                input.punctuation.reset_context();
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
        if let Some(panel) = state.panel.borrow().as_ref() {
            panel.update();
        }
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
        engine: std::cell::RefCell::new(None),
        retry_after: Cell::new(None),
        document: std::cell::RefCell::new(None),
        panel: std::cell::RefCell::new(None),
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
    assert!(state.engine.borrow().is_none());
    assert!(state.input.borrow().pending_memory.is_none());
    assert!(state.input.borrow().pending_study.is_none());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{Candidate, Translation};
    use crate::input::Input;
    use std::ffi::c_void;

    struct Surface {
        dc: HDC,
        bitmap: HBITMAP,
        original: HGDIOBJ,
        bits: *mut c_void,
        width: i32,
        height: i32,
    }
    impl Surface {
        fn new(width: i32, height: i32) -> Self {
            let dc = unsafe { CreateCompatibleDC(None) };
            assert!(!dc.is_invalid(), "Preview device context unavailable");
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
                unsafe { CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) }
                    .expect("Preview bitmap unavailable");
            let original = unsafe { SelectObject(dc, bitmap.into()) };
            Self {
                dc,
                bitmap,
                original,
                bits,
                width,
                height,
            }
        }
        fn pixels(&self) -> &[u8] {
            unsafe {
                std::slice::from_raw_parts(
                    self.bits.cast(),
                    (self.width * self.height * 4) as usize,
                )
            }
        }
        fn ink_pixels(&self, left: i32, top: i32, right: i32, bottom: i32) -> usize {
            let pixels = self.pixels();
            (top..bottom.min(self.height))
                .flat_map(|y| (left..right.min(self.width)).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let offset = ((y * self.width + x) * 4) as usize;
                    pixels[offset..offset + 3]
                        .iter()
                        .zip(&pixels[..3])
                        .any(|(value, background)| value.abs_diff(*background) > 32)
                })
                .count()
        }
        fn save_preview(&self, name: &str) {
            // Optional artifacts contain only the synthetic inputs below. Normal
            // test runs create no files and never open an input source or engine.
            let Some(directory) = std::env::var_os("LINGOMATE_CANDIDATE_PREVIEW_DIR") else {
                return;
            };
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            let pixels = self.pixels();
            let mut bytes = Vec::with_capacity(54 + pixels.len());
            bytes.extend_from_slice(b"BM");
            bytes.extend_from_slice(&(54 + pixels.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&[0; 4]);
            bytes.extend_from_slice(&54u32.to_le_bytes());
            bytes.extend_from_slice(&40u32.to_le_bytes());
            bytes.extend_from_slice(&self.width.to_le_bytes());
            bytes.extend_from_slice(&(-self.height).to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&32u16.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&[0; 16]);
            bytes.extend_from_slice(pixels);
            std::fs::write(directory.join(format!("candidates-{name}.bmp")), bytes).unwrap();
        }
    }
    impl Drop for Surface {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.original);
                let _ = DeleteObject(self.bitmap.into());
                let _ = DeleteDC(self.dc);
            }
        }
    }

    #[test]
    fn hidden_candidate_window_renders_empty_status_and_short_page_headers() {
        let foreground = unsafe { GetForegroundWindow() };
        let state = State::with_display_preferences(false);
        let panel =
            Panel::new(Rc::downgrade(&state)).expect("Isolated candidate window unavailable");
        assert!(!unsafe { IsWindowVisible(panel.window) }.as_bool());
        let style = unsafe { GetWindowLongPtrW(panel.window, GWL_EXSTYLE) } as u32;
        assert_ne!(style & WS_EX_NOACTIVATE.0, 0);
        assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
        for (name, error, count) in [
            ("engine-unavailable", true, 0),
            ("engine-unavailable-stale", true, 3),
            ("no-match", false, 0),
            ("one", false, 1),
            ("two", false, 2),
            ("three", false, 3),
            ("four", false, 4),
            ("five", false, 5),
        ] {
            let mut input = Input {
                raw: "wode".into(),
                error,
                ..Default::default()
            };
            input.frame.candidates = (0..count)
                .map(|_| Candidate {
                    text: "我的".into(),
                    translations: vec![Translation {
                        word: "my".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect();
            let l = crate::candidate_layout::Layout::new(
                if error { 0 } else { count as usize },
                0,
                18,
                96,
                1000,
            );
            let (width, height) = (l.width(), l.height());
            *state.input.borrow_mut() = input;
            unsafe {
                SetWindowPos(
                    panel.window,
                    None,
                    0,
                    0,
                    width,
                    height,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
                .unwrap();
            }
            let mut rect = RECT::default();
            unsafe { GetClientRect(panel.window, &mut rect) }.unwrap();
            let surface = Surface::new(rect.right, rect.bottom);
            // Exercise the actual HWND callback and the same native GDI painter
            // used for WM_PAINT; keep the window hidden throughout the capture.
            unsafe {
                SendMessageW(
                    panel.window,
                    WM_PRINTCLIENT,
                    Some(WPARAM(surface.dc.0 as usize)),
                    Some(LPARAM(PRF_CLIENT as isize)),
                );
                assert!(GdiFlush().as_bool());
            }
            assert!(
                surface.ink_pixels(228, 0, rect.right, 40) > 10,
                "{name}: composition title is outside the visible window"
            );
            if error || count == 0 {
                assert!(
                    surface.ink_pixels(12, 44, rect.right - 12, 84) > 10,
                    "{name}: empty candidate state is invisible"
                );
                assert!(
                    surface.ink_pixels(12, 88, rect.right - 12, 128) > 10,
                    "{name}: raw-letter submission guidance is invisible"
                );
                assert_eq!(
                    surface.ink_pixels(260, 44, rect.right, 128),
                    0,
                    "{name}: stale candidates are still visible during engine failure"
                );
            } else {
                for slot in 0..count {
                    let left = slot * l.column + 8;
                    assert!(surface.ink_pixels(left, 44, left + l.column - 16, 84) > 10);
                    assert!(surface.ink_pixels(left, 88, left + l.column - 16, 128) > 10);
                }
            }
            surface.save_preview(name);
            assert!(!unsafe { IsWindowVisible(panel.window) }.as_bool());
        }
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    }
}
