use crate::credentials::{SystemVault, Vault};
use crate::http::{NativeHTTP, Transport};
use crate::remote::{AIConfiguration, RemoteError};
use crate::sentence::TranslationRequest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const CONNECTION_TEST_SOURCE: &str = "今天下午开会。";

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub ai: Option<AIConfiguration>,
    pub local: Option<AIConfiguration>,
    // References contain no secrets; each complete endpoint selects its committed key revision.
    #[serde(default)]
    pub key_revisions: BTreeMap<String, String>,
}
impl Configuration {
    fn validate(&self) -> Result<(), RemoteError> {
        if let Some(ai) = &self.ai {
            ai.validate()?;
        }
        if let Some(local) = &self.local {
            local.validate_for(true)?;
        }
        if self.key_revisions.len() > 64 {
            return Err(RemoteError::Storage);
        }
        for (endpoint, revision) in &self.key_revisions {
            crate::remote::Endpoint::parse(endpoint, false)?;
            if revision.is_empty()
                || revision.len() > 128
                || revision
                    .bytes()
                    .any(|b| !b.is_ascii_alphanumeric() && b != b'-')
            {
                return Err(RemoteError::Storage);
            }
        }
        Ok(())
    }
    /// Whether `translate` has a service to use: enabled AI or enabled local translation.
    fn sentence_enabled(&self) -> bool {
        self.ai.as_ref().is_some_and(|v| v.enabled)
            || self.local.as_ref().is_some_and(|v| v.enabled)
    }
    fn fingerprint(&self) -> Result<u64, RemoteError> {
        if self.ai.is_none() && self.local.is_none() {
            Ok(0)
        } else {
            Ok(crate::storage::fingerprint(
                &serde_json::to_vec(self).map_err(|_| RemoteError::Storage)?,
            ))
        }
    }
    fn key_scope(&self, endpoint: &str) -> String {
        self.key_revisions.get(endpoint).map_or_else(
            || format!("ai/{endpoint}"),
            |revision| revision_scope(endpoint, revision),
        )
    }
    fn key(&self, endpoint: &str, vault: &impl Vault) -> Result<Vec<u8>, RemoteError> {
        // Old settings have no revision reference and retain their exact-endpoint legacy scope.
        // A missing modern revision must never resurrect a stale legacy key.
        let bytes = vault
            .read(&self.key_scope(endpoint))?
            .ok_or(RemoteError::MissingKey)?;
        if !std::str::from_utf8(&bytes).is_ok_and(crate::remote::valid_key) {
            return Err(RemoteError::MissingKey);
        }
        Ok(bytes)
    }
}
fn revision_scope(endpoint: &str, revision: &str) -> String {
    format!("ai/{endpoint}/revision/{revision}")
}
pub struct Store(crate::storage::Store<Configuration>, bool);
impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self(
            crate::storage::Store::new(root.join("Translation"), "settings.json", 16_384),
            true,
        )
    }
    pub fn isolated(root: PathBuf) -> Self {
        Self(
            crate::storage::Store::new(root.join("Translation"), "settings.json", 16_384),
            false,
        )
    }
    pub fn load(&self) -> Result<Configuration, RemoteError> {
        self.0.transaction(false, |value| {
            value.validate()?;
            Ok(value.clone())
        })
    }
    pub fn has_saved_key(&self, endpoint: &str, vault: &impl Vault) -> Result<bool, RemoteError> {
        crate::remote::Endpoint::parse(endpoint, false)?;
        vault.contains(&self.load()?.key_scope(endpoint))
    }
    pub fn test_ai(
        &self,
        mut value: AIConfiguration,
        key: &str,
        vault: &impl Vault,
        http: &impl Transport,
    ) -> Result<(), RemoteError> {
        value.validate()?;
        let initial = self.fingerprint()?;
        let key = if key.is_empty() {
            String::from_utf8(self.load()?.key(&value.endpoint, vault)?)
                .map_err(|_| RemoteError::MissingKey)?
        } else if crate::remote::valid_key(key) {
            key.to_owned()
        } else {
            return Err(RemoteError::MissingKey);
        };
        // Testing is an explicit, separate action; it never saves or enables AI translation.
        value.enabled = true;
        let body = crate::remote::request_body(CONNECTION_TEST_SOURCE, &value, false)?;
        if self.fingerprint()? != initial {
            return Err(RemoteError::Changed);
        }
        let reply = http.request(
            &value.endpoint,
            "POST",
            Some(&key),
            Some(&body),
            false,
            64_000,
        )?;
        if self.fingerprint()? != initial {
            return Err(RemoteError::Changed);
        }
        crate::remote::response(reply.status, &reply.bytes)?;
        Ok(())
    }
    pub fn fingerprint(&self) -> Result<u64, RemoteError> {
        self.load()?.fingerprint()
    }
    /// The TSF never reads this configuration; it only sees the revision and availability.
    pub fn publish(&self, shared: &crate::settings::Shared) -> Result<(), RemoteError> {
        let value = self.load()?;
        shared.set_sentence_available(value.sentence_enabled());
        shared.set_configuration(value.fingerprint()?);
        Ok(())
    }
    pub fn save_ai(
        &self,
        mut value: AIConfiguration,
        key: &str,
        vault: &impl Vault,
    ) -> Result<(), RemoteError> {
        value.validate()?;
        if !key.is_empty() && !crate::remote::valid_key(key) {
            return Err(RemoteError::MissingKey);
        }
        value.enabled = true;
        value.revision = crate::storage::nonce();
        let scope = revision_scope(&value.endpoint, &value.revision);
        let mut staged = false;
        let saved = self.0.transaction(true, |configuration| {
            configuration.validate()?;
            let bytes = if key.is_empty() {
                configuration.key(&value.endpoint, vault)?
            } else {
                key.as_bytes().to_vec()
            };
            configuration
                .key_revisions
                .insert(value.endpoint.clone(), value.revision.clone());
            configuration.validate()?;
            // This unique scope is unreachable by the active configuration until its atomic save.
            vault.write(&scope, &bytes)?;
            staged = true;
            configuration.ai = Some(value);
            Ok(())
        });
        if saved.is_err() && staged {
            // A failed cleanup may leave an unused revision, but cannot overwrite the active key.
            let _ = vault.remove(&scope);
        }
        saved?;
        self.announce()
    }
    pub fn disable_ai(&self) -> Result<(), RemoteError> {
        self.0.transaction(true, |value| {
            value.validate()?;
            if let Some(ai) = &mut value.ai {
                ai.enabled = false;
                ai.revision = crate::storage::nonce();
            }
            Ok(())
        })?;
        self.announce()
    }
    pub fn save_local(&self, mut value: AIConfiguration) -> Result<(), RemoteError> {
        value.validate_for(true)?;
        value.enabled = true;
        value.revision = crate::storage::nonce();
        self.0.transaction(true, |configuration| {
            configuration.validate()?;
            configuration.local = Some(value);
            Ok(())
        })?;
        self.announce()
    }
    fn announce(&self) -> Result<(), RemoteError> {
        if self.1
            && let Some(shared) = crate::settings::Shared::open()
        {
            self.publish(&shared)?;
        }
        Ok(())
    }
}

pub fn translate(
    root: &std::path::Path,
    request: &TranslationRequest,
) -> Result<String, RemoteError> {
    let store = Store::new(root.to_path_buf());
    let initial = store.fingerprint()?;
    if request.revision != format!("{initial:016x}") {
        return Err(RemoteError::Changed);
    }
    let configuration = store.load()?;
    let (selected, local, key) = if let Some(ai) = configuration.ai.as_ref().filter(|v| v.enabled) {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let bytes = configuration.key(&ai.endpoint, &SystemVault)?;
        let key = String::from_utf8(bytes).map_err(|_| RemoteError::MissingKey)?;
        (ai, false, Some(key))
    } else if let Some(local) = configuration.local.as_ref().filter(|v| v.enabled) {
        (local, true, None)
    } else {
        return Err(RemoteError::Unavailable);
    };
    if store.fingerprint()? != initial {
        return Err(RemoteError::Changed);
    }
    if local {
        super::local::verify(selected, &NativeHTTP)?;
    }
    let body = crate::remote::request_body(&request.source, selected, local)?;
    let reply = NativeHTTP.request(
        &selected.endpoint,
        "POST",
        key.as_deref(),
        Some(&body),
        local,
        64_000,
    )?;
    if store.fingerprint()? != initial {
        return Err(RemoteError::Changed);
    }
    crate::remote::response(reply.status, &reply.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::MemoryVault;
    struct TestHTTP {
        key: &'static str,
        status: u32,
        calls: std::cell::Cell<usize>,
    }
    impl Transport for TestHTTP {
        fn request(
            &self,
            url: &str,
            method: &str,
            token: Option<&str>,
            body: Option<&[u8]>,
            local: bool,
            limit: usize,
        ) -> Result<crate::http::Response, RemoteError> {
            self.calls.set(self.calls.get() + 1);
            assert_eq!(url, AIConfiguration::minimax(false).endpoint);
            assert_eq!(method, "POST");
            assert_eq!(token, Some(self.key));
            assert!(!local);
            assert_eq!(limit, 64_000);
            let value: serde_json::Value = serde_json::from_slice(body.unwrap()).unwrap();
            assert_eq!(value["messages"][1]["content"], CONNECTION_TEST_SOURCE);
            assert_eq!(value["messages"].as_array().unwrap().len(), 2);
            assert_eq!(value["store"], false);
            Ok(crate::http::Response { status: self.status, bytes: br#"{"choices":[{"finish_reason":"stop","message":{"content":"Meeting this afternoon."}}]}"#.to_vec() })
        }
    }
    #[test]
    fn connection_test_uses_draft_or_exact_saved_key_without_saving_or_enabling_ai() {
        let root =
            std::env::temp_dir().join(format!("lingomate-ai-test-{}", crate::storage::nonce()));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let configuration = AIConfiguration::minimax(false);
        let http = TestHTTP {
            key: "fixture-unsaved",
            status: 200,
            calls: std::cell::Cell::new(0),
        };
        store
            .test_ai(configuration.clone(), "fixture-unsaved", &vault, &http)
            .unwrap();
        assert!(store.load().unwrap().ai.is_none());
        assert!(
            !store
                .has_saved_key(&configuration.endpoint, &vault)
                .unwrap()
        );
        assert_eq!(http.calls.get(), 1);
        store
            .save_ai(configuration.clone(), "fixture-saved", &vault)
            .unwrap();
        store.disable_ai().unwrap();
        let before = std::fs::read(root.join("Translation/settings.json")).unwrap();
        assert!(
            store
                .has_saved_key(&configuration.endpoint, &vault)
                .unwrap()
        );
        assert!(
            !store
                .has_saved_key(&AIConfiguration::minimax(true).endpoint, &vault)
                .unwrap()
        );
        let http = TestHTTP {
            key: "fixture-saved",
            status: 200,
            calls: std::cell::Cell::new(0),
        };
        store
            .test_ai(configuration.clone(), "", &vault, &http)
            .unwrap();
        assert_eq!(
            std::fs::read(root.join("Translation/settings.json")).unwrap(),
            before
        );
        assert!(!store.load().unwrap().ai.unwrap().enabled);
        assert_eq!(
            store.test_ai(AIConfiguration::minimax(true), "", &vault, &http),
            Err(RemoteError::MissingKey)
        );
        assert_eq!(http.calls.get(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn connection_test_reports_authentication_and_quota_errors_without_persisting_draft_key() {
        let root =
            std::env::temp_dir().join(format!("lingomate-ai-errors-{}", crate::storage::nonce()));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        for (status, error) in [
            (401, RemoteError::Denied),
            (403, RemoteError::Denied),
            (402, RemoteError::Quota),
            (429, RemoteError::Quota),
            (503, RemoteError::Network),
        ] {
            let http = TestHTTP {
                key: "fixture-unsaved",
                status,
                calls: std::cell::Cell::new(0),
            };
            assert_eq!(
                store.test_ai(
                    AIConfiguration::minimax(false),
                    "fixture-unsaved",
                    &vault,
                    &http
                ),
                Err(error)
            );
            assert_eq!(http.calls.get(), 1);
            assert!(store.load().unwrap().ai.is_none());
        }
        assert!(
            !store
                .has_saved_key(&AIConfiguration::minimax(false).endpoint, &vault)
                .unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn saving_never_calls_network_and_keys_are_isolated_by_complete_endpoint() {
        let root = std::env::temp_dir().join(format!("lingomate-ai-{}", crate::storage::nonce()));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let config = AIConfiguration::minimax(false);
        assert!(store.save_ai(config.clone(), "", &vault).is_err());
        store
            .save_ai(config.clone(), "fixture-secret-only", &vault)
            .unwrap();
        let committed = store.load().unwrap();
        let first_scope = committed.key_scope(&config.endpoint);
        assert!(first_scope.contains("/revision/"));
        let first = store.fingerprint().unwrap();
        store.save_ai(config.clone(), "", &vault).unwrap();
        assert_ne!(first, store.fingerprint().unwrap());
        assert_eq!(
            store.load().unwrap().key(&config.endpoint, &vault).unwrap(),
            b"fixture-secret-only"
        );
        assert_eq!(
            store.save_ai(AIConfiguration::minimax(true), "", &vault),
            Err(RemoteError::MissingKey)
        );
        let json = std::fs::read_to_string(root.join("Translation/settings.json")).unwrap();
        assert!(!json.contains("fixture-secret-only"));
        store.disable_ai().unwrap();
        assert!(!store.load().unwrap().ai.unwrap().enabled);
        assert!(vault.read(&first_scope).unwrap().is_some());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sentence_service_is_published_only_while_ai_or_local_is_enabled() {
        let root =
            std::env::temp_dir().join(format!("lingomate-ai-sentence-{}", crate::storage::nonce()));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let shared = crate::settings::Shared::isolated().unwrap();
        let published = |available| {
            store.publish(&shared).unwrap();
            assert_eq!(shared.sentence_available(), available);
            assert_eq!(shared.configuration(), store.fingerprint().unwrap());
        };
        published(false);
        store
            .save_ai(AIConfiguration::minimax(false), "fixture-secret", &vault)
            .unwrap();
        published(true);
        store.disable_ai().unwrap();
        published(false);
        store
            .save_local(AIConfiguration {
                version: 1,
                enabled: false,
                endpoint: "http://127.0.0.1:11434/v1/chat/completions".into(),
                model: "fixture".into(),
                revision: String::new(),
            })
            .unwrap();
        published(true);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn blank_key_uses_only_the_committed_key_for_its_complete_endpoint() {
        let root = std::env::temp_dir().join(format!(
            "lingomate-ai-endpoints-{}",
            crate::storage::nonce()
        ));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let first = AIConfiguration::minimax(false);
        let second = AIConfiguration::minimax(true);
        store
            .save_ai(first.clone(), "fixture-first", &vault)
            .unwrap();
        store
            .save_ai(second.clone(), "fixture-second", &vault)
            .unwrap();
        store.save_ai(first.clone(), "", &vault).unwrap();
        assert_eq!(
            store.load().unwrap().key(&first.endpoint, &vault).unwrap(),
            b"fixture-first"
        );
        assert_eq!(
            store.load().unwrap().key(&second.endpoint, &vault).unwrap(),
            b"fixture-second"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupted_configuration_never_overwrites_the_committed_key() {
        let root =
            std::env::temp_dir().join(format!("lingomate-ai-corrupt-{}", crate::storage::nonce()));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let configuration = AIConfiguration::minimax(false);
        store
            .save_ai(configuration.clone(), "fixture-original", &vault)
            .unwrap();
        let committed = store.load().unwrap();
        let path = root.join("Translation/settings.json");
        std::fs::write(&path, b"damaged").unwrap();
        assert_eq!(
            store.save_ai(configuration.clone(), "fixture-replacement", &vault),
            Err(RemoteError::Storage)
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"damaged");
        assert_eq!(
            committed.key(&configuration.endpoint, &vault).unwrap(),
            b"fixture-original"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_endpoint_keys_migrate_without_fallback_from_a_missing_modern_revision() {
        let root =
            std::env::temp_dir().join(format!("lingomate-ai-legacy-{}", crate::storage::nonce()));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let mut ai = AIConfiguration::minimax(false);
        ai.enabled = true;
        ai.revision = "legacy-settings".into();
        let path = root.join("Translation/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"ai":ai,"local":null})).unwrap(),
        )
        .unwrap();
        let legacy_scope = format!("ai/{}", ai.endpoint);
        vault.write(&legacy_scope, b"fixture-legacy").unwrap();
        assert_eq!(
            store.load().unwrap().key(&ai.endpoint, &vault).unwrap(),
            b"fixture-legacy"
        );
        store.save_ai(ai.clone(), "", &vault).unwrap();
        let saved = store.load().unwrap();
        assert_eq!(saved.key(&ai.endpoint, &vault).unwrap(), b"fixture-legacy");
        let modern_scope = saved.key_scope(&ai.endpoint);
        assert_ne!(modern_scope, legacy_scope);
        vault.remove(&modern_scope).unwrap();
        assert_eq!(
            saved.key(&ai.endpoint, &vault),
            Err(RemoteError::MissingKey)
        );
        assert_eq!(
            vault.read(&legacy_scope).unwrap().unwrap(),
            b"fixture-legacy"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
