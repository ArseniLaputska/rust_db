// src/lib.rs

#![allow(unused_imports, unused_mut, unused_variables)]
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use tokio_rusqlite::{Connection, OpenFlags, Result as SqlResult, Error as TRusqliteError};
use log::{info, error, warn};
use uuid::Uuid;

mod db;
use db::objc_converters::*;
use db::monitor::*;
use crate::db::migrations::setup_migrations;

use crate::db::contact::*;
use crate::db::contacts_store::*;
// use crate::db::contact_book::ContactBookRepo;
use crate::db::contact_seen_at::ContactSeenAtRepo;
use crate::db::contact_status::ContactStatusRepo;
use crate::db::message::MessageRepo;

// ---------------------- Глобальные объекты ----------------------

/// Глобальное хранилище асинхронного соединения
/// (Мы храним Option<Arc<Connection>>, чтобы быть гибкими)
static GLOBAL_CONN: Lazy<Mutex<Option<Arc<tokio_rusqlite::Connection>>>> = Lazy::new(|| Mutex::new(None));

/// Swift callback (указатель на функцию) — global
static mut SWIFT_CALLBACK: Option<extern "C" fn(*const c_char)> = None;

/// Для хранения событий, пойманных из preupdate_hook, делаем mpsc
use std::sync::mpsc::{self, Sender, Receiver};

// Глобальная очередь PreUpdateEvent
//  - (Sender<PreUpdateEvent>, Mutex<Receiver<PreUpdateEvent>>)
// static EVENT_CHANNEL: Lazy<(Sender<PreUpdateEvent>, Mutex<Receiver<PreUpdateEvent>>)> = Lazy::new(|| {
//     let (tx, rx) = mpsc::channel::<PreUpdateEvent>();
//     (tx, Mutex::new(rx))
// });
//
// use std::sync::mpsc::{self, Sender, Receiver};

/// Версия схемы (example)
const LATEST_SCHEMA_VERSION: i32 = 1;

// ---------------------- Экспортируемые функции ----------------------


#[no_mangle]
pub extern "C" fn swift_main(
    db_path: *const c_char,
    db_key: *const c_char,
    callback: extern "C" fn(*const c_char)
) -> i32 {
    // Инициализируем базу
    let init_code = init_database(db_path, db_key);
    if init_code != 0 {
        return init_code;
    }

    // Регистрируем callback
    set_swift_callback(callback);

    // Запускаем фоновые процессы
    start_background_services();

    0
}

/// Фоновая служба для обработки событий
fn start_background_services() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Some(conn) = &*GLOBAL_CONN.lock().unwrap() {
                // let monitor = DataMonitor::new(conn.clone());
                // monitor.start().await;
            }
        });
    });
}

#[no_mangle]
pub extern "C" fn get_contacts_page(offset: i32, limit: i32) -> *mut c_char {
    // Захватываем глобальное соединение
    let conn_guard = GLOBAL_CONN.lock().unwrap();
    if let Some(conn) = &*conn_guard {
        let repo = ContactRepo::new(Arc::clone(conn));

        // Так как репозиторий работает асинхронно, создаём временный runtime:
        let rt = tokio::runtime::Runtime::new().unwrap();
        let fut = async {
            match repo.get_paginated(offset as i64, limit as i64).await {
                Ok(contacts) => {
                    // Преобразуем в JSON. Здесь желательно, чтобы тип ContactObjC был сериализуемым;
                    // если нет – можно сконвертировать их в внутреннюю структуру Contact перед сериализацией.
                    serde_json::to_string(&contacts).unwrap_or_else(|_| "[]".to_string())
                },
                Err(e) => {
                    error!("Failed to get contacts: {}", e);
                    "[]".to_string()
                }
            }
        };
        let json = rt.block_on(fut);
        CString::new(json).unwrap().into_raw()
    } else {
        CString::new("[]").unwrap().into_raw()
    }
}

/// Генерация тестовых данных
#[no_mangle]
pub extern "C" fn generate_test_data() -> i32 {
    let conn_guard = GLOBAL_CONN.lock().unwrap();
    if let Some(conn) = &*conn_guard {
        // Добавляем 100 контактов
        add_test_contacts();

        // Добавляем тестовые сообщения
        // add_test_messages();

        0
    } else {
        error!("Database not initialized");
        1
    }
}

#[no_mangle]
pub extern "C" fn add_test_contacts() -> i32 {
    let conn_guard = GLOBAL_CONN.lock();
    if let Some(conn) = &*conn_guard {
        let repo = ContactRepo::new(Arc::clone(conn));
        for i in 0..100 {
            let contact = Contact {
                first_name: format!("User {}", i),
                last_name: format!("Lastname {}", i),
                ..Contact::default()
            };

            let objc_contact = contact.to_objc();

            if let Err(e) = repo.add(objc_contact) {
                unsafe { free_contact_objc(objc_contact) };
                return 1;
            }

            unsafe { free_contact_objc(objc_contact) };
        }
        0
    } else {
        error!("Database not initialized");
        1
    }
}

#[no_mangle]
pub extern "C" fn create_contact_objc() -> *mut ContactObjC {
    Contact::default().to_objc()
}

#[no_mangle]
pub extern "C" fn add_single_contact(name: *const c_char, phone: *const c_char) -> i32 {
    let conn_guard = GLOBAL_CONN.lock();
    if let Some(conn) = &*conn_guard {
        let repo = ContactRepo::new(Arc::clone(conn));

        let contact = Contact {
            first_name: format!("User New"),
            last_name: format!("Lastname New"),
            ..Contact::default()
        };

        let contactObjC = contact.to_objc();

        match repo.add(&contactObjC) {
            Ok(_) => 0,
            Err(e) => {
                error!("Failed to add contact: {}", e);
                1
            }
        }
    } else {
        error!("Database not initialized");
        1
    }
}

// #[no_mangle]
// pub extern "C" fn get_contacts_page(
//     offset: i32,
//     limit: i32,
// ) -> *mut c_char {
//     let conn_guard = GLOBAL_CONN.lock();
//     if let Some(conn) = &*conn_guard {
//         let repo = ContactRepo::new(Arc::clone(conn));
//         match repo.get_paginated(offset, limit) {
//             Ok(contacts) => {
//                 let json = serde_json::to_string(&contacts).unwrap();
//                 CString::new(json).unwrap().into_raw()
//             },
//             Err(e) => {
//                 error!("Failed to get contacts: {}", e);
//                 CString::new("[]").unwrap().into_raw()
//             }
//         }
//     } else {
//         CString::new("[]").unwrap().into_raw()
//     }
// }

/// Инициализация базы данных (зашифрованной SQLCipher).
///
/// # Параметры
/// - `db_path`: путь к файлу .sqlite
/// - `db_key`: ключ (пароль) SQLCipher
///
/// Возвращает `0`, если всё ок, иначе != 0 для ошибок.
#[no_mangle]
pub extern "C" fn init_database(db_path: *const c_char, db_key: *const c_char) -> i32 {
    // 1. Считываем C-строки
    if db_path.is_null() || db_key.is_null() {
        error!("init_database: db_path or db_key is null");
        return 1;
    }
    let db_path_str = unsafe { CStr::from_ptr(db_path) }.to_string_lossy().to_string();
    let db_key_str = unsafe { CStr::from_ptr(db_key) }.to_string_lossy().to_string();

    // 2. Открываем зашифрованную БД
    match open_encrypted_db(&db_path_str, &db_key_str) {
        Ok(conn) => {
            // 3. Миграции
            if let Err(e) = setup_migrations(&conn) {
                error!("setup_migrations error: {}", e);
                return 2;
            }
            // 4. Регистрируем preupdate_hook
            register_preupdate_hook(&conn);

            // 5. Сохраняем
            {
                let mut guard = GLOBAL_CONN.lock().unwrap();
                *guard = Some(conn);
            }

            // 6. Запускаем диспетчер (если ещё не стартовал)
            // start_event_dispatcher();

            info!("init_database success");
            0
        },
        Err(e) => {
            error!("Cannot open encrypted db: {}", e);
            1
        }
    }
}

/// Регистрируем Swift callback для уведомления об изменениях
#[no_mangle]
pub extern "C" fn set_swift_callback(cb: extern "C" fn(*const c_char)) {
    register_swift_callback(cb);
}

/// Пример геттер для Swift, чтобы проверить, что БД готова. Возвращаем `1`, если нет.
#[no_mangle]
pub extern "C" fn check_db_ready() -> i32 {
    let guard = GLOBAL_CONN.lock().unwrap();
    if guard.is_some() { 0 } else { 1 }
}

// ---------------------- Внутренние функции ----------------------

fn open_encrypted_db(path: &str, key: &str) -> SqlResult<Connection> {
    // SQLite + SQLCipher
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    )?;

    // Устанавливаем ключ
    let sql = format!("PRAGMA key = '{}';", key);
    conn.execute(&sql, [])?;

    // Можно проверить: conn.query_row("PRAGMA cipher_version;", [], |r| r.get::<_, String>(0))?;

    Ok(conn)
}

// Helper function to convert C string to Rust string
unsafe fn c_str_to_string(s: *const c_char) -> String {
    CStr::from_ptr(s).to_string_lossy().into_owned()
}

// Helper function to convert Rust Result to C string
fn result_to_c_string<E: std::fmt::Display>(result: Result<String, E>) -> *mut c_char {
    match result {
        Ok(s) => CString::new(s).unwrap_or_default().into_raw(),
        Err(e) => CString::new(e.to_string()).unwrap_or_default().into_raw(),
    }
}

// ContactBookRepo wrappers
// #[no_mangle]
// pub unsafe extern "C" fn contact_book_add_json(conn_ptr: *mut Connection, json: *const c_char) -> *mut c_char {
//     let conn = &*conn_ptr;
//     let repo = ContactBookRepo::new(conn);
//     let json_str = c_str_to_string(json);
//     result_to_c_string(repo.add_contact_book_json(&json_str))
// }

// #[no_mangle]
// pub unsafe extern "C" fn contact_book_get_json(conn_ptr: *mut Connection, id: *const c_char) -> *mut c_char {
//     let conn = &*conn_ptr;
//     let repo = ContactBookRepo::new(conn);
//     let id_str = c_str_to_string(id);
//     result_to_c_string(repo.get_contact_book_json(&id_str))
// }

// #[no_mangle]
// pub unsafe extern "C" fn contact_book_update_json(
//     conn_ptr: *mut Connection,
//     id: *const c_char,
//     json: *const c_char
// ) -> *mut c_char {
//     let conn = &*conn_ptr;
//     let repo = ContactBookRepo::new(conn);
//     let id_str = c_str_to_string(id);
//     let json_str = c_str_to_string(json);
//     result_to_c_string(repo.update_contact_book_json(&id_str, &json_str))
// }

// #[no_mangle]
// pub unsafe extern "C" fn contact_book_delete_json(conn_ptr: *mut Connection, id: *const c_char) -> *mut c_char {
//     let conn = &*conn_ptr;
//     let repo = ContactBookRepo::new(conn);
//     let id_str = c_str_to_string(id);
//     result_to_c_string(repo.delete_contact_book_json(&id_str))
// }

// ContactSeenAtRepo wrappers
#[no_mangle]
pub unsafe extern "C" fn contact_seen_at_add_json(conn_ptr: *mut Connection, json: *const c_char) -> *mut c_char {
    let conn = &*conn_ptr;
    let repo = ContactSeenAtRepo::new(conn);
    let json_str = c_str_to_string(json);
    result_to_c_string(repo.add_seen_json(&json_str))
}

#[no_mangle]
pub unsafe extern "C" fn contact_seen_at_all_json(conn_ptr: *mut Connection) -> *mut c_char {
    let conn = &*conn_ptr;
    let repo = ContactSeenAtRepo::new(conn);
    result_to_c_string(repo.all_seen_json())
}

// ContactStatusRepo wrappers
#[no_mangle]
pub unsafe extern "C" fn contact_status_add_json(conn_ptr: *mut Connection, json: *const c_char) -> *mut c_char {
    let conn = &*conn_ptr;
    let repo = ContactStatusRepo::new(conn);
    let json_str = c_str_to_string(json);
    result_to_c_string(repo.add_status_json(&json_str))
}

#[no_mangle]
pub unsafe extern "C" fn contact_status_all_json(conn_ptr: *mut Connection) -> *mut c_char {
    let conn = &*conn_ptr;
    let repo = ContactStatusRepo::new(conn);
    result_to_c_string(repo.all_contacts_status_json())
}

// Helper function to free C strings created by Rust
#[no_mangle]
pub unsafe extern "C" fn free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

// Table creation wrappers
#[no_mangle]
pub unsafe extern "C" fn create_contact_seen_at_table(conn_ptr: *mut Connection) -> bool {
    let conn = &*conn_ptr;
    db::contact_seen_at::create_contact_seen_at_table(conn).is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn create_contact_status_table(conn_ptr: *mut Connection) -> bool {
    let conn = &*conn_ptr;
    db::contact_status::create_contact_status_table(conn).is_ok()
}

#[cfg(test)]
mod tests {
    use super::init_database;
    use std::ffi::CString;
    use super::check_db_ready;

    #[test]
    fn test_init() {
        let path = CString::new(":memory:").unwrap();
        let key = CString::new("my_secret").unwrap();

        let code = init_database(path.as_ptr(), key.as_ptr());
        assert_eq!(code, 0, "init_database failed");

        let ready = check_db_ready();
        assert_eq!(ready, 0, "DB not ready");
    }
}