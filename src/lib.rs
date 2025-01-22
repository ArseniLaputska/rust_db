// src/lib.rs

#![allow(unused_imports)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use rusqlite::{Connection, OpenFlags, Result as SqlResult};
use log::{info, error};

mod db;
use db::monitor::{register_preupdate_hook, start_event_dispatcher, register_swift_callback};
use crate::db::migrations::setup_migrations;

use crate::db::contact::ContactRepo;
use crate::db::contact_book::ContactBookRepo;
use crate::db::contact_seen_at::ContactSeenAtRepo;
use crate::db::contact_status::ContactStatusRepo;
use crate::db::message::MessageRepo;

// ---------------------- Глобальные объекты ----------------------

/// Глобальный держатель (Option<Connection>) под Mutex, чтобы другие функции могли его использовать
static GLOBAL_CONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| Mutex::new(None));

/// Версия схемы (example)
const LATEST_SCHEMA_VERSION: i32 = 1;

// ---------------------- Экспортируемые функции ----------------------

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
            start_event_dispatcher();

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
#[no_mangle]
pub unsafe extern "C" fn contact_book_add_json(conn_ptr: *mut Connection, json: *const c_char) -> *mut c_char {
    let conn = &*conn_ptr;
    let repo = ContactBookRepo::new(conn);
    let json_str = c_str_to_string(json);
    result_to_c_string(repo.add_contact_book_json(&json_str))
}

#[no_mangle]
pub unsafe extern "C" fn contact_book_get_json(conn_ptr: *mut Connection, id: *const c_char) -> *mut c_char {
    let conn = &*conn_ptr;
    let repo = ContactBookRepo::new(conn);
    let id_str = c_str_to_string(id);
    result_to_c_string(repo.get_contact_book_json(&id_str))
}

#[no_mangle]
pub unsafe extern "C" fn contact_book_update_json(
    conn_ptr: *mut Connection,
    id: *const c_char,
    json: *const c_char
) -> *mut c_char {
    let conn = &*conn_ptr;
    let repo = ContactBookRepo::new(conn);
    let id_str = c_str_to_string(id);
    let json_str = c_str_to_string(json);
    result_to_c_string(repo.update_contact_book_json(&id_str, &json_str))
}

#[no_mangle]
pub unsafe extern "C" fn contact_book_delete_json(conn_ptr: *mut Connection, id: *const c_char) -> *mut c_char {
    let conn = &*conn_ptr;
    let repo = ContactBookRepo::new(conn);
    let id_str = c_str_to_string(id);
    result_to_c_string(repo.delete_contact_book_json(&id_str))
}

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