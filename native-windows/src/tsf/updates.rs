//! Updating is an explicitly opened, separate desktop process, never a network task in TSF.
use super::{down, module_path};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn shortcut(key: usize) -> bool {
    key == 0x55 && down(0x11) && down(0x10) && !down(0x12) && !down(0x5B) && !down(0x5C)
}
pub(super) fn launch(background: bool) -> Result<(), ()> {
    if background {
        let app_data = std::env::var_os("LOCALAPPDATA").ok_or(())?;
        let path = PathBuf::from(app_data).join("LingoMate/update-settings.json");
        let metadata = std::fs::metadata(&path).map_err(|_| ())?;
        if metadata.len() > 65536 {
            return Err(());
        }
        let settings: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).map_err(|_| ())?).map_err(|_| ())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ())?
            .as_secs();
        if settings["auto_check"].as_bool() != Some(true)
            || now.saturating_sub(settings["last_checked"].as_u64().unwrap_or(now)) < 86400
        {
            return Ok(());
        }
    }
    let module = module_path().map_err(|_| ())?;
    let script = module.parent().ok_or(())?.join("update.ps1");
    if !script.is_file() {
        return Err(());
    }
    let windows = std::env::var_os("WINDIR").ok_or(())?;
    let executable = PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(executable);
    command
        .creation_flags(0x08000000)
        .args(["-NoProfile", "-STA", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script);
    if background {
        command.arg("-Background");
    }
    command.spawn().map(|_| ()).map_err(|_| ())
}
