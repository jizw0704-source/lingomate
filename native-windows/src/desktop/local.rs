//! Verify Ollama is in local-only mode before a candidate is given to its model.
use crate::http::Transport;
use crate::remote::{AIConfiguration, Endpoint, RemoteError};
use serde_json::Value;

pub fn verify(configuration: &AIConfiguration, http: &impl Transport) -> Result<(), RemoteError> {
    configuration.validate_for(true)?;
    let endpoint = Endpoint::parse(&configuration.endpoint, true)?;
    if endpoint.path != "/v1/chat/completions" {
        return Err(RemoteError::LocalSafety);
    }
    let base = configuration
        .endpoint
        .strip_suffix("/v1/chat/completions")
        .ok_or(RemoteError::LocalSafety)?;
    let status = http.request(&format!("{base}/api/status"), "GET", None, None, true, 8192)?;
    if status.status != 200 {
        return Err(RemoteError::LocalSafety);
    }
    let status: Value =
        serde_json::from_slice(&status.bytes).map_err(|_| RemoteError::LocalSafety)?;
    if status.pointer("/cloud/disabled").and_then(Value::as_bool) != Some(true) {
        return Err(RemoteError::LocalSafety);
    }
    let tags = http.request(
        &format!("{base}/api/tags"),
        "GET",
        None,
        None,
        true,
        1_000_000,
    )?;
    if tags.status != 200 {
        return Err(RemoteError::Unavailable);
    }
    let tags: Value =
        serde_json::from_slice(&tags.bytes).map_err(|_| RemoteError::InvalidResponse)?;
    let models = tags
        .get("models")
        .and_then(Value::as_array)
        .ok_or(RemoteError::InvalidResponse)?;
    let name = if configuration.model.contains(':') {
        configuration.model.clone()
    } else {
        format!("{}:latest", configuration.model)
    };
    let model = models
        .iter()
        .find(|v| {
            v.get("name").and_then(Value::as_str) == Some(&name)
                || v.get("model").and_then(Value::as_str) == Some(&name)
        })
        .ok_or(RemoteError::Unavailable)?;
    if model
        .get("size")
        .and_then(Value::as_u64)
        .is_none_or(|size| size < 1_000_000)
        || model
            .get("digest")
            .and_then(Value::as_str)
            .is_none_or(|v| v.len() != 64 || !v.bytes().all(|b| b.is_ascii_hexdigit()))
        || has_remote(model)
    {
        return Err(RemoteError::LocalSafety);
    }
    let body = serde_json::to_vec(&serde_json::json!({"model":configuration.model}))
        .map_err(|_| RemoteError::Configuration)?;
    let info = http.request(
        &format!("{base}/api/show"),
        "POST",
        None,
        Some(&body),
        true,
        1_000_000,
    )?;
    if info.status != 200 {
        return Err(RemoteError::Unavailable);
    }
    let info: Value =
        serde_json::from_slice(&info.bytes).map_err(|_| RemoteError::InvalidResponse)?;
    if has_remote(&info)
        || info
            .get("model_info")
            .and_then(Value::as_object)
            .is_none_or(|v| v.is_empty())
        || info
            .get("capabilities")
            .and_then(Value::as_array)
            .is_none_or(|v| !v.iter().any(|c| c.as_str() == Some("completion")))
    {
        return Err(RemoteError::LocalSafety);
    }
    Ok(())
}
fn has_remote(value: &Value) -> bool {
    ["remote_host", "remote_model"]
        .iter()
        .any(|key| value.get(key).is_some_and(|v| v.as_str() != Some("")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::Response;
    struct Fake {
        disabled: bool,
        remote: bool,
        requests: std::cell::Cell<usize>,
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
            self.requests.set(self.requests.get() + 1);
            if let Some(body) = body {
                assert!(!std::str::from_utf8(body).unwrap().contains("中文候选"));
            }
            let value = if url.ends_with("status") {
                serde_json::json!({"cloud":{"disabled":self.disabled}})
            } else if url.ends_with("tags") {
                serde_json::json!({"models":[{"name":"fixture:latest","size":123456789,"digest":"a".repeat(64),"remote_host":if self.remote {"https://cloud.example"}else{""}}]})
            } else {
                serde_json::json!({"capabilities":["completion"],"model_info":{"general.architecture":"fixture"}})
            };
            Ok(Response {
                status: 200,
                bytes: serde_json::to_vec(&value).unwrap(),
            })
        }
    }
    #[test]
    fn cloud_and_unverified_models_are_rejected_before_translation_text() {
        let config = AIConfiguration {
            version: 1,
            enabled: true,
            endpoint: "http://127.0.0.1:11434/v1/chat/completions".into(),
            model: "fixture".into(),
            revision: String::new(),
        };
        for (disabled, remote, count) in [(false, false, 1), (true, true, 2), (true, false, 3)] {
            let fake = Fake {
                disabled,
                remote,
                requests: Default::default(),
            };
            assert_eq!(verify(&config, &fake).is_ok(), disabled && !remote);
            assert_eq!(fake.requests.get(), count);
        }
    }
}
