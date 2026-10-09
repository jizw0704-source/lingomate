//! Synthetic fault-injection helper, excluded from the install package.
use std::io::{BufRead, Write};
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    for _ in std::io::stdin().lock().lines() {
        if mode == "hang" {
            std::thread::sleep(std::time::Duration::from_secs(30));
        } else if mode == "huge" {
            println!("{}", "x".repeat(4 * 1024 * 1024 + 1));
        } else {
            println!("not a JSON frame");
        }
        let _ = std::io::stdout().flush();
    }
}
