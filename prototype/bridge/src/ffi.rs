//! Opaque C ABI over the same local adapter as the CLI. No host or input logging.
use crate::Adapter;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Mutex;

pub const OK: u32 = 0;
pub const INVALID_ARGUMENT: u32 = 1;
pub const INVALID_UTF8: u32 = 2;
pub const INITIALIZATION_FAILED: u32 = 3;
pub const PANIC_OR_POISONED: u32 = 4;
pub const REQUEST_TOO_LARGE: u32 = 5;
pub const REPLY_TOO_LARGE: u32 = 6;
pub const INTERNAL_ERROR: u32 = 7;
pub const MAX_PATH_BYTES: usize = 32 * 1024;
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_REPLY_BYTES: usize = 4 * 1024 * 1024;

struct Handle {
    adapter: Mutex<Adapter>,
}

fn boundary(body: impl FnOnce() -> Result<(), u32>) -> u32 {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => OK,
        Ok(Err(code)) => code,
        Err(_) => PANIC_OR_POISONED,
    }
}

unsafe fn utf8<'a>(bytes: *const u8, length: usize) -> Result<&'a str, u32> {
    if length == 0 {
        return Ok("");
    }
    if bytes.is_null() {
        return Err(INVALID_ARGUMENT);
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(bytes, length) })
        .map_err(|_| INVALID_UTF8)
}

/// Create an independent adapter. A zero-length memory path disables vocabulary writes.
/// Initialization errors return only a fixed code; their potentially private text is discarded.
///
/// # Safety
/// Nonempty byte buffers must remain readable for this call; `out_handle` must be a
/// valid, aligned writable pointer. Successful handles must be destroyed exactly once
/// with `LingoEngineDestroy`, after all requests using the handle have returned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn LingoEngineCreate(
    root: *const u8,
    root_length: usize,
    memory: *const u8,
    memory_length: usize,
    out_handle: *mut *mut c_void,
) -> u32 {
    if out_handle.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { ptr::write(out_handle, ptr::null_mut()) };
    boundary(|| {
        if root_length == 0 || root_length > MAX_PATH_BYTES || memory_length > MAX_PATH_BYTES {
            return Err(INVALID_ARGUMENT);
        }
        let root = unsafe { utf8(root, root_length)? };
        let memory = unsafe { utf8(memory, memory_length)? };
        let mut adapter = Adapter::load(Path::new(root)).map_err(|_| INITIALIZATION_FAILED)?;
        if !memory.is_empty() {
            adapter.set_memory_path(Some(PathBuf::from(memory)));
        }
        let handle = Box::new(Handle {
            adapter: Mutex::new(adapter),
        });
        unsafe { ptr::write(out_handle, Box::into_raw(handle).cast()) };
        Ok(())
    })
}

/// Process one bounded UTF-8 JSON request. Protocol errors retain the existing JSON
/// error response. On success, the reply is UTF-8 without a trailing NUL or newline.
/// The caller owns its request buffer; the core owns the returned allocation.
///
/// # Safety
/// `handle` must be a live handle returned by `LingoEngineCreate`; nonempty request
/// bytes must be readable. Both output pointers must be valid, aligned writable
/// pointers. Call `LingoEngineFreeReply` exactly once with the original reply pointer
/// and length, including when the caller fails to decode the reply. Concurrent
/// requests are serialized; destruction must not overlap any request.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn LingoEngineRequest(
    handle: *mut c_void,
    request: *const u8,
    request_length: usize,
    out_bytes: *mut *mut u8,
    out_length: *mut usize,
) -> u32 {
    if out_bytes.is_null() || out_length.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe {
        ptr::write(out_bytes, ptr::null_mut());
        ptr::write(out_length, 0);
    }
    boundary(|| {
        if handle.is_null() {
            return Err(INVALID_ARGUMENT);
        }
        if request_length > MAX_REQUEST_BYTES {
            return Err(REQUEST_TOO_LARGE);
        }
        let request = unsafe { utf8(request, request_length)? };
        let handle = unsafe { &*handle.cast::<Handle>() };
        let mut adapter = handle.adapter.lock().map_err(|_| PANIC_OR_POISONED)?;
        let response = adapter.request_json(request);
        let bytes = serde_json::to_vec(&response).map_err(|_| INTERNAL_ERROR)?;
        if bytes.len() > MAX_REPLY_BYTES {
            return Err(REPLY_TOO_LARGE);
        }
        let bytes = bytes.into_boxed_slice();
        let length = bytes.len();
        let reply = Box::into_raw(bytes).cast::<u8>();
        unsafe {
            ptr::write(out_bytes, reply);
            ptr::write(out_length, length);
        }
        Ok(())
    })
}

/// Return a reply allocation to the same core that created it. A null reply is a no-op.
///
/// # Safety
/// Non-null pointers and lengths must be an unchanged pair returned successfully by
/// `LingoEngineRequest`, and must not previously have been freed. No other allocator
/// or Rust instance may free or resize this allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn LingoEngineFreeReply(bytes: *mut u8, length: usize) {
    if bytes.is_null() || length == 0 || length > MAX_REPLY_BYTES {
        return;
    }
    let _ = boundary(|| {
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(bytes, length)));
        }
        Ok(())
    });
}

/// Destroy an adapter without writing unconfirmed vocabulary. A null handle is a no-op.
///
/// # Safety
/// Non-null handles must have been returned by `LingoEngineCreate`, must not have
/// been destroyed previously, and must not be used by a concurrent or future request.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn LingoEngineDestroy(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    let _ = boundary(|| {
        unsafe { drop(Box::from_raw(handle.cast::<Handle>())) };
        Ok(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_can_move_to_an_owned_native_worker() {
        fn require_send<T: Send>() {}
        require_send::<Adapter>();
    }

    #[test]
    fn panics_are_bounded_and_error_codes_are_static() {
        assert_eq!(boundary(|| panic!("synthetic panic")), PANIC_OR_POISONED);
        assert_eq!(
            boundary(|| Err(INITIALIZATION_FAILED)),
            INITIALIZATION_FAILED
        );
    }

    #[test]
    fn initialization_rejects_bad_lengths_and_encoding_before_loading_data() {
        let mut handle = ptr::dangling_mut::<c_void>();
        let invalid = [0xff];
        unsafe {
            assert_eq!(
                LingoEngineCreate(ptr::null(), MAX_PATH_BYTES + 1, ptr::null(), 0, &mut handle),
                INVALID_ARGUMENT
            );
            assert!(handle.is_null());
            assert_eq!(
                LingoEngineCreate(invalid.as_ptr(), invalid.len(), ptr::null(), 0, &mut handle),
                INVALID_UTF8
            );
            assert!(handle.is_null());
            assert_eq!(
                LingoEngineCreate(ptr::null(), 1, ptr::null(), 0, &mut handle),
                INVALID_ARGUMENT
            );
        }
    }
}
