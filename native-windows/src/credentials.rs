//! Credential access belongs to the standalone desktop/translation helper only.
use crate::remote::RemoteError;
use windows::Win32::Security::Credentials::*;
use windows::core::{HSTRING, PWSTR};

pub trait Vault {
    fn read(&self, scope: &str) -> Result<Option<Vec<u8>>, RemoteError>;
    fn contains(&self, scope: &str) -> Result<bool, RemoteError> {
        Ok(self.read(scope)?.is_some())
    }
    fn write(&self, scope: &str, value: &[u8]) -> Result<(), RemoteError>;
    fn remove(&self, scope: &str) -> Result<(), RemoteError>;
}
pub struct SystemVault;
fn target(scope: &str) -> Result<HSTRING, RemoteError> {
    crate::helper::require_unpacked()?;
    if scope.is_empty() || scope.len() > 2048 || scope.chars().any(char::is_control) {
        return Err(RemoteError::Configuration);
    }
    Ok(HSTRING::from(format!("LingoMate/{scope}")))
}
impl Vault for SystemVault {
    fn contains(&self, scope: &str) -> Result<bool, RemoteError> {
        let name = target(scope)?;
        let mut credential = std::ptr::null_mut();
        if let Err(error) = unsafe { CredReadW(&name, CRED_TYPE_GENERIC, None, &mut credential) } {
            return if error.code().0 as u32 == 0x80070490 {
                Ok(false)
            } else {
                Err(RemoteError::Storage)
            };
        }
        if credential.is_null() {
            return Err(RemoteError::Storage);
        }
        // The settings badge needs existence only. Never copy the stored blob into an edit field.
        let value = unsafe { &*credential };
        let present =
            (1..=512).contains(&value.CredentialBlobSize) && !value.CredentialBlob.is_null();
        unsafe { CredFree(credential.cast()) };
        Ok(present)
    }
    fn read(&self, scope: &str) -> Result<Option<Vec<u8>>, RemoteError> {
        let name = target(scope)?;
        let mut credential = std::ptr::null_mut();
        if let Err(error) = unsafe { CredReadW(&name, CRED_TYPE_GENERIC, None, &mut credential) } {
            return if error.code().0 as u32 == 0x80070490 {
                Ok(None)
            } else {
                Err(RemoteError::Storage)
            };
        }
        if credential.is_null() {
            return Err(RemoteError::Storage);
        }
        let value = unsafe { &*credential };
        let result = if value.CredentialBlobSize > 2560 || value.CredentialBlob.is_null() {
            Err(RemoteError::Storage)
        } else {
            Ok(Some(
                unsafe {
                    std::slice::from_raw_parts(
                        value.CredentialBlob,
                        value.CredentialBlobSize as usize,
                    )
                }
                .to_vec(),
            ))
        };
        unsafe {
            CredFree(credential.cast());
        }
        result
    }
    fn write(&self, scope: &str, value: &[u8]) -> Result<(), RemoteError> {
        let name = target(scope)?;
        if value.is_empty() || value.len() > 2560 {
            return Err(RemoteError::Storage);
        }
        let user = HSTRING::from("LingoMate");
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(name.as_ptr() as *mut u16),
            CredentialBlobSize: value.len() as u32,
            CredentialBlob: value.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user.as_ptr() as *mut u16),
            ..Default::default()
        };
        unsafe { CredWriteW(&credential, 0) }.map_err(|_| RemoteError::Storage)
    }
    fn remove(&self, scope: &str) -> Result<(), RemoteError> {
        let name = target(scope)?;
        match unsafe { CredDeleteW(&name, CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if error.code().0 as u32 == 0x80070490 => Ok(()),
            Err(_) => Err(RemoteError::Storage),
        }
    }
}

#[derive(Default)]
pub struct MemoryVault(std::cell::RefCell<std::collections::BTreeMap<String, Vec<u8>>>);
impl Vault for MemoryVault {
    fn read(&self, scope: &str) -> Result<Option<Vec<u8>>, RemoteError> {
        Ok(self.0.borrow().get(scope).cloned())
    }
    fn write(&self, scope: &str, value: &[u8]) -> Result<(), RemoteError> {
        self.0.borrow_mut().insert(scope.into(), value.into());
        Ok(())
    }
    fn remove(&self, scope: &str) -> Result<(), RemoteError> {
        self.0.borrow_mut().remove(scope);
        Ok(())
    }
}
