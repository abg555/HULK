use std::ffi::CString;
use std::ffi::CStr;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn hulk_concat(left: *const c_char, right: *const c_char) -> *mut c_char {
    unsafe {
        let left = if left.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(left).to_str().unwrap_or("")
        };
        let right = if right.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(right).to_str().unwrap_or("")
        };

        CString::new(format!("{}{}", left, right))
            .unwrap_or_else(|_| CString::new("").unwrap())
            .into_raw()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hulk_concat_full(left: *const c_char, right: *const c_char) -> *mut c_char {
    unsafe {
        let left = if left.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(left).to_str().unwrap_or("")
        };
        let right = if right.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(right).to_str().unwrap_or("")
        };

        CString::new(format!("{} {}", left, right))
            .unwrap_or_else(|_| CString::new("").unwrap())
            .into_raw()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hulk_rand() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);

    (nanos as f64) / 1_000_000_000.0
}

#[unsafe(no_mangle)]
pub extern "C" fn hulk_format_number(n: f64) -> *mut c_char {
    unsafe {
        CString::new(format!("{}", n))
            .unwrap_or_else(|_| CString::new("0").unwrap())
            .into_raw()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hulk_panic(message: *const c_char) {
    unsafe {
        if message.is_null() {
            eprintln!("Runtime error: cast 'as' failed");
        } else {
            let msg = CStr::from_ptr(message).to_str().unwrap_or("Runtime error");
            eprintln!("{}", msg);
        }
    }

    std::process::abort();
}