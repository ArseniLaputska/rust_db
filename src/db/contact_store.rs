// src/db/contacts_store.rs

use objc2::declare::ClassDecl;
// Не используем импорт из objc2::runtime::ClassType – вместо этого для вызова методов класса используем методы из objc2_foundation.
// Импорт макроса sel! для построения селекторов:
use objc2::sel;
use objc2_foundation::{NSObject, NSArray, NSMutableArray, NSString};
use objc2::runtime::{Sel, Ivar, Class, Object};
use objc2::msg_send;
use std::ptr;
use std::sync::Once;
use std::ffi::{CString, CStr};
use once_cell::sync::Lazy;
use crate::db::objc_contact::{RustContact, register_rust_contact_class};
use crate::db::objc_converters::{convert_to_nsdata, convert_to_nsstring};

// Регистрация класса ContactsStore (наследника NSObject), который хранит массив контактов.
static CONTACTS_STORE_REGISTER: Once = Once::new();
static mut CONTACTS_STORE_CLASS: *const objc2::runtime::Class = ptr::null();

/// Регистрирует класс "ContactsStore" с одним ivar‑ом "_contacts" (NSMutableArray)
/// и добавляет геттер и сеттер для свойства "contacts" с KVO‑уведомлениями.
pub fn register_contacts_store_class() -> &'static Class {
    CONTACTS_STORE_REGISTER.call_once(|| {
        // Получаем класс NSObject через runtime: передаем имя как &CStr
        let nsobject_class = Class::get(CStr::from_bytes_with_nul(b"NSObject\0").unwrap())
            .expect("NSObject class not found");
        let class_name = CStr::from_bytes_with_nul(b"ContactsStore\0").unwrap();
        let mut decl = ClassDecl::new(class_name, nsobject_class)
            .expect("Failed to declare ContactsStore class");

        // Добавляем ivar "_contacts". Имя ivar передаем как &CStr.
        decl.add_ivar::<*mut NSMutableArray>(CStr::from_bytes_with_nul(b"_contacts\0").unwrap());

        unsafe {
            decl.add_method(
                sel!(contacts),
                contacts_getter as extern "C" fn(&ContactsStore, _cmd: _ ) -> *mut NSArray,
            );
            decl.add_method(
                sel!(setContacts:),
                contacts_setter as extern "C" fn(&mut ContactsStore, _cmd: _, *mut NSArray),
            );
        }

        unsafe {
            CONTACTS_STORE_CLASS = decl.register();
        }
    });
    unsafe { &*CONTACTS_STORE_CLASS }
}

/// Представление класса ContactsStore в Rust.
/// Поля не объявляются напрямую, данные хранятся в ivar "_contacts".
#[repr(C)]
pub struct ContactsStore {
    pub superclass: NSObject,
}

// Для того чтобы ContactsStore можно было использовать в FFI,
// возможно потребуется реализовать Encode. Например, вручную:
unsafe impl objc2::Encode for ContactsStore {
    const ENCODING: &'static str = "{ContactsStore=#}";
}

/// Геттер для свойства "contacts"
extern "C" fn contacts_getter(this: &ContactsStore, _cmd: Sel) -> *mut NSArray {
    unsafe {
        // Получаем значение ivar "_contacts"
        this.get_ivar::<*mut NSMutableArray>("_contacts")
            .map(|p| p as *mut NSArray)
            .unwrap_or(ptr::null_mut())
    }
}

/// Сеттер для свойства "contacts" с обёрткой KVO (will/didChangeValueForKey:)
extern "C" fn contacts_setter(this: &mut ContactsStore, _cmd: Sel, new_contacts: *mut NSArray) {
    unsafe {
        let key = CString::new("contacts").unwrap();
        let _: () = msg_send![this, willChangeValueForKey:key.as_ptr()];
        this.set_ivar("_contacts", new_contacts as *mut NSMutableArray);
        let _: () = msg_send![this, didChangeValueForKey:key.as_ptr()];
    }
}

/// Создает и возвращает новый экземпляр ContactsStore с инициализированным пустым массивом контактов.
pub fn new_contacts_store() -> *mut ContactsStore {
    let cls = register_contacts_store_class();
    unsafe {
        let store: *mut ContactsStore = msg_send![cls, new];
        // Получаем класс NSMutableArray через runtime
        let nsmutablearray_class = Class::get(CStr::from_bytes_with_nul(b"NSMutableArray\0").unwrap())
            .expect("NSMutableArray class not found");
        let empty_array: *mut NSMutableArray = msg_send![nsmutablearray_class, alloc];
        let empty_array: *mut NSMutableArray = msg_send![empty_array, init];
        (&mut (*store).superclass)
            .set_ivar(CStr::from_bytes_with_nul(b"_contacts\0").unwrap(), empty_array);
        store
    }
}

/// Обновляет массив контактов в ContactsStore. При вызове setter будут отправлены KVO‑уведомления.
/// Принимается вектор указателей на объекты RustContact (которые являются нашими представлениями контактов).
pub fn update_contacts(store: *mut ContactsStore, contacts: Vec<*mut RustContact>) {
    unsafe {
        let nsmutablearray_class = Class::get(CStr::from_bytes_with_nul(b"NSMutableArray\0").unwrap())
            .expect("NSMutableArray class not found");
        let ns_mut_array: *mut NSMutableArray = msg_send![nsmutablearray_class, alloc];
        let ns_mut_array: *mut NSMutableArray = msg_send![ns_mut_array, init];
        for contact in contacts {
            // Приводим RustContact к NSObject (первое поле – superclass)
            let obj: *mut NSObject = contact as *mut NSObject;
            let _: () = msg_send![ns_mut_array, addObject: obj];
        }
        let _: () = msg_send![store, setContacts: ns_mut_array];
    }
}
