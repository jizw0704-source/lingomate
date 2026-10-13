//! Existing line-JSON entry point; native clients can use the same adapter in-process.
use lingomate_engine::Adapter;
use std::io::{self, BufRead, Write};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("research root argument required")?;
    let mut adapter = Adapter::load(Path::new(&root))?;
    let mut arguments = std::env::args().skip(2);
    let mut memory = false;
    let mut focused = false;
    let mut expressions = false;
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--memory" if !memory => {
                let path = arguments.next().ok_or("memory path required")?;
                adapter.set_memory_path(Some(path.into()));
                memory = true;
            }
            "--mac-candidates" if !focused => {
                adapter.focus_candidates();
                focused = true;
            }
            "--fixed-expressions" if !expressions => {
                adapter.enable_fixed_expressions(Path::new(&root))?;
                expressions = true;
            }
            _ => return Err("unsupported or duplicate bridge option".into()),
        }
    }
    eprintln!(
        "Qingjian adapter ready; details={} words; local-only",
        adapter.detail_count()
    );
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let response = adapter.request_json(&line?);
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
