//! UI Automation exposes only this panel's candidate labels and enum-only actions.
#![allow(non_upper_case_globals)]
use super::Guard;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::*;
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BSTR, GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface, Result};
pub const INVOKE: u32 = WM_APP + 51;
#[derive(Clone)]
pub struct Item {
    pub name: String,
    pub bounds: UiaRect,
    pub action: u32,
    pub enabled: bool,
    pub selected: bool,
}
#[derive(Default)]
pub struct Model {
    pub revision: u64,
    pub name: String,
    pub bounds: UiaRect,
    pub items: Vec<Item>,
    pub visible: bool,
}
pub type Channel = Arc<RwLock<Model>>;
pub fn provider(window: HWND, channel: Channel) -> IRawElementProviderSimple {
    // This is an owned, non-null COM pointer with its initial reference count.
    unsafe {
        IRawElementProviderSimple::from_raw(Node::allocate(
            window.0 as usize,
            channel,
            None,
            0,
            SIMPLE,
        ))
    }
}

const SIMPLE: usize = 0;
const FRAGMENT: usize = 1;
const ROOT: usize = 2;
const INVOKER: usize = 3;

// Generated Windows Rust _Impl methods cannot represent S_OK plus a null
// interface. These ABI slots explicitly initialize every out pointer, including
// the normal "no neighbour / no pattern / no input focus" results required by UIA.
// No null pointer is ever constructed as a Rust Interface value.
#[repr(C)]
struct Slot {
    vtable: *const c_void,
    owner: *mut Node,
}
struct Node {
    slots: [Slot; 4],
    references: AtomicU32,
    window: usize,
    channel: Channel,
    index: Option<usize>,
    revision: u64,
    _guard: Guard,
}
impl Node {
    fn allocate(
        window: usize,
        channel: Channel,
        index: Option<usize>,
        revision: u64,
        slot: usize,
    ) -> *mut c_void {
        let node = Box::into_raw(Box::new(Self {
            slots: [
                Slot {
                    vtable: (&SIMPLE_VTABLE as *const IRawElementProviderSimple_Vtbl).cast(),
                    owner: std::ptr::null_mut(),
                },
                Slot {
                    vtable: (&FRAGMENT_VTABLE as *const IRawElementProviderFragment_Vtbl).cast(),
                    owner: std::ptr::null_mut(),
                },
                Slot {
                    vtable: (&ROOT_VTABLE as *const IRawElementProviderFragmentRoot_Vtbl).cast(),
                    owner: std::ptr::null_mut(),
                },
                Slot {
                    vtable: (&INVOKE_VTABLE as *const IInvokeProvider_Vtbl).cast(),
                    owner: std::ptr::null_mut(),
                },
            ],
            references: AtomicU32::new(1),
            window,
            channel,
            index,
            revision,
            _guard: Guard::new(),
        }));
        // The box is stable before any interface pointer escapes. Owners and
        // vtables remain immutable while atomic COM references keep it alive.
        unsafe {
            for interface in &mut (*node).slots {
                interface.owner = node;
            }
            (&*node).pointer(slot)
        }
    }
    fn pointer(&self, slot: usize) -> *mut c_void {
        std::ptr::from_ref(&self.slots[slot]).cast_mut().cast()
    }
    fn retain(&self, slot: usize) -> *mut c_void {
        self.references.fetch_add(1, Ordering::Relaxed);
        self.pointer(slot)
    }
    fn model(&self) -> Result<std::sync::RwLockReadGuard<'_, Model>> {
        let data = self
            .channel
            .read()
            .map_err(|_| windows::core::Error::from(E_FAIL))?;
        if !data.visible || self.index.is_some() && self.revision != data.revision {
            return Err(HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32).into());
        }
        Ok(data)
    }
    fn node(&self, index: Option<usize>, revision: u64, slot: usize) -> *mut c_void {
        Self::allocate(self.window, self.channel.clone(), index, revision, slot)
    }
    fn property(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        let data = self.model()?;
        let item = self.index.and_then(|i| data.items.get(i));
        Ok(match propertyid {
            UIA_NamePropertyId => VARIANT::from(BSTR::from(
                item.map_or(data.name.as_str(), |i| i.name.as_str()),
            )),
            UIA_ControlTypePropertyId => VARIANT::from(if item.is_some() {
                UIA_ButtonControlTypeId.0
            } else {
                UIA_PaneControlTypeId.0
            }),
            UIA_IsControlElementPropertyId | UIA_IsContentElementPropertyId => VARIANT::from(true),
            UIA_IsKeyboardFocusablePropertyId | UIA_HasKeyboardFocusPropertyId => {
                VARIANT::from(false)
            }
            UIA_IsEnabledPropertyId => VARIANT::from(item.is_none_or(|i| i.enabled)),
            UIA_ItemStatusPropertyId => {
                VARIANT::from(BSTR::from(if item.is_some_and(|i| i.selected) {
                    "已选中"
                } else {
                    ""
                }))
            }
            UIA_HelpTextPropertyId => VARIANT::from(BSTR::from(
                "在输入应用中使用方向键选择；空格确认中文，Shift＋空格确认就绪的英文。",
            )),
            UIA_NativeWindowHandlePropertyId => VARIANT::from(self.window as i32),
            UIA_IsOffscreenPropertyId => VARIANT::from(false),
            _ => VARIANT::default(),
        })
    }
    fn invoke(&self) -> Result<()> {
        let data = self.model()?;
        let item = self
            .index
            .and_then(|i| data.items.get(i))
            .ok_or(E_NOTIMPL)?;
        if !item.enabled {
            return Err(HRESULT(UIA_E_ELEMENTNOTENABLED as i32).into());
        }
        let (action, revision) = (item.action, data.revision);
        drop(data);
        let mut result = 0usize;
        if unsafe {
            SendMessageTimeoutW(
                HWND(self.window as *mut _),
                INVOKE,
                WPARAM(action as usize),
                LPARAM(revision as isize),
                SMTO_ABORTIFHUNG,
                100,
                Some(&mut result),
            )
        }
        .0 == 0
            || result != 1
        {
            return Err(HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32).into());
        }
        Ok(())
    }
}

unsafe fn owner(this: *mut c_void) -> Result<*mut Node> {
    if this.is_null() {
        return Err(E_POINTER.into());
    }
    // Every exported interface starts with its vtable and stable owner pointer.
    Ok(unsafe { (*this.cast::<Slot>()).owner })
}
unsafe fn output<T: Default>(out: *mut T, body: impl FnOnce() -> Result<T>) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe { out.write(T::default()) };
    match super::guarded(body) {
        Ok(value) => {
            unsafe { out.write(value) };
            S_OK
        }
        Err(error) => error.code(),
    }
}
unsafe extern "system" fn query_interface(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        output(out, || {
            if iid.is_null() {
                return Err(E_POINTER.into());
            }
            let node = &*owner(this)?;
            let iid = &*iid;
            let slot = if *iid == IUnknown::IID || *iid == IRawElementProviderSimple::IID {
                SIMPLE
            } else if *iid == IRawElementProviderFragment::IID {
                FRAGMENT
            } else if *iid == IRawElementProviderFragmentRoot::IID && node.index.is_none() {
                ROOT
            } else if *iid == IInvokeProvider::IID && node.index.is_some() {
                INVOKER
            } else {
                return Err(E_NOINTERFACE.into());
            };
            Ok(node.retain(slot))
        })
    }
}
unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    let Ok(node) = (unsafe { owner(this) }) else {
        return 0;
    };
    unsafe { (*node).references.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let Ok(node) = (unsafe { owner(this) }) else {
        return 0;
    };
    let remaining = unsafe { (*node).references.fetch_sub(1, Ordering::Release) - 1 };
    if remaining == 0 {
        std::sync::atomic::fence(Ordering::Acquire);
        unsafe { drop(Box::from_raw(node)) };
    }
    remaining
}
const fn unknown_vtable() -> IUnknown_Vtbl {
    IUnknown_Vtbl {
        QueryInterface: query_interface,
        AddRef: add_ref,
        Release: release,
    }
}
unsafe extern "system" fn options(this: *mut c_void, out: *mut ProviderOptions) -> HRESULT {
    unsafe {
        output(out, || {
            owner(this)?;
            Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
        })
    }
}
unsafe extern "system" fn pattern(
    this: *mut c_void,
    id: UIA_PATTERN_ID,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        output(out, || {
            let node = &*owner(this)?;
            let _data = node.model()?;
            Ok(if id == UIA_InvokePatternId && node.index.is_some() {
                node.retain(INVOKER)
            } else {
                std::ptr::null_mut()
            })
        })
    }
}
unsafe extern "system" fn property(
    this: *mut c_void,
    id: UIA_PROPERTY_ID,
    out: *mut VARIANT,
) -> HRESULT {
    unsafe { output(out, || (&*owner(this)?).property(id)) }
}
unsafe extern "system" fn host(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    unsafe {
        output(out, || {
            let node = &*owner(this)?;
            // UiaHostProviderFromHwnd may enter system provider code. Never hold
            // the model lock across an external COM/UIA call.
            drop(node.model()?);
            if node.index.is_none() {
                Ok(UiaHostProviderFromHwnd(HWND(node.window as *mut _))?.into_raw())
            } else {
                Ok(std::ptr::null_mut())
            }
        })
    }
}
unsafe extern "system" fn navigate(
    this: *mut c_void,
    direction: NavigateDirection,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        output(out, || {
            let node = &*owner(this)?;
            let data = node.model()?;
            let index = match (node.index, direction) {
                (None, NavigateDirection_FirstChild) if !data.items.is_empty() => Some(0),
                (None, NavigateDirection_LastChild) if !data.items.is_empty() => {
                    Some(data.items.len() - 1)
                }
                (Some(_), NavigateDirection_Parent) => return Ok(node.node(None, 0, FRAGMENT)),
                (Some(index), NavigateDirection_NextSibling) if index + 1 < data.items.len() => {
                    Some(index + 1)
                }
                (Some(index), NavigateDirection_PreviousSibling) if index > 0 => Some(index - 1),
                _ => return Ok(std::ptr::null_mut()),
            };
            Ok(node.node(index, data.revision, FRAGMENT))
        })
    }
}
unsafe extern "system" fn runtime_id(this: *mut c_void, out: *mut *mut SAFEARRAY) -> HRESULT {
    unsafe {
        output(out, || {
            let node = &*owner(this)?;
            let data = node.model()?;
            let values = [
                UiaAppendRuntimeId as i32,
                node.window as i32,
                (node.window >> 32) as i32,
                node.index.map_or(0, |i| i as i32 + 1),
                if node.index.is_none() {
                    0
                } else {
                    data.revision as i32
                },
            ];
            let array = SafeArrayCreateVector(VT_I4, 0, values.len() as u32);
            if array.is_null() {
                return Err(E_OUTOFMEMORY.into());
            }
            for (index, value) in values.iter().enumerate() {
                if let Err(error) =
                    SafeArrayPutElement(array, &(index as i32), (value as *const i32).cast())
                {
                    let _ = SafeArrayDestroy(array);
                    return Err(error);
                }
            }
            Ok(array)
        })
    }
}
unsafe extern "system" fn rectangle(this: *mut c_void, out: *mut UiaRect) -> HRESULT {
    unsafe {
        output(out, || {
            let node = &*owner(this)?;
            let data = node.model()?;
            Ok(node
                .index
                .and_then(|i| data.items.get(i))
                .map_or(data.bounds, |i| i.bounds))
        })
    }
}
unsafe extern "system" fn embedded(this: *mut c_void, out: *mut *mut SAFEARRAY) -> HRESULT {
    unsafe {
        output(out, || {
            let _data = (&*owner(this)?).model()?;
            Ok(std::ptr::null_mut())
        })
    }
}
unsafe extern "system" fn set_focus(this: *mut c_void) -> HRESULT {
    match super::guarded(|| {
        let _data = unsafe { (&*owner(this)?).model()? };
        Err::<(), _>(E_NOTIMPL.into())
    }) {
        Ok(()) => S_OK,
        Err(error) => error.code(),
    }
}
unsafe extern "system" fn fragment_root(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    unsafe {
        output(out, || {
            let node = &*owner(this)?;
            let _data = node.model()?;
            Ok(if node.index.is_none() {
                node.retain(ROOT)
            } else {
                node.node(None, 0, ROOT)
            })
        })
    }
}
fn contains(bounds: UiaRect, x: f64, y: f64) -> bool {
    x >= bounds.left
        && x < bounds.left + bounds.width
        && y >= bounds.top
        && y < bounds.top + bounds.height
}
unsafe extern "system" fn from_point(
    this: *mut c_void,
    x: f64,
    y: f64,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        output(out, || {
            let node = &*owner(this)?;
            let data = node.model()?;
            if !contains(data.bounds, x, y) {
                return Ok(std::ptr::null_mut());
            }
            let index = data.items.iter().position(|i| contains(i.bounds, x, y));
            Ok(if let Some(index) = index {
                node.node(Some(index), data.revision, FRAGMENT)
            } else {
                node.retain(FRAGMENT)
            })
        })
    }
}
unsafe extern "system" fn focus(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    unsafe {
        output(out, || {
            let _data = (&*owner(this)?).model()?;
            // Candidate selection is not input focus; the non-activating panel leaves
            // keyboard focus in the host document.
            Ok(std::ptr::null_mut())
        })
    }
}
unsafe extern "system" fn invoke(this: *mut c_void) -> HRESULT {
    match super::guarded(|| unsafe { (&*owner(this)?).invoke() }) {
        Ok(()) => S_OK,
        Err(error) => error.code(),
    }
}
static SIMPLE_VTABLE: IRawElementProviderSimple_Vtbl = IRawElementProviderSimple_Vtbl {
    base__: unknown_vtable(),
    ProviderOptions: options,
    GetPatternProvider: pattern,
    GetPropertyValue: property,
    HostRawElementProvider: host,
};
static FRAGMENT_VTABLE: IRawElementProviderFragment_Vtbl = IRawElementProviderFragment_Vtbl {
    base__: unknown_vtable(),
    Navigate: navigate,
    GetRuntimeId: runtime_id,
    BoundingRectangle: rectangle,
    GetEmbeddedFragmentRoots: embedded,
    SetFocus: set_focus,
    FragmentRoot: fragment_root,
};
static ROOT_VTABLE: IRawElementProviderFragmentRoot_Vtbl = IRawElementProviderFragmentRoot_Vtbl {
    base__: unknown_vtable(),
    ElementProviderFromPoint: from_point,
    GetFocus: focus,
};
static INVOKE_VTABLE: IInvokeProvider_Vtbl = IInvokeProvider_Vtbl {
    base__: unknown_vtable(),
    Invoke: invoke,
};

#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> Channel {
        Arc::new(RwLock::new(Model {
            revision: 1,
            name: "灵果候选".into(),
            bounds: UiaRect {
                left: 10.0,
                top: 20.0,
                width: 200.0,
                height: 100.0,
            },
            visible: true,
            items: vec![Item {
                name: "学习，中文候选 1".into(),
                bounds: UiaRect {
                    left: 20.0,
                    top: 30.0,
                    width: 100.0,
                    height: 44.0,
                },
                action: 100,
                enabled: true,
                selected: true,
            }],
        }))
    }
    fn sentinel() -> *mut c_void {
        std::ptr::without_provenance_mut(1)
    }
    #[test]
    fn accessible_names_contain_candidate_metadata_and_old_children_are_invalidated() {
        let channel = model();
        let provider = provider(HWND::default(), channel.clone());
        let root: IRawElementProviderFragment = provider.cast().unwrap();
        let child = unsafe { root.Navigate(NavigateDirection_FirstChild) }.unwrap();
        let child: IRawElementProviderSimple = child.cast().unwrap();
        let name = unsafe { child.GetPropertyValue(UIA_NamePropertyId) }.unwrap();
        assert_eq!(
            BSTR::try_from(&name).unwrap().to_string(),
            "学习，中文候选 1"
        );
        channel.write().unwrap().revision = 2;
        assert!(unsafe { child.GetPropertyValue(UIA_NamePropertyId) }.is_err());
        assert!(unsafe { provider.GetPropertyValue(UIA_NamePropertyId) }.is_ok());
    }
    #[test]
    fn abi_missing_navigation_patterns_host_and_focus_write_successful_nulls() {
        let provider = provider(HWND::default(), model());
        let fragment: IRawElementProviderFragment = provider.cast().unwrap();
        let root: IRawElementProviderFragmentRoot = provider.cast().unwrap();
        let child = unsafe { fragment.Navigate(NavigateDirection_FirstChild) }.unwrap();
        let simple: IRawElementProviderSimple = child.cast().unwrap();
        unsafe {
            for (node, direction) in [
                (&fragment, NavigateDirection_Parent),
                (&child, NavigateDirection_PreviousSibling),
                (&child, NavigateDirection_NextSibling),
                (&child, NavigateDirection_FirstChild),
            ] {
                let mut out = sentinel();
                assert_eq!(
                    (node.vtable().Navigate)(node.as_raw(), direction, &mut out),
                    S_OK
                );
                assert!(out.is_null());
            }
            let mut out = sentinel();
            assert_eq!(
                (simple.vtable().GetPatternProvider)(
                    simple.as_raw(),
                    UIA_SelectionPatternId,
                    &mut out
                ),
                S_OK
            );
            assert!(out.is_null());
            out = sentinel();
            assert_eq!(
                (provider.vtable().GetPatternProvider)(
                    provider.as_raw(),
                    UIA_InvokePatternId,
                    &mut out
                ),
                S_OK
            );
            assert!(out.is_null());
            out = sentinel();
            assert_eq!(
                (simple.vtable().HostRawElementProvider)(simple.as_raw(), &mut out),
                S_OK
            );
            assert!(out.is_null());
            out = sentinel();
            assert_eq!((root.vtable().GetFocus)(root.as_raw(), &mut out), S_OK);
            assert!(out.is_null());
            out = sentinel();
            assert_eq!(
                (root.vtable().ElementProviderFromPoint)(root.as_raw(), 0.0, 0.0, &mut out),
                S_OK
            );
            assert!(out.is_null());
            assert_eq!(
                (root.vtable().GetFocus)(root.as_raw(), std::ptr::null_mut()),
                E_POINTER
            );
        }
    }
    #[test]
    fn abi_queries_preserve_identity_and_invalidate_stale_children() {
        let channel = model();
        let provider = provider(HWND::default(), channel.clone());
        let fragment: IRawElementProviderFragment = provider.cast().unwrap();
        let unknown: IUnknown = fragment.cast().unwrap();
        assert_eq!(unknown.as_raw(), provider.as_raw());
        let root = unsafe { fragment.FragmentRoot() }.unwrap();
        assert_eq!(root.cast::<IUnknown>().unwrap().as_raw(), provider.as_raw());
        let child = unsafe { fragment.Navigate(NavigateDirection_FirstChild) }.unwrap();
        let simple: IRawElementProviderSimple = child.cast().unwrap();
        unsafe {
            let mut out = sentinel();
            assert_eq!(
                (simple.vtable().GetPatternProvider)(
                    simple.as_raw(),
                    UIA_InvokePatternId,
                    &mut out
                ),
                S_OK
            );
            assert!(!out.is_null());
            drop(IUnknown::from_raw(out));
            out = sentinel();
            assert_eq!(
                (simple.vtable().base__.QueryInterface)(
                    simple.as_raw(),
                    &IRawElementProviderFragmentRoot::IID,
                    &mut out
                ),
                E_NOINTERFACE
            );
            assert!(out.is_null());
            channel.write().unwrap().revision = 2;
            out = sentinel();
            assert_eq!(
                (child.vtable().Navigate)(child.as_raw(), NavigateDirection_Parent, &mut out),
                HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32)
            );
            assert!(out.is_null());
        }
    }
    #[test]
    fn all_interface_references_keep_the_snapshot_alive_until_release() {
        let channel = model();
        let provider = provider(HWND::default(), channel.clone());
        let fragment: IRawElementProviderFragment = provider.cast().unwrap();
        let unknown: IUnknown = fragment.cast().unwrap();
        assert_eq!(Arc::strong_count(&channel), 2);
        drop(provider);
        drop(fragment);
        assert_eq!(Arc::strong_count(&channel), 2);
        drop(unknown);
        assert_eq!(Arc::strong_count(&channel), 1);
    }
}
