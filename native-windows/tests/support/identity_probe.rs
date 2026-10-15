//! Opt-in package identity fixture. Reads no configuration, credentials or input content.
use lingomate_tsf::{helper, remote::RemoteError};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::core::PWSTR;

fn package_name() -> Result<String, RemoteError> {
    let mut length = 0;
    if unsafe { GetCurrentPackageFullName(&mut length, None) } != ERROR_INSUFFICIENT_BUFFER
        || !(2..=512).contains(&length)
    {
        return Err(RemoteError::Packaged);
    }
    let mut value = vec![0u16; length as usize];
    if unsafe { GetCurrentPackageFullName(&mut length, Some(PWSTR(value.as_mut_ptr()))) }
        != ERROR_SUCCESS
    {
        return Err(RemoteError::Packaged);
    }
    let end = value
        .iter()
        .position(|v| *v == 0)
        .ok_or(RemoteError::Packaged)?;
    String::from_utf16(&value[..end]).map_err(|_| RemoteError::Packaged)
}

pub fn run(args: &[String]) -> Result<bool, RemoteError> {
    let [expected, mode, result] = args else {
        return Err(RemoteError::Configuration);
    };
    if expected.is_empty()
        || expected.len() > 256
        || expected
            .bytes()
            .any(|b| !b.is_ascii_alphanumeric() && !matches!(b, b'.' | b'_' | b'-'))
        || !matches!(mode.as_str(), "allow" | "prevent")
    {
        return Err(RemoteError::Configuration);
    }
    let executable = std::env::current_exe().map_err(|_| RemoteError::Unavailable)?;
    let bin = executable.parent().ok_or(RemoteError::Unavailable)?;
    let directory = bin
        .parent()
        .ok_or(RemoteError::Unavailable)?
        .join("results");
    let result = Path::new(result);
    let file_name = format!("{mode}.json");
    if !result.is_absolute()
        || result.parent() != Some(directory.as_path())
        || result.file_name().and_then(|v| v.to_str()) != Some(file_name.as_str())
    {
        return Err(RemoteError::Configuration);
    }
    let identity_matches = package_name().is_ok_and(|name| name == expected.as_str());
    let parent_packaged = !helper::unpacked();
    let response = if identity_matches && parent_packaged {
        (|| {
            let child = helper::spawn(
                &bin.join("lingomate-desktop.exe"),
                &["--identity".into()],
                true,
            )?;
            let bytes = child.capture(b"", &AtomicBool::new(false), Duration::from_secs(10))?;
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| RemoteError::InvalidResponse)?;
            if value.as_object().is_none_or(|v| v.len() != 2)
                || value.get("version").and_then(serde_json::Value::as_str)
                    != Some(env!("CARGO_PKG_VERSION"))
                || value.get("unpacked").and_then(serde_json::Value::as_bool) != Some(true)
            {
                return Err(RemoteError::InvalidResponse);
            }
            Ok(value)
        })()
    } else {
        Err(RemoteError::Packaged)
    };
    let passed = response.is_ok();
    let (final_identity, error) = match response {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(error)),
    };
    let report = serde_json::json!({
        "schema":1,
        "kind":"existing_debug_package_helper_identity",
        "mode":mode,
        "version":env!("CARGO_PKG_VERSION"),
        "parent_pid":std::process::id(),
        "parent_packaged":parent_packaged,
        "parent_identity_matches":identity_matches,
        "parent_package_full_name":if identity_matches {Some(expected)} else {None},
        "final_identity":final_identity,
        "error":error,
        "passed":passed
    });
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(result)
        .map_err(|_| RemoteError::Storage)?;
    file.write_all(&serde_json::to_vec(&report).map_err(|_| RemoteError::Storage)?)
        .and_then(|_| file.sync_all())
        .map_err(|_| RemoteError::Storage)?;
    Ok(passed)
}
