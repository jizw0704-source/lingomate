//! App-owned display preferences; punctuation remains session-only like the Mac version.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
    System,
}
impl Theme {
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];
    pub fn title(self) -> &'static str {
        match self {
            Self::Light => "浅色",
            Self::Dark => "深色",
            Self::System => "跟随系统",
        }
    }
    pub fn value(self) -> u32 {
        self as u32
    }
    pub fn from_value(value: u32) -> Self {
        match value {
            1 => Self::Dark,
            2 => Self::System,
            _ => Self::Light,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Display {
    pub theme: Theme,
    pub font: u32,
}
impl Default for Display {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            font: 18,
        }
    }
}
impl Display {
    pub const SIZES: [u32; 4] = [16, 18, 20, 22];
    pub fn font(value: u32) -> u32 {
        if Self::SIZES.contains(&value) {
            value
        } else {
            18
        }
    }
}
#[cfg(windows)]
mod registry {
    use windows::Win32::System::Registry::*;
    use windows::core::{HSTRING, Result};
    pub(super) const KEY: &str = "Software\\LingoMate\\Candidates";
    pub(super) fn read(path: &str, name: &str) -> Option<u32> {
        let mut value = 0u32;
        let mut bytes = 4u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                &HSTRING::from(name),
                RRF_RT_REG_DWORD,
                None,
                Some((&mut value as *mut u32).cast()),
                Some(&mut bytes),
            )
        };
        (result.is_ok() && bytes == 4).then_some(value)
    }
    pub(super) fn write(path: &str, name: &str, value: u32) -> Result<()> {
        let mut key = HKEY::default();
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                Some(0),
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut key,
                None,
            )
        }
        .ok()?;
        let result = unsafe {
            RegSetValueExW(
                key,
                &HSTRING::from(name),
                Some(0),
                REG_DWORD,
                Some(&value.to_le_bytes()),
            )
        }
        .ok();
        let _ = unsafe { RegCloseKey(key) };
        result
    }
    #[test]
    fn isolated_registry_round_trip_and_invalid_values() {
        use super::*;
        let key = format!(
            "Software\\LingoMate\\Tests\\Display-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        assert_eq!(read(&key, "Theme"), None);
        write(&key, "ShowPartOfSpeech", 0).unwrap();
        for theme in Theme::ALL {
            write(&key, "Theme", theme.value()).unwrap();
            assert_eq!(Theme::from_value(read(&key, "Theme").unwrap()), theme);
            for font in Display::SIZES {
                write(&key, "FontSize", font).unwrap();
                assert_eq!(Display::font(read(&key, "FontSize").unwrap()), font);
                assert_eq!(read(&key, "ShowPartOfSpeech"), Some(0));
            }
        }
        write(&key, "Theme", 99).unwrap();
        write(&key, "FontSize", 99).unwrap();
        assert_eq!(
            Theme::from_value(read(&key, "Theme").unwrap()),
            Theme::Light
        );
        assert_eq!(Display::font(read(&key, "FontSize").unwrap()), 18);
        unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(&key)) }
            .ok()
            .unwrap();
    }
}
#[cfg(windows)]
pub fn load() -> bool {
    registry::read(registry::KEY, "ShowPartOfSpeech").is_none_or(|v| v != 0)
}
#[cfg(windows)]
pub fn save(show: bool) -> windows::core::Result<()> {
    registry::write(registry::KEY, "ShowPartOfSpeech", u32::from(show))
}
#[cfg(windows)]
pub fn display() -> Display {
    Display {
        theme: Theme::from_value(registry::read(registry::KEY, "Theme").unwrap_or(0)),
        font: Display::font(registry::read(registry::KEY, "FontSize").unwrap_or(18)),
    }
}
#[cfg(windows)]
pub fn save_theme(theme: Theme) -> windows::core::Result<()> {
    registry::write(registry::KEY, "Theme", theme.value())
}
#[cfg(windows)]
pub fn save_font(font: u32) -> windows::core::Result<()> {
    registry::write(registry::KEY, "FontSize", Display::font(font))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_preferences_have_safe_defaults() {
        assert_eq!(Theme::from_value(99), Theme::Light);
        assert_eq!(Display::font(0), 18);
        assert_eq!(Display::font(22), 22);
    }
}
