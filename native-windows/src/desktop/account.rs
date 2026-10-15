//! Email OTP and selected-word synchronization, isolated from local learning and Chinese memory.
use crate::credentials::Vault;
use crate::http::Transport;
use crate::learning::Word;
use crate::remote::{Endpoint, RemoteError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

// The backend accepts at most 1 MB per POST. Count encoded UTF-8 bytes with headroom.
const EVENT_BATCH_BYTES: usize = 900_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub id: String,
    pub email: String,
    pub project: String,
    pub session: String,
}
impl Identity {
    pub fn scope(&self) -> String {
        format!("{}|{}", self.project, self.id)
    }
    fn credential_scope(&self) -> String {
        format!("account/{}/session/{}", self.scope(), self.session)
    }
    pub fn fingerprint(&self) -> u64 {
        crate::storage::fingerprint(&serde_json::to_vec(self).unwrap_or_default())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub english: String,
    pub chinese: String,
    pub pos: String,
    pub kind: String,
    pub mastered: bool,
}
impl Event {
    fn word(&self) -> Word {
        Word {
            english: self.english.clone(),
            chinese: self.chinese.clone(),
            pos: self.pos.clone(),
            uses: 0,
            mastered: false,
        }
    }
    fn valid(&self) -> bool {
        valid_uuid(&self.id)
            && self.word().valid()
            && matches!(self.kind.as_str(), "study" | "mastered")
    }
}
fn event_batches(events: &[Event]) -> Result<Vec<&[Event]>, RemoteError> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut size = 2; // JSON array brackets.
    for (index, event) in events.iter().enumerate() {
        let encoded = serde_json::to_vec(event)
            .map_err(|_| RemoteError::Storage)?
            .len();
        if encoded + 2 > EVENT_BATCH_BYTES {
            return Err(RemoteError::Storage);
        }
        if size + usize::from(index > start) + encoded > EVENT_BATCH_BYTES {
            batches.push(&events[start..index]);
            start = index;
            size = 2;
        }
        size += usize::from(index > start) + encoded;
    }
    if start < events.len() {
        batches.push(&events[start..]);
    }
    Ok(batches)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    pub version: u32,
    pub active: Option<Identity>,
    // A separate epoch invalidates old login replies even after None -> Some -> None.
    #[serde(default)]
    pub auth_revision: String,
    pub words: BTreeMap<String, Vec<Word>>,
    pub pending: BTreeMap<String, Vec<Event>>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            version: 1,
            active: None,
            auth_revision: String::new(),
            words: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }
}
impl Data {
    pub fn validate(&self) -> Result<(), RemoteError> {
        if self.version != 1
            || self.words.len() > 64
            || self.pending.len() > 64
            || self.auth_revision.len() > 128
            || self.auth_revision.chars().any(char::is_control)
        {
            return Err(RemoteError::Storage);
        }
        if let Some(account) = &self.active
            && (!valid_uuid(&account.id)
                || !valid_email(&account.email)
                || Endpoint::parse(&account.project, false).is_err()
                || !valid_uuid(&account.session))
        {
            return Err(RemoteError::Storage);
        }
        for words in self.words.values() {
            let mut ids = BTreeSet::new();
            if words.len() > 5000
                || words
                    .iter()
                    .any(|w| !w.valid() || w.uses > 1_000_000_000 || !ids.insert(w.id()))
            {
                return Err(RemoteError::Storage);
            }
        }
        for pending in self.pending.values() {
            let mut ids = BTreeSet::new();
            if pending.len() > 5000
                || pending
                    .iter()
                    .any(|e| !e.valid() || !ids.insert(e.id.clone()))
            {
                return Err(RemoteError::Storage);
            }
        }
        Ok(())
    }
    pub fn visible(&self, account: &Identity) -> Vec<Word> {
        let mut words: BTreeMap<String, Word> = self
            .words
            .get(&account.scope())
            .into_iter()
            .flatten()
            .map(|word| (word.id(), word.clone()))
            .collect();
        for event in self.pending.get(&account.scope()).into_iter().flatten() {
            let word = words
                .entry(event.word().id())
                .or_insert_with(|| event.word());
            if event.kind == "study" {
                word.uses = word.uses.saturating_add(1).min(1_000_000_000)
            } else {
                word.mastered = event.mastered
            }
        }
        words.into_values().collect()
    }
}
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    base_url: Option<String>,
}
pub struct Store {
    pub directory: PathBuf,
    data: crate::storage::Store<Data>,
    config: crate::storage::Store<Configuration>,
    notify: bool,
}
impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self::with_notify(root, true)
    }
    pub fn isolated(root: PathBuf) -> Self {
        Self::with_notify(root, false)
    }
    fn with_notify(root: PathBuf, notify: bool) -> Self {
        let directory = root.join("Account");
        Self {
            data: crate::storage::Store::new(directory.clone(), "learning.json", 8_000_000),
            config: crate::storage::Store::new(directory.clone(), "configuration.json", 4096),
            directory,
            notify,
        }
    }
    pub fn snapshot(&self) -> Result<Data, RemoteError> {
        self.data.transaction(false, |d| {
            d.validate()?;
            Ok(d.clone())
        })
    }
    pub fn url(&self) -> Result<Option<String>, RemoteError> {
        self.config.transaction(false, |c| {
            if let Some(url) = &c.base_url {
                Endpoint::parse(url, false)?;
            }
            Ok(c.base_url.clone())
        })
    }
    pub fn save_url(&self, url: &str) -> Result<(), RemoteError> {
        Endpoint::parse(url, false)?;
        // All operations that need both stores take config first, then data.
        self.config.transaction(true, |c| {
            self.data.transaction(true, |data| {
                data.validate()?;
                if data.active.as_ref().is_some_and(|a| a.project != url) {
                    return Err(RemoteError::Changed);
                }
                data.auth_revision = crate::storage::nonce();
                Ok(())
            })?;
            c.base_url = Some(url.into());
            Ok(())
        })
    }
    fn begin_login(&self) -> Result<(String, String), RemoteError> {
        self.config.transaction(false, |configuration| {
            let base = configuration
                .base_url
                .as_ref()
                .ok_or(RemoteError::Configuration)?;
            Endpoint::parse(base, false)?;
            let revision = crate::storage::nonce();
            self.data.transaction(true, |data| {
                data.validate()?;
                data.auth_revision = revision.clone();
                Ok(())
            })?;
            Ok((base.clone(), revision))
        })
    }
    pub fn append(
        &self,
        word: &Word,
        kind: &str,
        mastered: bool,
        expected: u64,
    ) -> Result<(), RemoteError> {
        if !word.valid() || !matches!(kind, "study" | "mastered") {
            return Err(RemoteError::Configuration);
        }
        self.data.transaction(true, |data| {
            data.validate()?;
            let account = data.active.as_ref().ok_or(RemoteError::Denied)?;
            if account.fingerprint() != expected {
                return Err(RemoteError::Changed);
            }
            let pending = data.pending.entry(account.scope()).or_default();
            if pending.len() >= 5000 {
                return Err(RemoteError::Quota);
            }
            pending.push(Event {
                id: uuid(),
                english: word.english.clone(),
                chinese: word.chinese.clone(),
                pos: word.pos.clone(),
                kind: kind.into(),
                mastered,
            });
            data.validate()
        })
    }
    fn announce(&self) -> Result<(), RemoteError> {
        if self.notify
            && let Some(shared) = crate::settings::Shared::open()
        {
            shared.set_account(
                self.snapshot()?
                    .active
                    .as_ref()
                    .map_or(0, Identity::fingerprint),
            );
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct Tokens {
    access: String,
    refresh: Option<String>,
    expiry: u64,
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    user: User,
}
#[derive(Deserialize)]
struct User {
    id: String,
    email: String,
}
pub struct Service<'a, V: Vault, H: Transport> {
    pub store: &'a Store,
    pub vault: &'a V,
    pub http: &'a H,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn valid_email(value: &str) -> bool {
    if value.is_empty() || value.len() > 254 || value.bytes().any(|b| b <= 32 || b >= 127) {
        return false;
    }
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(&b))
        && domain.contains('.')
        && domain.split('.').all(|part| {
            !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn uuid() -> String {
    match unsafe { windows::Win32::System::Com::CoCreateGuid() } {
        Ok(value) => format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            value.data1,
            value.data2,
            value.data3,
            value.data4[0],
            value.data4[1],
            value.data4[2],
            value.data4[3],
            value.data4[4],
            value.data4[5],
            value.data4[6],
            value.data4[7]
        ),
        Err(_) => String::new(),
    }
}
impl<V: Vault, H: Transport> Service<'_, V, H> {
    fn request(
        &self,
        base: &str,
        path: &str,
        token: Option<&str>,
        json: Option<serde_json::Value>,
    ) -> Result<Vec<u8>, RemoteError> {
        let body = json
            .map(|v| serde_json::to_vec(&v).map_err(|_| RemoteError::Configuration))
            .transpose()?;
        let response = self.http.request(
            &format!("{}/{}", base.trim_end_matches('/'), path),
            if body.is_some() { "POST" } else { "GET" },
            token,
            body.as_deref(),
            false,
            8_000_000,
        )?;
        match response.status {
            401 | 403 => Err(RemoteError::Denied),
            429 => Err(RemoteError::Quota),
            200..=299 => Ok(response.bytes),
            _ => Err(RemoteError::Network),
        }
    }
    fn base(&self) -> Result<String, RemoteError> {
        self.store.url()?.ok_or(RemoteError::Configuration)
    }
    pub fn send_code(&self, email: &str) -> Result<(), RemoteError> {
        if !valid_email(email) {
            return Err(RemoteError::Configuration);
        }
        self.request(
            &self.base()?,
            "v1/auth/code",
            None,
            Some(serde_json::json!({"email":email})),
        )
        .map(|_| ())
    }
    pub fn verify(&self, email: &str, code: &str) -> Result<(), RemoteError> {
        if !valid_email(email) || code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(RemoteError::Configuration);
        }
        let (base, revision) = self.store.begin_login()?;
        let bytes = self.request(
            &base,
            "v1/auth/verify",
            None,
            Some(serde_json::json!({"email":email,"code":code})),
        )?;
        let response: TokenResponse =
            serde_json::from_slice(&bytes).map_err(|_| RemoteError::InvalidResponse)?;
        self.validate_tokens(&response, email)?;
        let account = Identity {
            id: response.user.id,
            email: response.user.email,
            project: base,
            session: uuid(),
        };
        if !valid_uuid(&account.session) {
            return Err(RemoteError::Storage);
        }
        let tokens = Tokens {
            access: response.access_token,
            refresh: response.refresh_token,
            expiry: now() + response.expires_in,
        };
        let scope = account.credential_scope();
        let mut staged = false;
        let saved = self.store.config.transaction(false, |configuration| {
            if configuration.base_url.as_deref() != Some(account.project.as_str()) {
                return Err(RemoteError::Changed);
            }
            self.store.data.transaction(true, |data| {
                data.validate()?;
                if data.auth_revision != revision {
                    return Err(RemoteError::Changed);
                }
                self.vault.write(
                    &scope,
                    &serde_json::to_vec(&tokens).map_err(|_| RemoteError::Storage)?,
                )?;
                staged = true;
                data.active = Some(account);
                data.auth_revision = crate::storage::nonce();
                Ok(())
            })
        });
        if saved.is_err() && staged {
            // The failed login's unique scope was never referenced by persisted active state.
            let _ = self.vault.remove(&scope);
        }
        saved?;
        self.store.announce()
    }
    fn validate_tokens(&self, response: &TokenResponse, email: &str) -> Result<(), RemoteError> {
        if !valid_uuid(&response.user.id)
            || !response.user.email.eq_ignore_ascii_case(email)
            || response.access_token.is_empty()
            || response.access_token.len() > 256
            || response
                .access_token
                .bytes()
                .any(|b| !(33..127).contains(&b))
            || response.refresh_token.as_ref().is_some_and(|v| {
                v.is_empty() || v.len() > 256 || v.bytes().any(|b| !(33..127).contains(&b))
            })
            || !(1..=2_592_000).contains(&response.expires_in)
        {
            return Err(RemoteError::InvalidResponse);
        }
        Ok(())
    }
    pub fn logout(&self) -> Result<(), RemoteError> {
        let snapshot = self.store.snapshot()?;
        let account = snapshot.active;
        let tokens = account
            .as_ref()
            .map(|a| self.vault.read(&a.credential_scope()))
            .transpose()?
            .flatten();
        self.store.data.transaction(true, |data| {
            data.validate()?;
            if data.active != account {
                return Err(RemoteError::Changed);
            }
            data.active = None;
            data.auth_revision = crate::storage::nonce();
            Ok(())
        })?;
        // Persist the local sign-out before deleting the old session's credentials.
        // Announce even on deletion failure so TSF cannot queue against the retired account.
        let removed = account
            .as_ref()
            .map(|a| self.vault.remove(&a.credential_scope()))
            .transpose()
            .map_err(|_| RemoteError::Storage);
        let announced = self.store.announce();
        if let (Some(account), Some(bytes)) = (account, tokens)
            && let Ok(tokens) = serde_json::from_slice::<Tokens>(&bytes)
        {
            let _ = self.request(
                &account.project,
                "v1/auth/logout",
                Some(&tokens.access),
                Some(serde_json::json!({"refresh_token":tokens.refresh.unwrap_or_default()})),
            );
        }
        removed?;
        announced?;
        Ok(())
    }
    pub fn sync(&self) -> Result<(), RemoteError> {
        std::fs::create_dir_all(&self.store.directory).map_err(|_| RemoteError::Storage)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.store.directory.join("sync.lock"))
            .map_err(|_| RemoteError::Storage)?;
        lock.try_lock().map_err(|_| RemoteError::Changed)?;
        let snapshot = self.store.snapshot()?;
        let account = snapshot.active.ok_or(RemoteError::Denied)?;
        if account.project != self.base()? {
            return Err(RemoteError::Changed);
        }
        let scope = account.credential_scope();
        let mut tokens: Tokens =
            serde_json::from_slice(&self.vault.read(&scope)?.ok_or(RemoteError::Denied)?)
                .map_err(|_| RemoteError::Storage)?;
        if tokens.expiry < now() + 60 {
            let refresh = tokens.refresh.clone().ok_or(RemoteError::Denied)?;
            let bytes = self.request(
                &account.project,
                "v1/auth/refresh",
                None,
                Some(serde_json::json!({"refresh_token":refresh})),
            )?;
            let response: TokenResponse =
                serde_json::from_slice(&bytes).map_err(|_| RemoteError::InvalidResponse)?;
            self.validate_tokens(&response, &account.email)?;
            if response.user.id != account.id {
                return Err(RemoteError::Changed);
            }
            tokens = Tokens {
                access: response.access_token,
                refresh: response.refresh_token.or(Some(refresh)),
                expiry: now() + response.expires_in,
            };
            self.store.data.transaction(false, |data| {
                if data.active.as_ref() != Some(&account) {
                    return Err(RemoteError::Changed);
                }
                self.vault.write(
                    &scope,
                    &serde_json::to_vec(&tokens).map_err(|_| RemoteError::Storage)?,
                )
            })?;
        }
        let pending = snapshot
            .pending
            .get(&account.scope())
            .cloned()
            .unwrap_or_default();
        let mut acknowledged = BTreeSet::new();
        for batch in event_batches(&pending)? {
            if self.store.snapshot()?.active.as_ref() != Some(&account) {
                return Err(RemoteError::Changed);
            }
            let body = serde_json::to_value(batch).map_err(|_| RemoteError::Storage)?;
            let bytes = self.request(
                &account.project,
                "v1/events",
                Some(&tokens.access),
                Some(body),
            )?;
            let ids: Vec<String> =
                serde_json::from_slice(&bytes).map_err(|_| RemoteError::InvalidResponse)?;
            if ids.len() > batch.len() || ids.iter().any(|id| !valid_uuid(id)) {
                return Err(RemoteError::InvalidResponse);
            }
            let sent: BTreeSet<_> = batch.iter().map(|e| &e.id).collect();
            acknowledged.extend(ids.into_iter().filter(|id| sent.contains(id)));
        }
        if self.store.snapshot()?.active.as_ref() != Some(&account) {
            return Err(RemoteError::Changed);
        }
        let bytes = self.request(&account.project, "v1/words", Some(&tokens.access), None)?;
        let words: Vec<Word> =
            serde_json::from_slice(&bytes).map_err(|_| RemoteError::InvalidResponse)?;
        let mut ids = BTreeSet::new();
        if words.len() > 5000
            || words
                .iter()
                .any(|word| !word.valid() || word.uses > 1_000_000_000 || !ids.insert(word.id()))
        {
            return Err(RemoteError::InvalidResponse);
        }
        self.store.data.transaction(true, |data| {
            data.validate()?;
            if data.active.as_ref() != Some(&account) {
                return Err(RemoteError::Changed);
            }
            // Retain all events until the final word snapshot is available. If a later batch
            // or GET fails, retry with the same UUIDs; the backend applies each event only once.
            data.pending
                .entry(account.scope())
                .or_default()
                .retain(|event| !acknowledged.contains(&event.id));
            data.words.insert(account.scope(), words);
            data.validate()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::MemoryVault;
    use crate::http::Response;
    use std::os::windows::fs::OpenOptionsExt;
    fn deny_replacement(path: &std::path::Path) -> std::io::Result<std::fs::File> {
        use windows::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
            .open(path)
    }
    struct FaultVault<'a> {
        inner: &'a MemoryVault,
        block_replace: Option<PathBuf>,
        held: std::cell::RefCell<Option<std::fs::File>>,
        staged: std::cell::RefCell<Option<String>>,
        fail_remove: bool,
    }
    impl<'a> FaultVault<'a> {
        fn new(inner: &'a MemoryVault) -> Self {
            Self {
                inner,
                block_replace: None,
                held: Default::default(),
                staged: Default::default(),
                fail_remove: false,
            }
        }
    }
    impl Vault for FaultVault<'_> {
        fn read(&self, scope: &str) -> Result<Option<Vec<u8>>, RemoteError> {
            self.inner.read(scope)
        }
        fn write(&self, scope: &str, bytes: &[u8]) -> Result<(), RemoteError> {
            if let Some(path) = &self.block_replace {
                *self.held.borrow_mut() =
                    Some(deny_replacement(path).map_err(|_| RemoteError::Storage)?);
            }
            self.inner.write(scope, bytes)?;
            *self.staged.borrow_mut() = Some(scope.into());
            Ok(())
        }
        fn remove(&self, scope: &str) -> Result<(), RemoteError> {
            if self.fail_remove {
                Err(RemoteError::Storage)
            } else {
                self.inner.remove(scope)
            }
        }
    }
    fn tokens_response() -> Vec<u8> {
        serde_json::json!({"access_token":"fixture-access","refresh_token":"fixture-refresh","expires_in":3600,"user":{"id":"00000000-0000-0000-0000-000000000001","email":"fixture@example.com"}}).to_string().into_bytes()
    }
    struct Fake {
        requests: std::cell::RefCell<Vec<(String, Option<serde_json::Value>)>>,
        pending: std::cell::RefCell<Vec<Event>>,
    }
    impl Transport for Fake {
        fn request(
            &self,
            url: &str,
            _method: &str,
            _token: Option<&str>,
            body: Option<&[u8]>,
            _local: bool,
            _limit: usize,
        ) -> Result<Response, RemoteError> {
            let value =
                body.map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).unwrap());
            self.requests.borrow_mut().push((url.into(), value.clone()));
            let bytes = if url.ends_with("/verify") {
                tokens_response()
            } else if url.ends_with("/events") {
                let events: Vec<Event> = serde_json::from_value(value.unwrap()).unwrap();
                *self.pending.borrow_mut() = events.clone();
                serde_json::to_vec(&events.iter().map(|e| e.id.clone()).collect::<Vec<_>>())
                    .unwrap()
            } else if url.ends_with("/words") {
                serde_json::to_vec(
                    &self
                        .pending
                        .borrow()
                        .iter()
                        .map(|event| {
                            let mut word = event.word();
                            word.uses = 1;
                            word
                        })
                        .collect::<Vec<_>>(),
                )
                .unwrap()
            } else {
                b"{}".to_vec()
            };
            Ok(Response { status: 200, bytes })
        }
    }
    #[derive(Default)]
    struct BatchHTTP<'a> {
        body_sizes: std::cell::RefCell<Vec<usize>>,
        received: std::cell::RefCell<Vec<String>>,
        accepted: std::cell::RefCell<BTreeSet<String>>,
        words: std::cell::RefCell<BTreeMap<String, Word>>,
        fail_on: std::cell::Cell<Option<usize>>,
        fail_words: std::cell::Cell<bool>,
        change: std::cell::RefCell<Option<Box<dyn FnOnce() + 'a>>>,
    }
    impl Transport for BatchHTTP<'_> {
        fn request(
            &self,
            url: &str,
            _method: &str,
            _token: Option<&str>,
            body: Option<&[u8]>,
            _local: bool,
            _limit: usize,
        ) -> Result<Response, RemoteError> {
            let bytes = if url.ends_with("/verify") {
                tokens_response()
            } else if url.ends_with("/events") {
                let body = body.unwrap();
                self.body_sizes.borrow_mut().push(body.len());
                let attempt = self.body_sizes.borrow().len();
                if body.len() > 1_000_000 {
                    return Ok(Response {
                        status: 400,
                        bytes: b"{}".to_vec(),
                    });
                }
                if self.fail_on.get() == Some(attempt) {
                    return Err(RemoteError::Network);
                }
                let events: Vec<Event> = serde_json::from_slice(body).unwrap();
                for event in &events {
                    assert!(event.valid());
                    self.received.borrow_mut().push(event.id.clone());
                    if self.accepted.borrow_mut().insert(event.id.clone()) {
                        let mut words = self.words.borrow_mut();
                        let word = words
                            .entry(event.word().id())
                            .or_insert_with(|| event.word());
                        if event.kind == "study" {
                            word.uses += 1;
                        } else {
                            word.mastered = event.mastered;
                        }
                    }
                }
                let change = self.change.borrow_mut().take();
                if let Some(change) = change {
                    change();
                }
                serde_json::to_vec(&events.iter().map(|e| &e.id).collect::<Vec<_>>()).unwrap()
            } else if url.ends_with("/words") {
                if self.fail_words.replace(false) {
                    return Err(RemoteError::Network);
                }
                serde_json::to_vec(&self.words.borrow().values().collect::<Vec<_>>()).unwrap()
            } else {
                panic!("unexpected account fixture request");
            };
            Ok(Response { status: 200, bytes })
        }
    }
    fn seed_batch_pending(store: &Store, account: &Identity) -> Vec<Event> {
        let events: Vec<_> = (0..5000)
            .map(|index| Event {
                id: uuid(),
                english: "a".repeat(80),
                chinese: "测".repeat(16),
                pos: "n".repeat(24),
                kind: if index == 4999 { "mastered" } else { "study" }.into(),
                mastered: index == 4999,
            })
            .collect();
        assert!(serde_json::to_vec(&events).unwrap().len() > 1_000_000);
        store
            .data
            .transaction(true, |data| {
                data.pending.insert(account.scope(), events.clone());
                data.validate()
            })
            .unwrap();
        events
    }
    #[test]
    fn large_offline_queue_is_sent_in_byte_bounded_ordered_batches() {
        let root = std::env::temp_dir().join(format!(
            "lingomate-account-batches-{}",
            crate::storage::nonce()
        ));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let http = BatchHTTP::default();
        let service = Service {
            store: &store,
            vault: &vault,
            http: &http,
        };
        store.save_url("https://example.com").unwrap();
        service.verify("fixture@example.com", "123456").unwrap();
        let account = store.snapshot().unwrap().active.unwrap();
        let events = seed_batch_pending(&store, &account);
        service.sync().unwrap();
        assert!(http.body_sizes.borrow().len() > 1);
        assert!(http.body_sizes.borrow().iter().all(|size| *size <= 900_000));
        assert_eq!(
            *http.received.borrow(),
            events.iter().map(|e| e.id.clone()).collect::<Vec<_>>()
        );
        let data = store.snapshot().unwrap();
        assert!(data.pending[&account.scope()].is_empty());
        let words = data.visible(&account);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].uses, 4999);
        assert!(words[0].mastered);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn later_batch_or_word_snapshot_failure_retries_the_same_ids_without_double_counting() {
        for fail_batch in [true, false] {
            let root = std::env::temp_dir().join(format!(
                "lingomate-account-batch-retry-{}",
                crate::storage::nonce()
            ));
            let store = Store::isolated(root.clone());
            let vault = MemoryVault::default();
            let http = BatchHTTP::default();
            http.fail_on.set(fail_batch.then_some(2));
            http.fail_words.set(!fail_batch);
            let service = Service {
                store: &store,
                vault: &vault,
                http: &http,
            };
            store.save_url("https://example.com").unwrap();
            service.verify("fixture@example.com", "123456").unwrap();
            let account = store.snapshot().unwrap().active.unwrap();
            let events = seed_batch_pending(&store, &account);
            assert_eq!(service.sync(), Err(RemoteError::Network));
            assert!(!http.accepted.borrow().is_empty());
            assert_eq!(http.accepted.borrow().len() < events.len(), fail_batch);
            assert_eq!(
                store.snapshot().unwrap().pending[&account.scope()].len(),
                events.len()
            );
            assert_eq!(store.snapshot().unwrap().visible(&account)[0].uses, 4999);
            http.fail_on.set(None);
            service.sync().unwrap();
            let data = store.snapshot().unwrap();
            assert!(data.pending[&account.scope()].is_empty());
            assert_eq!(http.accepted.borrow().len(), events.len());
            let words = data.visible(&account);
            assert_eq!(words[0].uses, 4999);
            assert!(words[0].mastered);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn session_change_stops_before_the_next_batch_and_preserves_pending_events() {
        let root = std::env::temp_dir().join(format!(
            "lingomate-account-batch-session-{}",
            crate::storage::nonce()
        ));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let http = BatchHTTP::default();
        let service = Service {
            store: &store,
            vault: &vault,
            http: &http,
        };
        store.save_url("https://example.com").unwrap();
        service.verify("fixture@example.com", "123456").unwrap();
        let account = store.snapshot().unwrap().active.unwrap();
        let events = seed_batch_pending(&store, &account);
        let mut newer = account.clone();
        newer.session = uuid();
        let replacement = newer.clone();
        let changing = &store;
        *http.change.borrow_mut() = Some(Box::new(move || {
            changing
                .data
                .transaction(true, |data| {
                    data.active = Some(replacement);
                    Ok(())
                })
                .unwrap();
        }));
        assert_eq!(service.sync(), Err(RemoteError::Changed));
        assert_eq!(http.body_sizes.borrow().len(), 1);
        let data = store.snapshot().unwrap();
        assert_eq!(data.active, Some(newer.clone()));
        assert_eq!(data.pending[&account.scope()].len(), events.len());
        assert!(!data.words.contains_key(&account.scope()));
        std::fs::remove_dir_all(root).unwrap();
    }
    struct DuringVerify<'a> {
        change: std::cell::RefCell<Option<Box<dyn FnOnce() + 'a>>>,
    }
    impl Transport for DuringVerify<'_> {
        fn request(
            &self,
            url: &str,
            method: &str,
            token: Option<&str>,
            body: Option<&[u8]>,
            local: bool,
            _limit: usize,
        ) -> Result<Response, RemoteError> {
            assert!(url.ends_with("/v1/auth/verify"));
            assert_eq!(method, "POST");
            assert!(token.is_none() && !local);
            let body: serde_json::Value = serde_json::from_slice(body.unwrap()).unwrap();
            assert_eq!(
                body,
                serde_json::json!({"email":"fixture@example.com","code":"123456"})
            );
            let change = self.change.borrow_mut().take();
            if let Some(change) = change {
                change();
            }
            Ok(Response {
                status: 200,
                bytes: tokens_response(),
            })
        }
    }
    #[test]
    fn stale_verify_cannot_relogin_after_none_some_none_or_replace_a_new_login() {
        for logout in [false, true] {
            let root = std::env::temp_dir().join(format!(
                "lingomate-stale-account-{}",
                crate::storage::nonce()
            ));
            let store = Store::isolated(root.clone());
            let vault = MemoryVault::default();
            store.save_url("https://example.com").unwrap();
            let newer = std::cell::RefCell::new(None);
            let http = DuringVerify {
                change: std::cell::RefCell::new(Some(Box::new(|| {
                    let next_http = Fake {
                        requests: Default::default(),
                        pending: Default::default(),
                    };
                    let next = Service {
                        store: &store,
                        vault: &vault,
                        http: &next_http,
                    };
                    next.verify("fixture@example.com", "123456").unwrap();
                    *newer.borrow_mut() = store.snapshot().unwrap().active;
                    if logout {
                        next.logout().unwrap();
                    }
                }))),
            };
            let service = Service {
                store: &store,
                vault: &vault,
                http: &http,
            };
            assert_eq!(
                service.verify("fixture@example.com", "123456"),
                Err(RemoteError::Changed)
            );
            let active = store.snapshot().unwrap().active;
            if logout {
                assert!(active.is_none());
                assert!(
                    vault
                        .read(&newer.borrow().as_ref().unwrap().credential_scope())
                        .unwrap()
                        .is_none()
                );
            } else {
                assert_eq!(active, *newer.borrow());
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn changing_service_address_invalidates_pending_verify_even_when_restored() {
        for restore in [false, true] {
            let root = std::env::temp_dir()
                .join(format!("lingomate-stale-url-{}", crate::storage::nonce()));
            let store = Store::isolated(root.clone());
            let vault = MemoryVault::default();
            store.save_url("https://example.com").unwrap();
            let http = DuringVerify {
                change: std::cell::RefCell::new(Some(Box::new(|| {
                    store.save_url("https://other.example.com").unwrap();
                    if restore {
                        store.save_url("https://example.com").unwrap();
                    }
                }))),
            };
            let service = Service {
                store: &store,
                vault: &vault,
                http: &http,
            };
            assert_eq!(
                service.verify("fixture@example.com", "123456"),
                Err(RemoteError::Changed)
            );
            assert!(store.snapshot().unwrap().active.is_none());
            assert!(
                vault
                    .read("account/https://example.com|00000000-0000-0000-0000-000000000001")
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                store.url().unwrap().as_deref(),
                Some(if restore {
                    "https://example.com"
                } else {
                    "https://other.example.com"
                })
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn existing_account_data_without_auth_revision_remains_readable() {
        let root = std::env::temp_dir().join(format!(
            "lingomate-account-legacy-{}",
            crate::storage::nonce()
        ));
        let store = Store::isolated(root.clone());
        std::fs::create_dir_all(&store.directory).unwrap();
        std::fs::write(
            store.directory.join("learning.json"),
            br#"{"version":1,"active":null,"words":{},"pending":{}}"#,
        )
        .unwrap();
        assert!(store.snapshot().unwrap().auth_revision.is_empty());
        store.save_url("https://example.com").unwrap();
        assert!(!store.snapshot().unwrap().auth_revision.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn otp_selected_words_acknowledgement_and_logout_are_separate_from_local_words() {
        let root =
            std::env::temp_dir().join(format!("lingomate-account-{}", crate::storage::nonce()));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let http = Fake {
            requests: Default::default(),
            pending: Default::default(),
        };
        let service = Service {
            store: &store,
            vault: &vault,
            http: &http,
        };
        assert!(store.save_url("http://example.com").is_err());
        store.save_url("https://example.com").unwrap();
        assert!(service.verify("fixture@example.com", "12345").is_err());
        service.send_code("fixture@example.com").unwrap();
        service.verify("fixture@example.com", "123456").unwrap();
        let identity = store.snapshot().unwrap().active.unwrap();
        let word = Word::new("study".into(), "学习".into(), "v.".into());
        store
            .append(&word, "study", false, identity.fingerprint())
            .unwrap();
        assert_eq!(store.snapshot().unwrap().visible(&identity)[0].uses, 1);
        service.sync().unwrap();
        let data = store.snapshot().unwrap();
        assert!(data.pending[&identity.scope()].is_empty());
        assert_eq!(data.visible(&identity)[0].uses, 1);
        service.logout().unwrap();
        assert!(store.snapshot().unwrap().active.is_none());
        assert!(vault.read(&identity.credential_scope()).unwrap().is_none());
        assert!(
            store
                .append(&word, "study", false, identity.fingerprint())
                .is_err()
        );
        let file = std::fs::read_to_string(root.join("Account/learning.json")).unwrap();
        assert!(!file.contains("fixture-access") && !file.contains("fixture-refresh"));
        assert!(!root.join("LocalWords/words.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_login_and_logout_persistence_preserve_the_previous_session_credentials() {
        let root = std::env::temp_dir().join(format!(
            "lingomate-account-atomic-{}",
            crate::storage::nonce()
        ));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let http = Fake {
            requests: Default::default(),
            pending: Default::default(),
        };
        let service = Service {
            store: &store,
            vault: &vault,
            http: &http,
        };
        store.save_url("https://example.com").unwrap();
        service.verify("fixture@example.com", "123456").unwrap();
        let active = store.snapshot().unwrap().active.unwrap();
        let previous = vault.read(&active.credential_scope()).unwrap().unwrap();
        let path = store.directory.join("learning.json");
        let mut fault = FaultVault::new(&vault);
        fault.block_replace = Some(path.clone());
        let failed = Service {
            store: &store,
            vault: &fault,
            http: &http,
        };
        assert_eq!(
            failed.verify("fixture@example.com", "123456"),
            Err(RemoteError::Storage)
        );
        drop(fault.held.borrow_mut().take());
        let staged = fault.staged.borrow().clone().unwrap();
        assert_ne!(staged, active.credential_scope());
        assert!(vault.read(&staged).unwrap().is_none());
        assert_eq!(store.snapshot().unwrap().active, Some(active.clone()));
        assert_eq!(
            vault.read(&active.credential_scope()).unwrap(),
            Some(previous.clone())
        );

        // Deny atomic file replacement before logout; credentials must remain untouched.
        let blocked = deny_replacement(&path).unwrap();
        assert_eq!(service.logout(), Err(RemoteError::Storage));
        drop(blocked);
        assert_eq!(store.snapshot().unwrap().active, Some(active.clone()));
        assert_eq!(
            vault.read(&active.credential_scope()).unwrap(),
            Some(previous)
        );
        service.logout().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_credential_removal_keeps_local_logout_and_cannot_restore_the_old_session() {
        let root = std::env::temp_dir().join(format!(
            "lingomate-account-delete-{}",
            crate::storage::nonce()
        ));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let http = Fake {
            requests: Default::default(),
            pending: Default::default(),
        };
        let service = Service {
            store: &store,
            vault: &vault,
            http: &http,
        };
        store.save_url("https://example.com").unwrap();
        service.verify("fixture@example.com", "123456").unwrap();
        let before = store.snapshot().unwrap();
        let active = before.active.unwrap();
        let mut fault = FaultVault::new(&vault);
        fault.fail_remove = true;
        let failed = Service {
            store: &store,
            vault: &fault,
            http: &http,
        };
        assert_eq!(failed.logout(), Err(RemoteError::Storage));
        let after = store.snapshot().unwrap();
        assert!(after.active.is_none());
        assert_ne!(after.auth_revision, before.auth_revision);
        assert!(vault.read(&active.credential_scope()).unwrap().is_some());
        assert_eq!(service.sync(), Err(RemoteError::Denied));
        // A new session never reuses the retained old token scope.
        service.verify("fixture@example.com", "123456").unwrap();
        let current = store.snapshot().unwrap().active.unwrap();
        assert_ne!(current.credential_scope(), active.credential_scope());
        service.logout().unwrap();
        vault.remove(&active.credential_scope()).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_account_maps_reject_a_65th_scope_without_changing_persisted_data() {
        let root = std::env::temp_dir().join(format!(
            "lingomate-account-bounds-{}",
            crate::storage::nonce()
        ));
        let store = Store::isolated(root.clone());
        let vault = MemoryVault::default();
        let http = Fake {
            requests: Default::default(),
            pending: Default::default(),
        };
        store.save_url("https://example.com").unwrap();
        let identity = Identity {
            id: "00000000-0000-0000-0000-000000000040".into(),
            email: "fixture@example.com".into(),
            project: "https://example.com".into(),
            session: uuid(),
        };
        store
            .data
            .transaction(true, |data| {
                data.active = Some(identity.clone());
                for index in 0..64 {
                    let scope = format!("https://example.com|00000000-0000-0000-0000-{index:012x}");
                    data.words.insert(scope.clone(), Vec::new());
                    data.pending.insert(scope, Vec::new());
                }
                data.validate()
            })
            .unwrap();
        let path = store.directory.join("learning.json");
        let before = std::fs::read(&path).unwrap();
        let word = Word::new("study".into(), "学习".into(), "v.".into());
        assert_eq!(
            store.append(&word, "study", false, identity.fingerprint()),
            Err(RemoteError::Storage)
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        vault
            .write(
                &identity.credential_scope(),
                &serde_json::to_vec(&Tokens {
                    access: "fixture-access".into(),
                    refresh: None,
                    expiry: now() + 3600,
                })
                .unwrap(),
            )
            .unwrap();
        let service = Service {
            store: &store,
            vault: &vault,
            http: &http,
        };
        assert_eq!(service.sync(), Err(RemoteError::Storage));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let after = store.snapshot().unwrap();
        assert_eq!(after.active, Some(identity));
        assert_eq!(after.pending.len(), 64);
        assert_eq!(after.words.len(), 64);
        assert_eq!(http.requests.borrow().len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
