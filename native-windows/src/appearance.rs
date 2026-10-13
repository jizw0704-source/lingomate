//! Native theme tokens; candidate panels cache these outside their paint callbacks.
use crate::preferences::Theme;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: COLORREF,
    pub foreground: COLORREF,
    pub secondary: COLORREF,
    pub selected: COLORREF,
    pub selected_text: COLORREF,
    pub accent: COLORREF,
    pub divider: COLORREF,
    pub dark: bool,
}
// Mac NativeTheme uses sRGB hex values. COLORREF stores red in the low byte.
const fn rgb(value: u32) -> COLORREF {
    COLORREF(((value >> 16) & 0xFF) | (value & 0xFF00) | ((value & 0xFF) << 16))
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
                accent: COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                divider: COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                dark: false,
            }
        };
    }
    let dark = theme == Theme::Dark || (theme == Theme::System && system_dark());
    if dark {
        Palette {
            background: rgb(0x171717),
            foreground: rgb(0xF7F7F7),
            secondary: rgb(0xBDBDBD),
            selected: rgb(0x262626),
            selected_text: rgb(0xF7F7F7),
            accent: rgb(0xB5DC70),
            divider: rgb(0x383838),
            dark,
        }
    } else {
        Palette {
            background: rgb(0xFFFFFF),
            foreground: rgb(0x101010),
            secondary: rgb(0x4A4A4A),
            selected: rgb(0xF7F7F7),
            selected_text: rgb(0x101010),
            accent: rgb(0x567700),
            divider: rgb(0xE4E4E4),
            dark,
        }
    }
}
fn system_dark() -> bool {
    // A text service must not initialize or alter its host's COM/WinRT apartment
    // just to read a color preference. Missing/invalid preferences use light.
    let mut light = 1u32;
    let mut size = std::mem::size_of_val(&light) as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            windows::core::w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut light as *mut u32).cast()),
            Some(&mut size),
        )
    }
    .is_ok()
        && light == 0
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

#[cfg(test)]
#[test]
fn srgb_native_theme_accents_use_colorref_channel_order() {
    assert_eq!(rgb(0x567700), COLORREF(0x007756));
    assert_eq!(rgb(0xB5DC70), COLORREF(0x70DCB5));
    assert_eq!(rgb(0x171717), COLORREF(0x171717));
}

#[cfg(test)]
#[test]
fn system_palette_preserves_sta_mta_and_uninitialized_apartments() {
    use windows::Win32::System::Com::*;
    fn apartment() -> std::result::Result<(i32, i32), windows::core::HRESULT> {
        let mut kind = APTTYPE::default();
        let mut qualifier = APTTYPEQUALIFIER::default();
        unsafe { CoGetApartmentType(&mut kind, &mut qualifier) }
            .map(|_| (kind.0, qualifier.0))
            .map_err(|error| error.code())
    }
    for model in [
        None,
        Some(COINIT_APARTMENTTHREADED),
        Some(COINIT_MULTITHREADED),
    ] {
        std::thread::spawn(move || {
            if let Some(model) = model {
                unsafe { CoInitializeEx(None, model) }.ok().unwrap();
            }
            let before = apartment();
            let _ = palette(Theme::System);
            assert_eq!(apartment(), before);
            if model.is_some() {
                unsafe { CoUninitialize() };
            }
        })
        .join()
        .unwrap();
    }
}
