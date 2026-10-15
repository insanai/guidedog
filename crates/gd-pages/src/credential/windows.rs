//! Windows: tokens are generic credentials in the user's Credential Manager, named
//! `SERVICE:PROFILE`. This is an FFI module: every foreign call is here, each `unsafe`
//! block with the reason it is sound.

use std::ffi::c_void;

use gd_plugins::Problem;

use super::{Secret, forget_bytes, missing_token, store_problem, stored_secret};

/// The store's name in messages.
pub(super) const STORE_NAME: &str = "the Windows Credential Manager";

const CRED_TYPE_GENERIC: u32 = 1;
const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
const ERROR_NOT_FOUND: i32 = 1168;

#[repr(C)]
struct FileTime {
    low: u32,
    high: u32,
}

/// `CREDENTIALW`.
#[repr(C)]
struct Credential {
    flags: u32,
    kind: u32,
    target_name: *mut u16,
    comment: *mut u16,
    last_written: FileTime,
    blob_size: u32,
    blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut c_void,
    target_alias: *mut u16,
    user_name: *mut u16,
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn CredWriteW(credential: *const Credential, flags: u32) -> i32;
    fn CredReadW(
        target: *const u16,
        kind: u32,
        flags: u32,
        credential: *mut *mut Credential,
    ) -> i32;
    fn CredDeleteW(target: *const u16, kind: u32, flags: u32) -> i32;
    fn CredFree(buffer: *mut c_void);
}

/// `text` as a NUL-terminated UTF-16 string.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn last_error() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn error_problem(what: &str, error: i32) -> Problem {
    store_problem(what, &format!(" (error {error})."))
}

pub(super) fn store(service: &str, profile: &str, token: &str) -> Result<(), Problem> {
    let mut target = wide(&format!("{service}:{profile}"));
    let mut user = wide(profile);
    let mut blob = token.as_bytes().to_vec();
    let credential = Credential {
        flags: 0,
        kind: CRED_TYPE_GENERIC,
        target_name: target.as_mut_ptr(),
        comment: std::ptr::null_mut(),
        last_written: FileTime { low: 0, high: 0 },
        blob_size: blob.len() as u32,
        blob: blob.as_mut_ptr(),
        persist: CRED_PERSIST_LOCAL_MACHINE,
        attribute_count: 0,
        attributes: std::ptr::null_mut(),
        target_alias: std::ptr::null_mut(),
        user_name: user.as_mut_ptr(),
    };
    // SAFETY: every pointer in `credential` points into a live buffer of this frame
    // (`target`, `user`, `blob`) or is null where the structure allows it; the call
    // copies what it keeps.
    let written = unsafe { CredWriteW(&credential, 0) } != 0;
    let error = last_error();
    forget_bytes(blob);
    if !written {
        return Err(error_problem("The Credential Manager refused to store the token", error));
    }
    Ok(())
}

pub(super) fn load(service: &str, profile: &str) -> Result<Secret, Problem> {
    let target = wide(&format!("{service}:{profile}"));
    let mut found: *mut Credential = std::ptr::null_mut();
    // SAFETY: `target` is NUL-terminated and `found` a valid place for the result, which
    // CredFree releases below.
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut found) } == 0 {
        let error = last_error();
        if error == ERROR_NOT_FOUND {
            return Err(missing_token(profile));
        }
        return Err(error_problem("The Credential Manager refused to read the token", error));
    }
    // SAFETY: CredReadW succeeded, so `found` points to a credential whose blob holds
    // `blob_size` bytes (or is null), valid until CredFree.
    let secret = unsafe {
        let c = &*found;
        if c.blob.is_null() {
            stored_secret(&[])
        } else {
            stored_secret(std::slice::from_raw_parts(c.blob, c.blob_size as usize))
        }
    };
    // SAFETY: `found` came from CredReadW and is freed once.
    unsafe { CredFree(found.cast()) };
    secret
}

pub(super) fn delete(service: &str, profile: &str) -> Result<bool, Problem> {
    let target = wide(&format!("{service}:{profile}"));
    // SAFETY: `target` is NUL-terminated.
    if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } != 0 {
        return Ok(true);
    }
    match last_error() {
        ERROR_NOT_FOUND => Ok(false),
        error => Err(error_problem("The Credential Manager refused to delete the token", error)),
    }
}
