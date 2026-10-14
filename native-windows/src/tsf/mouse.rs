//! Current-thread mouse-button metadata disarms Shift taps; no global hooks or message text.
use super::{Guard, State};
use std::cell::RefCell;
use std::rc::Weak;
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::Result;
thread_local! {static TARGETS:RefCell<Vec<(usize,Weak<State>)>>=const{RefCell::new(Vec::new())};}
pub struct Watch {
    hook: HHOOK,
    _guard: Guard,
}
impl Watch {
    pub fn new(state: Weak<State>) -> Result<Self> {
        let hook = unsafe {
            SetWindowsHookExW(WH_GETMESSAGE, Some(procedure), None, GetCurrentThreadId())
        }?;
        TARGETS.with(|targets| targets.borrow_mut().push((hook.0 as usize, state)));
        Ok(Self {
            hook,
            _guard: Guard::new(),
        })
    }
}
impl Drop for Watch {
    fn drop(&mut self) {
        TARGETS.with(|targets| {
            targets
                .borrow_mut()
                .retain(|(handle, _)| *handle != self.hook.0 as usize)
        });
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
        }
    }
}
fn observe(message: u32) {
    if !matches!(
        message,
        WM_LBUTTONDOWN
            | WM_RBUTTONDOWN
            | WM_MBUTTONDOWN
            | WM_XBUTTONDOWN
            | WM_NCLBUTTONDOWN
            | WM_NCRBUTTONDOWN
            | WM_NCMBUTTONDOWN
    ) {
        return;
    }
    TARGETS.with(|targets| {
        for (_, state) in targets.borrow().iter() {
            if let Some(state) = state.upgrade() {
                state
                    .mouse_revision
                    .set(state.mouse_revision.get().wrapping_add(1));
            }
        }
    });
}
unsafe extern "system" fn procedure(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && lparam.0 != 0 {
        let message = unsafe { &*(lparam.0 as *const MSG) };
        observe(message.message);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
#[cfg(test)]
#[test]
fn only_button_metadata_disarms_taps_and_does_not_change_composition() {
    let state = State::with_display_preferences(false);
    state.input.borrow_mut().raw = "synthetic".into();
    TARGETS.with(|targets| {
        targets
            .borrow_mut()
            .push((1, std::rc::Rc::downgrade(&state)))
    });
    observe(WM_MOUSEMOVE);
    observe(WM_KEYDOWN);
    assert_eq!(state.mouse_revision.get(), 0);
    observe(WM_LBUTTONDOWN);
    assert_eq!(state.mouse_revision.get(), 1);
    assert_eq!(state.input.borrow().raw, "synthetic");
    TARGETS.with(|targets| targets.borrow_mut().retain(|(id, _)| *id != 1));
}
