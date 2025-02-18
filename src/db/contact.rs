use tokio_rusqlite::{Connection, params, Result as SqlResult};
use uuid::{Uuid, Bytes};
use std::sync::Arc;
use std::ffi::{c_char, CStr};
use objc2_foundation::{NSData, NSString, NSUInteger};
use objc2::rc::{Retained, autoreleasepool};
use serde::Serialize;
use super::handler::EntityRepository;
use super::objc_converters::{
    convert_to_nsdata, optional_nsstring,
    convert_to_nsstring, optional_to_nsstring,
    nsdata_to_uuid, nsstring_to_string
};
use crate::db::cache::CacheHandler;

#[repr(C)]
pub struct ContactObjC {
    pub id: *mut NSData,
    pub first_name: *mut NSString,
    pub last_name: *mut NSString,
    pub relationship: NSUInteger,
    pub username: *mut NSString,
    pub language: *mut NSString,
    pub picture_url: *mut NSString,
    pub last_message_at: f64,
    pub created_at: f64,
    pub updated_at: f64,
    pub is_pro: bool,
}

pub struct ContactRepo {
    conn: Arc<Connection>,
    cache: CacheHandler,
}

impl ContactRepo {
    pub fn new(conn: Arc<Connection>, cache: CacheHandler) -> Self {
        Self { conn, cache }
    }

    /// Возвращает страницу контактов, отсортированную по времени создания.
    pub async fn get_paginated(&self, offset: i64, limit: i64) -> SqlResult<Vec<ContactObjC>> {
        let mut stmt = self.conn.call(
            r#"SELECT
                id, first_name, last_name, relationship,
                username, language, picture_url,
                last_message_at, created_at, updated_at, is_pro
             FROM contact
             ORDER BY created_at
             LIMIT ?1 OFFSET ?2"#
        ).await?;

        let mut rows = stmt.query(params![limit, offset]).await?;
        let mut contacts = Vec::new();

        while let Some(row) = rows.next().await? {
            contacts.push(Self::row_to_objc(row)?);
        }

        Ok(contacts)
    }

    /// Получаем контакт по UUID, сначала пытаемся найти в кэше
    pub async fn get(&self, id: Uuid) -> rusqlite::Result<Option<*mut super::contact::ContactObjC>> {
        // Сначала проверяем кэш
        if let Some(contact) = self.cache.get_contact(&id) {
            // Если найден, можно преобразовать в ObjC-формат (через to_objc)
            return Ok(Some(contact.to_objc()));
        }

        // Если в кэше нет, выполняем запрос в базу
        let mut stmt = self.conn.call(
            r#"SELECT
                id, first_name, last_name, relationship,
                username, language, picture_url,
                last_message_at, created_at, updated_at, is_pro
             FROM contact
             WHERE id = ?1"#,
        ).await?;

        let id_bytes = id.as_bytes().to_vec();
        let mut rows = stmt.query(rusqlite::params![id_bytes]).await?;

        if let Some(row) = rows.next().await? {
            let contact_rust = Self::row_to_rust(row)?;
            // Обновляем кэш
            self.cache.put_contact(id, contact_rust.clone());
            // Преобразуем в ObjC-формат и возвращаем
            Ok(Some(contact_rust.to_objc()))
        } else {
            Ok(None)
        }
    }

    pub async fn add(&self, contact: &ContactObjC) -> SqlResult<()> {
        let contact = Self::objc_to_rust(contact)?;
        let mut stmt = self.conn.call(
            r#"INSERT INTO contact (
                id, first_name, last_name, relationship,
                username, language, picture_url,
                last_message_at, created_at, updated_at, is_pro
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
        ).await?;

        stmt.execute(params![
            contact.id.as_bytes().to_vec(),
            contact.first_name,
            contact.last_name,
            contact.relationship,
            contact.username,
            contact.language,
            contact.picture_url,
            contact.last_message_at,
            contact.created_at,
            contact.updated_at,
            contact.is_pro as i64
        ]).await?;

        Ok(())
    }

    // Специфические методы
    pub async fn search_by_name(&self, query: &str) -> SqlResult<Vec<ContactObjC>> {
        let query = format!("%{}%", sanitize_like(query));
        let mut stmt = self.conn.call(
            "SELECT * FROM contact WHERE first_name LIKE ?1 OR last_name LIKE ?1"
        ).await?;

        let mut rows = stmt.query(params![query]).await?;
        let mut contacts = Vec::new();

        while let Some(row) = rows.next().await? {
            contacts.push(Self::row_to_objc(row)?);
        }

        Ok(contacts)
    }

    // Функция конвертации строки в внутреннюю структуру Contact
    fn row_to_rust(row: &rusqlite::Row<'_>) -> rusqlite::Result<super::contact::Contact> {
        // Пример преобразования (как раньше, но возвращает внутреннюю структуру)
        Ok(super::contact::Contact {
            id: {
                let bytes: Vec<u8> = row.get(0)?;
                Uuid::from_slice(&bytes).unwrap_or_else(|_| Uuid::nil())
            },
            first_name: row.get(1)?,
            last_name: row.get(2)?,
            relationship: row.get(3)?,
            username: row.get(4).ok(),
            language: row.get(5).ok(),
            picture_url: row.get(6).ok(),
            last_message_at: row.get(7).ok(),
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            is_pro: row.get(10)?,
        })
    }

    // Конвертация Rust <-> ObjC
    fn row_to_objc(row: &tokio_rusqlite::Row<'_>) -> SqlResult<ContactObjC> {
        autoreleasepool(|_| {
            let id_bytes: Vec<u8> = row.get(0_usize)?; // Явно указываем тип индекса

            Ok(ContactObjC {
                id: convert_to_nsdata(id_bytes),
                first_name: convert_to_nsstring(row.get(1_usize)?),
                last_name: convert_to_nsstring(row.get(2_usize)?),
                relationship: row.get::<_, usize>(3_usize)? as NSUInteger,
                username: optional_to_nsstring(row.get(4_usize).ok()),
                language: optional_to_nsstring(row.get(5_usize).ok()),
                picture_url: optional_to_nsstring(row.get(6_usize).ok()),
                last_message_at: row.get(7_usize)?,
                created_at: row.get(8_usize)?,
                updated_at: row.get(9_usize)?,
                is_pro: row.get::<_, i64>(10_usize)? != 0,
            })
        })
    }

    pub fn objc_to_rust(contact: &ContactObjC) -> SqlResult<Contact> {
        autoreleasepool(|_| {
            Ok(Contact {
                id: nsdata_to_uuid(contact.id)?,
                first_name: nsstring_to_string(contact.first_name),
                last_name: nsstring_to_string(contact.last_name),
                relationship: contact.relationship as i64,
                username: optional_nsstring(contact.username),
                language: optional_nsstring(contact.language),
                picture_url: optional_nsstring(contact.picture_url),
                last_message_at: Some(contact.last_message_at),
                created_at: contact.created_at,
                updated_at: contact.updated_at,
                is_pro: contact.is_pro as i64,
            })
        })
    }
}

fn sanitize_like(input: &str) -> String {
    input.replace("%", "\\%").replace("_", "\\_")
}

// Rust-представление для внутренних операций
#[derive(Debug, Clone, Default, Serialize)]
pub struct Contact {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub relationship: i64,
    pub username: Option<String>,
    pub language: Option<String>,
    pub picture_url: Option<String>,
    pub last_message_at: Option<f64>,
    pub created_at: f64,
    pub updated_at: f64,
    pub is_pro: i64,
}

// Реализация для FFI
#[no_mangle]
pub unsafe extern "C" fn create_contact() -> *mut Contact {
    Box::into_raw(Box::new(Contact::default()))
}

#[no_mangle]
pub unsafe extern "C" fn contact_set_first_name(ptr: *mut Contact, name: *const c_char) {
    let contact = &mut *ptr;
    contact.first_name = CStr::from_ptr(name).to_string_lossy().into_owned();
}