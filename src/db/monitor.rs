/*****************************************************************************************************
  ОБНОВЛЁННОЕ «БОЕВОЕ» РЕШЕНИЕ ДЛЯ DATA MONITOR, ИСПОЛЬЗУЮЩЕЕ АКТУАЛЬНЫЙ ПОДХОД В RUSQLITE

  Мы используем:
  - Новую реализацию preupdate_hook в rusqlite,
  - Enums: PreUpdateCase::{Insert, Delete, Update, Unknown}, PreUpdateNewValueAccessor, PreUpdateOldValueAccessor,
  - Action::{SQLITE_INSERT, SQLITE_DELETE, SQLITE_UPDATE, UNKNOWN},
  - Колбэки вида: FnMut(Action, &str, &str, &PreUpdateCase).

  Наша цель:
    1) Ловить события INSERT/UPDATE/DELETE (до их фактического исполнения, но уже определённых),
    2) Извлекать старые и новые значения (через OldValueAccessor / NewValueAccessor),
    3) Складывать информацию в очередь (mpsc),
    4) В отдельном потоке брать события из очереди, сериализовать в JSON, и звать Swift callback,
    5) Swift-код получает JSON и обновляет UI.

  Рассмотрим код по шагам:

  ----------------------------------------------------------------------------------------------
  1) ОБЩИЕ ИМПОРТЫ (актуальная версия preupdate_hook)
  ----------------------------------------------------------------------------------------------
*/

use std::sync::{Arc, Mutex};
use std::thread;
use std::os::raw::c_char;
use std::ffi::{CString, CStr};
use std::time::Duration;

use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};

use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio_rusqlite::{
    Connection, Result,
    types::ValueRef,
    hooks::{
        Action,                     // SQLITE_INSERT, SQLITE_DELETE, SQLITE_UPDATE, UNKNOWN
        PreUpdateCase,             // Insert(...), Delete(...), Update{...}, Unknown
        PreUpdateOldValueAccessor, // get_old_row_id, get_old_column_value, etc.
        PreUpdateNewValueAccessor, // get_new_row_id, get_new_column_value, etc.
    }
};
use log::{error, info, warn};
use uuid::Uuid;

use crate::db::history::*;
use crate::db::Result as DbResult; // Путь зависит от структуры проекта

#[allow(unused_imports)]
use rusqlite::ffi;

/// Структура события, получаемого из preupdate‑hook.
#[derive(Debug, Serialize, Deserialize)]
pub struct PreUpdateEvent {
    pub db_name: String,
    pub table: String,
    pub operation: String, // "INSERT", "UPDATE", "DELETE", "UNKNOWN"
    pub rowid: i64,
    pub old_values: Option<Vec<(String, String)>>,
    pub new_values: Option<Vec<(String, String)>>,
}

// Глобальный асинхронный канал для событий preupdate.
static EVENT_SENDER: Lazy<Mutex<Option<Sender<PreUpdateEvent>>>> = Lazy::new(|| Mutex::new(None));
static EVENT_RECEIVER: Lazy<Mutex<Option<Receiver<PreUpdateEvent>>>> = Lazy::new(|| Mutex::new(None));

/// Регистрируем preupdate‑hook для соединения rusqlite.
/// В колбэке формируется PreUpdateEvent и отправляется в канал.
pub fn register_preupdate_hook(conn: &Connection) {
    conn.preupdate_hook(Some(|action, db, tbl, case| {
        let operation = match action {
            Action::SQLITE_INSERT => "INSERT",
            Action::SQLITE_DELETE => "DELETE",
            Action::SQLITE_UPDATE => "UPDATE",
            _ => "UNKNOWN",
        };
        let (rowid, old_vals, new_vals) = match case {
            PreUpdateCase::Insert(new_acc) => {
                let rid = new_acc.get_new_row_id();
                let vals = collect_new_values(&new_acc);
                (rid, None, Some(vals))
            },
            PreUpdateCase::Delete(old_acc) => {
                let rid = old_acc.get_old_row_id();
                let vals = collect_old_values(&old_acc);
                (rid, Some(vals), None)
            },
            PreUpdateCase::Update { old_value_accessor, new_value_accessor } => {
                let rid = new_value_accessor.get_new_row_id();
                let oldv = collect_old_values(&old_value_accessor);
                let newv = collect_new_values(&new_value_accessor);
                (rid, Some(oldv), Some(newv))
            },
            PreUpdateCase::Unknown => (0, None, None),
        };

        let evt = PreUpdateEvent {
            db_name: db.to_string(),
            table: tbl.to_string(),
            operation: operation.to_string(),
            rowid,
            old_values: old_vals,
            new_values: new_vals,
        };

        // Отправляем событие в глобальный канал
        if let Some(ref tx) = *EVENT_SENDER.lock().unwrap() {
            if let Err(e) = tx.try_send(evt) {
                eprintln!("EVENT_SENDER try_send error: {:?}", e);
            }
        }
    }));
}

/// Сбор значений для старой строки.
fn collect_old_values(acc: &PreUpdateOldValueAccessor) -> Vec<(String, String)> {
    let col_count = acc.get_column_count();
    let mut out = Vec::new();
    for i in 0..col_count {
        if let Ok(valref) = acc.get_old_column_value(i) {
            let s = value_to_string(valref);
            let col_name = format!("col_{}", i);
            out.push((col_name, s));
        }
    }
    out
}

/// Сбор значений для новой строки.
fn collect_new_values(acc: &PreUpdateNewValueAccessor) -> Vec<(String, String)> {
    let col_count = acc.get_column_count();
    let mut out = Vec::new();
    for i in 0..col_count {
        if let Ok(valref) = acc.get_new_column_value(i) {
            let s = value_to_string(valref);
            let col_name = format!("col_{}", i);
            out.push((col_name, s));
        }
    }
    out
}

/// Преобразование ValueRef в строку.
fn value_to_string(v: tokio_rusqlite::types::ValueRef) -> String {
    match v {
        tokio_rusqlite::types::ValueRef::Null => "NULL".to_string(),
        tokio_rusqlite::types::ValueRef::Integer(i) => i.to_string(),
        tokio_rusqlite::types::ValueRef::Real(r) => r.to_string(),
        tokio_rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        tokio_rusqlite::types::ValueRef::Blob(b) => base64::encode(b),
    }
}

/// Инициализируем глобальный канал для событий.
/// Вызывается один раз при инициализации БД.
pub fn init_event_channel() {
    let mut sender_guard = EVENT_SENDER.lock().unwrap();
    let mut receiver_guard = EVENT_RECEIVER.lock().unwrap();
    if sender_guard.is_none() || receiver_guard.is_none() {
        let (tx, rx) = mpsc::channel::<PreUpdateEvent>(1000);
        *sender_guard = Some(tx);
        *receiver_guard = Some(rx);
    }
}

/// Запускаем диспетчер событий, который читает канал и уведомляет Swift через callback.
pub fn start_event_dispatcher_async() {
    init_event_channel(); // Убедимся, что канал инициализирован
    // Клонируем receiver
    let rx = EVENT_RECEIVER.lock().unwrap().take().unwrap();
    tokio::spawn(async move {
        let mut rx = rx;
        while let Some(evt) = rx.recv().await {
            // Сериализуем событие в JSON
            let json = serde_json::to_string(&evt).unwrap_or_else(|_| "{}".to_string());
            // Вызываем Swift callback, если он установлен
            unsafe {
                if let Some(cb) = SWIFT_CALLBACK {
                    let cstr = CString::new(json).unwrap();
                    cb(cstr.as_ptr());
                }
            }
        }
    });
}

/// Глобальный указатель на Swift callback-функцию.
/// Этот указатель устанавливается через FFI.
static mut SWIFT_CALLBACK: Option<extern "C" fn(*const c_char)> = None;

/// Функция для регистрации Swift callback.
/// Вызывается из Swift для передачи функции обратного вызова.
#[no_mangle]
pub extern "C" fn register_swift_callback(cb: extern "C" fn(*const c_char)) {
    unsafe {
        SWIFT_CALLBACK = Some(cb);
    }
}

pub struct DataMonitor {
    history: PersistentHistory,
    local_last_timestamp: f64,
    sender_last_timestamp: f64,
}

impl DataMonitor {
    pub async fn process_local_changes(&mut self) -> DbResult<()> {
        let records = self.history.get_records_after(self.local_last_timestamp).await.unwrap();

        for record in records {
            if record.author != "sender" {
                self.handle_local_change(&record).await?;
                self.local_last_timestamp = record.created_at;
            }
        }

        Ok(())
    }

    pub async fn process_sender_changes(&mut self) -> DbResult<()> {
        let records = self.history.get_records_after(self.sender_last_timestamp).await.unwrap();

        for record in records {
            if record.author == "sender" {
                self.handle_sender_change(&record).await?;
                // self.sender_last_timestamp = record.timestamp;
            }
        }

        Ok(())
    }

    async fn handle_local_change(&self, record: &HistoryRecord) -> DbResult<()> {
        match record.entity_name.as_str() {
            "ContactData" => {
                // let contact = self.contact_repo.get(record.entity_id).await?;
                // self.data_handler.sync_contact(contact).await?;
            }
            "MessageData" => {
                // let message = self.message_repo.get(record.entity_id).await?;
                // self.data_handler.process_message(message).await?;
            }
            _ => log::warn!("Unknown entity type: {}", record.entity_name),
        }
        Ok(())
    }

    async fn handle_sender_change(&self, record: &HistoryRecord) -> DbResult<()> {
        match record.entity_name.as_str() {
            "ContactData" => {
                // self.data_handler.upload_contact(record.entity_id).await?;
            }
            "MessageData" => {
                // self.data_handler.upload_message(record.entity_id).await?;
            }
            _ => log::warn!("Unsupported sender entity: {}", record.entity_name),
        }
        Ok(())
    }
}

/*
  ----------------------------------------------------------------------------------------------
  7) ТЕСТ: ПРИМЕР ИСПОЛЬЗОВАНИЯ
  ----------------------------------------------------------------------------------------------
*/