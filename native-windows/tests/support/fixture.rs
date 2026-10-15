//! Synthetic fault-injection helper, excluded from the install package.
#![cfg_attr(windows, windows_subsystem = "windows")]
use std::io::{BufRead, Write};
#[cfg(windows)]
mod identity_probe;
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    #[cfg(windows)]
    if mode == "identity-chain" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        let passed = identity_probe::run(&args).unwrap_or(false);
        std::process::exit(if passed { 0 } else { 1 });
    }
    for _ in std::io::stdin().lock().lines() {
        if mode == "slow-start-and-reply" {
            std::thread::sleep(std::time::Duration::from_millis(800));
            println!("{{\"input\":\"\",\"candidates\":[]}}");
        } else if mode == "hang" {
            std::thread::sleep(std::time::Duration::from_secs(30));
        } else if mode == "huge" {
            println!("{}", "x".repeat(4 * 1024 * 1024 + 1));
        } else {
            println!("not a JSON frame");
        }
        let _ = std::io::stdout().flush();
    }
}
