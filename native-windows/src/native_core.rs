//! Windows transport for the bundled core. No subprocess, inherited pipe or COM on this thread.
use crate::bridge::EngineFailure;
use serde_json::Value;
use std::ffi::c_void;
use std::path::Path;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
    LoadLibraryExW,
};
use windows::core::{PCSTR, PCWSTR};

type Create = unsafe extern "C" fn(*const u8, usize, *const u8, usize, *mut *mut c_void) -> u32;
type Request = unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut *mut u8, *mut usize) -> u32;
type FreeReply = unsafe extern "C" fn(*mut u8, usize);
type Destroy = unsafe extern "C" fn(*mut c_void);
const MAX_REPLY: usize = 4 * 1024 * 1024;
const MAX_REQUEST: usize = 64 * 1024;

struct Module(HMODULE);
impl Drop for Module {
    fn drop(&mut self) {
        let _ = unsafe { FreeLibrary(self.0) };
    }
}
pub(crate) struct NativeCore {
    module: Module,
    handle: *mut c_void,
    request: Request,
    free_reply: FreeReply,
    destroy: Destroy,
}
// Ownership is transferred to one engine worker. The core's handle also serializes requests.
unsafe impl Send for NativeCore {}

impl NativeCore {
    pub(crate) fn load(
        dll: &Path,
        resources: &Path,
        memory: Option<&Path>,
    ) -> Result<Self, EngineFailure> {
        use std::os::windows::ffi::OsStrExt;
        if !dll.is_absolute() {
            return Err(EngineFailure::NativeLoad { code: 87 });
        }
        let wide: Vec<u16> = dll.as_os_str().encode_wide().chain(Some(0)).collect();
        if wide[..wide.len() - 1].contains(&0) {
            return Err(EngineFailure::NativeLoad { code: 87 });
        }
        let module = Module(
            unsafe {
                LoadLibraryExW(
                    PCWSTR(wide.as_ptr()),
                    None,
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                )
            }
            .map_err(|error| EngineFailure::NativeLoad {
                code: error.code().0 as u32,
            })?,
        );
        let symbol = |name: &'static [u8]| {
            unsafe { GetProcAddress(module.0, PCSTR(name.as_ptr())) }
                .ok_or(EngineFailure::NativeLoad { code: 127 })
        };
        // These exact signatures are versioned with prototype/bridge/include/lingomate_engine.h.
        let create: Create = unsafe { std::mem::transmute(symbol(b"LingoEngineCreate\0")?) };
        let request: Request = unsafe { std::mem::transmute(symbol(b"LingoEngineRequest\0")?) };
        let free_reply: FreeReply =
            unsafe { std::mem::transmute(symbol(b"LingoEngineFreeReply\0")?) };
        let destroy: Destroy = unsafe { std::mem::transmute(symbol(b"LingoEngineDestroy\0")?) };
        let root = resources
            .to_str()
            .ok_or(EngineFailure::CoreInit { code: 2 })?;
        let memory = memory
            .map(|path| path.to_str().ok_or(EngineFailure::CoreInit { code: 2 }))
            .transpose()?
            .unwrap_or("");
        let mut handle = std::ptr::null_mut();
        let code = unsafe {
            create(
                root.as_ptr(),
                root.len(),
                memory.as_ptr(),
                memory.len(),
                &mut handle,
            )
        };
        if code != 0 || handle.is_null() {
            return Err(EngineFailure::CoreInit {
                code: if code == 0 { 1 } else { code },
            });
        }
        Ok(Self {
            module,
            handle,
            request,
            free_reply,
            destroy,
        })
    }
    pub(crate) fn request(&mut self, request: &Value) -> Result<Value, EngineFailure> {
        let request = serde_json::to_vec(request).map_err(|_| EngineFailure::InvalidReply)?;
        if request.len() > MAX_REQUEST {
            return Err(EngineFailure::CoreReply { code: 5 });
        }
        let mut bytes = std::ptr::null_mut();
        let mut length = 0;
        let code = unsafe {
            (self.request)(
                self.handle,
                request.as_ptr(),
                request.len(),
                &mut bytes,
                &mut length,
            )
        };
        let reply = ReplyBuffer {
            bytes,
            length,
            free: self.free_reply,
        };
        if code != 0 {
            return Err(EngineFailure::CoreReply { code });
        }
        if reply.bytes.is_null() || reply.length == 0 || reply.length > MAX_REPLY {
            return Err(EngineFailure::InvalidReply);
        }
        let slice = unsafe { std::slice::from_raw_parts(reply.bytes, reply.length) };
        serde_json::from_slice(slice).map_err(|_| EngineFailure::InvalidReply)
    }
}
struct ReplyBuffer {
    bytes: *mut u8,
    length: usize,
    free: FreeReply,
}
impl Drop for ReplyBuffer {
    fn drop(&mut self) {
        if !self.bytes.is_null() {
            unsafe { (self.free)(self.bytes, self.length) };
        }
    }
}
impl Drop for NativeCore {
    fn drop(&mut self) {
        // Destroy must execute while the owning module is still mapped.
        unsafe { (self.destroy)(self.handle) };
        let _ = &self.module;
    }
}
