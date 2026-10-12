//! OS appearance is read on repaint; no event subscriptions survive DLL unload.
use crate::preferences::Theme;
use windows::UI::ViewManagement::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::WinRT::*;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: COLORREF,
    pub foreground: COLORREF,
    pub secondary: COLORREF,
    pub selected: COLORREF,
    pub selected_text: COLORREF,
    pub dark: bool,
}
pub fn palette(theme: Theme) -> Palette {
    let mut contrast = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    if unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok()
        && contrast.dwFlags.contains(HCF_HIGHCONTRASTON)
    {
        return unsafe {
            Palette {
                background: COLORREF(GetSysColor(COLOR_WINDOW)),
                foreground: COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                secondary: COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                selected: COLORREF(GetSysColor(COLOR_HIGHLIGHT)),
                selected_text: COLORREF(GetSysColor(COLOR_HIGHLIGHTTEXT)),
                dark: false,
            }
        };
    }
    let dark = theme == Theme::Dark || (theme == Theme::System && system_dark());
    if dark {
        Palette {
            background: COLORREF(0x191919),
            foreground: COLORREF(0xF4F4F4),
            secondary: COLORREF(0xC4C4C4),
            selected: COLORREF(0x3D3D3D),
            selected_text: COLORREF(0xFFFFFF),
            dark,
        }
    } else {
        Palette {
            background: COLORREF(0xFFFFFF),
            foreground: COLORREF(0x171717),
            secondary: COLORREF(0x555555),
            selected: COLORREF(0xE9E9E9),
            selected_text: COLORREF(0x111111),
            dark,
        }
    }
}
fn system_dark() -> bool {
    let initialized = unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.is_ok();
    let result = UISettings::new()
        .and_then(|s| s.GetColorValue(UIColorType::Foreground))
        .map(|c| 5 * u32::from(c.G) + 2 * u32::from(c.R) + u32::from(c.B) > 8 * 128)
        .unwrap_or(false);
    if initialized {
        unsafe { RoUninitialize() };
    }
    result
}
pub fn title(window: HWND, palette: Palette) {
    let dark = windows::core::BOOL::from(palette.dark);
    let _ = unsafe {
        windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            window,
            windows::Win32::Graphics::Dwm::DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const windows::core::BOOL).cast(),
            std::mem::size_of_val(&dark) as u32,
        )
    };
}
pub fn dpi(window: HWND) -> i32 {
    unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(window) }.max(96) as i32
}
pub fn px(value: i32, dpi: i32) -> i32 {
    (value * dpi + 48) / 96
}
