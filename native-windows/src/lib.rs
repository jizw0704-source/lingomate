//! Windows TSF shell over the shared, validated local engine protocol.
pub mod bridge;
pub mod input;
#[cfg(windows)]
mod tsf;
