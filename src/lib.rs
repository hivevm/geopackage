use libsqlite3_sys as ffi;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

// Callback-Funktion für eine benutzerdefinierte SQL-Funktion
unsafe extern "C" fn my_function(
    context: *mut ffi::sqlite3_context,
    argc: c_int,
    argv: *mut *mut ffi::sqlite3_value,
) {
    if argc != 2 {
        let err = CString::new("Expected 2 arguments").unwrap();
        ffi::sqlite3_result_error(context, err.as_ptr(), -1);
        return;
    }

    let arg1 = ffi::sqlite3_value_int(*argv.offset(0));
    let arg2 = ffi::sqlite3_value_int(*argv.offset(1));
    
    let result = arg1 + arg2;
    ffi::sqlite3_result_int(context, result);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_extension_init(
    db: *mut ffi::sqlite3,
    pz_err_msg: *mut *mut c_char,
    p_api: *mut ffi::sqlite3_api_routines,
) -> c_int {
    let fn_name = CString::new("add_numbers").unwrap();
    
    let result = ffi::sqlite3_create_function_v2(
        db,
        fn_name.as_ptr(),
        2,  // Anzahl der Argumente
        ffi::SQLITE_UTF8 | ffi::SQLITE_DETERMINISTIC,
        std::ptr::null_mut(),
        Some(my_function),
        None,
        None,
        None,
    );
    
    result
}