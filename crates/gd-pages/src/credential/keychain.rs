//! macOS: tokens are generic passwords in the user's default keychain, through the
//! Security framework's item interface. This is an FFI module: every foreign call is
//! here, each `unsafe` block with the reason it is sound.

use std::ffi::c_void;

use gd_plugins::Problem;

use super::{Secret, missing_token, store_problem, stored_secret};

/// The store's name in messages.
pub(super) const STORE_NAME: &str = "the macOS Keychain";

/// A Core Foundation object: a string, data, a dictionary, or a boolean.
type CfRef = *const c_void;

const UTF8: u32 = 0x0800_0100;
const NOT_FOUND: i32 = -25300;
const DUPLICATE: i32 = -25299;
const LABEL: &str = "Guidedog Pages publisher token";

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    // The callback tables are structures of 6 and 5 words; only their addresses are used.
    static kCFTypeDictionaryKeyCallBacks: [usize; 6];
    static kCFTypeDictionaryValueCallBacks: [usize; 5];
    static kCFBooleanTrue: CfRef;
    fn CFDictionaryCreate(
        allocator: CfRef,
        keys: *const CfRef,
        values: *const CfRef,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CfRef;
    fn CFStringCreateWithBytes(
        allocator: CfRef,
        bytes: *const u8,
        count: isize,
        encoding: u32,
        external: u8,
    ) -> CfRef;
    fn CFDataCreate(allocator: CfRef, bytes: *const u8, count: isize) -> CfRef;
    fn CFDataGetLength(data: CfRef) -> isize;
    fn CFDataGetBytePtr(data: CfRef) -> *const u8;
    fn CFRelease(object: CfRef);
}

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    static kSecClass: CfRef;
    static kSecClassGenericPassword: CfRef;
    static kSecAttrService: CfRef;
    static kSecAttrAccount: CfRef;
    static kSecAttrLabel: CfRef;
    static kSecValueData: CfRef;
    static kSecReturnData: CfRef;
    static kSecMatchLimit: CfRef;
    static kSecMatchLimitOne: CfRef;
    fn SecItemAdd(attributes: CfRef, result: *mut CfRef) -> i32;
    fn SecItemCopyMatching(query: CfRef, result: *mut CfRef) -> i32;
    fn SecItemUpdate(query: CfRef, attributes: CfRef) -> i32;
    fn SecItemDelete(query: CfRef) -> i32;
}

/// A Core Foundation object this module created; dropping it releases it.
struct Owned(CfRef);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer came from a CF "Create" or "Copy" function, so this
            // module owns one reference to it, released exactly once here.
            unsafe { CFRelease(self.0) }
        }
    }
}

impl Owned {
    fn checked(self) -> Result<Owned, Problem> {
        if self.0.is_null() {
            return Err(store_problem("Memory for the keychain request was refused", "."));
        }
        Ok(self)
    }
}

fn cf_string(text: &str) -> Result<Owned, Problem> {
    // SAFETY: the pointer and length describe `text`, which outlives the call; CF copies
    // the bytes (external = 0) and returns a new object or null.
    let s = unsafe {
        CFStringCreateWithBytes(std::ptr::null(), text.as_ptr(), text.len() as isize, UTF8, 0)
    };
    Owned(s).checked()
}

fn cf_data(bytes: &[u8]) -> Result<Owned, Problem> {
    // SAFETY: the pointer and length describe `bytes`, which CF copies.
    let d = unsafe { CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize) };
    Owned(d).checked()
}

/// A dictionary of `keys` to `values`, which the dictionary retains.
fn dictionary(keys: &[CfRef], values: &[CfRef]) -> Result<Owned, Problem> {
    // SAFETY: both slices have the same length and hold live CF objects; the callback
    // tables are the system's own statics, of which only the addresses are taken.
    let d = unsafe {
        CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            keys.len().min(values.len()) as isize,
            (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
            (&raw const kCFTypeDictionaryValueCallBacks).cast(),
        )
    };
    Owned(d).checked()
}

/// The query naming the item of `service` and `profile`.
fn item_query(service: &Owned, profile: &Owned) -> Result<Owned, Problem> {
    // SAFETY: the Security constants are immutable CF objects the framework exports.
    let (class, generic, attr_service, attr_account) =
        unsafe { (kSecClass, kSecClassGenericPassword, kSecAttrService, kSecAttrAccount) };
    dictionary(&[class, attr_service, attr_account], &[generic, service.0, profile.0])
}

fn status_problem(what: &str, status: i32) -> Problem {
    store_problem(what, &format!(" (status {status})."))
}

pub(super) fn store(service: &str, profile: &str, token: &str) -> Result<(), Problem> {
    let (name, account, label) = (cf_string(service)?, cf_string(profile)?, cf_string(LABEL)?);
    let data = cf_data(token.as_bytes())?;
    // SAFETY: the Security constants are immutable CF objects the framework exports.
    let (class, generic, attr_service, attr_account, attr_label, value) = unsafe {
        (
            kSecClass,
            kSecClassGenericPassword,
            kSecAttrService,
            kSecAttrAccount,
            kSecAttrLabel,
            kSecValueData,
        )
    };
    let item = dictionary(
        &[class, attr_service, attr_account, attr_label, value],
        &[generic, name.0, account.0, label.0, data.0],
    )?;
    // SAFETY: `item` is a live dictionary; a null result pointer asks for no result.
    let mut status = unsafe { SecItemAdd(item.0, std::ptr::null_mut()) };
    if status == DUPLICATE {
        let query = item_query(&name, &account)?;
        let change = dictionary(&[value], &[data.0])?;
        // SAFETY: both arguments are live dictionaries.
        status = unsafe { SecItemUpdate(query.0, change.0) };
    }
    if status != 0 {
        return Err(status_problem("The keychain refused to store the token", status));
    }
    Ok(())
}

pub(super) fn load(service: &str, profile: &str) -> Result<Secret, Problem> {
    let (name, account) = (cf_string(service)?, cf_string(profile)?);
    // SAFETY: the Security and CF constants are immutable objects the frameworks export.
    let (class, generic, attr_service, attr_account, ret, limit, one, yes) = unsafe {
        (
            kSecClass,
            kSecClassGenericPassword,
            kSecAttrService,
            kSecAttrAccount,
            kSecReturnData,
            kSecMatchLimit,
            kSecMatchLimitOne,
            kCFBooleanTrue,
        )
    };
    let query = dictionary(
        &[class, attr_service, attr_account, ret, limit],
        &[generic, name.0, account.0, yes, one],
    )?;
    let mut found: CfRef = std::ptr::null();
    // SAFETY: `query` is a live dictionary and `found` a valid place for the result,
    // which the caller then owns.
    let status = unsafe { SecItemCopyMatching(query.0, &mut found) };
    let found = Owned(found);
    if status == NOT_FOUND {
        return Err(missing_token(profile));
    }
    if status != 0 || found.0.is_null() {
        return Err(status_problem("The keychain refused to read the token", status));
    }
    // SAFETY: with kSecReturnData and one match, the result is a CFData this module owns.
    let length = unsafe { CFDataGetLength(found.0) };
    if length <= 0 {
        return stored_secret(&[]);
    }
    // SAFETY: the data's bytes stay valid while `found` is alive, for `length` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(CFDataGetBytePtr(found.0), length as usize) };
    stored_secret(bytes)
}

pub(super) fn delete(service: &str, profile: &str) -> Result<bool, Problem> {
    let (name, account) = (cf_string(service)?, cf_string(profile)?);
    let query = item_query(&name, &account)?;
    // SAFETY: `query` is a live dictionary.
    let status = unsafe { SecItemDelete(query.0) };
    match status {
        0 => Ok(true),
        NOT_FOUND => Ok(false),
        _ => Err(status_problem("The keychain refused to delete the token", status)),
    }
}
