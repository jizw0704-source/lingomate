//! Thread-local TSF mode and language-bar integration. No host text is inspected.
use super::{CLASS, Guard, MODULE, State, guarded};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::System::Ole::{
    CONNECT_E_ADVISELIMIT, CONNECT_E_CANNOTCONNECT, CONNECT_E_NOCONNECTION,
};
use windows::Win32::System::Variant::{VARIANT, VT_EMPTY, VT_I4};
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, BSTR, GUID, IUnknown, Interface, PCWSTR, Ref, Result, implement};

const ITEM_COOKIE: u32 = 1;
const MENU_CHINESE: u32 = 1;
const MENU_ENGLISH: u32 = 2;
const MENU_SETTINGS: u32 = 3;

fn i4(value: &VARIANT) -> Result<Option<i32>> {
    match value.vt() {
        VT_EMPTY => Ok(None),
        VT_I4 => i32::try_from(value).map(Some),
        _ => Err(E_INVALIDARG.into()),
    }
}

fn conversion_mode(previous: u32, english: bool) -> u32 {
    if english {
        previous & !TF_CONVERSIONMODE_NATIVE
    } else {
        previous | TF_CONVERSIONMODE_NATIVE
    }
}

fn is_english(open: Option<i32>, conversion: Option<i32>) -> bool {
    open.unwrap_or(1) == 0
        || conversion.unwrap_or(TF_CONVERSIONMODE_NATIVE as i32) as u32 & TF_CONVERSIONMODE_NATIVE
            == 0
}

struct Busy<'a>(&'a Cell<bool>);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

struct ItemData {
    english: Cell<bool>,
    hidden: Cell<bool>,
    stopped: Cell<bool>,
    notifying: Cell<bool>,
    sink: RefCell<Option<ITfLangBarItemSink>>,
}
impl ItemData {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            english: Cell::new(false),
            hidden: Cell::new(false),
            stopped: Cell::new(false),
            notifying: Cell::new(false),
            sink: RefCell::new(None),
        })
    }
    fn notify(&self, flags: u32) {
        if self.notifying.replace(true) {
            return;
        }
        let _busy = Busy(&self.notifying);
        // The system can synchronously query this item from OnUpdate.
        let sink = self.sink.borrow().clone();
        if let Some(sink) = sink {
            let _ = unsafe { sink.OnUpdate(flags) };
        }
    }
    fn update(&self, english: bool) {
        if self.english.replace(english) != english {
            self.notify(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP | TF_LBI_STATUS);
        }
    }
}

struct Subscription {
    source: ITfSource,
    cookie: u32,
}

pub(super) struct ModeController {
    state: Weak<State>,
    client: u32,
    open: ITfCompartment,
    conversion: ITfCompartment,
    langbar: ITfLangBarItemMgr,
    subscriptions: RefCell<Vec<Subscription>>,
    item: RefCell<Option<ITfLangBarItem>>,
    data: Rc<ItemData>,
    writing: Cell<bool>,
    reading: Cell<bool>,
    reread: Cell<bool>,
    stopped: Cell<bool>,
}
impl ModeController {
    pub(super) fn new(manager: &ITfThreadMgr, client: u32, state: Weak<State>) -> Result<Rc<Self>> {
        let compartments: ITfCompartmentMgr = manager.cast()?;
        let controller = Rc::new(Self {
            state,
            client,
            open: unsafe { compartments.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)? },
            conversion: unsafe {
                compartments.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)?
            },
            langbar: manager.cast()?,
            subscriptions: RefCell::new(Vec::new()),
            item: RefCell::new(None),
            data: ItemData::new(),
            writing: Cell::new(false),
            reading: Cell::new(false),
            reread: Cell::new(false),
            stopped: Cell::new(false),
        });
        if let Err(error) = controller.initialize() {
            let _ = controller.shutdown();
            return Err(error);
        }
        Ok(controller)
    }

    fn initialize(self: &Rc<Self>) -> Result<()> {
        let sink: ITfCompartmentEventSink = ModeSink {
            controller: Rc::downgrade(self),
            _guard: Guard::new(),
        }
        .into();
        for compartment in [&self.open, &self.conversion] {
            let source: ITfSource = compartment.cast()?;
            let cookie = unsafe { source.AdviseSink(&ITfCompartmentEventSink::IID, &sink)? };
            self.subscriptions
                .borrow_mut()
                .push(Subscription { source, cookie });
        }

        // Empty compartments get a native-mode default. Existing mode and all
        // unrelated conversion flags belong to the thread and are preserved.
        self.writing.set(true);
        {
            let _busy = Busy(&self.writing);
            if i4(&unsafe { self.open.GetValue()? })?.is_none() {
                unsafe { self.open.SetValue(self.client, &VARIANT::from(1i32))? };
            }
            if i4(&unsafe { self.conversion.GetValue()? })?.is_none() {
                unsafe {
                    self.conversion
                        .SetValue(self.client, &VARIANT::from(TF_CONVERSIONMODE_NATIVE as i32))?
                };
            }
        }

        let button: ITfLangBarItemButton = ModeButton {
            controller: Rc::downgrade(self),
            data: self.data.clone(),
            _guard: Guard::new(),
        }
        .into();
        let item: ITfLangBarItem = button.cast()?;
        // Retain before AddItem: shell callbacks are allowed during that call.
        *self.item.borrow_mut() = Some(item.clone());
        unsafe { self.langbar.AddItem(&item)? };
        self.synchronize()
    }

    pub(super) fn set_english(&self, english: bool) -> Result<()> {
        if self.stopped.get() {
            return Err(E_ABORT.into());
        }
        if self.writing.replace(true) {
            return Err(E_UNEXPECTED.into());
        }
        let result = {
            let _busy = Busy(&self.writing);
            (|| {
                let previous_open = i4(&unsafe { self.open.GetValue()? })?;
                let previous_conversion = i4(&unsafe { self.conversion.GetValue()? })?;
                let conversion = conversion_mode(
                    previous_conversion.unwrap_or(TF_CONVERSIONMODE_NATIVE as i32) as u32,
                    english,
                ) as i32;
                let open = i32::from(!english);
                if previous_conversion != Some(conversion) {
                    unsafe {
                        self.conversion
                            .SetValue(self.client, &VARIANT::from(conversion))?
                    };
                }
                if previous_open != Some(open) {
                    unsafe { self.open.SetValue(self.client, &VARIANT::from(open))? };
                }
                Ok(())
            })()
        };
        // Read the actual state even after a partial write; never roll back
        // unrelated thread mode changes or report an unobserved desired mode.
        let synchronized = self.synchronize();
        result.and(synchronized)
    }

    fn synchronize(&self) -> Result<()> {
        if self.stopped.get() || self.writing.get() {
            return Ok(());
        }
        if self.reading.replace(true) {
            self.reread.set(true);
            return Ok(());
        }
        let _busy = Busy(&self.reading);
        // A shell notification can synchronously change mode again. Observe
        // that new value after returning; never spin indefinitely in the host.
        for _ in 0..4 {
            self.reread.set(false);
            let open = i4(&unsafe { self.open.GetValue()? })?;
            let conversion = i4(&unsafe { self.conversion.GetValue()? })?;
            let english = is_english(open, conversion);
            if let Some(state) = self.state.upgrade() {
                state.english_from_system(english);
            }
            self.data.update(english);
            if !self.reread.get() {
                return Ok(());
            }
        }
        Err(E_UNEXPECTED.into())
    }

    fn user_english(&self, english: bool) {
        if !self.stopped.get()
            && let Some(state) = self.state.upgrade()
        {
            state.english(english);
        }
    }

    pub(super) fn shutdown(&self) -> Result<()> {
        if self.stopped.replace(true) {
            return Ok(());
        }
        self.data.stopped.set(true);
        self.data.notify(TF_LBI_STATUS);
        let item = self.item.borrow_mut().take();
        let subscriptions = self
            .subscriptions
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        let mut result = Ok(());
        if let Some(item) = item {
            result = unsafe { self.langbar.RemoveItem(&item) };
        }
        for subscription in subscriptions {
            let next = unsafe { subscription.source.UnadviseSink(subscription.cookie) };
            if result.is_ok() {
                result = next;
            }
        }
        let sink = self.data.sink.borrow_mut().take();
        drop(sink);
        result
    }
}
impl Drop for ModeController {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[implement(Agile = false, ITfCompartmentEventSink)]
struct ModeSink {
    controller: Weak<ModeController>,
    _guard: Guard,
}
impl ITfCompartmentEventSink_Impl for ModeSink_Impl {
    fn OnChange(&self, guid: *const GUID) -> Result<()> {
        guarded(|| {
            if guid.is_null() {
                return Err(E_POINTER.into());
            }
            let guid = unsafe { *guid };
            if (guid == GUID_COMPARTMENT_KEYBOARD_OPENCLOSE
                || guid == GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)
                && let Some(controller) = self.controller.upgrade()
            {
                controller.synchronize()?;
            }
            Ok(())
        })
    }
}

#[implement(Agile = false, ITfLangBarItemButton, ITfSource)]
struct ModeButton {
    controller: Weak<ModeController>,
    data: Rc<ItemData>,
    _guard: Guard,
}
impl ITfLangBarItem_Impl for ModeButton_Impl {
    fn GetInfo(&self, info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        guarded(|| {
            if info.is_null() {
                return Err(E_POINTER.into());
            }
            let mut value = TF_LANGBARITEMINFO {
                clsidService: CLASS,
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_BTN_MENU | TF_LBI_STYLE_SHOWNINTRAY,
                ..Default::default()
            };
            for (slot, ch) in value
                .szDescription
                .iter_mut()
                .zip("灵果 · LingoMate".encode_utf16())
            {
                *slot = ch;
            }
            unsafe { *info = value };
            Ok(())
        })
    }
    fn GetStatus(&self) -> Result<u32> {
        guarded(|| {
            Ok(if self.data.stopped.get() {
                TF_LBI_STATUS_HIDDEN | TF_LBI_STATUS_DISABLED
            } else if self.data.hidden.get() {
                TF_LBI_STATUS_HIDDEN
            } else {
                0
            })
        })
    }
    fn Show(&self, show: BOOL) -> Result<()> {
        guarded(|| {
            let hidden = !show.as_bool();
            if self.data.hidden.replace(hidden) != hidden {
                self.data.notify(TF_LBI_STATUS);
            }
            Ok(())
        })
    }
    fn GetTooltipString(&self) -> Result<BSTR> {
        guarded(|| {
            Ok(BSTR::from(if self.data.english.get() {
                "灵果 · LingoMate：英文"
            } else {
                "灵果 · LingoMate：中文"
            }))
        })
    }
}
impl ITfLangBarItemButton_Impl for ModeButton_Impl {
    fn OnClick(&self, click: TfLBIClick, _point: &POINT, _area: *const RECT) -> Result<()> {
        guarded(|| {
            if click == TF_LBI_CLK_LEFT
                && let Some(controller) = self.controller.upgrade()
            {
                controller.user_english(!self.data.english.get());
            }
            Ok(())
        })
    }
    fn InitMenu(&self, menu: Ref<ITfMenu>) -> Result<()> {
        guarded(|| {
            let menu = menu.ok()?;
            for (id, title, checked) in [
                (MENU_CHINESE, "中文", !self.data.english.get()),
                (MENU_ENGLISH, "英文", self.data.english.get()),
                (MENU_SETTINGS, "灵果设置", false),
            ] {
                unsafe {
                    menu.AddMenuItem(
                        id,
                        if checked { TF_LBMENUF_RADIOCHECKED } else { 0 },
                        HBITMAP::default(),
                        HBITMAP::default(),
                        &title.encode_utf16().collect::<Vec<_>>(),
                        std::ptr::null_mut(),
                    )?
                };
            }
            if let Some(controller) = self.controller.upgrade()
                && let Some(state) = controller.state.upgrade()
            {
                let input = state.input.borrow();
                let entries = [
                    (10, "同时显示英文译词".to_string(), input.bilingual),
                    (11, "中文选词记忆".to_string(), input.remember),
                    (
                        12,
                        format!("标点：{}", input.punctuation_mode.title()),
                        false,
                    ),
                    (20, "翻译设置与 API 密钥…".into(), false),
                    (21, "准备本地整句翻译…".into(), false),
                    (22, "账号与我的学习…".into(), false),
                    (23, "本地学习记录…".into(), false),
                    (24, "外观与字号…".into(), false),
                    (25, "检查版本更新…".into(), false),
                ];
                drop(input);
                for (id, title, checked) in entries {
                    unsafe {
                        menu.AddMenuItem(
                            id,
                            if checked { TF_LBMENUF_CHECKED } else { 0 },
                            HBITMAP::default(),
                            HBITMAP::default(),
                            &title.encode_utf16().collect::<Vec<_>>(),
                            std::ptr::null_mut(),
                        )?;
                    }
                }
            }
            Ok(())
        })
    }
    fn OnMenuSelect(&self, id: u32) -> Result<()> {
        guarded(|| {
            if let Some(controller) = self.controller.upgrade() {
                match id {
                    MENU_CHINESE => controller.user_english(false),
                    MENU_ENGLISH => controller.user_english(true),
                    MENU_SETTINGS if !controller.stopped.get() => {
                        if let Some(state) = controller.state.upgrade() {
                            state.settings();
                        }
                    }
                    10..=25 if !controller.stopped.get() => {
                        if let Some(state) = controller.state.upgrade() {
                            match id {
                                10 => {
                                    let value = !state.input.borrow().bilingual;
                                    state.apply_setting(
                                        crate::settings::Setting::Bilingual(value),
                                        true,
                                    );
                                }
                                11 => {
                                    let value = !state.input.borrow().remember;
                                    state.apply_setting(
                                        crate::settings::Setting::Remember(value),
                                        true,
                                    );
                                }
                                12 => {
                                    let mode = state.input.borrow().punctuation_mode;
                                    let next = (crate::punctuation::Mode::ALL
                                        .iter()
                                        .position(|m| *m == mode)
                                        .unwrap_or(0)
                                        + 1)
                                        % 3;
                                    state.apply_setting(
                                        crate::settings::Setting::Punctuation(next as u32),
                                        true,
                                    );
                                }
                                20 => state.open_settings("translation"),
                                21 => state.open_settings("local"),
                                22 => state.open_settings("account"),
                                23 => state.open_settings("learning"),
                                24 => state.open_settings("appearance"),
                                25 => {
                                    let _ = super::updates::launch(false);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        })
    }
    fn GetIcon(&self) -> Result<HICON> {
        guarded(|| {
            // Branding stays at resource 101; the mode icon itself reflects
            // the current state. The caller owns and destroys each fresh HICON.
            let instance = HINSTANCE(MODULE.load(Ordering::SeqCst));
            if instance.0.is_null() {
                return Err(E_FAIL.into());
            }
            let icon = unsafe {
                LoadImageW(
                    Some(instance),
                    PCWSTR(if self.data.english.get() {
                        103usize
                    } else {
                        102usize
                    } as *const u16),
                    IMAGE_ICON,
                    GetSystemMetrics(SM_CXSMICON),
                    GetSystemMetrics(SM_CYSMICON),
                    IMAGE_FLAGS(0),
                )?
            };
            Ok(HICON(icon.0))
        })
    }
    fn GetText(&self) -> Result<BSTR> {
        guarded(|| {
            Ok(BSTR::from(if self.data.english.get() {
                "英"
            } else {
                "中"
            }))
        })
    }
}
impl ITfSource_Impl for ModeButton_Impl {
    fn AdviseSink(&self, iid: *const GUID, unknown: Ref<IUnknown>) -> Result<u32> {
        guarded(|| {
            if iid.is_null() {
                return Err(E_POINTER.into());
            }
            if unsafe { *iid } != ITfLangBarItemSink::IID {
                return Err(CONNECT_E_CANNOTCONNECT.into());
            }
            if self.data.stopped.get() {
                return Err(E_ABORT.into());
            }
            if self.data.sink.borrow().is_some() {
                return Err(CONNECT_E_ADVISELIMIT.into());
            }
            let sink = unknown.ok()?.cast::<ITfLangBarItemSink>()?;
            let mut current = self.data.sink.borrow_mut();
            if current.is_some() {
                return Err(CONNECT_E_ADVISELIMIT.into());
            }
            *current = Some(sink);
            Ok(ITEM_COOKIE)
        })
    }
    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        guarded(|| {
            if cookie != ITEM_COOKIE || self.data.sink.borrow().is_none() {
                return Err(CONNECT_E_NOCONNECTION.into());
            }
            let sink = self.data.sink.borrow_mut().take();
            drop(sink);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_objects_do_not_advertise_cross_apartment_access() {
        let sink: IUnknown = ModeSink {
            controller: Weak::new(),
            _guard: Guard::new(),
        }
        .into();
        let button: IUnknown = ModeButton {
            controller: Weak::new(),
            data: ItemData::new(),
            _guard: Guard::new(),
        }
        .into();
        super::super::assert_apartment_bound(&sink);
        super::super::assert_apartment_bound(&button);
    }

    #[test]
    fn conversion_only_changes_native_bit_and_keeps_every_other_flag() {
        let flags = TF_CONVERSIONMODE_FULLSHAPE | TF_CONVERSIONMODE_SYMBOL | 0x8000_0000;
        assert_eq!(
            conversion_mode(flags, false),
            flags | TF_CONVERSIONMODE_NATIVE
        );
        assert_eq!(
            conversion_mode(flags | TF_CONVERSIONMODE_NATIVE, true),
            flags
        );
    }

    #[test]
    fn system_modes_distinguish_closed_native_and_alphanumeric() {
        assert!(!is_english(None, None));
        assert!(is_english(Some(0), Some(TF_CONVERSIONMODE_NATIVE as i32)));
        assert!(is_english(
            Some(1),
            Some(TF_CONVERSIONMODE_FULLSHAPE as i32)
        ));
        assert!(!is_english(
            Some(1),
            Some((TF_CONVERSIONMODE_NATIVE | TF_CONVERSIONMODE_FULLSHAPE) as i32),
        ));
        assert_eq!(i4(&VARIANT::default()).unwrap(), None);
        assert_eq!(i4(&VARIANT::from(1i32)).unwrap(), Some(1));
        assert!(i4(&VARIANT::from(true)).is_err());
    }

    #[test]
    fn language_bar_exposes_owned_name_mode_and_hidden_status() {
        let data = ItemData::new();
        let button: ITfLangBarItemButton = ModeButton {
            controller: Weak::new(),
            data: data.clone(),
            _guard: Guard::new(),
        }
        .into();
        let mut info = TF_LANGBARITEMINFO::default();
        unsafe { button.GetInfo(&mut info) }.unwrap();
        assert_eq!(info.clsidService, CLASS);
        assert_eq!(info.guidItem, GUID_LBI_INPUTMODE);
        assert_ne!(info.dwStyle & TF_LBI_STYLE_SHOWNINTRAY, 0);
        assert_eq!(unsafe { button.GetText() }.unwrap().to_string(), "中");
        data.update(true);
        assert_eq!(unsafe { button.GetText() }.unwrap().to_string(), "英");
        assert_eq!(
            unsafe { button.GetTooltipString() }.unwrap().to_string(),
            "灵果 · LingoMate：英文"
        );
        unsafe { button.Show(false) }.unwrap();
        assert_eq!(unsafe { button.GetStatus() }.unwrap(), TF_LBI_STATUS_HIDDEN);
        unsafe { button.Show(true) }.unwrap();
        data.stopped.set(true);
        assert_eq!(
            unsafe { button.GetStatus() }.unwrap(),
            TF_LBI_STATUS_HIDDEN | TF_LBI_STATUS_DISABLED
        );
    }

    #[implement(ITfLangBarItemSink)]
    struct ReentrantItemSink {
        calls: Rc<Cell<u32>>,
        item: ITfLangBarItemButton,
    }
    impl ITfLangBarItemSink_Impl for ReentrantItemSink_Impl {
        fn OnUpdate(&self, _flags: u32) -> Result<()> {
            self.calls.set(self.calls.get() + 1);
            // A normal shell notification can immediately query the same item.
            let _ = unsafe { self.item.GetStatus()? };
            let _ = unsafe { self.item.GetText()? };
            Ok(())
        }
    }

    #[test]
    fn language_bar_subscription_queries_are_reentrant_and_unadvise_stops_updates() {
        let data = ItemData::new();
        let button: ITfLangBarItemButton = ModeButton {
            controller: Weak::new(),
            data: data.clone(),
            _guard: Guard::new(),
        }
        .into();
        let source: ITfSource = button.cast().unwrap();
        let calls = Rc::new(Cell::new(0));
        let sink: ITfLangBarItemSink = ReentrantItemSink {
            calls: calls.clone(),
            item: button,
        }
        .into();
        let cookie = unsafe { source.AdviseSink(&ITfLangBarItemSink::IID, &sink) }.unwrap();
        assert_eq!(cookie, ITEM_COOKIE);
        assert_eq!(
            unsafe { source.AdviseSink(&ITfLangBarItemSink::IID, &sink) }
                .unwrap_err()
                .code(),
            CONNECT_E_ADVISELIMIT
        );
        assert_eq!(
            unsafe { source.AdviseSink(&IUnknown::IID, &sink) }
                .unwrap_err()
                .code(),
            CONNECT_E_CANNOTCONNECT
        );
        data.update(true);
        assert_eq!(calls.get(), 1);
        data.update(true);
        assert_eq!(calls.get(), 1);
        assert_eq!(
            unsafe { source.UnadviseSink(cookie + 1) }
                .unwrap_err()
                .code(),
            CONNECT_E_NOCONNECTION
        );
        unsafe { source.UnadviseSink(cookie) }.unwrap();
        data.update(false);
        assert_eq!(calls.get(), 1);
        assert_eq!(
            unsafe { source.UnadviseSink(cookie) }.unwrap_err().code(),
            CONNECT_E_NOCONNECTION
        );
    }
}
