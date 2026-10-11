//! Windows TSF shell over the shared, validated local engine protocol.
pub mod bridge;
pub mod input;
pub mod preferences;
#[cfg(windows)]
mod tsf;
