//! Windows TSF shell over the shared, validated local engine protocol.
pub mod bridge;
pub mod input;
pub mod learning;
#[cfg(windows)]
mod learning_window;
#[cfg(windows)]
mod native_core;
pub mod preferences;
pub mod punctuation;
#[cfg(windows)]
mod tsf;
pub mod worker;

#[cfg(windows)]
mod appearance;

pub mod candidate_layout;
