use super::{Guard, State, guarded};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::mem::ManuallyDrop;
use std::rc::{Rc, Weak};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::TextServices::*;
use windows::core::{Interface, Ref, Result, implement};

pub(super) struct Document {
    pub context: ITfContext,
    client: u32,
    composition: RefCell<Option<ITfComposition>>,
    queue: RefCell<VecDeque<(Option<String>, String)>>,
    scheduled: Cell<bool>,
    ending: Cell<bool>,
    pub terminated: Cell<bool>,
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
            ending: Cell::new(false),
            terminated: Cell::new(false),
            state,
        })
    }
    pub fn enqueue(self: &Rc<Self>, commit: Option<String>, preedit: String) -> Result<()> {
        self.queue.borrow_mut().push_back((commit, preedit));
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
            self.queue.borrow_mut().clear();
        }
        result
    }
    fn end(&self, composition: &ITfComposition, cookie: u32) -> Result<()> {
        self.ending.set(true);
        let result = unsafe { composition.EndComposition(cookie) };
        self.ending.set(false);
        result
    }
    fn apply(self: &Rc<Self>, cookie: u32, commit: Option<&str>, preedit: &str) -> Result<()> {
        if let Some(text) = commit {
            let text: Vec<_> = text.encode_utf16().collect();
            let composition = self.composition.borrow_mut().take();
            let range = if let Some(composition) = composition {
                let range = unsafe { composition.GetRange()? };
                unsafe {
                    range.SetText(cookie, 0, &text)?;
                    self.end(&composition, cookie)?;
                }
                range
            } else {
                let insert: ITfInsertAtSelection = self.context.cast()?;
                unsafe {
                    insert.InsertTextAtSelection(
                        cookie,
                        INSERT_TEXT_AT_SELECTION_FLAGS(0),
                        &text,
                    )?
                }
            };
            move_caret(&self.context, cookie, &range)?;
        }
        if preedit.is_empty() {
            let composition = self.composition.borrow_mut().take();
            if let Some(composition) = composition {
                let range = unsafe { composition.GetRange()? };
                unsafe {
                    range.SetText(cookie, 0, &[])?;
                    self.end(&composition, cookie)?;
                }
            }
            return Ok(());
        }
        if self.composition.borrow().is_none() {
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = unsafe { insert.InsertTextAtSelection(cookie, TF_IAS_QUERYONLY, &[])? };
            let manager: ITfContextComposition = self.context.cast()?;
            let sink: ITfCompositionSink = Sink {
                _guard: Guard::new(),
                document: Rc::downgrade(self),
            }
            .into();
            let composition = unsafe { manager.StartComposition(cookie, &range, &sink)? };
            *self.composition.borrow_mut() = Some(composition);
        }
        let composition = self.composition.borrow().clone().unwrap();
        let range = unsafe { composition.GetRange()? };
        unsafe {
            range.SetText(cookie, 0, &preedit.encode_utf16().collect::<Vec<_>>())?;
        }
        move_caret(&self.context, cookie, &range)?;
        if let Ok(view) = unsafe { self.context.GetActiveView() } {
            let mut rect = RECT::default();
            let mut clipped = false.into();
            if unsafe { view.GetTextExt(cookie, &range, &mut rect, &mut clipped) }.is_ok()
                && let Some(state) = self.state.upgrade()
                && let Some(panel) = state.panel.borrow().as_ref()
            {
                panel.position(rect.left, rect.bottom);
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
#[implement(ITfEditSession)]
struct Edit {
    document: Rc<Document>,
    _guard: Guard,
}
impl ITfEditSession_Impl for Edit_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guarded(|| {
            let operations: Vec<_> = self.document.queue.borrow_mut().drain(..).collect();
            self.document.scheduled.set(false);
            for (commit, preedit) in operations {
                self.document.apply(cookie, commit.as_deref(), &preedit)?;
            }
            Ok(())
        })
    }
}
#[implement(ITfCompositionSink)]
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
                document.queue.borrow_mut().clear();
                document.terminated.set(true);
            }
            Ok(())
        })
    }
}
