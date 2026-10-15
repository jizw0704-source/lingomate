#![cfg(windows)]
use lingomate_tsf::{helper, remote::RemoteError};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;

#[test]
fn identity_and_relay_use_only_fixed_metadata_without_user_state() {
    let executable = Path::new(env!("CARGO_BIN_EXE_lingomate-desktop"));
    let stop = AtomicBool::new(false);
    for args in [
        vec!["--identity".into()],
        vec!["--relay".into(), "--pipe".into(), "--identity".into()],
    ] {
        let child = helper::spawn(executable, &args, true).unwrap();
        let bytes = child.capture(b"", &stop, Duration::from_secs(5)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["unpacked"], true);
        assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value.as_object().unwrap().len(), 2);
    }
}

#[test]
fn owned_helper_cancellation_terminates_the_created_process_promptly() {
    let child = helper::spawn(
        Path::new(env!("CARGO_BIN_EXE_lingomate-fixture")),
        &["hang".into()],
        true,
    )
    .unwrap();
    let observer = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            child.id,
        )
    }
    .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let cancelling = stop.clone();
    let trigger = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        cancelling.store(true, Ordering::Release);
    });
    let started = Instant::now();
    assert_eq!(
        child.capture(b"synthetic\n", &stop, Duration::from_secs(5)),
        Err(RemoteError::Changed)
    );
    trigger.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        unsafe { WaitForSingleObject(observer, 1000) },
        WAIT_OBJECT_0
    );
    unsafe {
        let _ = CloseHandle(observer);
    }
}

#[test]
fn cancellation_before_pipe_handoff_rejects_the_body() {
    let child = helper::spawn(
        Path::new(env!("CARGO_BIN_EXE_lingomate-fixture")),
        &["hang".into()],
        true,
    )
    .unwrap();
    let stop = AtomicBool::new(true);
    assert_eq!(
        child.capture(b"synthetic\n", &stop, Duration::from_secs(5)),
        Err(RemoteError::Changed)
    );
}
