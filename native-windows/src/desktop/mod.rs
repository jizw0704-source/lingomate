//! Standalone native settings/account/translation process; never starts a second TSF service.
pub mod account;
mod local;
pub mod translation;
pub(crate) mod updates;
mod window;
use crate::remote::RemoteError;
use std::io::{Read, Write};

pub fn data_root() -> Result<std::path::PathBuf, RemoteError> {
    crate::helper::require_unpacked()?;
    std::env::var_os("LOCALAPPDATA")
        .map(|v| std::path::PathBuf::from(v).join("LingoMate"))
        .ok_or(RemoteError::Storage)
}
pub fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = std::panic::catch_unwind(|| {
        if args.first().map(String::as_str) == Some("--relay") {
            return crate::helper::relay(&args[1..]);
        }
        if args.first().map(String::as_str) == Some("--identity") {
            let value = serde_json::json!({"unpacked":crate::helper::unpacked(),"version":env!("CARGO_PKG_VERSION")});
            return std::io::stdout()
                .write_all(value.to_string().as_bytes())
                .map_err(|_| RemoteError::Network);
        }
        crate::helper::require_unpacked()?;
        match args.first().map(String::as_str) {
            Some("--updates") => updates::run(&args[1..])?,
            Some("--translate") => {
                let mut bytes = Vec::new();
                std::io::stdin()
                    .take(8193)
                    .read_to_end(&mut bytes)
                    .map_err(|_| RemoteError::InvalidResponse)?;
                let response = if bytes.len() > 8192 {
                    Err(RemoteError::InvalidResponse)
                } else {
                    serde_json::from_slice::<crate::sentence::TranslationRequest>(&bytes)
                        .map_err(|_| RemoteError::InvalidResponse)
                        .and_then(|request| translation::translate(&data_root()?, &request))
                };
                let response = match response {
                    Ok(english) => crate::sentence::TranslationResponse {
                        english: Some(english),
                        error: None,
                    },
                    Err(error) => crate::sentence::TranslationResponse {
                        english: None,
                        error: Some(error),
                    },
                };
                std::io::stdout()
                    .write_all(
                        &serde_json::to_vec(&response).map_err(|_| RemoteError::InvalidResponse)?,
                    )
                    .map_err(|_| RemoteError::Network)?;
            }
            Some("--preferences") => {
                let shared = crate::settings::Shared::open().ok_or(RemoteError::Storage)?;
                let mut snapshot = shared.snapshot().ok_or(RemoteError::Storage)?;
                snapshot.display = crate::preferences::display();
                snapshot.show_pos = crate::preferences::load();
                shared.set_display(snapshot);
                translation::Store::new(data_root()?).publish(&shared)?;
                shared.set_account(
                    account::Store::new(data_root()?)
                        .snapshot()?
                        .active
                        .as_ref()
                        .map_or(0, account::Identity::fingerprint),
                );
                std::io::stdout()
                    .write_all(b"{}")
                    .map_err(|_| RemoteError::Network)?;
            }
            Some("--prepare-memory") => {
                let path = crate::personal::path().ok_or(RemoteError::Storage)?;
                crate::personal::preserve_legacy(
                    &data_root()?.join("PersonalVocabulary/words.json"),
                    &path,
                )?;
                std::io::stdout()
                    .write_all(b"{}")
                    .map_err(|_| RemoteError::Network)?;
            }
            Some("--record-study") => {
                let mut bytes = Vec::new();
                std::io::stdin()
                    .take(8193)
                    .read_to_end(&mut bytes)
                    .map_err(|_| RemoteError::InvalidResponse)?;
                if bytes.len() > 8192 {
                    return Err(RemoteError::InvalidResponse);
                }
                let request: crate::learning::StudyRequest =
                    serde_json::from_slice(&bytes).map_err(|_| RemoteError::InvalidResponse)?;
                if !request.word.valid() {
                    return Err(RemoteError::InvalidResponse);
                }
                let root = data_root()?;
                crate::learning::Store::new(root.join("LocalWords"))
                    .record(request.word.clone())
                    .map_err(|_| RemoteError::Storage)?;
                if request.account != 0 {
                    let store = account::Store::new(root);
                    if store
                        .append(&request.word, "study", false, request.account)
                        .is_ok()
                    {
                        let service = account::Service {
                            store: &store,
                            vault: &crate::credentials::SystemVault,
                            http: &crate::http::NativeHTTP,
                        };
                        let _ = service.sync();
                    }
                }
                std::io::stdout()
                    .write_all(b"{}")
                    .map_err(|_| RemoteError::Network)?;
            }
            _ => window::run(&args)?,
        }
        Ok::<(), RemoteError>(())
    });
    if !matches!(result, Ok(Ok(()))) {
        std::process::exit(1);
    }
}
