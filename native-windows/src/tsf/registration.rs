use super::{CLASS, CLASS_TEXT, PROFILE, module_path};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::*;
use windows::core::{GUID, HSTRING, Interface, Result};

// Keep declarations limited to implemented desktop capabilities. In particular,
// a native candidate HWND is not the TSF UIElement API or immersive support.
// https://learn.microsoft.com/windows/win32/tsf/predefined-category-values
const CATEGORIES: [GUID; 4] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_COMLESS,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
];
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn scope(body: impl FnOnce() -> Result<()>) -> Result<()> {
    let initialized = match unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok() {
        Ok(()) => true,
        Err(error) if error.code() == RPC_E_CHANGED_MODE => false,
        Err(error) => return Err(error),
    };
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
fn register_servers(mut register: impl FnMut(HKEY) -> Result<()>) -> Result<()> {
    // Windows' COMLESS loader reads HKLM directly. Ordinary COM also uses the
    // per-user merged view. Require machine access before touching user state.
    register(HKEY_LOCAL_MACHINE)?;
    register(HKEY_CURRENT_USER)
}
fn register_server(hive: HKEY, path: &std::path::Path) -> Result<()> {
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            hive,
            &HSTRING::from(format!("{}\\InprocServer32", com_key())),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_WOW64_64KEY,
            None,
            &mut key,
            None,
        )
    }
    .ok()?;
    let result = write(key, "", &path.to_string_lossy())
        .and_then(|_| write(key, "ThreadingModel", "Apartment"));
    let _ = unsafe { RegCloseKey(key) };
    result
}
fn remove_categories(mut remove: impl FnMut(&GUID) -> Result<()>) -> Result<()> {
    let mut result = Ok(());
    for category in &CATEGORIES {
        // Uninstall must attempt every owned category even after a failed removal.
        let next = remove(category);
        if result.is_ok() {
            result = next;
        }
    }
    result
}
pub(super) fn register() -> Result<()> {
    let path = module_path()?;
    register_servers(|hive| register_server(hive, &path))?;
    scope(|| {
        let profiles: ITfInputProcessorProfileMgr = unsafe {
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?
        };
        let description: Vec<u16> = "灵果 · LingoMate".encode_utf16().collect();
        let icon_file: Vec<u16> = path.to_string_lossy().encode_utf16().collect();
        unsafe {
            // The Windows IME installation contract registers service and profile
            // together. No local-process/thread flags or implicit source switching.
            // https://learn.microsoft.com/windows/apps/develop/input/input-method-editor-requirements
            profiles.RegisterProfile(
                &CLASS,
                0x0804,
                &PROFILE,
                &description,
                &icon_file,
                0,
                HKL::default(),
                0,
                true,
                0,
            )?;
            // Retain the current-user enablement performed by previous installers;
            // RegisterProfile's default flag is separate from selecting the TIP.
            profiles
                .cast::<ITfInputProcessorProfiles>()?
                .EnableLanguageProfile(&CLASS, 0x0804, &PROFILE, true)?;
        }
        let category: ITfCategoryMgr =
            unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
        for capability in &CATEGORIES {
            unsafe { category.RegisterCategory(&CLASS, capability, &CLASS)? };
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
        let categories_result = remove_categories(|capability| unsafe {
            category.UnregisterCategory(&CLASS, capability, &CLASS)
        });
        let profile_result = unsafe { profiles.Unregister(&CLASS) };
        categories_result.and(profile_result)
    })?;
    let mut result = Ok(());
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let status = unsafe { RegDeleteTreeW(hive, &HSTRING::from(com_key())) };
        if status != windows::Win32::Foundation::ERROR_FILE_NOT_FOUND && result.is_ok() {
            result = status.ok();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::E_ACCESSDENIED;

    #[test]
    fn denied_machine_registration_preserves_existing_user_server() {
        let mut attempted = Vec::new();
        let error = register_servers(|hive| {
            attempted.push(hive);
            Err(E_ACCESSDENIED.into())
        })
        .unwrap_err();
        assert_eq!(error.code(), E_ACCESSDENIED);
        assert_eq!(attempted, vec![HKEY_LOCAL_MACHINE]);
    }

    #[test]
    fn uninstall_attempts_all_categories_after_a_failure() {
        let mut attempted = Vec::new();
        let result = remove_categories(|category| {
            attempted.push(*category);
            if *category == GUID_TFCAT_TIP_KEYBOARD {
                Err(E_ACCESSDENIED.into())
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code(), E_ACCESSDENIED);
        assert_eq!(attempted, CATEGORIES);
    }

    #[test]
    fn registration_scope_preserves_existing_mta_without_system_registration() {
        std::thread::spawn(|| {
            unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
                .ok()
                .unwrap();
            let mut apartment = APTTYPE::default();
            let mut qualifier = APTTYPEQUALIFIER::default();
            scope(|| unsafe { CoGetApartmentType(&mut apartment, &mut qualifier) }).unwrap();
            assert_eq!(apartment, APTTYPE_MTA);
            unsafe { CoGetApartmentType(&mut apartment, &mut qualifier) }.unwrap();
            assert_eq!(apartment, APTTYPE_MTA);
            unsafe { CoUninitialize() };
        })
        .join()
        .unwrap();
    }
}
