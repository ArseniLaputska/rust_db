// use std::ffi::{CStr, CString};
// use std::os::raw::c_char;
// use rusqlite::Connection;
// use uuid::Uuid;
// use base64;
//
// // Импорт необходимых типов и функций
// use serde::{Serialize, Deserialize};
// use serde_json;
//
// #[no_mangle]
// pub extern "C" fn rust_add_contact_pb_json(json_ptr: *const c_char) -> bool {
//     if json_ptr.is_null() {
//         return false;
//     }
//
//     let c_str = unsafe { CStr::from_ptr(json_ptr) };
//     let json_str = match c_str.to_str() {
//         Ok(s) => s,
//         Err(_) => return false,
//     };
//
//     let contact: Contact = match serde_json::from_str(json_str) {
//         Ok(c) => c,
//         Err(_) => return false,
//     };
//
//     let conn = match Connection::open("contacts.db") {
//         Ok(c) => c,
//         Err(_) => return false,
//     };
//
//     let repo = ContactRepo::new(&conn);
//
//     match repo.add_contact_pb_json(&contact) {
//         Ok(_) => true,
//         Err(_) => false,
//     }
// }
//
// #[no_mangle]
// pub extern "C" fn rust_get_contact_json(id_ptr: *const c_char) -> *mut c_char {
//     if id_ptr.is_null() {
//         return std::ptr::null_mut();
//     }
//
//     let c_str = unsafe { CStr::from_ptr(id_ptr) };
//     let id_str = match c_str.to_str() {
//         Ok(s) => s,
//         Err(_) => return std::ptr::null_mut(),
//     };
//
//     let id = match Uuid::parse_str(id_str) {
//         Ok(uuid) => uuid,
//         Err(_) => return std::ptr::null_mut(),
//     };
//
//     let conn = match Connection::open("contacts.db") {
//         Ok(c) => c,
//         Err(_) => return std::ptr::null_mut(),
//     };
//
//     let repo = ContactRepo::new(&conn);
//
//     match repo.get_contact(id) {
//         Ok(Some(contact)) => {
//             match serde_json::to_string(&contact) {
//                 Ok(json) => {
//                     match CString::new(json) {
//                         Ok(c_string) => c_string.into_raw(),
//                         Err(_) => std::ptr::null_mut(),
//                     }
//                 },
//                 Err(_) => std::ptr::null_mut(),
//             }
//         },
//         _ => std::ptr::null_mut(),
//     }
// }
//
// #[no_mangle]
// pub extern "C" fn rust_update_contact_relationship(id_ptr: *const c_char, relationship: i64) -> bool {
//     if id_ptr.is_null() {
//         return false;
//     }
//
//     let c_str = unsafe { CStr::from_ptr(id_ptr) };
//     let id_str = match c_str.to_str() {
//         Ok(s) => s,
//         Err(_) => return false,
//     };
//
//     let id = match Uuid::parse_str(id_str) {
//         Ok(uuid) => uuid,
//         Err(_) => return false,
//     };
//
//     let conn = match Connection::open("contacts.db") {
//         Ok(c) => c,
//         Err(_) => return false,
//     };
//
//     let repo = ContactRepo::new(&conn);
//
//     match repo.update_contact_relationship(id, relationship, None) {
//         Ok(_) => true,
//         Err(e) => {
//             eprintln!("Ошибка при обновлении отношения контакта: {}", e);
//             false
//         }
//     }
// }
//
// #[no_mangle]
// pub extern "C" fn rust_delete_contact(id_ptr: *const c_char) -> bool {
//     if id_ptr.is_null() {
//         return false;
//     }
//
//     let c_str = unsafe { CStr::from_ptr(id_ptr) };
//     let id_str = match c_str.to_str() {
//         Ok(s) => s,
//         Err(_) => return false,
//     };
//
//     let id = match Uuid::parse_str(id_str) {
//         Ok(uuid) => uuid,
//         Err(_) => return false,
//     };
//
//     let conn = match Connection::open("contacts.db") {
//         Ok(c) => c,
//         Err(_) => return false,
//     };
//
//     let repo = ContactRepo::new(&conn);
//
//     match repo.delete_contact(id) {
//         Ok(_) => true,
//         Err(e) => {
//             eprintln!("Ошибка при удалении контакта: {}", e);
//             false
//         }
//     }
// }
//
// #[no_mangle]
// pub extern "C" fn rust_get_image_data(id_ptr: *const c_char) -> *mut c_char {
//     if id_ptr.is_null() {
//         return std::ptr::null_mut();
//     }
//
//     let c_str = unsafe { CStr::from_ptr(id_ptr) };
//     let id_str = match c_str.to_str() {
//         Ok(s) => s,
//         Err(_) => return std::ptr::null_mut(),
//     };
//
//     let id = match Uuid::parse_str(id_str) {
//         Ok(uuid) => uuid,
//         Err(_) => return std::ptr::null_mut(),
//     };
//
//     let conn = match Connection::open("contacts.db") {
//         Ok(c) => c,
//         Err(_) => return std::ptr::null_mut(),
//     };
//
//     let repo = ContactRepo::new(&conn);
//
//     match repo.get_image_data(id) {
//         Ok(Some(data)) => {
//             let base64_str = base64::encode(&data);
//             match CString::new(base64_str) {
//                 Ok(c_string) => c_string.into_raw(),
//                 Err(_) => std::ptr::null_mut(),
//             }
//         },
//         Ok(None) | Err(_) => std::ptr::null_mut(),
//     }
// }
//
// #[no_mangle]
// pub extern "C" fn rust_free_cstring(ptr: *mut c_char) {
//     if !ptr.is_null() {
//         unsafe {
//             let _ = CString::from_raw(ptr);
//         }
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use rusqlite::Connection;
//     use serde_json::json;
//     use std::ffi::CString;
//
//     fn setup_db() -> Result<Connection, rusqlite::Error> {
//         let conn = Connection::open_in_memory()?;
//         crate::db::contact::create_contact_table(&conn)?;
//         Ok(conn)
//     }
//
//     #[test]
//     fn test_add_contact() -> Result<(), Box<dyn std::error::Error>> {
//         let id = Uuid::now_v7();
//         let contact_json = json!({
//             "id": id.to_string(),
//             "first_name": "John",
//             "last_name": "Doe",
//             "relationship": 1,
//             "username": "jdoe",
//             "language": "en",
//             "picture_url": "http://example.com/img",
//             "last_message_at": null,
//             "created_at": 1234567890,
//             "updated_at": 1234567890,
//             "is_pro": 1
//         }).to_string();
//
//         let c_json = CString::new(contact_json)?;
//         assert!(rust_add_contact_pb_json(c_json.as_ptr()));
//         Ok(())
//     }
//
//     #[test]
//     fn test_get_contact() -> Result<(), Box<dyn std::error::Error>> {
//         let id = Uuid::now_v7();
//
//         // Сначала добавляем контакт
//         let contact_json = json!({
//             "id": id.to_string(),
//             "first_name": "John",
//             "last_name": "Doe",
//             "relationship": 1,
//             "username": "jdoe",
//             "language": "en",
//             "picture_url": "http://example.com/img",
//             "last_message_at": null,
//             "created_at": 1234567890,
//             "updated_at": 1234567890,
//             "is_pro": 1
//         }).to_string();
//
//         let c_json = CString::new(contact_json.clone())?;
//         assert!(rust_add_contact_pb_json(c_json.as_ptr()));
//
//         // Затем получаем его
//         let c_id = CString::new(id.to_string())?;
//         let result_ptr = rust_get_contact_json(c_id.as_ptr());
//         assert!(!result_ptr.is_null());
//
//         let result_str = unsafe { CStr::from_ptr(result_ptr) }.to_str()?;
//         let result_json: serde_json::Value = serde_json::from_str(result_str)?;
//
//         assert_eq!(result_json["first_name"], "John");
//         assert_eq!(result_json["last_name"], "Doe");
//         assert_eq!(result_json["username"], "jdoe");
//
//         unsafe { rust_free_cstring(result_ptr) };
//         Ok(())
//     }
//
//     #[test]
//     fn test_update_relationship() -> Result<(), Box<dyn std::error::Error>> {
//         let id = Uuid::now_v7();
//
//         // Добавляем контакт
//         let contact_json = json!({
//             "id": id.to_string(),
//             "first_name": "John",
//             "last_name": "Doe",
//             "relationship": 1,
//             "username": "jdoe",
//             "language": "en",
//             "picture_url": "http://example.com/img",
//             "last_message_at": null,
//             "created_at": 1234567890,
//             "updated_at": 1234567890,
//             "is_pro": 1
//         }).to_string();
//
//         let c_json = CString::new(contact_json)?;
//         assert!(rust_add_contact_pb_json(c_json.as_ptr()));
//
//         // Обновляем отношение
//         let c_id = CString::new(id.to_string())?;
//         assert!(rust_update_contact_relationship(c_id.as_ptr(), 2));
//
//         // Проверяем обновление
//         let c_id_check = CString::new(id.to_string())?;
//         let result_ptr = rust_get_contact_json(c_id_check.as_ptr());
//         assert!(!result_ptr.is_null());
//
//         let result_str = unsafe { CStr::from_ptr(result_ptr) }.to_str()?;
//         let result_json: serde_json::Value = serde_json::from_str(result_str)?;
//         assert_eq!(result_json["relationship"], 2);
//
//         unsafe { rust_free_cstring(result_ptr) };
//         Ok(())
//     }
//
//     #[test]
//     fn test_delete_contact() -> Result<(), Box<dyn std::error::Error>> {
//         let id = Uuid::now_v7();
//
//         // Добавляем контакт
//         let contact_json = json!({
//             "id": id.to_string(),
//             "first_name": "John",
//             "last_name": "Doe",
//             "relationship": 1,
//             "username": "jdoe",
//             "language": "en",
//             "picture_url": "http://example.com/img",
//             "last_message_at": null,
//             "created_at": 1234567890,
//             "updated_at": 1234567890,
//             "is_pro": 1
//         }).to_string();
//
//         let c_json = CString::new(contact_json)?;
//         assert!(rust_add_contact_pb_json(c_json.as_ptr()));
//
//         // Удаляем контакт
//         let c_id = CString::new(id.to_string())?;
//         assert!(rust_delete_contact(c_id.as_ptr()));
//
//         // Проверяем что контакт удалён
//         let c_id_check = CString::new(id.to_string())?;
//         let result_ptr = rust_get_contact_json(c_id_check.as_ptr());
//         assert!(result_ptr.is_null());
//
//         Ok(())
//     }
//
//     #[test]
//     fn test_get_image_data() -> Result<(), Box<dyn std::error::Error>> {
//         let id = Uuid::now_v7();
//
//         // Добавляем контакт с изображением
//         let contact_json = json!({
//             "id": id.to_string(),
//             "first_name": "John",
//             "last_name": "Doe",
//             "relationship": 1,
//             "username": "jdoe",
//             "language": "en",
//             "picture_url": "http://example.com/img",
//             "picture_data": [1, 2, 3, 4, 5],
//             "last_message_at": null,
//             "created_at": 1234567890,
//             "updated_at": 1234567890,
//             "is_pro": 1
//         }).to_string();
//
//         let c_json = CString::new(contact_json)?;
//         assert!(rust_add_contact_pb_json(c_json.as_ptr()));
//
//         // Получаем изображение
//         let c_id = CString::new(id.to_string())?;
//         let result_ptr = rust_get_image_data(c_id.as_ptr());
//         assert!(!result_ptr.is_null());
//
//         let image_str = unsafe { CStr::from_ptr(result_ptr) }.to_str()?;
//         let decoded_image = base64::decode(image_str)?;
//         assert_eq!(decoded_image, vec![1, 2, 3, 4, 5]);
//
//         unsafe { rust_free_cstring(result_ptr) };
//         Ok(())
//     }
//
//     #[test]
//     fn test_invalid_uuid() {
//         let invalid_id = CString::new("invalid-uuid").unwrap();
//
//         assert!(!rust_delete_contact(invalid_id.as_ptr()));
//         assert!(rust_get_contact_json(invalid_id.as_ptr()).is_null());
//         assert!(rust_get_image_data(invalid_id.as_ptr()).is_null());
//         assert!(!rust_update_contact_relationship(invalid_id.as_ptr(), 1));
//     }
//
//     #[test]
//     fn test_null_pointer() {
//         assert!(!rust_delete_contact(std::ptr::null()));
//         assert!(rust_get_contact_json(std::ptr::null()).is_null());
//         assert!(rust_get_image_data(std::ptr::null()).is_null());
//         assert!(!rust_update_contact_relationship(std::ptr::null(), 1));
//         assert!(!rust_add_contact_pb_json(std::ptr::null()));
//     }
//
//     #[test]
//     fn test_invalid_json() {
//         let invalid_json = CString::new("{invalid_json}").unwrap();
//         assert!(!rust_add_contact_pb_json(invalid_json.as_ptr()));
//     }
// }
