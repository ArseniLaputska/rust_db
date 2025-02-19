use objc2::rc::{Retained, autoreleasepool};
use objc2::msg_send;
use objc2::runtime::AnyClass;
use objc2_foundation::{NSData, NSUTF8StringEncoding, NSString, NSUInteger};
use objc2::__macro_helpers::MaybeOptionRetained;
use uuid::Uuid;
use rusqlite::{Result as SqlResult};
use std::ffi::{c_void, CStr};
use std::fmt::Display;

use crate::db::contact::{Contact, ContactObjC};

/// Создаём `Id<NSData>` из байтового вектора, вызывая `[NSData dataWithBytes:length:]` напрямую.
fn create_nsdata(bytes: &[u8]) -> Retained<NSData> {
    unsafe {
        let nsdata_class = AnyClass::get(CStr::from_bytes_with_nul(b"NSData\0").unwrap())
            .expect("NSData class not found");
        let raw: *mut NSData = msg_send![nsdata_class, dataWithBytes: bytes.as_ptr(), length: bytes.len()];
        Retained::retain(raw).unwrap()
    }
}

/// Создаём `Id<NSString>` из обычной строки, вызывая `[NSString initWithBytes:length:encoding:]`.
fn create_nsstring(s: &str) -> Retained<NSString> {
    unsafe {
        // Получаем класс NSString через Class::get
        let nsstring_class = AnyClass::get(CStr::from_bytes_with_nul(b"NSString\0").unwrap())
            .expect("NSString class not found");
        let raw: *mut NSString = msg_send![nsstring_class, alloc];
        let raw: *mut NSString = msg_send![
            raw,
            initWithBytes: s.as_ptr(),
            length: s.len(),
            encoding: NSUTF8StringEncoding
        ];
        Retained::retain(raw).unwrap()
    }
}

pub fn convert_to_nsdata(bytes: Vec<u8>) -> *mut NSData {
    let data = NSData::from_vec(bytes);
    Retained::autorelease_return(data)
}

pub fn nsdata_to_uuid(nsdata: *mut NSData) -> SqlResult<Uuid> {
    autoreleasepool(|_| {
        let data = unsafe { Retained::retain(nsdata) }.ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("Null NSData pointer".into())
        })?;

        unsafe {
            let bytes = data.as_bytes_unchecked();
            Uuid::from_slice(bytes)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string().into()))
        }
    })
}

pub fn convert_to_nsstring(s: String) -> *mut NSString {
    let ns_str = NSString::from_str(&s);
    Retained::autorelease_return(ns_str)
}

pub fn nsstring_to_string(ns_str: *mut NSString) -> String {
    if ns_str.is_null() {
        String::new()
    } else {
        autoreleasepool(|_| {
            unsafe { Retained::retain(ns_str) }
                .map(|s| s.to_string())
                .unwrap_or_default()
        })
    }
}

pub fn optional_to_nsstring(opt: Option<String>) -> *mut NSString {
    opt.map(|s| {
        let ns_str = NSString::from_str(&s);
        Retained::autorelease_return(ns_str)
    }).unwrap_or_else(|| std::ptr::null_mut())
}

pub fn optional_nsstring(ns_str: *mut NSString) -> Option<String> {
    unsafe {
        if ns_str.is_null() {
            None
        } else {
            // Сначала получаем ссылку &NSString из *mut NSString
            let nsref = ns_str.as_ref();
            // Проверяем длину
            if let Some(ns) = nsref {
                if ns.len() == 0 {
                    return None;
                }
                // Конвертируем в String
                return Some(ns.to_string());
            }
            None
        }
    }
}

pub fn optional_nsdata_to_uuid(nsdata: *mut NSData) -> Option<Uuid> {
    if nsdata.is_null() {
        None
    } else {
        nsdata_to_uuid(nsdata).ok()
    }
}

impl Contact {
    pub fn to_objc(&self) -> *mut ContactObjC {
        unsafe {
            let objc_contact = ContactObjC_new();

            // UUID -> NSData -> *mut NSData
            let bytes = self.id.as_bytes();
            let data_id = create_nsdata(bytes);
            let data_ptr = Retained::into_raw(data_id);
            ContactObjC_setId(objc_contact, data_ptr);

            // first_name
            let fname_id = create_nsstring(&self.first_name);
            let fname_ptr = Retained::into_raw(fname_id);
            ContactObjC_setFirstName(objc_contact, fname_ptr);

            // last_name
            let lname_id = create_nsstring(&self.last_name);
            let lname_ptr = Retained::into_raw(lname_id);
            ContactObjC_setLastName(objc_contact, lname_ptr);

            // Остальные поля (например, relationship) устанавливайте через setter, если нужно.

            objc_contact
        }
    }

    pub fn from_objc(objc_contact: *mut ContactObjC) -> Self {
        unsafe {
            Self {
                id: nsdata_to_uuid((*objc_contact).id).unwrap(),
                first_name: nsstring_to_str((*objc_contact).first_name),
                last_name: nsstring_to_str((*objc_contact).last_name),
                created_at: (*objc_contact).created_at,
                last_message_at: Some((*objc_contact).last_message_at),
                updated_at: (*objc_contact).updated_at,
                relationship: (*objc_contact).relationship as i64,
                username: optional_nsstring((*objc_contact).username),
                language: optional_nsstring((*objc_contact).language),
                picture_url: optional_nsstring((*objc_contact).picture_url),
                is_pro: (*objc_contact).is_pro as i64,
            }
        }
    }
}
pub unsafe fn nsstring_to_str(nsstr: *mut NSString) -> String {
    autoreleasepool(|_| {
        let ns_str = Retained::retain(nsstr).unwrap();
        let c_str = ns_str.UTF8String() as *const u8;
        let len = ns_str.len();
        String::from_utf8_lossy(std::slice::from_raw_parts(c_str, len)).into_owned()
    })
}

pub unsafe fn uuid_to_nsdata(uuid: Uuid) -> Retained<NSData> {
    let bytes = uuid.as_bytes();
    NSData::dataWithBytes_length(
        bytes.as_ptr() as *const c_void,
        bytes.len() as NSUInteger
    )
}

pub unsafe fn free_contact_objc(ptr: *mut ContactObjC) {
    if !ptr.is_null() {
        ContactObjC_release(ptr);
    }
}

extern "C" {
    fn ContactObjC_new() -> *mut ContactObjC;
    fn ContactObjC_setId(obj: *mut ContactObjC, data: *mut NSData);
    fn ContactObjC_setFirstName(obj: *mut ContactObjC, name: *mut NSString);
    fn ContactObjC_setLastName(obj: *mut ContactObjC, name: *mut NSString);
    fn ContactObjC_release(obj: *mut ContactObjC);
}
