use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
use loom_ffi::ffi::{loom_decode, set_panic_sentinel, LoomError};

fn call_decode_code(input: &[u8]) -> i32 {
    let mut ffi_array: FFI_ArrowArray = unsafe { std::mem::zeroed() };
    let mut ffi_schema: FFI_ArrowSchema = unsafe { std::mem::zeroed() };
    unsafe {
        loom_decode(
            input.as_ptr(),
            input.len(),
            &mut ffi_array as *mut _,
            &mut ffi_schema as *mut _,
        )
    }
}

#[test]
fn ffi_contract_malformed_lmp1_returns_decode_failed() {
    assert_eq!(
        call_decode_code(b"LMP1"),
        LoomError::DecodeFailed.code(),
        "malformed raw layout bytes must fail closed across FFI"
    );
}

#[test]
fn ffi_contract_malformed_lmc1_returns_decode_failed() {
    assert_eq!(
        call_decode_code(b"LMC1"),
        LoomError::DecodeFailed.code(),
        "malformed container bytes must fail closed across FFI"
    );
}

#[test]
fn ffi_contract_panic_sentinel_returns_panicked() {
    set_panic_sentinel();
    assert_eq!(
        call_decode_code(&[]),
        LoomError::Panicked.code(),
        "panic sentinel must be contained by catch_unwind"
    );
}

