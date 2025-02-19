// src/db/contacts_store.rs

use objc2::declare::ClassDecl;
use objc2_foundation::{NSObject, NSArray, NSMutableArray};
use objc2::runtime::{Sel, AnyClass, AnyObject, Object};
use objc2::{msg_send, sel, Encode, Encoding, RefEncode, Message};
use std::ptr;
use std::sync::Once;
use std::ffi::{CString, CStr};
use crate::db::objc_contact::{RustContact};

extern "C" {
    fn object_getInstanceVariable(
        obj: *mut Object,
        name: *const i8,
        out_val: *mut *mut std::os::raw::c_void
    ) -> *mut std::os::raw::c_void;

    fn object_setInstanceVariable(
        obj: *mut Object,
        name: *const i8,
        value: *mut std::os::raw::c_void
    ) -> *mut std::os::raw::c_void;
}

unsafe fn get_ivar_raw<T>(obj: *mut Object, ivar_name: &str) -> *mut T {
    let c_name = CString::new(ivar_name).unwrap();
    let mut out_val: *mut std::os::raw::c_void = std::ptr::null_mut();
    object_getInstanceVariable(obj, c_name.as_ptr(), &mut out_val);
    out_val as *mut T
}

unsafe fn set_ivar_raw<T>(obj: *mut Object, ivar_name: &str, value: *mut T) {
    let c_name = CString::new(ivar_name).unwrap();
    let _old_val = object_setInstanceVariable(obj, c_name.as_ptr(), value as *mut _);
}

// Регистрация класса ContactsStore (наследника NSObject), который хранит массив контактов.
static CONTACTS_STORE_REGISTER: Once = Once::new();
static mut CONTACTS_STORE_CLASS: *const objc2::runtime::Class = ptr::null();

/// Регистрирует класс "ContactsStore" с одним ivar‑ом "_contacts" (NSMutableArray)
/// и добавляет геттер и сеттер для свойства "contacts" с KVO‑уведомлениями.
pub fn register_contacts_store_class() -> &'static AnyClass {
    CONTACTS_STORE_REGISTER.call_once(|| {
        let nsobject_class = AnyClass::get(CStr::from_bytes_with_nul(b"NSObject\0").unwrap())
            .expect("NSObject class not found");
        let class_name = CStr::from_bytes_with_nul(b"ContactsStore\0").unwrap();

        let mut decl = ClassDecl::new(class_name, nsobject_class)
            .expect("Failed to declare ContactsStore class");

        // Добавляем ivar "_contacts"
        decl.add_ivar::<*mut NSMutableArray>(CStr::from_bytes_with_nul(b"_contacts\0").unwrap());

        unsafe {
            // Методы объявляем как extern "C" fn(*mut ContactsStore, Sel, ...)
            decl.add_method(
                sel!(contacts),
                contacts_getter as extern "C" fn(*mut ContactsStore, Sel) -> *mut NSArray,
            );
            decl.add_method(
                sel!(setContacts:),
                contacts_setter as extern "C" fn(*mut ContactsStore, Sel, *mut NSArray),
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
// Исправляем ENCODING и реализуем необходимые трейты.
unsafe impl Encode for ContactsStore {
    // Вместо &str указываем Encoding
    const ENCODING: Encoding = Encoding::Struct("{ContactsStore=}", &[]);
}
unsafe impl RefEncode for ContactsStore {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}
unsafe impl Message for ContactsStore {}

/// Геттер для свойства "contacts"
extern "C" fn contacts_getter(this: *mut ContactsStore, _cmd: Sel) -> *mut NSArray {
    unsafe {
        let obj_ptr = &mut (*this).superclass as *mut NSObject as *mut Object;

        // Читаем ivar "_contacts"
        let arr_ptr = get_ivar_raw::<NSMutableArray>(obj_ptr, "_contacts");
        if arr_ptr.is_null() {
            ptr::null_mut()
        } else {
            arr_ptr as *mut NSArray
        }
    }
}

/// Сеттер для свойства "contacts" с обёрткой KVO (will/didChangeValueForKey:)

extern "C" fn contacts_setter(this: *mut ContactsStore, _cmd: Sel, new_contacts: *mut NSArray) {
    unsafe {
        let key = CString::new("contacts").unwrap();
        let obj_ptr = &mut (*this).superclass as *mut NSObject as *mut Object;

        // willChangeValueForKey:
        let _: () = msg_send![obj_ptr, willChangeValueForKey: key.as_ptr()];

        // Записываем в ivar "_contacts"
        let new_mmarr = new_contacts as *mut NSMutableArray;
        set_ivar_raw(obj_ptr, "_contacts", new_mmarr);

        // didChangeValueForKey:
        let _: () = msg_send![obj_ptr, didChangeValueForKey: key.as_ptr()];
    }
}

/// Создает и возвращает новый экземпляр ContactsStore с инициализированным пустым массивом контактов.
pub fn new_contacts_store() -> *mut ContactsStore {
    let cls = register_contacts_store_class();
    unsafe {
        let store: *mut ContactsStore = msg_send![cls, new];

        // Создаём пустой NSMutableArray
        let nsma_class = AnyClass::get(CStr::from_bytes_with_nul(b"NSMutableArray\0").unwrap())
            .expect("NSMutableArray class not found");
        let empty_arr: *mut NSMutableArray = msg_send![nsma_class, alloc];
        let empty_arr: *mut NSMutableArray = msg_send![empty_arr, init];

        // Пишем в ivar
        let obj_ptr = &mut (*store).superclass as *mut NSObject as *mut Object;
        set_ivar_raw(obj_ptr, "_contacts", empty_arr);

        store
    }
}

/// Обновляет массив контактов в ContactsStore. При вызове setter будут отправлены KVO‑уведомления.
/// Принимается вектор указателей на объекты RustContact (которые являются нашими представлениями контактов).
pub fn update_contacts(store: *mut ContactsStore, contacts: Vec<*mut RustContact>) {
    unsafe {
        let nsma_class = AnyClass::get(CStr::from_bytes_with_nul(b"NSMutableArray\0").unwrap())
            .expect("NSMutableArray class not found");

        let arr: *mut NSMutableArray = msg_send![nsma_class, alloc];
        let arr: *mut NSMutableArray = msg_send![arr, init];

        // Добавляем объекты
        for c in contacts {
            // RustContact -> superclass -> *mut NSObject
            let c_obj = c as *mut NSObject;
            let _: () = msg_send![arr, addObject: c_obj];
        }
        // Вызываем сеттер (setContacts:) => KVO
        let _: () = msg_send![store, setContacts: arr];
    }
}
