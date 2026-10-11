//! Only the app-owned display preference; no input history or account configuration.
#[cfg(windows)]
mod registry {
    use windows::Win32::System::Registry::*;
    use windows::core::{HSTRING, Result};
    pub(super) const KEY: &str = "Software\\LingoMate\\Candidates";
    pub(super) fn load_at(path: &str) -> bool {
        let mut value = 1u32;
        let mut bytes = 4u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                &HSTRING::from("ShowPartOfSpeech"),
                RRF_RT_REG_DWORD,
                None,
                Some((&mut value as *mut u32).cast()),
                Some(&mut bytes),
            )
        };
        if result.is_ok() && bytes == 4 && value <= 1 {
            value == 1
        } else {
            true
        }
    }
    pub(super) fn save_at(path: &str, show: bool) -> Result<()> {
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
        let value = u32::from(show).to_le_bytes();
        let result = unsafe {
            RegSetValueExW(
                key,
                &HSTRING::from("ShowPartOfSpeech"),
                Some(0),
                REG_DWORD,
                Some(&value),
            )
        }
        .ok();
        let _ = unsafe { RegCloseKey(key) };
        result
    }
    #[cfg(test)]
    #[test]
    fn isolated_registry_round_trip() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let key = format!(
            "Software\\LingoMate\\Tests\\POS-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        assert!(load_at(&key));
        for show in [false, true, false] {
            save_at(&key, show).unwrap();
            assert_eq!(load_at(&key), show);
        }
        unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(&key)) }
            .ok()
            .unwrap();
    }
}

#[cfg(windows)]
pub fn load() -> bool {
    registry::load_at(registry::KEY)
}
#[cfg(windows)]
pub fn save(show: bool) -> windows::core::Result<()> {
    registry::save_at(registry::KEY, show)
}
