//! Minimal owned UTF-8 C ABI. Every returned string must be released exactly once.
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn atlas_abi_version() -> u32 {
    1
}

/// Invoke the JSON API synchronously. Run long operations on a worker thread.
///
/// # Safety
/// `request_utf8` must be null or point to a valid, NUL-terminated readable string
/// that remains allocated for this call. Input is borrowed and is never freed.
#[no_mangle]
pub unsafe extern "C" fn atlas_call(request_utf8: *const c_char) -> *mut c_char {
    let response = std::panic::catch_unwind(|| {
        if request_utf8.is_null() { return r#"{"ok":false,"error":{"code":"invalid_request","message":"Null request"}}"#.to_owned(); }
        let raw = unsafe { CStr::from_ptr(request_utf8) };
        match raw.to_str() {
            Ok(s) => atlas_service::dispatch_json(s),
            Err(_) => r#"{"ok":false,"error":{"code":"invalid_utf8","message":"Request must be UTF-8"}}"#.to_owned(),
        }
    }).unwrap_or_else(|_| r#"{"ok":false,"error":{"code":"internal","message":"Core panic contained at ABI boundary"}}"#.to_owned());
    CString::new(response).unwrap_or_default().into_raw()
}

/// Release exactly one pointer returned by `atlas_call`; null is accepted.
///
/// # Safety
/// A non-null pointer must have been returned by `atlas_call`, and must not have
/// been freed before. Do not use it after this call.
#[no_mangle]
pub unsafe extern "C" fn atlas_string_free(response: *mut c_char) {
    if !response.is_null() {
        drop(unsafe { CString::from_raw(response) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_response_and_null_input() {
        assert_eq!(atlas_abi_version(), 1);
        unsafe {
            let p = atlas_call(std::ptr::null());
            assert!(CStr::from_ptr(p)
                .to_str()
                .unwrap()
                .contains("invalid_request"));
            atlas_string_free(p);
            atlas_string_free(std::ptr::null_mut());
        }
    }
}
