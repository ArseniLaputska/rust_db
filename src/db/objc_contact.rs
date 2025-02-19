use objc2::declare::ClassDecl;
use objc2::runtime::{AnyClass, Sel, AnyObject};
use objc2_foundation::{NSObject, NSString, NSData, NSNumber};
use objc2::{msg_send, sel, Encode, Encoding, RefEncode, Message};
use objc2::rc::Retained;
use std::ptr;
use std::sync::Once;
use std::ffi::{CString, CStr};
use uuid::Uuid;
use crate::db::contact::Contact;
use crate::db::objc_converters::{convert_to_nsdata, convert_to_nsstring};

// Реализуем трейты для RustContact
unsafe impl Encode for RustContact {
    const ENCODING: Encoding = Encoding::Struct("{RustContact=}", &[]);
}
unsafe impl RefEncode for RustContact {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}
unsafe impl Message for RustContact {}

static REGISTER: Once = Once::new();
static mut RUST_CONTACT_CLASS: *const AnyClass = ptr::null();

/// Регистрирует класс RustContact (наследник NSObject) с динамическими свойствами.
/// Свойства: _id, _firstName, _lastName, _relationship.
pub fn register_rust_contact_class() -> &'static AnyClass {
    REGISTER.call_once(|| {
        // Получаем класс NSObject – передаём именно &CStr.
        let nsobject_name = CStr::from_bytes_with_nul(b"NSObject\0").unwrap();
        let nsobject_class = AnyClass::get(nsobject_name)
            .expect("NSObject class not found");
        let class_name = CStr::from_bytes_with_nul(b"RustContact\0").unwrap();

        let mut decl = ClassDecl::new(class_name, nsobject_class)
            .expect("Failed to declare RustContact class");

        // Добавляем ivar‑ы; передаём имена как &CStr
        decl.add_ivar::<*mut NSData>(CStr::from_bytes_with_nul(b"_id\0").unwrap());
        decl.add_ivar::<*mut NSString>(CStr::from_bytes_with_nul(b"_firstName\0").unwrap());
        decl.add_ivar::<*mut NSString>(CStr::from_bytes_with_nul(b"_lastName\0").unwrap());
        decl.add_ivar::<*mut NSNumber>(CStr::from_bytes_with_nul(b"_relationship\0").unwrap());

        unsafe {
            // Регистрируем методы. Функции теперь имеют сигнатуру с параметром *mut RustContact.
            decl.add_method(
                sel!(id),
                rust_contact_id as extern "C" fn(*mut RustContact, Sel) -> *mut NSData,
            );
            decl.add_method(
                sel!(firstName),
                rust_contact_first_name as extern "C" fn(*mut RustContact, Sel) -> *mut NSString,
            );
            decl.add_method(
                sel!(lastName),
                rust_contact_last_name as extern "C" fn(*mut RustContact, Sel) -> *mut NSString,
            );
            decl.add_method(
                sel!(relationship),
                rust_contact_relationship as extern "C" fn(*mut RustContact, Sel) -> *mut NSNumber,
            );
            decl.add_method(
                sel!(setFirstName:),
                rust_contact_set_first_name as extern "C" fn(*mut RustContact, Sel, *mut NSString),
            );
            decl.add_method(
                sel!(setLastName:),
                rust_contact_set_last_name as extern "C" fn(*mut RustContact, Sel, *mut NSString),
            );
            decl.add_method(
                sel!(setRelationship:),
                rust_contact_set_relationship as extern "C" fn(*mut RustContact, Sel, *mut NSNumber),
            );
        }

        unsafe {
            RUST_CONTACT_CLASS = decl.register();
        }
    });
    unsafe { &*RUST_CONTACT_CLASS }
}

/// Представление RustContact в Rust.
/// Поле superclass хранит объект NSObject.
#[repr(C)]
pub struct RustContact {
    pub superclass: NSObject,
}

/// Helper: получение значения через KVC (valueForKey:).
/// Ограничение T: RefEncode добавлено для устранения ошибки.
unsafe fn get_value_for_key<T: RefEncode>(obj: &NSObject, key: &str) -> Option<*mut T> {
    let key_c = CString::new(key).unwrap();
    log::debug!("get_value_for_key: key = {:?}", key_c);
    let result: *mut T = msg_send![obj, valueForKey: key_c.as_ptr()];
    log::debug!("get_value_for_key: result = {:?}", result);
    if result.is_null() {
        None
    } else {
        Some(result)
    }
}

/// Helper: установка значения через KVC (setValue:forKey:).
/// Чтобы избежать ошибки MessageReceiver для &mut NSObject, приводим к &NSObject.
unsafe fn set_value_for_key(obj: &mut NSObject, key: &str, value: *mut std::os::raw::c_void) {
    let key_c = CString::new(key).unwrap();
    log::debug!("set_value_for_key: key = {:?}", key_c);
    // Приводим &mut NSObject к &NSObject:
    let obj_imm: &NSObject = &*obj;
    let _: () = msg_send![obj_imm, setValue: value forKey: key_c.as_ptr()];
}

/// Геттеры: получаем значения через KVC.
extern "C" fn rust_contact_id(this: *mut RustContact, _cmd: Sel) -> *mut NSData {
    unsafe {
        match get_value_for_key::<NSData>(&(*this).superclass, "_id") {
            Some(ptr) => ptr,
            None => ptr::null_mut(),
        }
    }
}

extern "C" fn rust_contact_first_name(this: *mut RustContact, _cmd: Sel) -> *mut NSString {
    unsafe {
        match get_value_for_key::<NSString>(&(*this).superclass, "_firstName") {
            Some(ptr) => ptr,
            None => ptr::null_mut(),
        }
    }
}

extern "C" fn rust_contact_last_name(this: *mut RustContact, _cmd: Sel) -> *mut NSString {
    unsafe {
        match get_value_for_key::<NSString>(&(*this).superclass, "_lastName") {
            Some(ptr) => ptr,
            None => ptr::null_mut(),
        }
    }
}

extern "C" fn rust_contact_relationship(this: *mut RustContact, _cmd: Sel) -> *mut NSNumber {
    unsafe {
        match get_value_for_key::<NSNumber>(&(*this).superclass, "_relationship") {
            Some(ptr) => ptr,
            None => ptr::null_mut(),
        }
    }
}

/// Сеттеры с KVO уведомлениями (через KVC).
extern "C" fn rust_contact_set_first_name(this: *mut RustContact, _cmd: Sel, new_first_name: *mut NSString) {
    unsafe {
        log::debug!("rust_contact_set_first_name: Устанавливаем firstName");
        let key = CString::new("firstName").unwrap();
        // Приводим &mut NSObject к &NSObject:
        let superclass_ref: &NSObject = &(*this).superclass;
        let _: () = msg_send![superclass_ref, willChangeValueForKey: key.as_ptr()];
        // Для установки значения используем нашу helper-функцию:
        set_value_for_key(&mut (*this).superclass, "_firstName", new_first_name as *mut _);
        let _: () = msg_send![superclass_ref, didChangeValueForKey: key.as_ptr()];
    }
}

extern "C" fn rust_contact_set_last_name(this: *mut RustContact, _cmd: Sel, new_last_name: *mut NSString) {
    unsafe {
        log::debug!("rust_contact_set_last_name: Устанавливаем lastName");
        let key = CString::new("lastName").unwrap();
        let superclass_ref: &NSObject = &(*this).superclass;
        let _: () = msg_send![superclass_ref, willChangeValueForKey: key.as_ptr()];
        set_value_for_key(&mut (*this).superclass, "_lastName", new_last_name as *mut _);
        let _: () = msg_send![superclass_ref, didChangeValueForKey: key.as_ptr()];
    }
}

extern "C" fn rust_contact_set_relationship(this: *mut RustContact, _cmd: Sel, new_rel: *mut NSNumber) {
    unsafe {
        log::debug!("rust_contact_set_relationship: Устанавливаем relationship");
        let key = CString::new("relationship").unwrap();
        let superclass_ref: &NSObject = &(*this).superclass;
        let _: () = msg_send![superclass_ref, willChangeValueForKey: key.as_ptr()];
        set_value_for_key(&mut (*this).superclass, "_relationship", new_rel as *mut _);
        let _: () = msg_send![superclass_ref, didChangeValueForKey: key.as_ptr()];
    }
}

/// Функция создания нового объекта RustContact из внутреннего типа Contact.
pub fn contact_to_objc(contact: &Contact) -> *mut RustContact {
    log::debug!("contact_to_objc: Создаём RustContact для контакта: {:?}", contact);
    let cls = register_rust_contact_class();
    unsafe {
        let obj: *mut RustContact = msg_send![cls, new];

        // Устанавливаем _id через KVC
        let id_nsdata = convert_to_nsdata(contact.id.as_bytes().to_vec());
        {
            // Вместо &mut NSObject используем *mut AnyObject
            let obj_super: *mut AnyObject =
                &mut (*obj).superclass as *mut NSObject as *mut AnyObject;

            let key = CStr::from_bytes_with_nul(b"_id\0").unwrap();
            log::debug!("contact_to_objc: Устанавливаем _id");
            let _: () = msg_send![obj_super, setValue: id_nsdata forKey: key.as_ptr()];
        }

        let first_name = convert_to_nsstring(contact.first_name.clone());
        log::debug!("contact_to_objc: Устанавливаем firstName");
        let _: () = msg_send![obj, setFirstName: first_name];

        let last_name = convert_to_nsstring(contact.last_name.clone());
        log::debug!("contact_to_objc: Устанавливаем lastName");
        let _: () = msg_send![obj, setLastName: last_name];

        let superclass_ptr: *mut AnyObject = &mut (*obj).superclass as *mut NSObject as *mut AnyObject;

        let rel_num = NSNumber::new_i64(contact.relationship);
        let rel_ptr: *mut NSNumber = Retained::into_raw(rel_num);
        let key = CString::new("_relationship").unwrap();
        let _: () = msg_send![superclass_ptr, setValue: rel_ptr forKey: key.as_ptr()];

        let obj_super2: *mut AnyObject =
            &mut (*obj).superclass as *mut NSObject as *mut AnyObject;

        let _: () = msg_send![obj_super2, setValue: rel_ptr forKey: key.as_ptr()];

        obj
    }
}
