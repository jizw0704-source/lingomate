use super::{Guard, State, guarded};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::mem::ManuallyDrop;
use std::rc::{Rc, Weak};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::TextServices::*;
use windows::core::{Interface, Ref, Result, implement};

type Operation = (
    Option<String>,
    String,
    Option<crate::learning::Word>,
    Option<crate::bridge::Confirmation>,
    u64,
    u64,
);

pub(super) struct Document {
    pub context: ITfContext,
    client: u32,
    composition: RefCell<Option<ITfComposition>>,
    queue: RefCell<VecDeque<Operation>>,
    scheduled: Cell<bool>,
    applying: Cell<u32>,
    ending: Cell<bool>,
    pub terminated: Cell<bool>,
    pub epoch: Cell<u64>,
    state: Weak<State>,
}
impl Document {
    pub fn new(context: ITfContext, client: u32, state: Weak<State>) -> Rc<Self> {
        Rc::new(Self {
            context,
            client,
            composition: RefCell::new(None),
            queue: RefCell::new(VecDeque::new()),
            scheduled: Cell::new(false),
            applying: Cell::new(0),
            ending: Cell::new(false),
            terminated: Cell::new(false),
            epoch: Cell::new(0),
            state,
        })
    }
    pub fn is_idle(&self) -> bool {
        !self.scheduled.get() && self.applying.get() == 0 && self.queue.borrow().is_empty()
    }
    pub fn enqueue(
        self: &Rc<Self>,
        commit: Option<String>,
        preedit: String,
        study: Option<crate::learning::Word>,
        memory: Option<crate::bridge::Confirmation>,
    ) -> Result<()> {
        let account = self.state.upgrade().map_or(0, |s| s.account_scope());
        self.enqueue_scoped(commit, preedit, study, memory, account)
    }
    pub fn enqueue_scoped(
        self: &Rc<Self>,
        commit: Option<String>,
        preedit: String,
        study: Option<crate::learning::Word>,
        memory: Option<crate::bridge::Confirmation>,
        account: u64,
    ) -> Result<()> {
        if self.queue.borrow().len() >= crate::worker::QUEUE_CAPACITY {
            return Err(windows::Win32::Foundation::E_FAIL.into());
        }
        self.queue.borrow_mut().push_back((
            commit,
            preedit,
            study,
            memory,
            self.epoch.get(),
            account,
        ));
        if self.scheduled.replace(true) {
            return Ok(());
        }
        let session: ITfEditSession = Edit {
            _guard: Guard::new(),
            document: self.clone(),
        }
        .into();
        let result = unsafe {
            self.context
                .RequestEditSession(self.client, &session, TF_ES_ASYNC | TF_ES_READWRITE)
        }
        .and_then(|status| status.ok());
        if result.is_err() {
            self.scheduled.set(false);
            let cancelled: Vec<_> = self
                .queue
                .borrow_mut()
                .drain(..)
                .filter_map(|(_, _, _, memory, _, _)| memory)
                .collect();
            if let Some(state) = self.state.upgrade() {
                for receipt in cancelled {
                    state.confirm_memory(&receipt, false, None);
                }
            }
        }
        result
    }
    fn end(&self, composition: &ITfComposition, cookie: u32) -> Result<()> {
        self.ending.set(true);
        let result = unsafe { composition.EndComposition(cookie) };
        self.ending.set(false);
        result
    }
    fn valid(&self, epoch: u64) -> bool {
        !super::blocked(&self.context) && self.epoch.get() == epoch && !self.terminated.get()
    }
    fn check_epoch(&self, epoch: u64) -> Result<()> {
        if self.valid(epoch) {
            Ok(())
        } else {
            Err(windows::Win32::Foundation::E_ABORT.into())
        }
    }
    fn apply(
        self: &Rc<Self>,
        cookie: u32,
        commit: Option<&str>,
        preedit: &str,
        epoch: u64,
    ) -> Result<()> {
        self.check_epoch(epoch)?;
        if let Some(text) = commit {
            let text: Vec<_> = text.encode_utf16().collect();
            let composition = self.composition.borrow_mut().take();
            let range = if let Some(composition) = composition {
                let range = unsafe { composition.GetRange()? };
                self.check_epoch(epoch)?;
                unsafe {
                    range.SetText(cookie, 0, &text)?;
                    self.check_epoch(epoch)?;
                    self.end(&composition, cookie)?;
                }
                range
            } else {
                let insert: ITfInsertAtSelection = self.context.cast()?;
                self.check_epoch(epoch)?;
                unsafe {
                    insert.InsertTextAtSelection(
                        cookie,
                        INSERT_TEXT_AT_SELECTION_FLAGS(0),
                        &text,
                    )?
                }
            };
            self.check_epoch(epoch)?;
            move_caret(&self.context, cookie, &range)?;
        }
        self.check_epoch(epoch)?;
        if preedit.is_empty() {
            let composition = self.composition.borrow_mut().take();
            if let Some(composition) = composition {
                let range = unsafe { composition.GetRange()? };
                self.check_epoch(epoch)?;
                unsafe {
                    range.SetText(cookie, 0, &[])?;
                    self.check_epoch(epoch)?;
                    self.end(&composition, cookie)?;
                }
            }
            return Ok(());
        }
        if self.composition.borrow().is_none() {
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = unsafe { insert.InsertTextAtSelection(cookie, TF_IAS_QUERYONLY, &[])? };
            self.check_epoch(epoch)?;
            let manager: ITfContextComposition = self.context.cast()?;
            let sink: ITfCompositionSink = Sink {
                _guard: Guard::new(),
                document: Rc::downgrade(self),
            }
            .into();
            self.check_epoch(epoch)?;
            let composition = unsafe { manager.StartComposition(cookie, &range, &sink)? };
            self.check_epoch(epoch)?;
            *self.composition.borrow_mut() = Some(composition);
        }
        let composition = self.composition.borrow().clone().unwrap();
        let range = unsafe { composition.GetRange()? };
        self.check_epoch(epoch)?;
        unsafe {
            range.SetText(cookie, 0, &preedit.encode_utf16().collect::<Vec<_>>())?;
        }
        self.check_epoch(epoch)?;
        move_caret(&self.context, cookie, &range)?;
        if let Ok(view) = unsafe { self.context.GetActiveView() } {
            let mut rect = RECT::default();
            let mut clipped = false.into();
            if unsafe { view.GetTextExt(cookie, &range, &mut rect, &mut clipped) }.is_ok()
                && let Some(state) = self.state.upgrade()
            {
                let current = !state.retiring.get()
                    && state
                        .document
                        .borrow()
                        .as_ref()
                        .is_some_and(|doc| Rc::ptr_eq(doc, self));
                if current && let Some(panel) = state.panel.borrow().as_ref() {
                    panel.position(rect.left, rect.bottom);
                }
            }
        }
        Ok(())
    }
}
fn move_caret(context: &ITfContext, cookie: u32, range: &ITfRange) -> Result<()> {
    let end = unsafe { range.Clone()? };
    unsafe {
        end.Collapse(cookie, TF_ANCHOR_END)?;
    }
    let selection = TF_SELECTION {
        range: ManuallyDrop::new(Some(end)),
        style: TF_SELECTIONSTYLE {
            ase: TF_AE_END,
            fInterimChar: false.into(),
        },
    };
    let result = unsafe { context.SetSelection(cookie, std::slice::from_ref(&selection)) };
    drop(ManuallyDrop::into_inner(selection.range));
    result
}
#[implement(Agile = false, ITfEditSession)]
struct Edit {
    document: Rc<Document>,
    _guard: Guard,
}
struct Editing(Rc<Document>);
impl Drop for Editing {
    fn drop(&mut self) {
        self.0.applying.set(self.0.applying.get() - 1);
    }
}
impl ITfEditSession_Impl for Edit_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guarded(|| {
            self.document.applying.set(self.document.applying.get() + 1);
            let _editing = Editing(self.document.clone());
            let operations: Vec<_> = self.document.queue.borrow_mut().drain(..).collect();
            self.document.scheduled.set(false);
            let mut failure = None;
            for (commit, preedit, study, memory, epoch, account) in operations {
                if failure.is_some() || !self.document.valid(epoch) {
                    if let Some(state) = self.document.state.upgrade()
                        && let Some(receipt) = memory
                    {
                        state.confirm_memory(&receipt, false, None);
                    }
                    continue;
                }
                let result = self
                    .document
                    .apply(cookie, commit.as_deref(), &preedit, epoch);
                if result.is_err() {
                    self.document
                        .epoch
                        .set(self.document.epoch.get().wrapping_add(1));
                    self.document.terminated.set(true);
                }
                let eligible = self.document.valid(epoch);
                if let Some(state) = self.document.state.upgrade() {
                    if eligible && let Some(receipt) = &memory {
                        state.confirm_memory(receipt, result.is_ok(), commit.as_deref());
                    }
                    let current = state
                        .document
                        .borrow()
                        .as_ref()
                        .is_some_and(|doc| Rc::ptr_eq(doc, &self.document));
                    if result.is_err() && current {
                        state.cancel_composition();
                    }
                    if !eligible && let Some(receipt) = &memory {
                        state.confirm_memory(receipt, false, None);
                    }
                }
                if let Err(error) = result {
                    failure = Some(error);
                    continue;
                }
                if !eligible {
                    continue;
                }
                crate::learning::after_output(
                    Ok::<(), windows::core::Error>(()),
                    commit.as_deref(),
                    study,
                    |word| crate::learning::confirmed_scoped(word, account),
                )?;
            }
            failure.map_or(Ok(()), Err)
        })
    }
}
#[implement(Agile = false, ITfCompositionSink)]
struct Sink {
    document: Weak<Document>,
    _guard: Guard,
}
impl ITfCompositionSink_Impl for Sink_Impl {
    fn OnCompositionTerminated(
        &self,
        _cookie: u32,
        _composition: Ref<ITfComposition>,
    ) -> Result<()> {
        guarded(|| {
            if let Some(document) = self.document.upgrade() {
                if document.ending.get() {
                    return Ok(());
                }
                document.composition.borrow_mut().take();
                let cancelled: Vec<_> = document
                    .queue
                    .borrow_mut()
                    .drain(..)
                    .filter_map(|(_, _, _, memory, _, _)| memory)
                    .collect();
                document.terminated.set(true);
                document.epoch.set(document.epoch.get().wrapping_add(1));
                if let Some(state) = document.state.upgrade() {
                    for receipt in cancelled {
                        state.confirm_memory(&receipt, false, None);
                    }
                    let current = state
                        .document
                        .borrow()
                        .as_ref()
                        .is_some_and(|doc| Rc::ptr_eq(doc, &document));
                    if current {
                        state.cancel_composition();
                    }
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
#[test]
fn composition_sink_does_not_advertise_cross_apartment_access() {
    let sink: windows::core::IUnknown = Sink {
        document: Weak::new(),
        _guard: Guard::new(),
    }
    .into();
    super::assert_apartment_bound(&sink);
}
