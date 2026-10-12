//! Windows TSF shell over the shared, validated local engine protocol.
pub mod bridge;
pub mod input;
pub mod learning;
#[cfg(windows)]
mod learning_window;
pub mod preferences;
pub mod punctuation;
#[cfg(windows)]
mod tsf;

#[cfg(windows)]
mod appearance;

pub mod candidate_layout;
