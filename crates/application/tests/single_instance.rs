//! S5 feasibility spike: prove a named Windows mutex can detect a second
//! application instance. Probe only; it does not own the app lifecycle.

#![cfg(windows)]

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GetLastError, BOOL, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;

#[test]
fn named_mutex_detects_second_instance() {
    let name = w!("Global\\v2rayN-R-T01-single-instance-probe");

    unsafe {
        let first = CreateMutexW(None, BOOL(0), name).expect("first CreateMutexW");
        assert!(!first.is_invalid(), "first mutex handle invalid");

        let second = CreateMutexW(None, BOOL(0), name).expect("second CreateMutexW");
        assert!(!second.is_invalid(), "second mutex handle invalid");

        let last_error = GetLastError();
        println!("T01-S5 second instance GetLastError={}", last_error.0);
        assert_eq!(
            last_error, ERROR_ALREADY_EXISTS,
            "second instance must observe ERROR_ALREADY_EXISTS"
        );

        let _ = CloseHandle(second);
        let _ = CloseHandle(first);
    }
}
