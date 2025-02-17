use objc2::__framework_prelude::Retained;
use objc2::rc::autoreleasepool;
use objc2_foundation::{NSData, NSUTF8StringEncoding, NSString, NSUInteger};
use uuid::Uuid;
use rusqlite::{Result as SqlResult};
use crate::db::contact::{Contact, ContactObjC};
use std::ffi::c_void;
use objc2::__macro_helpers::MaybeOptionRetained;

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
        if ns_str.is_null() || ns_str.length() == 0 {
            None
        } else {
            Some(ns_str.to_string())
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

            // Конвертация UUID
            let uuid_bytes = self.id.as_bytes();
            let uuid_data: *mut NSData = NSData::dataWithBytes_length(
                uuid_bytes.as_ptr() as *const c_void,
                uuid_bytes.len() as NSUInteger
            )
                .autorelease_return()
                .into();

            ContactObjC_setId(objc_contact, uuid_data);

            // Конвертация имени
            let first_name: *mut NSString = NSString::from_str(&self.first_name)
                .autorelease_return()
                .into();

            let last_name: *mut NSString = NSString::from_str(&self.last_name)
                .autorelease_return()
                .into();

            ContactObjC_setFirstName(objc_contact, first_name);
            ContactObjC_setLastName(objc_contact, last_name);

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
