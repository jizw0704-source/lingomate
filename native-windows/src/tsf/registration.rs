use super::{CLASS, CLASS_TEXT, PROFILE, module_path};
use windows::Win32::System::Com::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::TextServices::*;
use windows::core::{HSTRING, Result};
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn scope(body: impl FnOnce() -> Result<()>) -> Result<()> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = body();
    if initialized {
        unsafe {
            CoUninitialize();
        }
    }
    result
}
fn write(key: HKEY, name: &str, value: &str) -> Result<()> {
    let bytes = wide(value)
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect::<Vec<_>>();
    unsafe { RegSetValueExW(key, &HSTRING::from(name), Some(0), REG_SZ, Some(&bytes)) }.ok()
}
fn com_key() -> String {
    format!("Software\\Classes\\CLSID\\{CLASS_TEXT}")
}
pub(super) fn register() -> Result<()> {
    let path = module_path()?;
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(format!("{}\\InprocServer32", com_key())),
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
    let result = write(key, "", &path.to_string_lossy())
        .and_then(|_| write(key, "ThreadingModel", "Apartment"));
    let _ = unsafe { RegCloseKey(key) };
    result?;
    scope(|| {
        let profiles: ITfInputProcessorProfiles = unsafe {
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?
        };
        unsafe {
            profiles.Register(&CLASS)?;
            profiles.AddLanguageProfile(
                &CLASS,
                0x0804,
                &PROFILE,
                &wide("灵果 · LingoMate"),
                &wide(""),
                0,
            )?;
            profiles.EnableLanguageProfile(&CLASS, 0x0804, &PROFILE, true)?;
        }
        let category: ITfCategoryMgr =
            unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
        unsafe {
            category.RegisterCategory(&CLASS, &GUID_TFCAT_TIP_KEYBOARD, &CLASS)?;
        }
        Ok(())
    })
}
pub(super) fn unregister() -> Result<()> {
    scope(|| {
        let profiles: ITfInputProcessorProfiles = unsafe {
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?
        };
        let category: ITfCategoryMgr =
            unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
        unsafe {
            category.UnregisterCategory(&CLASS, &GUID_TFCAT_TIP_KEYBOARD, &CLASS)?;
            profiles.Unregister(&CLASS)?;
        }
        Ok(())
    })?;
    unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(com_key())) }.ok()
}
