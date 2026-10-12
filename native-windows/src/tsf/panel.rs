use super::{Guard, Key, MODULE, State, down, updates};
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

const PANEL_WIDTH: i32 = 560;

fn panel_size(input: &crate::input::Input) -> (i32, i32) {
    let extra = if input.expanded && !input.error {
        input
            .frame
            .candidates
            .get(input.selected)
            .map_or(0, |c| c.translations.len().min(6))
    } else {
        0
    };
    // The five toolbar targets occupy 220 px before the composition title.
    // Keep that title visible even on an empty or short final candidate page.
    (PANEL_WIDTH, 132 + extra as i32 * 44)
}

pub(super) struct Panel {
    window: HWND,
    owner: *mut Weak<State>,
    _guard: Guard,
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
                PANEL_WIDTH,
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
            _guard: Guard::new(),
        })
    }
    pub fn update(&self) {
        let state = unsafe { &*self.owner }.upgrade();
        let Some(state) = state else {
            self.hide();
            return;
        };
        let input = state.input.borrow();
        if input.raw.is_empty() {
            self.hide();
            return;
        }
        let (width, height) = panel_size(&input);
        unsafe {
            let _ = SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOACTIVATE,
            );
            let _ = InvalidateRect(Some(self.window), None, false);
            let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE);
        }
    }
    pub fn position(&self, x: i32, y: i32) {
        unsafe {
            let _ = SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                x.max(0),
                (y + 4).max(0),
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    pub fn hide(&self) {
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
        if message == WM_PAINT {
            paint(window, &state);
            return LRESULT(0);
        }
        if message == WM_PRINTCLIENT {
            let mut rect = RECT::default();
            if unsafe { GetClientRect(window, &mut rect) }.is_ok() {
                draw(
                    HDC(wparam.0 as *mut _),
                    &rect,
                    &state.input.borrow(),
                    state.show_pos.get(),
                );
            }
            return LRESULT(0);
        }
        if message == WM_LBUTTONUP {
            let x = (lparam.0 as u16) as i16 as i32;
            let y = ((lparam.0 >> 16) as u16) as i16 as i32;
            if y < 44 {
                match x / 44 {
                    0 => state.key(Key::Previous),
                    1 => state.key(Key::Next),
                    2 => state.key(Key::Tab(false)),
                    3 => {
                        let _ = updates::launch(false);
                    }
                    4 => settings(window, &state),
                    _ => {}
                }
            } else if y < 132 && x >= 0 {
                let slot = x as usize / 112;
                let mut input = state.input.borrow_mut();
                let index = input.page() * 5 + slot;
                if slot < 5 && index < input.frame.candidates.len() {
                    input.selected = index;
                    input.sense = 0;
                    drop(input);
                    state.key(Key::Space(y >= 88));
                }
            } else if y >= 132 {
                let mut input = state.input.borrow_mut();
                if input.expanded {
                    let start = input.sense / 6 * 6;
                    input.sense = start + ((y - 132) / 44) as usize;
                    drop(input);
                    state.key(Key::Space(true));
                }
            }
            return LRESULT(0);
        }
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
fn text(dc: HDC, content: &str, x: i32, y: i32, width: i32, color: COLORREF) {
    text_height(dc, content, x, y, width, 40, color);
}
fn text_height(dc: HDC, content: &str, x: i32, y: i32, width: i32, height: i32, color: COLORREF) {
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
fn paint(window: HWND, state: &Rc<State>) {
    let mut ps = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(window, &mut ps) };
    let mut rect = RECT::default();
    if unsafe { GetClientRect(window, &mut rect) }.is_ok() {
        draw(dc, &rect, &state.input.borrow(), state.show_pos.get());
    }
    unsafe {
        let _ = EndPaint(window, &ps);
    }
}
fn draw(dc: HDC, rect: &RECT, input: &crate::input::Input, show_pos: bool) {
    let font = unsafe {
        CreateFontW(
            -14,
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
    };
    let original = unsafe { SelectObject(dc, font.into()) };
    unsafe {
        FillRect(dc, rect, HBRUSH(GetStockObject(WHITE_BRUSH).0));
        SetBkMode(dc, TRANSPARENT);
    }
    text(dc, "‹", 16, 0, 28, COLORREF(0x101010));
    text(dc, "›", 60, 0, 28, COLORREF(0x101010));
    text(dc, "译法", 90, 0, 44, COLORREF(0x101010));
    text(dc, "更新", 138, 0, 44, COLORREF(0x101010));
    text(dc, "设置", 180, 0, 44, COLORREF(0x101010));
    let empty = input.error || input.frame.candidates.is_empty();
    let title = if empty {
        input.raw.clone()
    } else {
        format!(
            "{}   {}/{}",
            input.raw,
            input.page() + 1,
            input.page_count().max(1)
        )
    };
    text(dc, &title, 228, 0, 322, COLORREF(0x4A4A4A));
    if empty {
        text(
            dc,
            if input.error {
                "引擎暂不可用"
            } else {
                "未找到候选"
            },
            12,
            44,
            PANEL_WIDTH - 24,
            COLORREF(0x101010),
        );
        text(
            dc,
            "回车可提交原样字母",
            12,
            88,
            PANEL_WIDTH - 24,
            COLORREF(0x4A4A4A),
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
        let x = slot as i32 * 112;
        if input.page() * 5 + slot == input.selected {
            let brush = unsafe { CreateSolidBrush(COLORREF(0xF0F0F0)) };
            unsafe {
                FillRect(
                    dc,
                    &RECT {
                        left: x,
                        top: 44,
                        right: x + 112,
                        bottom: 132,
                    },
                    brush,
                );
                let _ = DeleteObject(brush.into());
            }
        }
        text(
            dc,
            &format!("{} {}", slot + 1, candidate.text),
            x + 8,
            44,
            96,
            COLORREF(0x101010),
        );
        if let Some(translation) = candidate.translations.first() {
            let pos = translation.visible_pos(show_pos);
            if pos.is_empty() {
                text(dc, &translation.word, x + 8, 88, 96, COLORREF(0x4A4A4A));
            } else {
                text_height(dc, &translation.word, x + 8, 88, 96, 22, COLORREF(0x101010));
                let small = unsafe {
                    CreateFontW(
                        -11,
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
                };
                let old = unsafe { SelectObject(dc, small.into()) };
                text_height(dc, pos, x + 8, 110, 96, 20, COLORREF(0x4A4A4A));
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
            let marker = if row == input.sense { "› " } else { "  " };
            text(
                dc,
                &format!("{marker}{} · {}", sense.display_word(show_pos), sense.note),
                12,
                132 + (row % 6) as i32 * 44,
                530,
                COLORREF(0x101010),
            );
        }
    }
    unsafe {
        SelectObject(dc, original);
        let _ = DeleteObject(font.into());
    }
}

pub(super) fn settings_shortcut(key: usize) -> bool {
    key == 0x50 && down(0x11) && down(0x10) && !down(0x12) && !down(0x5B) && !down(0x5C)
}
fn settings(window: HWND, state: &Rc<State>) {
    state.cancel_memory();
    // Native checked menu; no activation or host text access. Cancel makes no change.
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    let current = crate::preferences::load();
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
        let _ = InvalidateRect(Some(window), None, false);
    }
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
        fn dark_pixels(&self, left: i32, top: i32, right: i32, bottom: i32) -> usize {
            let pixels = self.pixels();
            (top..bottom.min(self.height))
                .flat_map(|y| (left..right.min(self.width)).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let offset = ((y * self.width + x) * 4) as usize;
                    pixels[offset..offset + 3].iter().any(|value| *value < 128)
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
            let (width, height) = panel_size(&input);
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
                surface.dark_pixels(228, 0, rect.right, 40) > 10,
                "{name}: composition title is outside the visible window"
            );
            if error || count == 0 {
                assert!(
                    surface.dark_pixels(12, 44, rect.right - 12, 84) > 10,
                    "{name}: empty candidate state is invisible"
                );
                assert!(
                    surface.dark_pixels(12, 88, rect.right - 12, 128) > 10,
                    "{name}: raw-letter submission guidance is invisible"
                );
                assert_eq!(
                    surface.dark_pixels(160, 44, rect.right, 128),
                    0,
                    "{name}: stale candidates are still visible during engine failure"
                );
            } else {
                for slot in 0..count {
                    let left = slot * 112 + 8;
                    assert!(surface.dark_pixels(left, 44, left + 96, 84) > 10);
                    assert!(surface.dark_pixels(left, 88, left + 96, 128) > 10);
                }
            }
            surface.save_preview(name);
            assert!(!unsafe { IsWindowVisible(panel.window) }.as_bool());
        }
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    }
}
