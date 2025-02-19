use objc2_foundation::{NSData, NSString, NSNumber};
use objc2::rc::{Retained, autoreleasepool};
use tokio_rusqlite::{Connection, params, Result as SqlResult};
use uuid::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use super::objc_converters::{
    convert_to_nsdata, optional_nsstring,
    optional_to_nsstring, nsdata_to_uuid,
    optional_nsdata_to_uuid
};

#[repr(C)]
pub struct MessageObjC {
    pub id: *mut NSData,
    pub from: *mut NSData,
    pub to: *mut NSData,
    pub prev: *mut NSData,
    pub contact_id: *mut NSData,
    pub status: i64,
    pub audio_url: *mut NSString,
    pub duration: f64,
    pub text: *mut NSString,
    pub client_text: *mut NSString,
    pub gpt_text: *mut NSString,
    pub server_text: *mut NSString,
    pub translated_text: *mut NSData, // JSON as NSData
    pub language: *mut NSString,
    pub error: *mut NSString,
    pub created_at: f64,
    pub updated_at: f64,
    pub try_count: i64,
}

// Обеспечиваем, что MessageObjC можно отправлять между потоками.
unsafe impl Send for MessageObjC {}
unsafe impl Sync for MessageObjC {}

pub struct MessageRepo {
    conn: Arc<Connection>,
}

impl MessageRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    // Основные CRUD-операции
    pub async fn get(&self, id: Uuid) -> SqlResult<Option<MessageObjC>> {
        let conn = self.conn.clone();
        let result = conn.call(move |conn| {
            let mut stmt = conn.prepare(
                r#"SELECT
                    id, from_uuid, to_uuid, prev_uuid, contact_id,
                    status, audio_url, duration, text, client_text,
                    gpt_text, server_text, translated_text, language,
                    error, created_at, updated_at, try_count
                 FROM message
                 WHERE id = ?1"#
            )?;
            let id_bytes = id.as_bytes().to_vec();
            let mut rows = stmt.query(params![id_bytes])?;
            if let Some(row) = rows.next()? {
                Ok(Some(Self::row_to_objc(row)?))
            } else {
                Ok(None)
            }
        }).await?;
        Ok(result)
    }

    pub async fn add(&self, message: &MessageObjC) -> SqlResult<()> {
        let message = Self::objc_to_rust(message)?;
        let conn = self.conn.clone();
        conn.call(move |conn| {
            let mut stmt = conn.prepare(
                r#"INSERT INTO message (
                    id, from_uuid, to_uuid, prev_uuid, contact_id,
                    status, audio_url, duration, text, client_text,
                    gpt_text, server_text, translated_text, language,
                    error, created_at, updated_at, try_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"#
            )?;
            stmt.execute(params![
                message.id.as_bytes().to_vec(),
                message.from.as_bytes().to_vec(),
                message.to.as_bytes().to_vec(),
                message.prev.map(|u| u.as_bytes().to_vec()),
                message.contact_id.as_bytes().to_vec(),
                message.status,
                message.audio_url,
                message.duration,
                message.text,
                message.client_text,
                message.gpt_text,
                message.server_text,
                serde_json::to_vec(&message.translated_text)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?,
                message.language,
                message.error,
                message.created_at,
                message.updated_at,
                message.try_count
            ])?;
            Ok(())
        }).await?;
        Ok(())
    }

    // Специфические методы
    pub async fn get_by_status(&self, status: i64) -> SqlResult<Vec<MessageObjC>> {
        let conn = self.conn.clone();
        let messages = conn.call(move |conn| {
            let mut stmt = conn.prepare(
                r#"SELECT * FROM message WHERE status = ?1 ORDER BY created_at DESC"#
            )?;
            let mut rows = stmt.query(params![status])?;
            let mut messages = Vec::new();
            while let Some(row) = rows.next()? {
                messages.push(Self::row_to_objc(row)?);
            }
            Ok(messages)
        }).await?;
        Ok(messages)
    }

    fn row_to_objc(row: &tokio_rusqlite::Row<'_>) -> SqlResult<MessageObjC> {
        autoreleasepool(|_| {
            Ok(MessageObjC {
                id: convert_to_nsdata(row.get(0_usize)?),
                from: convert_to_nsdata(row.get(1_usize)?),
                to: convert_to_nsdata(row.get(2_usize)?),
                prev: optional_to_nsdata(row.get(3_usize).ok()),
                contact_id: convert_to_nsdata(row.get(4_usize)?),
                status: row.get(5_usize)?,
                audio_url: optional_to_nsstring(row.get(6_usize).ok()),
                duration: row.get(7_usize)?,
                text: optional_to_nsstring(row.get(8_usize).ok()),
                client_text: optional_to_nsstring(row.get(9_usize).ok()),
                gpt_text: optional_to_nsstring(row.get(10_usize).ok()),
                server_text: optional_to_nsstring(row.get(11_usize).ok()),
                translated_text: convert_to_nsdata(row.get::<_, Vec<u8>>(12_usize)?),
                language: optional_to_nsstring(row.get(13_usize).ok()),
                error: optional_to_nsstring(row.get(14_usize).ok()),
                created_at: row.get(15_usize)?,
                updated_at: row.get(16_usize)?,
                try_count: row.get(17_usize)?,
            })
        })
    }

    fn objc_to_rust(message: &MessageObjC) -> SqlResult<Message> {
        autoreleasepool(|_| {
            Ok(Message {
                id: nsdata_to_uuid(message.id)?,
                from: nsdata_to_uuid(message.from)?,
                to: nsdata_to_uuid(message.to)?,
                prev: optional_nsdata_to_uuid(message.prev),
                contact_id: nsdata_to_uuid(message.contact_id)?,
                status: message.status,
                audio_url: optional_nsstring(message.audio_url),
                duration: message.duration,
                text: optional_nsstring(message.text),
                client_text: optional_nsstring(message.client_text),
                gpt_text: optional_nsstring(message.gpt_text),
                server_text: optional_nsstring(message.server_text),
                translated_text: serde_json::from_slice(&nsdata_to_bytes(message.translated_text)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?,
                language: optional_nsstring(message.language),
                error: optional_nsstring(message.error),
                created_at: message.created_at,
                updated_at: message.updated_at,
                try_count: message.try_count,
            })
        })
    }
}

fn optional_to_nsdata(bytes: Option<Vec<u8>>) -> *mut NSData {
    bytes.map(convert_to_nsdata).unwrap_or_else(|| std::ptr::null_mut())
}

fn nsdata_to_bytes(nsdata: *mut NSData) -> SqlResult<Vec<u8>> {
    if nsdata.is_null() {
        return Ok(Vec::new());
    }

    let data = unsafe { Retained::retain(nsdata) }
        .ok_or_else(|| rusqlite::Error::InvalidParameterName("Null NSData".into()))?;

    unsafe {
        Ok(data.as_bytes_unchecked().to_vec())
    }
}

// Остальные функции конвертации аналогичны contact.rs

// Внутреннее Rust-представление
struct Message {
    id: Uuid,
    from: Uuid,
    to: Uuid,
    prev: Option<Uuid>,
    contact_id: Uuid,
    status: i64,
    audio_url: Option<String>,
    duration: f64,
    text: Option<String>,
    client_text: Option<String>,
    gpt_text: Option<String>,
    server_text: Option<String>,
    translated_text: HashMap<String, String>,
    language: Option<String>,
    error: Option<String>,
    created_at: f64,
    updated_at: f64,
    try_count: i64,
}