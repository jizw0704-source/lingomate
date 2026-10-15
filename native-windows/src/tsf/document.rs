use super::{Guard, State, guarded};
use crate::input::Key;
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
    invalidated: Cell<bool>,
    preedit_len: Cell<i32>,
    pub epoch: Cell<u64>,
    state: Weak<State>,
    subscription: RefCell<Option<(ITfSource, u32)>>,
    raw: RefCell<String>,
    expanded: Cell<bool>,
    expandable: Cell<bool>,
    bilingual: Cell<bool>,
    keys: RefCell<VecDeque<(u64, Key)>>,
}
impl Document {
    pub fn new(context: ITfContext, client: u32, state: Weak<State>) -> Rc<Self> {
        let document = Rc::new(Self {
            context,
            client,
            composition: RefCell::new(None),
            queue: RefCell::new(VecDeque::new()),
            scheduled: Cell::new(false),
            applying: Cell::new(0),
            ending: Cell::new(false),
            terminated: Cell::new(false),
            invalidated: Cell::new(false),
            preedit_len: Cell::new(0),
            epoch: Cell::new(0),
            state,
            subscription: RefCell::new(None),
            raw: RefCell::new(String::new()),
            expanded: Cell::new(false),
            expandable: Cell::new(false),
            bilingual: Cell::new(true),
            keys: RefCell::new(VecDeque::new()),
        });
        if let Ok(source) = document.context.cast::<ITfSource>() {
            let sink: ITfTextEditSink = EditSink {
                document: Rc::downgrade(&document),
                _guard: Guard::new(),
            }
            .into();
            if let Ok(cookie) = unsafe { source.AdviseSink(&ITfTextEditSink::IID, &sink) } {
                *document.subscription.borrow_mut() = Some((source, cookie));
            }
        }
        document
    }
    pub fn accepted(&self, id: u64, key: Key) {
        self.keys.borrow_mut().push_back((id, key));
    }
    pub fn settled(&self, id: u64, input: &crate::input::Input) {
        *self.raw.borrow_mut() = input.raw.clone();
        self.expanded.set(input.expanded);
        self.bilingual.set(input.bilingual);
        self.expandable.set(
            input
                .frame
                .candidates
                .get(input.selected)
                .is_some_and(|candidate| !candidate.translations.is_empty()),
        );
        self.keys.borrow_mut().retain(|(pending, _)| *pending > id);
    }
    fn fallback(&self) -> String {
        // Preserve accepted letters if navigation arrives before an engine reply.
        // Pending candidate selections fall back to their original Pinyin; they
        // cannot be guessed or synchronously queried from a keyboard callback.
        let mut raw = self.raw.borrow().clone();
        let mut expanded = self.expanded.get();
        let mut expandable = Some(self.expandable.get());
        let mut bilingual = self.bilingual.get();
        let mut output = String::new();
        for (_, key) in self.keys.borrow().iter() {
            match key {
                Key::Letter(ch) if raw.len() < 240 => {
                    raw.push(*ch);
                    expanded = false;
                    expandable = None;
                }
                Key::Backspace => {
                    raw.pop();
                    expanded = false;
                    expandable = None;
                }
                Key::Escape if expanded => expanded = false,
                Key::Escape => raw.clear(),
                Key::Tab(_) if bilingual && !raw.is_empty() && expandable != Some(false) => {
                    // A pending query may not have supplied its translations yet.
                    // Preserve its accepted letters if Escape may only collapse it.
                    expanded = true;
                }
                Key::Up | Key::Down | Key::Previous | Key::Next => {
                    expanded = false;
                    expandable = None;
                }
                Key::ToggleBilingual => {
                    bilingual = !bilingual;
                    expanded &= bilingual;
                }
                Key::Return | Key::Space(_) | Key::Digit(_) if !raw.is_empty() => {
                    output.push_str(&raw);
                    raw.clear();
                    expanded = false;
                    expandable = Some(false);
                }
                Key::Space(_) => output.push(' '),
                Key::Digit(digit) => output.push(char::from(b'1' + *digit as u8)),
                Key::Literal(ch) | Key::Punctuation(ch) => {
                    output.push_str(&raw);
                    raw.clear();
                    output.push(*ch);
                    expanded = false;
                    expandable = Some(false);
                }
                _ => {}
            }
        }
        output.push_str(&raw);
        output
    }
    pub fn release_for_host(self: &Rc<Self>) {
        if self.terminated.get()
            || (self.composition.borrow().is_none()
                && self.queue.borrow().is_empty()
                && self.keys.borrow().is_empty()
                && self.raw.borrow().is_empty())
        {
            return;
        }
        let session: ITfEditSession = Release {
            document: self.clone(),
            fallback: self.fallback(),
            _guard: Guard::new(),
        }
        .into();
        // Keystroke handling permits a synchronous edit. Never wait for the
        // engine, pump messages, or replay the host's key after returning FALSE.
        let result = unsafe {
            self.context
                .RequestEditSession(self.client, &session, TF_ES_SYNC | TF_ES_READWRITE)
        }
        .and_then(|status| status.ok());
        if result.is_err() {
            // A host may refuse the write lock. Invalidate every late rewrite
            // and end only the owned composition, preserving existing host text.
            self.invalidate();
        }
    }
    pub fn observe_host(self: &Rc<Self>) {
        if self.terminated.get() || self.composition.borrow().is_none() {
            return;
        }
        let unchanged = Rc::new(Cell::new(false));
        let session: ITfEditSession = Observe {
            document: self.clone(),
            unchanged: unchanged.clone(),
            _guard: Guard::new(),
        }
        .into();
        let result = unsafe {
            self.context
                .RequestEditSession(self.client, &session, TF_ES_SYNC | TF_ES_READ)
        }
        .and_then(|status| status.ok());
        // Request cleanup only after leaving the read-only edit session.
        if result.is_err() || !unchanged.get() {
            self.invalidate();
        }
    }
    fn unchanged(&self, cookie: u32) -> Result<bool> {
        let Some(composition) = self.composition.borrow().clone() else {
            return Ok(true);
        };
        let range = unsafe { composition.GetRange()? };
        // Host-originated ITextStoreACP changes need not produce OnEndEdit.
        // Compare only range/selection metadata; never read host text.
        if let Ok(acp) = range.cast::<ITfRangeACP>() {
            let (mut start, mut length) = (0, 0);
            unsafe {
                acp.GetExtent(&mut start, &mut length)?;
            }
            if length != self.preedit_len.get() {
                return Ok(false);
            }
        }
        let mut selections = [TF_SELECTION::default()];
        let mut fetched = 0;
        unsafe {
            self.context.GetSelection(
                cookie,
                TF_DEFAULT_SELECTION,
                &mut selections,
                &mut fetched,
            )?;
        }
        let selected = unsafe { ManuallyDrop::take(&mut selections[0].range) };
        let Some(selected) = selected.filter(|_| fetched == 1) else {
            return Ok(false);
        };
        unsafe {
            Ok(selected.CompareStart(cookie, &range, TF_ANCHOR_END)? == 0
                && selected.CompareEnd(cookie, &range, TF_ANCHOR_END)? == 0)
        }
    }
    fn invalidate(self: &Rc<Self>) {
        self.terminated.set(true);
        // An edit failure may already mark the document terminated. Cleanup of
        // its owned composition must still run exactly once.
        if self.invalidated.replace(true) {
            return;
        }
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.raw.borrow_mut().clear();
        self.keys.borrow_mut().clear();
        let cancelled: Vec<_> = self
            .queue
            .borrow_mut()
            .drain(..)
            .filter_map(|(_, _, _, memory, _, _)| memory)
            .collect();
        let composition = self.composition.borrow_mut().take();
        if let Some(composition) = composition {
            let session: ITfEditSession = End {
                document: self.clone(),
                composition,
                _guard: Guard::new(),
            }
            .into();
            let _ = unsafe {
                self.context.RequestEditSession(
                    self.client,
                    &session,
                    TF_ES_ASYNC | TF_ES_READWRITE,
                )
            };
        }
        if let Some(state) = self.state.upgrade() {
            for receipt in cancelled {
                state.confirm_memory(&receipt, false, None);
            }
            if state
                .document
                .borrow()
                .as_ref()
                .is_some_and(|doc| Rc::ptr_eq(doc, self))
            {
                state.cancel_composition();
            }
        }
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
            let composition = self.composition.borrow().clone();
            let range = if let Some(composition) = composition {
                let range = unsafe { composition.GetRange()? };
                self.check_epoch(epoch)?;
                unsafe {
                    range.SetText(cookie, 0, &text)?;
                    self.check_epoch(epoch)?;
                    self.end(&composition, cookie)?;
                }
                self.composition.borrow_mut().take();
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
            let composition = self.composition.borrow().clone();
            if let Some(composition) = composition {
                let range = unsafe { composition.GetRange()? };
                self.check_epoch(epoch)?;
                unsafe {
                    range.SetText(cookie, 0, &[])?;
                    self.check_epoch(epoch)?;
                    self.end(&composition, cookie)?;
                }
                self.composition.borrow_mut().take();
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
        self.preedit_len.set(preedit.encode_utf16().count() as i32);
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
impl Drop for Document {
    fn drop(&mut self) {
        if let Some((source, cookie)) = self.subscription.get_mut().take() {
            let _ = unsafe { source.UnadviseSink(cookie) };
        }
    }
}
#[implement(Agile = false, ITfEditSession)]
struct Observe {
    document: Rc<Document>,
    unchanged: Rc<Cell<bool>>,
    _guard: Guard,
}
impl ITfEditSession_Impl for Observe_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guarded(|| {
            self.unchanged
                .set(matches!(self.document.unchanged(cookie), Ok(true)));
            Ok(())
        })
    }
}
#[implement(Agile = false, ITfEditSession)]
struct Release {
    document: Rc<Document>,
    fallback: String,
    _guard: Guard,
}
impl ITfEditSession_Impl for Release_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guarded(|| {
            // Completed engine outputs already in the document queue precede
            // the raw fallback for keys whose engine replies have not arrived.
            let edit = Edit {
                document: self.document.clone(),
                _guard: Guard::new(),
            };
            let edit: ITfEditSession = edit.into();
            unsafe {
                edit.DoEditSession(cookie)?;
            }
            let epoch = self.document.epoch.get();
            self.document.apply(
                cookie,
                (!self.fallback.is_empty()).then_some(self.fallback.as_str()),
                "",
                epoch,
            )?;
            self.document.invalidate();
            Ok(())
        })
    }
}
#[implement(Agile = false, ITfEditSession)]
struct End {
    document: Rc<Document>,
    composition: ITfComposition,
    _guard: Guard,
}
impl ITfEditSession_Impl for End_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guarded(|| self.document.end(&self.composition, cookie))
    }
}
#[implement(Agile = false, ITfTextEditSink)]
struct EditSink {
    document: Weak<Document>,
    _guard: Guard,
}
impl ITfTextEditSink_Impl for EditSink_Impl {
    fn OnEndEdit(
        &self,
        context: Ref<ITfContext>,
        _cookie: u32,
        record: Ref<ITfEditRecord>,
    ) -> Result<()> {
        guarded(|| {
            let Some(document) = self.document.upgrade() else {
                return Ok(());
            };
            let context = context.ok()?;
            if document.terminated.get()
                || document.applying.get() > 0
                || unsafe { context.InWriteSession(document.client) }.is_ok_and(|own| own.as_bool())
            {
                return Ok(());
            }
            let record = record.ok()?;
            let selection = unsafe { record.GetSelectionStatus()? }.as_bool();
            let ranges = unsafe { record.GetTextAndPropertyUpdates(TF_GTP_INCL_TEXT, &[])? };
            let mut changed = [None];
            let mut count = 0;
            unsafe { ranges.Next(&mut changed, &mut count) }?;
            // Only change metadata is observed. Never read host text or retain it.
            if selection || count > 0 {
                document.invalidate();
            }
            Ok(())
        })
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
            if !matches!(self.document.unchanged(cookie), Ok(true)) {
                self.document.invalidate();
                return Ok(());
            }
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
                    self.document.invalidate();
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
