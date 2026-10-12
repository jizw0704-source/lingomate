use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=tools/make-input-icon.ps1");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
    {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let icon = out.join("lingomate.ico");
    assert!(
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                "tools/make-input-icon.ps1",
                "-Output"
            ])
            .arg(&icon)
            .status()
            .unwrap()
            .success(),
        "icon generation failed"
    );
    let rc = out.join("lingomate.rc");
    std::fs::write(
        &rc,
        format!(
            "101 ICON \"{}\"\n",
            icon.to_string_lossy().replace('\\', "\\\\")
        ),
    )
    .unwrap();
    let res = out.join("lingomate.res");
    let mut compiler = PathBuf::from("rc.exe");
    if Command::new(&compiler).arg("/?").output().is_err() {
        let kits =
            PathBuf::from(env::var_os("ProgramFiles(x86)").unwrap()).join("Windows Kits/10/bin");
        let mut versions: Vec<_> = std::fs::read_dir(kits)
            .unwrap()
            .flatten()
            .map(|v| v.path().join("x64/rc.exe"))
            .filter(|p| p.exists())
            .collect();
        versions.sort();
        compiler = versions
            .pop()
            .expect("Windows SDK resource compiler required");
    }
    assert!(
        Command::new(compiler)
            .arg("/nologo")
            .arg(format!("/fo{}", res.display()))
            .arg(&rc)
            .status()
            .unwrap()
            .success(),
        "resource compilation failed"
    );
    println!("cargo:rustc-link-arg={}", res.display());
}
