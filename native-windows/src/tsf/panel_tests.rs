use super::*;
use crate::bridge::{Candidate, Translation};
use crate::input::Input;
use std::ffi::c_void;
use windows::Win32::UI::HiDpi::{AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext};

#[test]
fn mouse_leave_does_not_rearm_itself_or_flood_the_host_queue() {
    let foreground = unsafe { GetForegroundWindow() };
    let state = preview_state(crate::preferences::Theme::Light, 18);
    let panel = Panel::new(Rc::downgrade(&state)).unwrap();
    let window = panel.window;
    *state.panel.borrow_mut() = Some(panel);
    let leave = windows::Win32::UI::Controls::WM_MOUSELEAVE;
    unsafe {
        SendMessageW(window, leave, None, None);
    }
    let mut generated = 0;
    for _ in 0..32 {
        let mut message = MSG::default();
        if !unsafe { PeekMessageW(&mut message, Some(window), leave, leave, PM_REMOVE) }.as_bool() {
            break;
        }
        generated += 1;
        unsafe {
            DispatchMessageW(&message);
        }
    }
    // A hidden window has no real cursor inside. Re-arming on leave would
    // immediately post another leave forever; never use an unbounded pump.
    assert_eq!(
        generated, 0,
        "leave callback generated its own next leave notification"
    );
    assert_eq!(unsafe { GetForegroundWindow() }, foreground);
}

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
            std::slice::from_raw_parts(self.bits.cast(), (self.width * self.height * 4) as usize)
        }
    }
    fn ink_pixels(&self, left: i32, top: i32, right: i32, bottom: i32) -> usize {
        let pixels = self.pixels();
        (top.max(0)..bottom.min(self.height))
            .flat_map(|y| (left.max(0)..right.min(self.width)).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let offset = ((y * self.width + x) * 4) as usize;
                pixels[offset..offset + 3]
                    .iter()
                    .zip(&pixels[..3])
                    .any(|(value, background)| value.abs_diff(*background) > 32)
            })
            .count()
    }
    fn ink(&self, r: BoxRect) -> usize {
        self.ink_pixels(r.x, r.y, r.x + r.width, r.y + r.height)
    }
    fn color_bounds(&self, r: BoxRect, color: COLORREF) -> Option<BoxRect> {
        // DIB bytes are BGR, while COLORREF has red in its low byte. Only
        // Strong glyph pixels count; ClearType fringes remain optional.
        let expected = [
            ((color.0 >> 16) & 0xff) as u8,
            ((color.0 >> 8) & 0xff) as u8,
            (color.0 & 0xff) as u8,
        ];
        self.pixel_bounds(r, |pixel| {
            pixel[..3]
                .iter()
                .zip(expected)
                .all(|(actual, wanted)| actual.abs_diff(wanted) <= 40)
        })
    }
    /// Bounds of glyphs in a `role_palette` capture whose role darkens
    /// `channel` (DIB byte order: blue, green, red).
    fn role_bounds(&self, r: BoxRect, channel: usize) -> Option<BoxRect> {
        self.pixel_bounds(r, |pixel| pixel[channel] != u8::MAX)
    }
    fn pixel_bounds(&self, r: BoxRect, matches: impl Fn(&[u8]) -> bool) -> Option<BoxRect> {
        let mut bounds: Option<(i32, i32, i32, i32)> = None;
        let pixels = self.pixels();
        for y in r.y.max(0)..(r.y + r.height).min(self.height) {
            for x in r.x.max(0)..(r.x + r.width).min(self.width) {
                let offset = ((y * self.width + x) * 4) as usize;
                if matches(&pixels[offset..offset + 4]) {
                    bounds = Some(bounds.map_or((x, y, x, y), |(left, top, right, bottom)| {
                        (left.min(x), top.min(y), right.max(x), bottom.max(y))
                    }));
                }
            }
        }
        bounds.map(|(x, y, right, bottom)| BoxRect {
            x,
            y,
            width: right - x + 1,
            height: bottom - y + 1,
        })
    }
    fn differences(&self, other: &Self, r: BoxRect) -> usize {
        assert_eq!((self.width, self.height), (other.width, other.height));
        let left = self.pixels();
        let right = other.pixels();
        (r.y.max(0)..(r.y + r.height).min(self.height))
            .flat_map(|y| (r.x.max(0)..(r.x + r.width).min(self.width)).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let offset = ((y * self.width + x) * 4) as usize;
                left[offset..offset + 3] != right[offset..offset + 3]
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
    for (theme_name, theme) in [
        ("light", crate::preferences::Theme::Light),
        ("dark", crate::preferences::Theme::Dark),
    ] {
        let state = hidden_preview(theme);
        let mut unavailable = None;
        let mut navigating = None;
        for (name, busy, error, count) in [
            ("engine-unavailable", false, true, 0),
            ("engine-unavailable-stale", false, true, 3),
            ("loading-empty", true, false, 0),
            ("navigating-with-frame", true, false, 3),
            ("no-match", false, false, 0),
            ("one", false, false, 1),
            ("two", false, false, 2),
            ("three", false, false, 3),
            ("four", false, false, 4),
            ("five", false, false, 5),
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
            *state.input.borrow_mut() = input;
            state.busy.set(busy);
            let panel = state.panel.borrow();
            panel.as_ref().unwrap().loading.set(busy.then(Instant::now));
            drop(panel);
            let (l, surface) = capture(&state);
            assert!(
                surface.ink(l.title_rect()) > 10,
                "{theme_name}/{name}: composition title is outside its visible rectangle"
            );
            assert!(surface.ink(l.mode_rect()) > 5);
            for action in 0..5 {
                assert!(
                    surface.ink(l.header_item(action)) > 2,
                    "{theme_name}/{name}: header action {action} is invisible"
                );
            }
            if error || count == 0 {
                let body = l.body_rect();
                assert!(
                    surface.ink(BoxRect {
                        height: l.row,
                        ..body
                    }) > 10,
                    "{theme_name}/{name}: empty candidate status is invisible"
                );
                assert!(
                    surface.ink(BoxRect {
                        y: body.y + l.row,
                        height: l.row,
                        ..body
                    }) > 10,
                    "{theme_name}/{name}: raw-letter submission guidance is invisible"
                );
                assert_eq!(
                    l.count, 0,
                    "empty/error frames must have no candidate targets"
                );
                if name == "engine-unavailable-stale" {
                    assert_eq!(
                        surface.differences(unavailable.as_ref().unwrap(), body),
                        0,
                        "cached candidates leaked into the failure-state painter"
                    );
                }
            } else {
                assert_eq!(l.count, count);
                for slot in 0..count {
                    assert!(surface.ink(l.candidate_rect(slot, false)) > 10);
                    assert!(surface.ink(l.candidate_rect(slot, true)) > 5);
                }
                if name == "three" {
                    let (busy_surface, busy_tips) = navigating.as_ref().unwrap();
                    assert_eq!(surface.differences(busy_surface, l.body_rect()), 0);
                    assert_eq!(&tooltip_texts(&state, l), busy_tips);
                }
            }
            surface.save_preview(&format!("{theme_name}-{name}"));
            if name == "engine-unavailable" {
                unavailable = Some(surface);
            } else if name == "navigating-with-frame" {
                // Navigation keeps the last frame visible while the worker
                // selects the next view; submission remains blocked by State.
                navigating = Some((surface, tooltip_texts(&state, l)));
            }
            assert_hidden(&state);
        }
    }
    assert_eq!(unsafe { GetForegroundWindow() }, foreground);
}

fn hidden_preview(theme: crate::preferences::Theme) -> Rc<State> {
    // This constructor has no preference, vocabulary or installation reads.
    let state = preview_state(theme, 18);
    let host = unsafe { GetThreadDpiAwarenessContext() };
    let panel = Panel::new(Rc::downgrade(&state)).expect("Isolated candidate window unavailable");
    let style = unsafe { GetWindowLongPtrW(panel.window, GWL_EXSTYLE) } as u32;
    assert_ne!(style & WS_EX_NOACTIVATE.0, 0);
    assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
    unsafe {
        assert!(
            AreDpiAwarenessContextsEqual(
                GetWindowDpiAwarenessContext(panel.window),
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
            )
            .as_bool(),
            "candidate window would be bitmap-stretched in hosts without per-monitor DPI"
        );
        assert!(
            AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), host).as_bool(),
            "the host thread DPI context leaked"
        );
    }
    *state.panel.borrow_mut() = Some(panel);
    assert_hidden(&state);
    state
}

#[test]
fn host_points_map_between_monitor_dpi_views() {
    // A 150% monitor seen by a system-DPI-aware host and by the panel itself.
    let host = RECT {
        left: -653,
        top: -1440,
        right: 1907,
        bottom: 0,
    };
    let panel = RECT {
        left: -979,
        top: -2160,
        right: 2861,
        bottom: 0,
    };
    let point = |x, y| POINT { x, y };
    assert_eq!(
        map_point(point(-653, -1440), host, panel),
        point(-979, -2160)
    );
    assert_eq!(map_point(point(627, -720), host, panel), point(941, -1080));
    assert_eq!(map_point(point(100, -200), panel, panel), point(100, -200));
}

fn assert_hidden(state: &State) {
    let window = state.panel.borrow().as_ref().unwrap().window;
    assert!(!unsafe { IsWindowVisible(window) }.as_bool());
    assert!(state.worker.borrow().is_none());
    assert!(state.document.borrow().is_none());
    assert!(state.input.borrow().pending_memory.is_none());
    assert!(state.input.borrow().pending_study.is_none());
}

fn capture(state: &Rc<State>) -> (Layout, Surface) {
    let window = state.panel.borrow().as_ref().unwrap().window;
    let host = unsafe { GetThreadDpiAwarenessContext() };
    let scope = DpiScope::window(window);
    let l = layout(window, state);
    unsafe {
        SetWindowPos(
            window,
            None,
            0,
            0,
            l.width(),
            l.height(),
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
        .unwrap();
    }
    let mut client = RECT::default();
    unsafe { GetClientRect(window, &mut client) }.unwrap();
    drop(scope);
    assert_eq!((client.right, client.bottom), (l.width(), l.height()));
    let surface = Surface::new(client.right, client.bottom);
    // Exercise the actual HWND callback and WM_PAINT's GDI painter. A hidden
    // window never becomes an input source, takes focus or captures host text.
    unsafe {
        SendMessageW(
            window,
            WM_PRINTCLIENT,
            Some(WPARAM(surface.dc.0 as usize)),
            Some(LPARAM(PRF_CLIENT as isize)),
        );
        assert!(GdiFlush().as_bool());
        assert!(AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), host).as_bool());
    }
    assert_hidden(state);
    (l, surface)
}

// Theme colors cannot be matched pixel by pixel: ClearType tints glyph edges
// and thin strokes never reach the text color. On a white role surface each
// text role darkens a single channel, and every smoothing mode blends channels
// independently, so any changed channel identifies its role exactly.
const ACCENT_ROLE: usize = 2;
const SECONDARY_ROLE: usize = 0;

fn role_palette() -> appearance::Palette {
    let white = COLORREF(0xFF_FF_FF);
    let text = COLORREF(0xFF_00_FF);
    appearance::Palette {
        background: white,
        foreground: text,
        secondary: COLORREF(0x00_FF_FF),
        selected: white,
        selected_text: text,
        accent: COLORREF(0xFF_FF_00),
        divider: white,
        dark: false,
    }
}

fn capture_roles(state: &Rc<State>) -> Surface {
    let theme = state
        .panel
        .borrow()
        .as_ref()
        .unwrap()
        .palette
        .replace(role_palette());
    let (_, surface) = capture(state);
    state.panel.borrow().as_ref().unwrap().set_palette(theme);
    surface
}

#[test]
fn hidden_candidate_window_wraps_long_chinese_and_preserves_full_tooltips() {
    let foreground = unsafe { GetForegroundWindow() };
    let state = hidden_preview(crate::preferences::Theme::Light);
    let chinese = "这是用于隔离绘图测试的长中文候选句子需要完整换行显示而不是截断";
    let english = "a complete synthetic English translation that is wider than its compact label";
    let note = "仅用于测试的完整释义说明";
    let example = "A synthetic example must remain complete in the tooltip.";
    let raw = "changjuziceshi".repeat(8);
    {
        let mut input = state.input.borrow_mut();
        input.raw = raw.clone();
        input.expanded = false;
        input.frame.candidates = vec![Candidate {
            text: chinese.into(),
            translations: vec![Translation {
                word: english.into(),
                pos: "n.".into(),
                note: note.into(),
                example: example.into(),
            }],
            ..Default::default()
        }];
    }
    let window = state.panel.borrow().as_ref().unwrap().window;
    let (l, surface) = capture(&state);
    let chinese_rect = l.candidate_rect(0, false);
    let palette = state.panel.borrow().as_ref().unwrap().palette.get();
    let chinese_ink = surface
        .color_bounds(chinese_rect, palette.selected_text)
        .expect("long Chinese candidate was not painted");
    assert!(chinese_rect.height > l.row);
    assert!(
        chinese_ink.height > l.row,
        "long Chinese candidate was truncated to one line instead of wrapping"
    );
    let tips = tooltip_texts(&state, l);
    assert!(
        tips.iter()
            .any(|(r, text)| { *r == l.title_rect() && text == raw })
    );
    assert!(
        tips.iter()
            .any(|(r, text)| { *r == chinese_rect && text == chinese })
    );
    assert!(tips.iter().any(|(r, text)| {
        *r == l.candidate_rect(0, true) && *text == format!("{english} n.\n{note}\n{example}")
    }));
    assert!(surface.ink(l.candidate_rect(0, true)) > 10);
    surface.save_preview("light-long-chinese");

    state
        .panel
        .borrow()
        .as_ref()
        .unwrap()
        .limit
        .set(Some(px(320, appearance::dpi(window))));
    let (narrow, narrow_surface) = capture(&state);
    assert!(narrow.narrow);
    let chinese = narrow.candidate_rect(0, false);
    let english = narrow.candidate_rect(0, true);
    assert_eq!(english.y, chinese.y + chinese.height);
    assert!(narrow_surface.ink(chinese) > 10);
    assert!(narrow_surface.ink(english) > 10);
    narrow_surface.save_preview("light-long-chinese-narrow");
    assert_eq!(unsafe { GetForegroundWindow() }, foreground);
}

#[test]
fn part_of_speech_is_muted_on_the_same_english_line_and_can_be_hidden() {
    let foreground = unsafe { GetForegroundWindow() };
    for (name, theme) in [
        ("light", crate::preferences::Theme::Light),
        ("dark", crate::preferences::Theme::Dark),
    ] {
        let state = hidden_preview(theme);
        {
            let mut input = state.input.borrow_mut();
            input.expanded = false;
            input.frame.candidates.truncate(1);
            input.frame.candidates[0].translations.truncate(1);
        }
        state.show_pos.set(true);
        let (l, shown) = capture(&state);
        shown.save_preview(&format!("{name}-pos-shown"));
        let english = l.candidate_rect(0, true);
        let roles = capture_roles(&state);
        let word = roles
            .role_bounds(english, ACCENT_ROLE)
            .expect("English word has no accent-colored glyphs");
        let pos = roles
            .role_bounds(english, SECONDARY_ROLE)
            .expect("enabled part of speech is invisible");
        assert!(pos.x >= word.x + word.width);
        assert!(pos.y < word.y + word.height && pos.y + pos.height > word.y);
        state.show_pos.set(false);
        let (_, hidden) = capture(&state);
        let roles = capture_roles(&state);
        assert!(roles.role_bounds(english, SECONDARY_ROLE).is_none());
        assert!(roles.role_bounds(english, ACCENT_ROLE).is_some());
        assert!(shown.differences(&hidden, english) > 2);
        assert_eq!(shown.differences(&hidden, l.title_rect()), 0);
        assert_eq!(shown.differences(&hidden, l.candidate_rect(0, false)), 0);
        assert_eq!(
            state.input.borrow().frame.candidates[0].translations[0].word,
            "study"
        );
        let tips = tooltip_texts(&state, l);
        let tip = tips
            .iter()
            .find(|(r, _)| *r == english)
            .unwrap()
            .1
            .to_string();
        assert!(tip.starts_with("study\n"));
        assert!(!tip.contains("v."));
        shown.save_preview(&format!("{name}-pos-shown"));
        hidden.save_preview(&format!("{name}-pos-hidden"));
    }
    assert_eq!(unsafe { GetForegroundWindow() }, foreground);
}

#[test]
fn untranslated_candidate_keeps_a_visible_muted_placeholder() {
    let foreground = unsafe { GetForegroundWindow() };
    let state = hidden_preview(crate::preferences::Theme::Dark);
    {
        let mut input = state.input.borrow_mut();
        input.expanded = false;
        input.frame.candidates.truncate(1);
        input.frame.candidates[0].translations.clear();
    }
    let (l, surface) = capture(&state);
    let english = l.candidate_rect(0, true);
    surface.save_preview("dark-no-english");
    assert!(surface.ink(l.candidate_rect(0, false)) > 10);
    let roles = capture_roles(&state);
    assert!(roles.role_bounds(english, SECONDARY_ROLE).is_some());
    assert!(roles.role_bounds(english, ACCENT_ROLE).is_none());
    assert!(!tooltip_texts(&state, l).iter().any(|(r, _)| *r == english));
    surface.save_preview("dark-no-english");
    assert_eq!(unsafe { GetForegroundWindow() }, foreground);
}

#[test]
fn expanded_senses_render_notes_examples_footer_and_complete_paged_tooltips() {
    let foreground = unsafe { GetForegroundWindow() };
    let state = hidden_preview(crate::preferences::Theme::Dark);
    let candidate = Candidate {
        text: "学习".into(),
        translations: (0..8)
            .map(|i| Translation {
                word: format!("synthetic meaning {i}"),
                pos: "v.".into(),
                note: format!("第 {i} 项隔离测试释义"),
                example: format!("Synthetic example number {i} stays complete."),
            })
            .collect(),
        ..Default::default()
    };
    let mut input = Input {
        raw: "xuexi".into(),
        selected: 5,
        sense: 6,
        expanded: true,
        ..Default::default()
    };
    input.frame.candidates = (0..6).map(|_| candidate.clone()).collect();
    *state.input.borrow_mut() = input.clone();
    let (l, full) = capture(&state);
    assert_eq!(l.count, 1);
    assert_eq!(l.extra, 8);
    assert!(full.ink(l.details_title_rect()) > 10);
    assert!(full.ink(l.footer_rect()) > 10);
    let palette = state.panel.borrow().as_ref().unwrap().palette.get();
    for row in l.visible_senses() {
        assert!(
            full.color_bounds(l.sense_rect(row), palette.accent)
                .is_some()
        );
    }
    let tips = tooltip_texts(&state, l);
    for row in l.visible_senses() {
        let meaning = &candidate.translations[row];
        assert!(tips.iter().any(|(r, text)| {
            *r == l.visible(l.sense_rect(row))
                && *text == format!("{} v.\n{}\n{}", meaning.word, meaning.note, meaning.example)
        }));
    }
    for candidate in &mut input.frame.candidates {
        for sense in &mut candidate.translations {
            sense.note.clear();
            sense.example.clear();
        }
    }
    *state.input.borrow_mut() = input;
    let (_, without_metadata) = capture(&state);
    for row in l.visible_senses() {
        let r = l.sense_rect(row);
        if l.visible(r) != r {
            continue;
        }
        let third = r.height / 3;
        assert!(
            full.differences(
                &without_metadata,
                BoxRect {
                    y: r.y + third,
                    height: third,
                    ..r
                }
            ) > 10,
            "expanded note was not painted"
        );
        assert!(
            full.differences(
                &without_metadata,
                BoxRect {
                    y: r.y + third * 2,
                    height: r.height - third * 2,
                    ..r
                }
            ) > 10,
            "expanded example was not painted"
        );
    }
    full.save_preview("dark-expanded-meaning-page-two");
    assert_eq!(unsafe { GetForegroundWindow() }, foreground);
}

#[test]
fn scrolled_candidate_geometry_clips_targets_below_the_fixed_header() {
    for dpi in [96, 144, 192] {
        for width in [320, 560] {
            let full = Layout::with_lengths(&[9, 32, 240, 12, 80], 6, 18, dpi, px(width, dpi));
            let viewport_height = px(240, dpi);
            let first = full.with_viewport(viewport_height, -1);
            assert_eq!(first.scroll_offset(), 0);
            assert_eq!(first.height(), viewport_height);
            assert_eq!(first.full_height(), full.full_height());
            assert!(first.scroll_max() > 0);
            let last = full.with_viewport(viewport_height, i32::MAX);
            assert_eq!(last.scroll_offset(), first.scroll_max());
            assert_eq!(last.full_height(), full.full_height());
            assert_eq!(
                last.footer_rect().y + last.footer_rect().height,
                last.height() - last.padding
            );
            for l in [
                first,
                full.with_viewport(viewport_height, first.scroll_max() / 2),
                last,
            ] {
                assert_eq!(l.title_rect(), full.title_rect());
                assert_eq!(l.mode_rect(), full.mode_rect());
                for action in 0..5 {
                    let header = l.header_item(action);
                    assert_eq!(header, full.header_item(action));
                    assert_eq!(
                        l.hit(header.x + header.width / 2, header.y + header.height / 2),
                        None
                    );
                }
                assert_eq!(l.hit(l.padding, l.height() - 1), None);
                for slot in 0..5 {
                    for english in [false, true] {
                        let original = full.candidate_rect(slot, english);
                        let shifted = l.candidate_rect(slot, english);
                        assert_eq!(shifted.y, original.y - l.scroll_offset());
                        let visible = l.visible(shifted);
                        if visible.height > 0 {
                            assert!(visible.y >= l.body_rect().y);
                            assert!(visible.y + visible.height <= l.height() - l.padding);
                            assert_eq!(
                                l.hit(
                                    visible.x + visible.width / 2,
                                    visible.y + visible.height / 2,
                                ),
                                Some((slot, english))
                            );
                        }
                    }
                }
                for row in 0..6 {
                    let visible = l.visible(l.sense_rect(row));
                    if visible.height > 0 {
                        assert!(visible.y >= l.body_rect().y);
                        assert!(visible.y + visible.height <= l.height() - l.padding);
                        assert_eq!(
                            l.hit(
                                visible.x + visible.width / 2,
                                visible.y + visible.height / 2
                            ),
                            None
                        );
                    }
                }
                let above = BoxRect {
                    x: l.padding,
                    y: l.body_rect().y - l.row,
                    width: l.column,
                    height: l.row,
                };
                let below = BoxRect {
                    y: l.height(),
                    ..above
                };
                assert_eq!(l.visible(above).height, 0);
                assert_eq!(l.visible(below).height, 0);
            }
        }
    }
}
