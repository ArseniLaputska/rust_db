// src/db/objc_contact.rs

use objc2::declare::ClassDecl;
use objc2::runtime::{Class, Sel, Object, Ivar};
use objc2_foundation::{NSObject, NSString, NSData, NSNumber};
use objc2::rc::{autoreleasepool, Id};
use objc2::msg_send;
use objc2::__macro_helpers::MaybeOptionRetained;
use objc2::Encode;
use objc2::sel;
use std::ptr;
use std::sync::Once;
use std::ffi::{CString, CStr};
use uuid::Uuid;
use crate::db::contact::Contact;
use crate::db::objc_converters::{convert_to_nsdata, convert_to_nsstring};

// Если возможно, используем derive или ручную реализацию Encode:
unsafe impl Encode for RustContact {
    const ENCODING: &'static str = "{RustContact=#}";
}

/// Регистрация класса RustContact с поддержкой KVO
static REGISTER: Once = Once::new();
static mut RUST_CONTACT_CLASS: *const Class = ptr::null();

/// Регистрирует класс `RustContact` (наследник NSObject) с динамическими свойствами.
/// Свойства: _id, _firstName, _lastName, _relationship (для примера).
pub fn register_rust_contact_class() -> &'static Class {
    REGISTER.call_once(|| {
        let nsobject_class = Class::get(CStr::from_bytes_with_nul(b"NSObject\0").unwrap())
            .expect("NSObject class not found");

        // Имя класса RustContact как &CStr
        let class_name = CStr::from_bytes_with_nul(b"RustContact\0").unwrap();

        let mut decl = ClassDecl::new(class_name, nsobject_class)
            .expect("Failed to declare RustContact class");

        // Добавляем ivar, передавая имя также как &CStr
        decl.add_ivar::<*mut NSData>(CStr::from_bytes_with_nul(b"_id\0").unwrap());
        decl.add_ivar::<*mut NSString>(CStr::from_bytes_with_nul(b"_firstName\0").unwrap());
        decl.add_ivar::<*mut NSString>(CStr::from_bytes_with_nul(b"_lastName\0").unwrap());
        decl.add_ivar::<*mut NSNumber>(CStr::from_bytes_with_nul(b"_relationship\0").unwrap());

        // Добавляем методы (getter/setter)
        unsafe {
            decl.add_method(
                sel!(id),
                rust_contact_id as extern "C" fn(&RustContact, Sel) -> *mut NSData,
            );
            decl.add_method(
                sel!(firstName),
                rust_contact_first_name as extern "C" fn(&RustContact, Sel) -> *mut NSString,
            );
            decl.add_method(
                sel!(lastName),
                rust_contact_last_name as extern "C" fn(&RustContact, Sel) -> *mut NSString,
            );
            decl.add_method(
                sel!(relationship),
                rust_contact_relationship as extern "C" fn(&RustContact, Sel) -> *mut NSNumber,
            );

            decl.add_method(
                sel!(setFirstName:),
                rust_contact_set_first_name as extern "C" fn(&mut RustContact, Sel, *mut NSString),
            );
            decl.add_method(
                sel!(setLastName:),
                rust_contact_set_last_name as extern "C" fn(&mut RustContact, Sel, *mut NSString),
            );
            decl.add_method(
                sel!(setRelationship:),
                rust_contact_set_relationship as extern "C" fn(&mut RustContact, Sel, *mut NSNumber),
            );
        }

        unsafe {
            RUST_CONTACT_CLASS = decl.register();
        }
    });
    unsafe { &*RUST_CONTACT_CLASS }
}

/// Представление нашего Objective‑C класса в Rust
#[repr(C)]
pub struct RustContact {
    pub superclass: NSObject, // Наследуемся от NSObject
    // Остальные поля хранятся в ivar‑ах, доступ к которым осуществляется через runtime.
}

/// Getter‑ы

extern "C" fn rust_contact_id(this: &RustContact, _cmd: Sel) -> *mut NSData {
    unsafe { this.get_ivar::<*mut NSData>("_id").unwrap_or(ptr::null_mut()) }
}

extern "C" fn rust_contact_first_name(this: &RustContact, _cmd: Sel) -> *mut NSString {
    unsafe { this.get_ivar::<*mut NSString>("_firstName").unwrap_or(ptr::null_mut()) }
}

extern "C" fn rust_contact_last_name(this: &RustContact, _cmd: Sel) -> *mut NSString {
    unsafe { this.get_ivar::<*mut NSString>("_lastName").unwrap_or(ptr::null_mut()) }
}

extern "C" fn rust_contact_relationship(this: &RustContact, _cmd: Sel) -> *mut NSNumber {
    unsafe { this.get_ivar::<*mut NSNumber>("_relationship").unwrap_or(ptr::null_mut()) }
}

/// Setter‑ы с вызовом KVO уведомлений
extern "C" fn rust_contact_set_first_name(this: &mut RustContact, _cmd: Sel, new_first_name: *mut NSString) {
    unsafe {
        // Для вызова willChangeValueForKey:/didChangeValueForKey: используем CString:
        let key = std::ffi::CString::new("firstName").unwrap();
        let _: () = msg_send![this, willChangeValueForKey:key.as_ptr()];
        this.set_ivar("_firstName", new_first_name);
        let _: () = msg_send![this, didChangeValueForKey:key.as_ptr()];
    }
}

extern "C" fn rust_contact_set_last_name(this: &mut RustContact, _cmd: Sel, new_last_name: *mut NSString) {
    unsafe {
        let key = std::ffi::CString::new("lastName").unwrap();
        let _: () = msg_send![this, willChangeValueForKey:key.as_ptr()];
        this.set_ivar("_lastName", new_last_name);
        let _: () = msg_send![this, didChangeValueForKey:key.as_ptr()];
    }
}

extern "C" fn rust_contact_set_relationship(this: &mut RustContact, _cmd: Sel, new_rel: *mut NSNumber) {
    unsafe {
        let key = std::ffi::CString::new("relationship").unwrap();
        let _: () = msg_send![this, willChangeValueForKey:key.as_ptr()];
        this.set_ivar("_relationship", new_rel);
        let _: () = msg_send![this, didChangeValueForKey:key.as_ptr()];
    }
}

/// Функция для создания нового экземпляра RustContact из внутреннего типа Contact.
/// При вызове эта функция заполняет объект значениями, полученными через наши конвертеры,
/// что позволит Swift наблюдать за изменениями через KVO.
pub fn contact_to_objc(contact: &Contact) -> *mut RustContact {
    let cls = register_rust_contact_class();
    unsafe {
        let obj: *mut RustContact = msg_send![cls, new];

        // Устанавливаем поля через set_ivar или через методы-сеттеры, если хотим генерировать KVO:
        let id_nsdata = convert_to_nsdata(contact.id.as_bytes().to_vec());
        (&mut (*obj).superclass)
            .set_ivar(CStr::from_bytes_with_nul(b"_id\0").unwrap(), id_nsdata);

        let first_name = convert_to_nsstring(contact.first_name.clone());
        let _: () = msg_send![obj, setFirstName:first_name];

        let last_name = convert_to_nsstring(contact.last_name.clone());
        let _: () = msg_send![obj, setLastName:last_name];

        // Предполагаем наличие метода `NSNumber::new_i64(...)` в используемой версии objc2_foundation
        let rel_num = NSNumber::new_i64(contact.relationship);
        let _: () = msg_send![obj, setRelationship:rel_num];

        obj
    }
}