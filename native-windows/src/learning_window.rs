//! Dedicated native helper in the installed DLL; never activates the TSF candidate window.
use crate::learning::{Store, Word};
use std::cell::RefCell;
use std::path::PathBuf;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::Dialogs::*;
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODS_FOCUS, ODS_SELECTED};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, PWSTR, Result, w};

pub fn launch(dll: PathBuf) -> std::io::Result<()> {
    let root = std::env::var_os("WINDIR")
        .ok_or_else(|| std::io::Error::other("Missing Windows directory"))?;
    use std::os::windows::process::CommandExt;
    std::process::Command::new(PathBuf::from(root).join("System32/rundll32.exe"))
        .raw_arg(format!("\"{}\",LingoMateLearningW", dll.display()))
        .spawn()
        .map(|_| ())
}
struct Font(HFONT);
impl Drop for Font {
    fn drop(&mut self) {
        let _ = unsafe { DeleteObject(self.0.into()) };
    }
}
struct Window {
    store: Store,
    search: HWND,
    filter: HWND,
    list: HWND,
    status: HWND,
    master: HWND,
    delete: HWND,
    refresh: HWND,
    export: HWND,
    visible: RefCell<Vec<Word>>,
}
impl Window {
    fn selected(&self) -> Option<Word> {
        let index = unsafe { SendMessageW(self.list, LB_GETCURSEL, None, None) }.0;
        usize::try_from(index)
            .ok()
            .and_then(|i| self.visible.borrow().get(i).cloned())
    }
    fn status(&self, text: &str) {
        let _ = unsafe { SetWindowTextW(self.status, &HSTRING::from(text)) };
    }
    fn reload(&self) {
        let id = self.selected().map(|w| w.id());
        let words = match self.store.snapshot() {
            Ok(words) => words,
            Err(error) => {
                self.visible.borrow_mut().clear();
                unsafe {
                    SendMessageW(self.list, LB_RESETCONTENT, None, None);
                }
                self.status(&error);
                self.selection();
                return;
            }
        };
        let mut query = [0u16; 256];
        let size = unsafe { GetWindowTextW(self.search, &mut query) } as usize;
        let query = String::from_utf16_lossy(&query[..size])
            .trim()
            .to_ascii_lowercase();
        let filter = unsafe { SendMessageW(self.filter, CB_GETCURSEL, None, None) }.0;
        let visible: Vec<_> = words
            .iter()
            .filter(|w| {
                (filter == 0 || w.mastered == (filter == 2))
                    && (query.is_empty()
                        || w.english.to_ascii_lowercase().contains(&query)
                        || w.chinese.contains(&query))
            })
            .cloned()
            .collect();
        *self.visible.borrow_mut() = visible.clone();
        unsafe {
            SendMessageW(self.list, LB_RESETCONTENT, None, None);
            for word in &visible {
                let row = HSTRING::from(format!(
                    "{} {}  ·  {}  ·  {} 次  ·  {}",
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
                    self.list,
                    LB_ADDSTRING,
                    None,
                    Some(LPARAM(row.as_ptr() as isize)),
                );
            }
            if let Some(index) = visible.iter().position(|w| Some(w.id()) == id) {
                SendMessageW(self.list, LB_SETCURSEL, Some(WPARAM(index)), None);
            }
        }
        self.status(&if words.is_empty() {
            "暂无记录。确认输出一个英文候选即可开始。".into()
        } else {
            format!(
                "共 {} 个词条 · 已掌握 {} · 当前显示 {}",
                words.len(),
                words.iter().filter(|w| w.mastered).count(),
                visible.len()
            )
        });
        self.selection();
    }
    fn selection(&self) {
        let selected = self.selected();
        unsafe {
            let _ = EnableWindow(self.master, selected.is_some());
            let _ = EnableWindow(self.delete, selected.is_some());
            let _ = SetWindowTextW(
                self.master,
                if selected.is_some_and(|w| w.mastered) {
                    w!("重新学习")
                } else {
                    w!("标记掌握")
                },
            );
        }
    }
    fn export(&self, owner: HWND) {
        let mut path = [0u16; 32768];
        for (i, c) in "LingoMate-words.csv".encode_utf16().enumerate() {
            path[i] = c;
        }
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFilter: w!("CSV 文件\0*.csv\0\0"),
            lpstrFile: PWSTR(path.as_mut_ptr()),
            nMaxFile: path.len() as u32,
            lpstrDefExt: w!("csv"),
            Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
            ..Default::default()
        };
        if !unsafe { GetSaveFileNameW(&mut dialog) }.as_bool() {
            return;
        }
        let size = path.iter().position(|v| *v == 0).unwrap_or(path.len());
        let result = self.store.snapshot().and_then(|words| {
            std::fs::write(
                PathBuf::from(String::from_utf16_lossy(&path[..size])),
                crate::learning::csv(&words),
            )
            .map_err(|_| "导出失败，请选择其他保存位置。".into())
        });
        match result {
            Ok(()) => self.status("已导出全部本地词条。"),
            Err(error) => self.status(&error),
        }
    }
    fn layout(&self, owner: HWND) {
        let mut rect = RECT::default();
        if unsafe { GetClientRect(owner, &mut rect) }.is_err() {
            return;
        }
        let width = rect.right.max(440);
        let height = rect.bottom.max(400);
        let controls = [
            (self.search, 16, 92, width - 168, 44),
            (self.filter, width - 144, 92, 128, 220),
            (self.list, 16, 144, width - 32, height - 276),
            (self.status, 16, height - 124, width - 32, 44),
        ];
        for (control, x, y, w, h) in controls {
            let _ = unsafe { MoveWindow(control, x, y, w, h, true) };
        }
        if let Ok(label) = unsafe { GetDlgItem(Some(owner), 9) } {
            let _ = unsafe { MoveWindow(label, 16, 66, width - 32, 24, true) };
        }
        let button = (width - 44) / 4;
        for (i, control) in [self.master, self.delete, self.refresh, self.export]
            .into_iter()
            .enumerate()
        {
            let _ = unsafe {
                MoveWindow(
                    control,
                    16 + i as i32 * (button + 4),
                    height - 64,
                    button,
                    44,
                    true,
                )
            };
        }
    }
}
fn child(
    owner: HWND,
    instance: HINSTANCE,
    font: HFONT,
    class: PCWSTR,
    text: PCWSTR,
    id: usize,
    style: WINDOW_STYLE,
) -> Result<HWND> {
    let tabstop = if matches!(id, 1..=3 | 5..=8) {
        WS_TABSTOP
    } else {
        WINDOW_STYLE(0)
    };
    let control = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            text,
            WS_CHILD | WS_VISIBLE | tabstop | style,
            16,
            16,
            400,
            44,
            Some(owner),
            Some(HMENU(id as *mut _)),
            Some(instance),
            None,
        )?
    };
    unsafe {
        SendMessageW(
            control,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
    }
    Ok(control)
}
fn run(instance: HINSTANCE, preview: bool, narrow: bool, checks: bool) -> Result<()> {
    let preview_root =
        std::env::temp_dir().join(format!("lingomate-learning-preview-{}", std::process::id()));
    let store = if preview {
        let store = Store::new(preview_root.clone());
        for word in [
            Word::new("learn".into(), "学习".into(), "v.".into()),
            Word::new("apple".into(), "苹果".into(), "n.".into()),
        ] {
            let _ = store.record(word);
        }
        store
    } else {
        crate::learning::local_store().map_err(|_| windows::core::Error::from_hresult(E_FAIL))?
    };
    let class = WNDCLASSW {
        hInstance: instance,
        lpszClassName: w!("LingoMate.LocalLearning.v1"),
        lpfnWndProc: Some(procedure),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&class);
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.lpszClassName,
            if preview {
                w!("灵果 · 本地学习 · 隔离样例")
            } else {
                w!("灵果 · 本地单词学习")
            },
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            if narrow { 480 } else { 620 },
            560,
            None,
            None,
            Some(instance),
            None,
        )?
    };
    let font = Font(unsafe {
        CreateFontW(
            -16,
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
    });
    child(
        hwnd,
        instance,
        font.0,
        w!("STATIC"),
        w!("仅保存在本机，不自动上传。掌握状态由你标记。"),
        10,
        WINDOW_STYLE(0),
    )?;
    child(
        hwnd,
        instance,
        font.0,
        w!("STATIC"),
        w!("搜索英文或中文"),
        9,
        WINDOW_STYLE(0),
    )?;
    let state = Box::new(Window {
        store,
        search: child(
            hwnd,
            instance,
            font.0,
            w!("EDIT"),
            w!(""),
            1,
            WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
        )?,
        filter: child(
            hwnd,
            instance,
            font.0,
            w!("COMBOBOX"),
            w!(""),
            2,
            WINDOW_STYLE((CBS_DROPDOWNLIST | CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32)
                | WS_VSCROLL,
        )?,
        list: child(
            hwnd,
            instance,
            font.0,
            w!("LISTBOX"),
            w!(""),
            3,
            WS_BORDER | WS_VSCROLL | WS_HSCROLL | WINDOW_STYLE(LBS_NOTIFY as u32),
        )?,
        status: child(
            hwnd,
            instance,
            font.0,
            w!("STATIC"),
            w!(""),
            4,
            WINDOW_STYLE(0),
        )?,
        master: child(
            hwnd,
            instance,
            font.0,
            w!("BUTTON"),
            w!("标记掌握"),
            5,
            WINDOW_STYLE(0),
        )?,
        delete: child(
            hwnd,
            instance,
            font.0,
            w!("BUTTON"),
            w!("删除词条"),
            6,
            WINDOW_STYLE(0),
        )?,
        refresh: child(
            hwnd,
            instance,
            font.0,
            w!("BUTTON"),
            w!("刷新"),
            7,
            WINDOW_STYLE(0),
        )?,
        export: child(
            hwnd,
            instance,
            font.0,
            w!("BUTTON"),
            w!("导出全部…"),
            8,
            WINDOW_STYLE(0),
        )?,
        visible: RefCell::new(Vec::new()),
    });
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&*state as *const Window) as isize);
        SendMessageW(
            state.search,
            0x1501, // EM_SETCUEBANNER: supported EDIT control message.
            Some(WPARAM(1)),
            Some(LPARAM(w!("搜索英文或中文").as_ptr() as isize)),
        );
        for text in [w!("全部"), w!("已学习"), w!("已掌握")] {
            SendMessageW(
                state.filter,
                CB_ADDSTRING,
                None,
                Some(LPARAM(text.as_ptr() as isize)),
            );
        }
        SendMessageW(
            state.filter,
            CB_SETITEMHEIGHT,
            Some(WPARAM(usize::MAX)),
            Some(LPARAM(40)),
        );
        SendMessageW(
            state.filter,
            CB_SETITEMHEIGHT,
            Some(WPARAM(0)),
            Some(LPARAM(44)),
        );
        SendMessageW(state.filter, CB_SETCURSEL, Some(WPARAM(0)), None);
        SendMessageW(
            state.list,
            LB_SETITEMHEIGHT,
            Some(WPARAM(0)),
            Some(LPARAM(44)),
        );
        SendMessageW(state.list, LB_SETHORIZONTALEXTENT, Some(WPARAM(1000)), None);
    }
    state.layout(hwnd);
    state.reload();
    if checks {
        window_checks(hwnd, &state);
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_dir_all(preview_root);
        return Ok(());
    }
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetFocus(Some(state.search));
    }
    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            if !IsDialogMessageW(hwnd, &message).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    if preview {
        let _ = std::fs::remove_dir_all(preview_root);
    }
    Ok(())
}
unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if message == WM_MEASUREITEM && wparam.0 == 2 {
            let item = unsafe { &mut *(lparam.0 as *mut MEASUREITEMSTRUCT) };
            item.itemHeight = 44;
            return LRESULT(1);
        }
        if message == WM_DRAWITEM && wparam.0 == 2 {
            let item = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
            let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
            let background = if selected {
                COLOR_HIGHLIGHT
            } else {
                COLOR_WINDOW
            };
            let foreground = if selected {
                COLOR_HIGHLIGHTTEXT
            } else {
                COLOR_WINDOWTEXT
            };
            let index = if item.itemID == u32::MAX {
                unsafe { SendMessageW(item.hwndItem, CB_GETCURSEL, None, None) }
                    .0
                    .max(0) as usize
            } else {
                item.itemID as usize
            };
            let value = ["全部", "已学习", "已掌握"]
                .get(index)
                .copied()
                .unwrap_or("");
            let mut text: Vec<_> = value.encode_utf16().collect();
            let mut rect = item.rcItem;
            rect.left += 8;
            unsafe {
                let saved = SaveDC(item.hDC);
                FillRect(item.hDC, &item.rcItem, GetSysColorBrush(background));
                let font = SendMessageW(item.hwndItem, WM_GETFONT, None, None).0;
                if font != 0 {
                    SelectObject(item.hDC, HGDIOBJ(font as *mut _));
                }
                SetBkMode(item.hDC, TRANSPARENT);
                SetTextColor(item.hDC, COLORREF(GetSysColor(foreground)));
                DrawTextW(
                    item.hDC,
                    &mut text,
                    &mut rect,
                    DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
                );
                if item.itemState.0 & ODS_FOCUS.0 != 0 {
                    let _ = DrawFocusRect(item.hDC, &item.rcItem);
                }
                let _ = RestoreDC(item.hDC, saved);
            }
            return LRESULT(1);
        }
        if message == WM_DESTROY {
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                PostQuitMessage(0);
            }
            return LRESULT(0);
        }
        if message == WM_GETMINMAXINFO {
            let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            info.ptMinTrackSize = POINT { x: 480, y: 440 };
            return LRESULT(0);
        }
        let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Window;
        if !pointer.is_null() {
            let state = unsafe { &*pointer };
            if message == WM_SIZE {
                state.layout(hwnd);
                return LRESULT(0);
            }
            if message == WM_COMMAND {
                let id = wparam.0 & 0xffff;
                let code = (wparam.0 >> 16) as u32;
                match id {
                    1 if code == EN_CHANGE => state.reload(),
                    2 if code == CBN_SELCHANGE => state.reload(),
                    3 if code == LBN_SELCHANGE => state.selection(),
                    5 if code == BN_CLICKED => {
                        if let Some(word) = state.selected() {
                            match state.store.mastered(&word.id(), !word.mastered) {
                                Ok(()) => state.reload(),
                                Err(error) => state.status(&error),
                            }
                        }
                    }
                    6 if code == BN_CLICKED => {
                        if let Some(word) = state.selected()
                            && unsafe {
                                MessageBoxW(
                                    Some(hwnd),
                                    &HSTRING::from(format!(
                                        "删除“{}”的本地记录？再次确认输出该词时，会重新建立记录。",
                                        word.english
                                    )),
                                    w!("删除词条"),
                                    MB_OKCANCEL | MB_ICONQUESTION,
                                )
                            } == IDOK
                        {
                            match state.store.delete(&word.id()) {
                                Ok(()) => state.reload(),
                                Err(error) => state.status(&error),
                            }
                        }
                    }
                    7 if code == BN_CLICKED => state.reload(),
                    8 if code == BN_CLICKED => state.export(hwnd),
                    _ => {}
                }
                return LRESULT(0);
            }
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }))
    .unwrap_or_else(|_| unsafe { DefWindowProcW(hwnd, message, wparam, lparam) })
}
// Exact rundll32 Unicode callback ABI. Command-line payload is deliberately ignored.
#[unsafe(no_mangle)]
pub extern "system" fn LingoMateLearningW(
    _owner: HWND,
    instance: HINSTANCE,
    _command: PCWSTR,
    _show: i32,
) {
    let _ = std::panic::catch_unwind(|| run(instance, false, false, false));
}
#[unsafe(no_mangle)]
pub extern "system" fn LingoMateLearningPreviewW(
    _owner: HWND,
    instance: HINSTANCE,
    _command: PCWSTR,
    _show: i32,
) {
    let _ = std::panic::catch_unwind(|| run(instance, true, false, false));
}

#[unsafe(no_mangle)]
pub extern "system" fn LingoMateLearningNarrowPreviewW(
    _owner: HWND,
    instance: HINSTANCE,
    _command: PCWSTR,
    _show: i32,
) {
    let _ = std::panic::catch_unwind(|| run(instance, true, true, false));
}
fn window_checks(owner: HWND, state: &Window) {
    assert_eq!(
        unsafe { SendMessageW(state.list, LB_GETCOUNT, None, None) }.0,
        2
    );
    for control in [
        state.master,
        state.delete,
        state.refresh,
        state.export,
        state.search,
    ] {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(control, &mut rect) }.unwrap();
        assert!(rect.bottom - rect.top >= 44);
    }
    assert!(
        unsafe {
            SendMessageW(
                state.filter,
                CB_GETITEMHEIGHT,
                Some(WPARAM(usize::MAX)),
                None,
            )
        }
        .0 >= 40
    );
    assert!(unsafe { SendMessageW(state.list, LB_GETITEMHEIGHT, Some(WPARAM(0)), None) }.0 >= 44);
    let word = state.visible.borrow()[0].clone();
    state.store.mastered(&word.id(), true).unwrap();
    state.reload();
    unsafe {
        SendMessageW(state.filter, CB_SETCURSEL, Some(WPARAM(2)), None);
    }
    state.reload();
    assert_eq!(state.visible.borrow().len(), 1);
    assert!(state.visible.borrow()[0].mastered);
    let _ = unsafe { SetWindowTextW(state.search, w!("missing")) };
    state.reload();
    assert!(state.visible.borrow().is_empty());
    state.store.delete(&word.id()).unwrap();
    assert_eq!(state.store.snapshot().unwrap().len(), 1);
    assert!(unsafe { IsWindow(Some(owner)) }.as_bool());
}
#[cfg(test)]
#[test]
fn isolated_native_learning_window_wide_and_narrow() {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }.unwrap().0);
    for narrow in [false, true] {
        run(instance, true, narrow, true).unwrap();
    }
}
