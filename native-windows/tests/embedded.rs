#![cfg(windows)]
use lingomate_tsf::bridge::{Bridge, EngineFailure};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn embedded_core_rejects_a_request_without_poisoning_a_valid_handle() {
    let core = PathBuf::from(std::env::var("LINGOMATE_TEST_CORE").unwrap());
    let resources = PathBuf::from(std::env::var("LINGOMATE_TEST_RESOURCES").unwrap());
    let mut bridge = Bridge::start_embedded(&core, &resources, None).unwrap();
    let generation = bridge.generation;
    assert!(bridge.call(json!({"action":"missing","input":""})).is_err());
    assert_eq!(bridge.failure, Some(EngineFailure::Rejected));
    let frame = bridge
        .call(json!({"action":"query","input":"wode"}))
        .unwrap();
    assert_eq!(frame.candidates[0].text, "我的");
    assert_eq!(frame.generation, generation);
    assert_eq!(bridge.failure, None);
}

#[test]
fn embedded_core_reports_static_load_and_initialization_failures() {
    let core = PathBuf::from(std::env::var("LINGOMATE_TEST_CORE").unwrap());
    let resources = PathBuf::from(std::env::var("LINGOMATE_TEST_RESOURCES").unwrap());
    let missing = resources.join("owned-test-missing-engine.dll");
    assert!(matches!(
        Bridge::start_embedded(&missing, &resources, None),
        Err(EngineFailure::NativeLoad { .. })
    ));
    assert!(matches!(
        Bridge::start_embedded(&core, &resources.join("owned-test-missing-data"), None),
        Err(EngineFailure::CoreInit { code: 3 })
    ));
    assert!(matches!(
        Bridge::start_embedded(&PathBuf::from("lingomate_engine.dll"), &resources, None),
        Err(EngineFailure::NativeLoad { code: 87 })
    ));
}
