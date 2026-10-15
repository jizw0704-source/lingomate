//! Updating is an explicitly opened, separate desktop process, never a network task in TSF.
use super::{down, module_path};
use crate::desktop::updates::Mode;

pub(super) fn shortcut(key: usize) -> bool {
    key == 0x55 && down(0x11) && down(0x10) && !down(0x12) && !down(0x5B) && !down(0x5C)
}
pub(super) fn launch(background: bool) -> Result<(), ()> {
    let module = module_path().map_err(|_| ())?;
    let executable = crate::helper::desktop_path(&module).map_err(|_| ())?;
    let mode = if background {
        Mode::Background
    } else {
        Mode::Manual
    };
    // No preference reads in the TSF host: packaged desktop callers must leave
    // their private AppData view before either update gating or PowerShell.
    crate::helper::spawn(&executable, &mode.arguments(), false)
        .map(|_| ())
        .map_err(|_| ())
}
