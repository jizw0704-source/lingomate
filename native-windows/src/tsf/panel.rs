use super::{Guard, Key, MODULE, State};
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

pub(super) struct Panel {
    window: HWND,
    owner: *mut Weak<State>,
    _guard: Guard,
}
impl Panel {
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
                560,
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
        let count = input
            .frame
            .candidates
            .len()
            .saturating_sub(input.page() * 5)
            .min(5);
        let extra = if input.expanded {
            input
                .frame
                .candidates
                .get(input.selected)
                .map_or(0, |c| c.translations.len().min(6))
        } else {
            0
        };
        unsafe {
            let _ = SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                0,
                0,
                (count.max(2) * 112) as i32,
                132 + extra as i32 * 44,
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
        if message == WM_LBUTTONUP {
            let x = (lparam.0 as u16) as i16 as i32;
            let y = ((lparam.0 >> 16) as u16) as i16 as i32;
            if y < 44 {
                match x / 44 {
                    0 => state.key(Key::Previous),
                    1 => state.key(Key::Next),
                    2 => state.key(Key::Tab(false)),
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
    let mut rect = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + 40,
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
        FillRect(dc, &ps.rcPaint, HBRUSH(GetStockObject(WHITE_BRUSH).0));
        SetBkMode(dc, TRANSPARENT);
    }
    let input = state.input.borrow();
    text(dc, "‹", 16, 0, 28, COLORREF(0x101010));
    text(dc, "›", 60, 0, 28, COLORREF(0x101010));
    text(dc, "译法", 90, 0, 44, COLORREF(0x101010));
    let title = if input.error {
        format!("{} · 引擎未连接 · 回车提交字母", input.raw)
    } else {
        format!(
            "{}   {}/{}",
            input.raw,
            input.page() + 1,
            input.page_count().max(1)
        )
    };
    text(dc, &title, 140, 0, 410, COLORREF(0x4A4A4A));
    for (slot, candidate) in input
        .frame
        .candidates
        .iter()
        .skip(input.page() * 5)
        .take(5)
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
            text(dc, &translation.word, x + 8, 88, 96, COLORREF(0x4A4A4A));
        }
    }
    if input.expanded
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
                &format!("{marker}{} {} · {}", sense.word, sense.pos, sense.note),
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
        let _ = EndPaint(window, &ps);
    }
}
