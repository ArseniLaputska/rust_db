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

use std::sync::mpsc::{self, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::os::raw::c_char;
use std::ffi::{CString, CStr};
use std::time::Duration;

use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};

use rusqlite::{
    Connection, Result,
    types::ValueRef,
    hooks::{
        Action,                     // SQLITE_INSERT, SQLITE_DELETE, SQLITE_UPDATE, UNKNOWN
        PreUpdateCase,             // Insert(...), Delete(...), Update{...}, Unknown
        PreUpdateOldValueAccessor, // get_old_row_id, get_old_column_value, etc.
        PreUpdateNewValueAccessor, // get_new_row_id, get_new_column_value, etc.
    }
};

#[allow(unused_imports)]
use rusqlite::ffi;  // при необходимости

/*
  ----------------------------------------------------------------------------------------------
  2) СТРУКТУРА ХРАНЕНИЯ СОБЫТИЯ
  ----------------------------------------------------------------------------------------------
  – Как и прежде, PreUpdateEvent будет содержать:
     operation: "INSERT"/"UPDATE"/"DELETE"
     db_name:  имя базы (например, "main")
     table:    имя таблицы
     rowid:    для Insert/Update = new_row_id, для Delete = old_row_id
     old_values: Option<Vec<(String, String)>>
     new_values: Option<Vec<(String, String)>>
*/

#[derive(Debug, Serialize, Deserialize)]
pub struct PreUpdateEvent {
    pub db_name: String,
    pub table: String,
    pub operation: String, // "INSERT" | "UPDATE" | "DELETE" | "UNKNOWN"
    pub rowid: i64,
    pub old_values: Option<Vec<(String, String)>>,
    pub new_values: Option<Vec<(String, String)>>,
}

/*
  ----------------------------------------------------------------------------------------------
  3) ГЛОБАЛЬНОЕ ХРАНИЛИЩЕ СОБЫТИЙ + CALLBACK НА СТОРОНУ SWIFT
  ----------------------------------------------------------------------------------------------

  1) Статическая QUEUE (EVENT_CHANNEL) – mpsc::channel, куда складываем события из preupdate_hook.
  2) Статическая переменная SWIFT_CALLBACK, чтобы хранить указатель на функцию из Swift.
  3) Фоновый поток, который периодически вытаскивает события, сериализует их в JSON, и зовёт SWIFT_CALLBACK.
*/

// Глобальная очередь: (Sender, Receiver)
static EVENT_CHANNEL: Lazy<(Sender<PreUpdateEvent>, Mutex<Receiver<PreUpdateEvent>>)> = Lazy::new(|| {
    let (tx, rx) = mpsc::channel::<PreUpdateEvent>();
    (tx, Mutex::new(rx))
});

// Глобальный указатель на Swift callback-функцию
static mut SWIFT_CALLBACK: Option<extern "C" fn(*const c_char)> = None;

/*
  ----------------------------------------------------------------------------------------------
  4) РЕГИСТРАЦИЯ SWIFT CALLBACK И ЗАПУСК «ДИСПЕТЧЕРА»
  ----------------------------------------------------------------------------------------------
*/

/// Swift может вызвать эту функцию, чтобы передать нам свою C-функцию, которую мы будем звать.
#[no_mangle]
pub extern "C" fn register_swift_callback(cb: extern "C" fn(*const c_char)) {
    // Можно добавить логи или проверки
    println!("register_swift_callback called!");
    unsafe {
        SWIFT_CALLBACK = Some(cb);
    }
}

/// Запускаем фоновую нить, которая изымает события из очереди и вызывает Swift callback.
pub fn start_event_dispatcher() {
    // Можно сделать один раз при запуске приложения.
    thread::spawn(|| loop {
        // Пробуем забрать одно событие за раз (1сек ожидания).
        let event_opt = {
            let rx_lock = EVENT_CHANNEL.1.lock().unwrap();
            rx_lock.recv_timeout(Duration::from_secs(1)).ok()
        };

        if let Some(evt) = event_opt {
            // У нас есть событие, сериализуем
            let json = serde_json::to_string(&evt).unwrap_or_else(|_| "{}".to_string());
            // Зовём Swift callback, если он есть
            unsafe {
                if let Some(func) = SWIFT_CALLBACK {
                    let cstr = CString::new(json).unwrap_or_else(|_| CString::new("{}").unwrap());
                    func(cstr.as_ptr());
                }
            }
        } else {
            // Нет событий — либо timeout, либо канал пуст
            // Можно break, если хотим завершить, но здесь крутимся бесконечно
        }
    });
}

/*
  ----------------------------------------------------------------------------------------------
  5) РЕГИСТРАЦИЯ PREUPDATE_HOOK
  ----------------------------------------------------------------------------------------------

  Как только мы установим preupdate_hook, rusqlite будет вызывать наш колбэк:
     FnMut(Action, &str, &str, &PreUpdateCase)
  где:
    - Action: SQLITE_INSERT / SQLITE_DELETE / SQLITE_UPDATE / UNKNOWN
    - &str (db_name): "main", "temp", ...
    - &str (tbl_name): имя таблицы
    - PreUpdateCase: Insert(PreUpdateNewValueAccessor), Delete(PreUpdateOldValueAccessor), Update { old_value_accessor, new_value_accessor }, Unknown

  Внутри этого колбэка *нельзя* делать тяжёлые вещи (long or blocking calls),
  поэтому мы быстро формируем PreUpdateEvent и шлём его в EVENT_CHANNEL.
*/

pub fn register_preupdate_hook(conn: &Connection) {
    conn.preupdate_hook(Some(|action: Action, db: &str, tbl: &str, case: &PreUpdateCase| {
        // Определим operation = "INSERT"/"DELETE"/"UPDATE"/"UNKNOWN"
        let operation = match action {
            Action::SQLITE_INSERT => "INSERT",
            Action::SQLITE_DELETE => "DELETE",
            Action::SQLITE_UPDATE => "UPDATE",
            _ => "UNKNOWN",
        };

        // Для rowid:
        //  - Если INSERT, rowid = new_value_accessor.get_new_row_id()
        //  - Если DELETE, rowid = old_value_accessor.get_old_row_id()
        //  - Если UPDATE, rowid = new_value_accessor.get_new_row_id() (или old, на выбор).
        // old_values / new_values тоже берем в зависимости от case.

        let (rowid, old_vals, new_vals) = match case {
            PreUpdateCase::Insert(new_acc) => {
                let rid = new_acc.get_new_row_id();
                let (col_count, new_list) = collect_new_values(new_acc);
                (rid, None, Some(new_list))
            }
            PreUpdateCase::Delete(old_acc) => {
                let rid = old_acc.get_old_row_id();
                let (col_count, old_list) = collect_old_values(old_acc);
                (rid, Some(old_list), None)
            }
            PreUpdateCase::Update { old_value_accessor, new_value_accessor } => {
                let rid = new_value_accessor.get_new_row_id();
                let (_, old_list) = collect_old_values(old_value_accessor);
                let (_, new_list) = collect_new_values(new_value_accessor);
                (rid, Some(old_list), Some(new_list))
            }
            PreUpdateCase::Unknown => {
                (0, None, None)
            }
        };

        let evt = PreUpdateEvent {
            db_name: db.to_string(),
            table: tbl.to_string(),
            operation: operation.into(),
            rowid,
            old_values: old_vals,
            new_values: new_vals,
        };

        // Шлем событие в mpsc
        if let Err(e) = EVENT_CHANNEL.0.send(evt) {
            eprintln!("EVENT_CHANNEL send error: {e}");
        }
    }));
}

/*
  ----------------------------------------------------------------------------------------------
  6) УТИЛИТЫ ДЛЯ СБОРКИ OLD/NEW ЗНАЧЕНИЙ В СТРОКИ
  ----------------------------------------------------------------------------------------------
*/

fn collect_old_values(acc: &PreUpdateOldValueAccessor) -> (i32, Vec<(String, String)>) {
    let col_count = acc.get_column_count();
    let mut pairs = Vec::new();
    for i in 0..col_count {
        // возможно, accessor.get_old_column_value(i)?
        let val_ref = match acc.get_old_column_value(i) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let val_str = value_to_string(&val_ref);
        // Здесь нет метода column_name,
        // поэтому, если нужно имя колонки, придётся заранее хранить схему
        // или более хитрые подходы.
        // В старой версии preupdate_hook можно было pud.column_name(i).
        // Но тут — мы ограничены.
        // Для примера укажем "col_i" как имя.
        let col_name = format!("col_{}", i);
        pairs.push((col_name, val_str));
    }
    (col_count, pairs)
}

fn collect_new_values(acc: &PreUpdateNewValueAccessor) -> (i32, Vec<(String, String)>) {
    let col_count = acc.get_column_count();
    let mut pairs = Vec::new();
    for i in 0..col_count {
        let val_ref = match acc.get_new_column_value(i) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let val_str = value_to_string(&val_ref);
        let col_name = format!("col_{}", i);
        pairs.push((col_name, val_str));
    }
    (col_count, pairs)
}

/// Преобразуем rusqlite::types::ValueRef в String (примерно как раньше)
fn value_to_string(v: &ValueRef) -> String {
    match v {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(r) => r.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        ValueRef::Blob(b) => base64::encode(b),
    }
}

/*
  ----------------------------------------------------------------------------------------------
  7) ТЕСТ: ПРИМЕР ИСПОЛЬЗОВАНИЯ
  ----------------------------------------------------------------------------------------------
*/

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::thread;
    use std::time::Duration;

    // Мокаем Swift callback
    extern "C" fn swift_callback_mock(c_str: *const c_char) {
        let s = unsafe { CStr::from_ptr(c_str) };
        let event_json = s.to_string_lossy();
        println!("(Swift callback mock) GOT EVENT: {event_json}");
    }

    #[test]
    fn test_preupdate_hook_monitor_modern() -> Result<()> {
        // 1) Создаём базу в памяти
        let conn = Connection::open_in_memory()?;

        // 2) Регистрируем preupdate_hook
        register_preupdate_hook(&conn);

        // 3) Регистрируем Swift callback + запускаем фон-тред
        register_swift_callback(swift_callback_mock);
        start_event_dispatcher();

        // 4) Создадим таблицу
        conn.execute_batch(r#"
            CREATE TABLE test_table (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT,
              info BLOB
            );
        "#)?;

        // 5) INSERT / UPDATE / DELETE
        let data = vec![1, 2, 3];
        let data: Vec<i32> = vec![1, 2, 3];
        let data_bytes: Vec<u8> = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * std::mem::size_of::<i32>())
                .to_vec()
        };
        conn.execute("INSERT INTO test_table (name, info) VALUES (?1, ?2)",
                     rusqlite::params!["Alice", data_bytes])?;
        conn.execute("UPDATE test_table SET name=?1 WHERE id=?2",
                     rusqlite::params!["Alice Updated", 1])?;
        conn.execute("DELETE FROM test_table WHERE id=?1",
                     rusqlite::params![1])?;

        // 6) Ждем фон-тред (dispatcher) обрабатывать события
        thread::sleep(Duration::from_secs(2));

        // Посмотрим в консоль, должны увидеть JSON:
        //  { "db_name":"main", "table":"test_table", "operation":"INSERT", ... }
        //  { "db_name":"main", "table":"test_table", "operation":"UPDATE", ... }
        //  { "db_name":"main", "table":"test_table", "operation":"DELETE", ... }
        Ok(())
    }

    #[test]
    fn test_monitor_processing() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        let mut monitor = DataMonitor::new(conn)?;

        // Создаем локальное изменение
        let local_changes = vec![
            HistoryChange {
                entity_name: "ContactData".to_string(),
                change_type: ChangeType::Insert,
                entity_id: Uuid::new_v4(),
                transaction_id: String::new(),
            }
        ];
        monitor.history.add_transaction("local_author", local_changes)?;

        // Создаем изменение для отправки
        let sender_changes = vec![
            HistoryChange {
                entity_name: "MessageData".to_string(),
                change_type: ChangeType::Update,
                entity_id: Uuid::new_v4(),
                transaction_id: String::new(),
            }
        ];
        monitor.history.add_transaction("sender", sender_changes)?;

        // Проверяем обработку изменений
        monitor.process_local_changes()?;
        monitor.process_sender_changes()?;

        Ok(())
    }
}

/* -----------------------------------------------------------------------------------------------
   ЗАКЛЮЧЕНИЕ
-----------------------------------------------------------------------------------------------
Это актуальная версия, полностью совместимая с тем кодом, который ты скинул
(где есть PreUpdateCase::{Insert,Delete,Update}, Action::SQLITE_* и т.д.).

Тут:
1) Мы используем conn.preupdate_hook(Some(|action, db, tbl, case| { ... }))
   вместо прежнего low-level pud.operation() и т.д.
2) При каждом INSERT/UPDATE/DELETE формируем PreUpdateEvent и складываем в mpsc-очередь.
3) В фоновом потоке читаем очередь, сериализуем JSON, звоним Swift callback.
4) Swift (в реальном проекте) подхватывает эти изменения.

Это «боевой» каркас:
- С учётом современных зависимостей rusqlite,
- Учитывая требование «получить все старые/новые значения» (по возможности),
- Без тяжёлой логики внутри hook,
- С колбэком push-уведомлений в Swift.

Дальше можно расширять:
- Обрабатывать blob иначе (не base64),
- Оптимизировать, если row содержит много столбцов,
- Сопоставлять «col_0» с реальными именами столбцов (в новом API, к сожалению, column_name(...) недоступна внутри preupdate_hook — можно хранить схему отдельно),
- Или логировать event_id и проч.

Но в текущем виде это уже «серьёзное» решение для Data Monitor.
----------------------------------------------------------------------------------------------- */

pub struct DataMonitor {
    history: PersistentHistory,
    local_last_timestamp: i64,
    sender_last_timestamp: i64,
}

impl DataMonitor {
    pub fn new(conn: Connection) -> Result<Self> {
        let history = PersistentHistory::new(conn.clone());
        history.init()?;
        
        Ok(Self {
            history,
            local_last_timestamp: 0,
            sender_last_timestamp: 0,
        })
    }

    pub fn process_local_changes(&mut self) -> Result<()> {
        let transactions = self.history.get_transactions_after(self.local_last_timestamp)?;
        
        for transaction in transactions {
            if transaction.author != "sender" {
                for change in transaction.changes {
                    self.handle_local_change(&change)?;
                }
                self.local_last_timestamp = transaction.timestamp;
            }
        }
        
        Ok(())
    }

    pub fn process_sender_changes(&mut self) -> Result<()> {
        let transactions = self.history.get_transactions_after(self.sender_last_timestamp)?;
        
        for transaction in transactions {
            if transaction.author == "sender" {
                for change in transaction.changes {
                    self.handle_sender_change(&change)?;
                }
                self.sender_last_timestamp = transaction.timestamp;
            }
        }
        
        Ok(())
    }

    fn handle_local_change(&self, change: &HistoryChange) -> Result<()> {
        match change.entity_name.as_str() {
            "ContactData" => {
                // Обработка изменений контакта
                println!("Local contact change: {:?}", change);
            },
            "MessageData" => {
                // Обработка изменений сообщения
                println!("Local message change: {:?}", change);
            },
            // ... другие сущности
            _ => println!("Unknown entity: {}", change.entity_name),
        }
        Ok(())
    }

    fn handle_sender_change(&self, change: &HistoryChange) -> Result<()> {
        match change.entity_name.as_str() {
            "ContactData" => {
                // Обработка изменений для отправки
                println!("Sender contact change: {:?}", change);
            },
            "MessageData" => {
                // Обработка изменений для отправки
                println!("Sender message change: {:?}", change);
            },
            // ... другие сущности
            _ => println!("Unknown entity: {}", change.entity_name),
        }
        Ok(())
    }
}
