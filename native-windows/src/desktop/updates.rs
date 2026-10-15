//! Fixed updater entry point. Identity is checked before preferences are read.
use crate::remote::RemoteError;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Manual,
    Background,
}
impl Mode {
    pub(crate) fn parse(args: &[String]) -> Result<Self, RemoteError> {
        match args {
            [mode] if mode == "manual" => Ok(Self::Manual),
            [mode] if mode == "background" => Ok(Self::Background),
            _ => Err(RemoteError::Configuration),
        }
    }
    pub(crate) fn arguments(self) -> [String; 2] {
        [
            "--updates".into(),
            match self {
                Self::Manual => "manual",
                Self::Background => "background",
            }
            .into(),
        ]
    }
}

fn due(bytes: &[u8], now: u64) -> bool {
    if bytes.len() > 65536 {
        return false;
    }
    let Ok(settings) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return false;
    };
    settings["auto_check"].as_bool() == Some(true)
        && settings["last_checked"]
            .as_u64()
            .is_some_and(|last| now.saturating_sub(last) >= 86400)
}

fn execute(
    mode: Mode,
    identity: impl FnOnce() -> Result<(), RemoteError>,
    read: impl FnOnce() -> Result<Vec<u8>, RemoteError>,
    now: impl FnOnce() -> Result<u64, RemoteError>,
    launch: impl FnOnce(Mode) -> Result<(), RemoteError>,
) -> Result<(), RemoteError> {
    identity()?;
    if mode == Mode::Background && !due(&read()?, now()?) {
        return Ok(());
    }
    launch(mode)
}

fn read_preferences() -> Result<Vec<u8>, RemoteError> {
    let path = super::data_root()?.join("update-settings.json");
    read_settings(&path)
}
fn read_settings(path: &Path) -> Result<Vec<u8>, RemoteError> {
    let file = std::fs::File::open(path).map_err(|_| RemoteError::Storage)?;
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| RemoteError::Storage)?;
    Ok(bytes)
}

fn command(powershell: &Path, script: &Path, mode: Mode) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(powershell);
    command
        .creation_flags(0x08000000)
        .args(["-NoProfile", "-STA", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script);
    if mode == Mode::Background {
        command.arg("-Background");
    }
    command
}

pub(crate) fn run(args: &[String]) -> Result<(), RemoteError> {
    execute(
        Mode::parse(args)?,
        crate::helper::require_unpacked,
        read_preferences,
        || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_secs())
                .map_err(|_| RemoteError::Unavailable)
        },
        |mode| {
            let executable = std::env::current_exe().map_err(|_| RemoteError::Unavailable)?;
            let script = executable
                .parent()
                .ok_or(RemoteError::Unavailable)?
                .join("update.ps1");
            if !script.is_file() {
                return Err(RemoteError::Unavailable);
            }
            let windows = std::env::var_os("WINDIR").ok_or(RemoteError::Unavailable)?;
            let powershell =
                PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe");
            command(&powershell, &script, mode)
                .spawn()
                .map(|_| ())
                .map_err(|_| RemoteError::Unavailable)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Write;

    #[test]
    fn updater_modes_have_only_fixed_helper_arguments() {
        for mode in [Mode::Manual, Mode::Background] {
            assert_eq!(Mode::parse(&mode.arguments()[1..]), Ok(mode));
            assert_eq!(mode.arguments()[0], "--updates");
        }
        for args in [
            vec![],
            vec!["install".into()],
            vec!["manual".into(), "-File".into()],
            vec!["background -File other.ps1".into()],
        ] {
            assert_eq!(Mode::parse(&args), Err(RemoteError::Configuration));
        }
    }

    #[test]
    fn updater_packaged_identity_is_rejected_before_any_state_or_launch() {
        for mode in [Mode::Manual, Mode::Background] {
            assert_eq!(
                execute(
                    mode,
                    || Err(RemoteError::Packaged),
                    || panic!("must not read preferences before identity validation"),
                    || panic!("must not access the clock before identity validation"),
                    |_| panic!("must not launch from package identity"),
                ),
                Err(RemoteError::Packaged)
            );
        }
    }

    #[test]
    fn updater_manual_launch_does_not_access_automatic_check_preferences() {
        let launched = Cell::new(false);
        execute(
            Mode::Manual,
            || Ok(()),
            || panic!("manual checks must not read daily preferences"),
            || panic!("manual checks do not depend on a timestamp"),
            |mode| {
                assert_eq!(mode, Mode::Manual);
                launched.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(launched.get());
    }

    #[test]
    fn updater_background_requires_opt_in_and_a_full_day() {
        let now = 172800;
        for (bytes, expected) in [
            (
                br#"{"auto_check":true,"last_checked":86400}"#.as_slice(),
                true,
            ),
            (
                br#"{"auto_check":false,"last_checked":0}"#.as_slice(),
                false,
            ),
            (
                br#"{"auto_check":true,"last_checked":86401}"#.as_slice(),
                false,
            ),
            (
                br#"{"auto_check":true,"last_checked":999999}"#.as_slice(),
                false,
            ),
            (br#"{"auto_check":true}"#.as_slice(), false),
            (
                br#"{"auto_check":"true","last_checked":0}"#.as_slice(),
                false,
            ),
            (
                br#"{"auto_check":true,"last_checked":-1}"#.as_slice(),
                false,
            ),
            (b"malformed".as_slice(), false),
        ] {
            let launched = Cell::new(false);
            execute(
                Mode::Background,
                || Ok(()),
                || Ok(bytes.to_vec()),
                || Ok(now),
                |mode| {
                    assert_eq!(mode, Mode::Background);
                    launched.set(true);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(launched.get(), expected);
        }
        assert!(!due(&vec![b' '; 65537], now));
        assert_eq!(
            execute(
                Mode::Background,
                || Ok(()),
                || Err(RemoteError::Storage),
                || panic!("a failed read must stop the check"),
                |_| panic!("missing preferences must not opt in"),
            ),
            Err(RemoteError::Storage)
        );
    }

    #[test]
    fn updater_powershell_arguments_use_the_owned_script_as_one_argument() {
        let powershell = Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        let script = Path::new(r"C:\synthetic package with spaces\update.ps1");
        for mode in [Mode::Manual, Mode::Background] {
            let launch = command(powershell, script, mode);
            assert_eq!(launch.get_program(), powershell);
            let mut expected = vec![
                "-NoProfile",
                "-STA",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                script.to_str().unwrap(),
            ];
            if mode == Mode::Background {
                expected.push("-Background");
            }
            assert_eq!(launch.get_args().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn updater_preference_reader_is_bounded_using_only_an_owned_temporary_file() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "lingomate-updater-test-{}-{nonce}.json",
            std::process::id()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _fixture = Fixture(path.clone());
        let settings = br#"{"auto_check":true,"last_checked":0}"#;
        file.write_all(settings).unwrap();
        drop(file);
        assert_eq!(read_settings(&path).unwrap(), settings);
        assert!(due(&read_settings(&path).unwrap(), 86400));
        std::fs::write(&path, vec![b' '; 100000]).unwrap();
        let bytes = read_settings(&path).unwrap();
        assert_eq!(bytes.len(), 65537);
        assert!(!due(&bytes, 86400));
    }
}
