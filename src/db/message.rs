use rusqlite::{Connection, Result, params, Transaction};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::str::FromStr;

#[derive(Debug)]
pub enum MessageRepoError {
    SqlError(String),
    JsonError(String),
    InvalidUuid(String),
    Other(String),
}

impl std::fmt::Display for MessageRepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRepoError::SqlError(e) => write!(f, "SqlError: {e}"),
            MessageRepoError::JsonError(e) => write!(f, "JsonError: {e}"),
            MessageRepoError::InvalidUuid(u) => write!(f, "InvalidUuid: {u}"),
            MessageRepoError::Other(e) => write!(f, "Other: {e}"),
        }
    }
}
impl Error for MessageRepoError {}

struct Logger {}
impl Logger {
    fn debug(msg: &str) {
        println!("[DEBUG] {msg}");
    }
    fn error(msg: &str) {
        eprintln!("[ERROR] {msg}");
    }
    fn warning(msg: &str) {
        eprintln!("[WARNING] {msg}");
    }
    fn info(msg: &str) {
        println!("[INFO] {msg}");
    }
    fn trace(msg: &str) {
        println!("[TRACE] {msg}");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub id: Uuid,
    pub from: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub picture_url: Option<String>,
    pub to: Uuid,
    pub status: i64,
    pub text: Option<String>,
    pub ip: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateDescription {
    pub updated_fields: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageProtobuf {
    pub id: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub contact_id: Option<String>,
    pub status: TolkiMessageV1Status,
    pub audio_data: Option<Vec<u8>>,
    pub text: Option<String>,
    pub client_text: Option<String>,
    pub server_text: Option<String>,
    pub gpt_text: Option<String>,
    pub translated_text: HashMap<String, String>,
    pub language: Option<String>,
    pub audio_url: Option<String>,
    pub duration: Option<f64>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

impl MessageProtobuf {
    // Возвращает true, если `from` задано.
    pub fn has_from(&self) -> bool {
        self.from.is_some()
    }

    // Очищает значение `from`.
    pub fn clear_from(&mut self) {
        self.from = None;
    }

    // Возвращает true, если `to` задано.
    pub fn has_to(&self) -> bool {
        self.to.is_some()
    }

    // Очищает значение `to`.
    pub fn clear_to(&mut self) {
        self.to = None;
    }

    // Возвращает true, если `contact_id` задано.
    pub fn has_contact_id(&self) -> bool {
        self.contact_id.is_some()
    }

    // Очищает значение `contact_id`.
    pub fn clear_contact_id(&mut self) {
        self.contact_id = None;
    }

    // Возвращает true, если `audio_data` задано.
    pub fn has_audio_data(&self) -> bool {
        self.audio_data.is_some()
    }

    // Очищает значение `audio_data`.
    pub fn clear_audio_data(&mut self) {
        self.audio_data = None;
    }

    // Возвращает true, если `text` задано.
    pub fn has_text(&self) -> bool {
        self.text.is_some()
    }

    // Очищает значение `text`.
    pub fn clear_text(&mut self) {
        self.text = None;
    }

    // Возвращает true, если `client_text` задано.
    pub fn has_client_text(&self) -> bool {
        self.client_text.is_some()
    }

    // Очищает значение `client_text`.
    pub fn clear_client_text(&mut self) {
        self.client_text = None;
    }

    // Возвращает true, если `server_text` задано.
    pub fn has_server_text(&self) -> bool {
        self.server_text.is_some()
    }

    // Очищает значение `server_text`.
    pub fn clear_server_text(&mut self) {
        self.server_text = None;
    }

    // Возвращает true, если `gpt_text` задано.
    pub fn has_gpt_text(&self) -> bool {
        self.gpt_text.is_some()
    }

    // Очищает значение `gpt_text`.
    pub fn clear_gpt_text(&mut self) {
        self.gpt_text = None;
    }

    // Возвращает true, если `language` задано.
    pub fn has_language(&self) -> bool {
        self.language.is_some()
    }

    // Очищает значение `language`.
    pub fn clear_language(&mut self) {
        self.language = None;
    }

    // Возвращает true, если `audio_url` задано.
    pub fn has_audio_url(&self) -> bool {
        self.audio_url.is_some()
    }

    // Очищает значение `audio_url`.
    pub fn clear_audio_url(&mut self) {
        self.audio_url = None;
    }

    // Возвращает true, если `duration` задано.
    pub fn has_duration(&self) -> bool {
        self.duration.is_some()
    }

    // Очищает значение `duration`.
    pub fn clear_duration(&mut self) {
        self.duration = None;
    }

    // Возвращает true, если `created_at` задано.
    pub fn has_created_at(&self) -> bool {
        self.created_at.is_some()
    }

    // Очищает значение `created_at`.
    pub fn clear_created_at(&mut self) {
        self.created_at = None;
    }

    // Возвращает true, если `updated_at` задано.
    pub fn has_updated_at(&self) -> bool {
        self.updated_at.is_some()
    }

    // Очищает значение `updated_at`.
    pub fn clear_updated_at(&mut self) {
        self.updated_at = None;
    }
}

// Дополнительные перечисления для состояния сообщения
#[derive(Debug, Serialize, Deserialize)]
pub enum TolkiMessageV1Status {
    Sent,
    Delivered,
    Read,
    Failed,
}

impl Default for TolkiMessageV1Status {
    fn default() -> Self {
        TolkiMessageV1Status::Sent
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub from: Uuid,
    pub to: Uuid,
    pub prev: Option<Uuid>,         // swift: prev: MessageData? -> UUID?
    pub contact_id: Uuid,   // swift: contactId: UUID?
    pub status: i64,
    pub audio_url: Option<String>,
    pub duration: f64,              // double
    pub text: Option<String>,
    pub client_text: Option<String>,
    pub gpt_text: Option<String>,
    pub server_text: Option<String>,
    pub translated_text: Option<String>, // NSDictionary -> JSON
    pub language: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub try_count: i64,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            id: Uuid::now_v7(),
            from: Uuid::now_v7(),
            to: Uuid::now_v7(),
            prev: None,
            contact_id: Uuid::now_v7(),
            status: 0,
            audio_url: None,
            duration: 0.0,
            text: None,
            client_text: None,
            gpt_text: None,
            server_text: None,
            translated_text: None,
            language: None,
            error: None,
            created_at: Self::current_timestamp(),
            updated_at: 0,
            try_count: 0,
        }
    }
}

impl Message {
    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
}


pub fn create_message_table(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS message_data (
            id BLOB PRIMARY KEY,
            "from" BLOB,
            "to" BLOB,
            prev BLOB,
            contact_id BLOB,
            status INTEGER,
            audio_url TEXT,
            duration REAL,
            text TEXT,
            client_text TEXT,
            gpt_text TEXT,
            server_text TEXT,
            translated_text TEXT,
            language TEXT,
            error TEXT,
            created_at INTEGER,
            updated_at INTEGER,
            try_count INTEGER
        )
        "#,
        [],
    )?;
    Ok(())
}

pub struct MessageRepo<'a> {
    conn: &'a Connection,
}

impl<'a> MessageRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn delete_message(&self, id: Uuid) -> Result<(), MessageRepoError> {
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let rows_affected = tx.execute(
            "DELETE FROM message_data WHERE id = ?1",
            params![&id.as_bytes()],
        ).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        // В Swift нет ошибки, если message нет.
        // Мы тоже просто продолжим.
        if rows_affected > 0 {
            Logger::debug(&format!("Deleted message {}", id));
        }

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn add_message_struct(&self, json_input: &str, send: bool) -> Result<(), MessageRepoError> {
        // Swift:
        // let add = {
        //   if let oldMessage = get(...) { oldMessage.update(with: messageStruct) }
        //   else { create new record }
        //   save
        // }
        // if send { modelContext.send(add) } else { add() }

        // У нас "send" условно трактуем как "делать транзакцию"?
        // (В Swift modelContext.send — транзакция + author = sender).
        // Для упрощения — всегда делаем транзакцию:
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let msg: Message = serde_json::from_str(json_input)
            .map_err(|e| MessageRepoError::JsonError(e.to_string()))?;

        let existing = self.get_message_inner(&tx, msg.id)?;
        if let Some(mut old_msg) = existing {
            // Обновляем поля (т.е. "update(with: messageStruct)")
            old_msg.from = msg.from;
            old_msg.to = msg.to;
            old_msg.contact_id = msg.contact_id;
            old_msg.updated_at = Message::current_timestamp();
            old_msg.client_text = msg.client_text;
            // ... при желании копировать остальные

            self.update_message_inner(&tx, &old_msg)?;
        } else {
            // create new
            let new_data = Message {
                id: msg.id,
                from: msg.from,
                to: msg.to,
                prev: None,
                contact_id: msg.contact_id,
                status: 0,
                audio_url: None,
                duration: 0.0,
                text: None,
                client_text: msg.client_text,
                gpt_text: None,
                server_text: None,
                translated_text: None,
                language: None,
                error: None,
                created_at: Message::current_timestamp(),
                updated_at: Message::current_timestamp(),
                try_count: 0,
            };
            self.insert_message_inner(&tx, &new_data)?;
        }

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn add_incomming_message(&self, json_input: &str) -> Result<(), MessageRepoError> {
        // Swift:
        // if let _ = get(message: incommingMessage.id) { return }
        // ... create new record ...
        let inc: IncomingMessage = serde_json::from_str(json_input)
            .map_err(|e| MessageRepoError::JsonError(e.to_string()))?;

        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let exists = self.get_message_inner(&tx, inc.id)?;
        if exists.is_some() {
            Logger::debug("Message already exists, skip");
            tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            return Ok(());
        }

        let new_data = Message {
            id: inc.id,
            text: inc.text,
            created_at: Message::current_timestamp(),
            updated_at: Message::current_timestamp(),
            ..Default::default()
        };

        self.insert_message_inner(&tx, &new_data)?;

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn add_message_payload(&self, payload: &HashMap<String, String>) -> Result<(), MessageRepoError> {
        let start_time = std::time::Instant::now();

        // Swift-код:
        // if let id = pushPayload["id"] as? String, let uuid = UUID(uuidString: id), get(message: uuid) != nil { return }
        // else -> insert

        let maybe_id = payload.get("id");
        let id_uuid = if let Some(id_str) = maybe_id {
            if let Ok(u) = Uuid::parse_str(id_str) {
                u
            } else {
                Logger::warning("Invalid UUID in pushPayload");
                return Ok(());
            }
        } else {
            Logger::warning("No 'id' in pushPayload");
            return Ok(());
        };

        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let existing = self.get_message_inner(&tx, id_uuid)?;
        if existing.is_some() {
            Logger::debug("Message already exists, skip");
            tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            return Ok(());
        }

        // Создаём запись
        let new_data = Message {
            id: id_uuid,
            text: payload.get("text").cloned(),
            created_at: Message::current_timestamp(),
            updated_at: Message::current_timestamp(),
            ..Default::default()
        };
        self.insert_message_inner(&tx, &new_data)?;

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let duration = start_time.elapsed().as_secs_f64();
        Logger::debug(&format!("Add message took {duration} seconds"));
        Ok(())
    }

    pub fn add_message_pb(&self, json_input: &str) -> Result<(), MessageRepoError> {
        // guard let messageId = UUID(uuidString: messagepb.id) else { throw .badValues }
        let pb: MessageProtobuf = serde_json::from_str(json_input)
            .map_err(|e| MessageRepoError::JsonError(e.to_string()))?;
        let uuid = Uuid::parse_str(&pb.id).map_err(|_| MessageRepoError::Other("Wrong uuid".to_string()))?;

        // if messagepb.hasAudioData && !messagepb.audioData.isEmpty => сохранить файл
        if pb.has_audio_data() {
            if let Some(audio_data) = pb.audio_data {
                // Логирование для отладки
                println!("Saving audio for message {}", pb.id);

                // Создание каталога, если он не существует
                let _ = fs::create_dir_all("audio");

                // Путь для записи
                let path = format!("audio/{}.ogg", pb.id);

                // Запись аудио в файл
                let _ = fs::write(path, audio_data);
            }
        }

        // let _: Message = try await add(messagepb, id: messageId)
        // => Здесь делаем "если уже есть, обновить, иначе вставить"
        // Для простоты — insert or update
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let existing = self.get_message_inner(&tx, uuid)?;
        if let Some(mut old_msg) = existing {
            // update
            old_msg.updated_at = Message::current_timestamp();
            self.update_message_inner(&tx, &old_msg)?;
        } else {
            // insert
            let new_msg = Message {
                id: uuid,
                created_at: Message::current_timestamp(),
                updated_at: Message::current_timestamp(),
                ..Default::default()
            };
            self.insert_message_inner(&tx, &new_msg)?;
        }

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        Ok(())
    }

    pub fn update_message_client_text(&self, id_str: &str, client_text: &str) -> Result<(), MessageRepoError> {
        let id = Uuid::from_str(id_str).unwrap();

        if client_text.is_empty() {
            Logger::error(&format!("update message id: {id} text: (empty) not found"));
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let mut old_msg = match self.get_message_inner(&tx, id)? {
            Some(m) => m,
            None => {
                Logger::error(&format!("update message id: {id} text: {client_text} not found"));
                tx.commit().unwrap_or_default();
                return Ok(());
            }
        };
        Logger::trace(&format!("update message id: {id} text: {client_text}"));
        old_msg.client_text = Some(client_text.to_string());
        old_msg.updated_at = Message::current_timestamp();
        self.update_message_inner(&tx, &old_msg)?;

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn update_message_status(&self, id_str: &str, status: i64) -> Result<(), MessageRepoError> {
        let id = Uuid::from_str(id_str).unwrap();

        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let mut old_msg = match self.get_message_inner(&tx, id)? {
            Some(m) => m,
            None => return Ok(()),
        };
        Logger::trace(&format!("update status message id: {id} status: {status}"));
        old_msg.status = status;
        if status == 0 {
            // Swift-код: if status == .unspecified => old.error = nil
            old_msg.error = None;
        }
        old_msg.updated_at = Message::current_timestamp();
        self.update_message_inner(&tx, &old_msg)?;

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn update_message_status_with_date(&self, id_str: &str, status: i64, updated_at: i64) -> Result<(), MessageRepoError> {
        let id = Uuid::from_str(id_str).unwrap();

        Logger::trace(&format!("update message status: {}", id));
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let mut old_msg = match self.get_message_inner(&tx, id)? {
            Some(m) => m,
            None => {
                tx.commit().unwrap_or_default();
                return Ok(())
            },
        };
        old_msg.status = status;
        old_msg.updated_at = updated_at;
        self.update_message_inner(&tx, &old_msg)?;

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn retry_message(&self, id_str: &str, force: bool) -> Result<(), MessageRepoError> {
        let id = Uuid::from_str(id_str).unwrap();

        Logger::trace(&format!("retry message: {id}"));
        // let dt = DataTransport::shared();
        // if !force && !dt.monitor_current_path_satisfied() {
        //     // в Swift: guard force || DataTransport.shared.monitor.currentPath.status == .satisfied else { return }
        //     return Ok(());
        // }

        // await RetryCounter.shared.remove(for: id)
        // RetryCounter::shared().remove(id);

        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let mut old_msg = match self.get_message_inner(&tx, id)? {
            Some(m) => m,
            None => {
                tx.commit().unwrap_or_default();
                return Ok(())
            }
        };

        old_msg.status = -1;
        old_msg.updated_at = Message::current_timestamp();
        old_msg.error = None;
        self.update_message_inner(&tx, &old_msg)?;

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn update_message_error(&self, id_str: &str, err: &str) -> Result<(), MessageRepoError> {
        let id = Uuid::from_str(id_str).unwrap();
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let mut msg = match self.get_message_inner(&tx, id)? {
            Some(m) => m,
            None => {
                tx.commit().unwrap_or_default();
                return Ok(())
            }
        };
        msg.status = /* MessageStatusCode.error.rawValue */ 999;
        msg.error = Some(err.to_string());
        msg.updated_at = Message::current_timestamp();

        self.update_message_inner(&tx, &msg)?;

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn update_message_with_description(
        &self,
        id_str: &str,
        desc_json: &str
    ) -> Result<(), MessageRepoError> {
        let id = Uuid::from_str(id_str).unwrap();
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        let mut msg = match self.get_message_inner(&tx, id)? {
            Some(m) => m,
            None => {
                Logger::warning(&format!("Could not find message {}", id));
                tx.commit().unwrap_or_default();
                return Ok(())
            }
        };

        let update_desc: UpdateDescription = serde_json::from_str(desc_json)
            .map_err(|e| MessageRepoError::JsonError(e.to_string()))?;

        for (key, value) in &update_desc.updated_fields {
            // match ключей
            match key.as_str() {
                "text" => msg.text = Some(value.clone()),
                "clientText" => msg.client_text = Some(value.clone()),
                "gptText" => msg.gpt_text = Some(value.clone()),
                "serverText" => msg.server_text = Some(value.clone()),
                "translatedText" => msg.translated_text = Some(value.clone()),
                "language" => msg.language = Some(value.clone()),
                "error" => msg.error = Some(value.clone()),
                "status" => {
                    if let Ok(num) = value.parse::<i64>() {
                        msg.status = num;
                    }
                }
                "tryCount" => {
                    if let Ok(num) = value.parse::<i64>() {
                        msg.try_count = num;
                    }
                }
                "updatedAt" => {
                    if let Ok(num) = value.parse::<i64>() {
                        msg.updated_at = num;
                    }
                }
                _ => {
                    Logger::debug(&format!("update_message_with_description: unknown field {key}"));
                }
            }
        }
        msg.updated_at = Message::current_timestamp();

        self.update_message_inner(&tx, &msg)?;
        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn update_messages_status(&self, contact_id_str: &str, id_str: &str, last_seen_at: f64) -> Result<(), MessageRepoError> {
        let userId = Uuid::from_str(id_str).unwrap();
        let contact_id = Uuid::from_str(contact_id_str).unwrap();
        Logger::debug(&format!("updateMessagesStatus contactId: {contact_id}, lastSeenAt: {last_seen_at}"));

        // let user_id = UserModel::id();
        // if user_id.is_none() {
        //     return Ok(());
        // }
        // let user_id = user_id.unwrap();

        // Swift:
        // status != 4, status != -1, createdAt <= lastSeenAt, from != userID, contactId == contact_id
        // set status=4
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        let sql = r#"
            UPDATE message_data
            SET status = 4
            WHERE contact_id = ?1
              AND "from" != ?2
              AND status != 4
              AND status != -1
              AND created_at <= ?3
        "#;
        // lastSeenAt: f64 -> i64?
        // В Swift Date(timeIntervalSince1970: lastSeenAt)
        // Будем считать, что created_at <= lastSeenAt (секунды).
        let last_seen_at_i64 = last_seen_at.floor() as i64;

        let result = tx.execute(
            sql,
            params![&contact_id.as_bytes(), &userId.as_bytes(), last_seen_at_i64],
        ).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Logger::info(&format!("Updated message status result: {result} rows"));

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn get_unread_messages(&self, id_str: &str) -> Result<i64, MessageRepoError> {
        // let user_id = match UserModel::id() {
        //     Some(u) => u,
        //     None => return Ok(0),
        // };
        let user_id = Uuid::from_str(id_str).unwrap();

        let sql = r#"
            SELECT COUNT(*) FROM message_data
            WHERE contact_id IS NOT NULL
              AND "from" != ?1
              AND "from" != "to"
              AND status != 4
        "#;
        let mut stmt = self.conn.prepare(sql).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        let mut rows = stmt.query(params![&user_id.as_bytes()]).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| MessageRepoError::SqlError(e.to_string()))? {
            let count: i64 = row.get(0).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            Ok(count)
        } else {
            Ok(0)
        }
    }

    pub fn add_messages_for_contact(&self, contact_id_str: &str, count: i64) -> Result<(), MessageRepoError> {
        // Swift: for _ in 0..<count { ... }
        let contact_id = Uuid::from_str(contact_id_str).unwrap();
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        for _ in 0..count {
            let mid = Uuid::now_v7();
            let random_text = self.random_string(20);
            let created = 1731330738; // from Swift code
            let msg = Message {
                id: mid,
                contact_id: contact_id,
                from: contact_id,
                to: contact_id,
                text: None,
                client_text: Some(random_text),
                created_at: created,
                updated_at: created,
                ..Default::default()
            };
            self.insert_message_inner(&tx, &msg)?;
        }

        tx.commit().map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    pub fn random_string(&self, length: usize) -> String {
        use rand::{thread_rng, Rng};
        use rand::distr::Alphanumeric;
        let rng = thread_rng();
        rng.sample_iter(&Alphanumeric)
            .take(length)
            .map(char::from)
            .collect()
    }

    fn insert_message_inner(&self, tx: &Transaction, msg: &Message) -> Result<(), MessageRepoError> {
        let sql = r#"
        INSERT INTO message_data (
            id, "from", "to", prev,
            contact_id, status, audio_url, duration,
            text, client_text, gpt_text, server_text,
            translated_text, language, error,
            created_at, updated_at, try_count
        )
        VALUES (?1, ?2, ?3, ?4,
                ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12,
                ?13, ?14, ?15,
                ?16, ?17, ?18)
    "#;
        tx.execute(
            sql,
            params![
            &msg.id.as_bytes(),                     // ?1: &[u8]
            msg.from.as_bytes(),                    // ?2: &[u8]
            msg.to.as_bytes(),                      // ?3: &[u8]
            msg.prev.as_ref().map(|u| &u.as_bytes()[..]), // ?4: Option<&[u8]>
            msg.contact_id.as_bytes(),              // ?5: &[u8]
            msg.status,                             // ?6: i64
            msg.audio_url.as_ref().map(|s| s.as_str()), // ?7: Option<&str>
            msg.duration,                           // ?8: f64
            msg.text.as_ref().map(|s| s.as_str()), // ?9: Option<&str>
            msg.client_text.as_ref().map(|s| s.as_str()), // ?10: Option<&str>
            msg.gpt_text.as_ref().map(|s| s.as_str()),    // ?11: Option<&str>
            msg.server_text.as_ref().map(|s| s.as_str()), // ?12: Option<&str>
            msg.translated_text.as_ref().map(|s| s.as_str()), // ?13: Option<&str>
            msg.language.as_ref().map(|s| s.as_str()),       // ?14: Option<&str>
            msg.error.as_ref().map(|s| s.as_str()),          // ?15: Option<&str>
            msg.created_at,                                 // ?16: i64
            msg.updated_at,                                 // ?17: i64
            msg.try_count,                                  // ?18: i64
        ],
        ).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        Ok(())
    }

    fn update_message_inner(&self, tx: &Transaction, msg: &Message) -> Result<(), MessageRepoError> {
        let sql = r#"
        UPDATE message_data
        SET "from" = ?1,
            "to" = ?2,
            prev = ?3,
            contact_id = ?4,
            status = ?5,
            audio_url = ?6,
            duration = ?7,
            text = ?8,
            client_text = ?9,
            gpt_text = ?10,
            server_text = ?11,
            translated_text = ?12,
            language = ?13,
            error = ?14,
            created_at = ?15,
            updated_at = ?16,
            try_count = ?17
        WHERE id = ?18
    "#;

        // Преобразуем `Option<Uuid>` в `Option<&[u8]>`
        let prev_param: Option<&[u8]> = msg.prev.as_ref().map(|u| &u.as_bytes()[..]);

        // Преобразуем остальные опциональные поля
        let audio_url = msg.audio_url.as_ref().map(|s| s.as_str());
        let text = msg.text.as_ref().map(|s| s.as_str());
        let client_text = msg.client_text.as_ref().map(|s| s.as_str());
        let gpt_text = msg.gpt_text.as_ref().map(|s| s.as_str());
        let server_text = msg.server_text.as_ref().map(|s| s.as_str());
        let translated_text = msg.translated_text.as_ref().map(|s| s.as_str());
        let language = msg.language.as_ref().map(|s| s.as_str());
        let error = msg.error.as_ref().map(|s| s.as_str());

        tx.execute(
            sql,
            params![
            msg.from.as_bytes(),          // ?1: &[u8]
            msg.to.as_bytes(),            // ?2: &[u8]
            prev_param,                   // ?3: Option<&[u8]>
            msg.contact_id.as_bytes(),    // ?4: &[u8]
            msg.status,                   // ?5: i64
            audio_url,                    // ?6: Option<&str>
            msg.duration,                 // ?7: f64
            text,                         // ?8: Option<&str>
            client_text,                  // ?9: Option<&str>
            gpt_text,                     // ?10: Option<&str>
            server_text,                  // ?11: Option<&str>
            translated_text,              // ?12: Option<&str>
            language,                     // ?13: Option<&str>
            error,                        // ?14: Option<&str>
            msg.created_at,               // ?15: i64
            msg.updated_at,               // ?16: i64
            msg.try_count,                // ?17: i64
            msg.id.as_bytes(),            // ?18: &[u8]
        ],
        ).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        Ok(())
    }

    fn get_message_inner(&self, tx: &Transaction, id: Uuid) -> Result<Option<Message>, MessageRepoError> {
        let sql = r#"
        SELECT
            "from",
            "to",
            prev,
            contact_id,
            status,
            audio_url,
            duration,
            text,
            client_text,
            gpt_text,
            server_text,
            translated_text,
            language,
            error,
            created_at,
            updated_at,
            try_count
        FROM message_data
        WHERE id = ?1
    "#;
        let mut stmt = tx.prepare(sql).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
        let mut rows = stmt.query(params![&id.as_bytes()]).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| MessageRepoError::SqlError(e.to_string()))? {
            // parse
            let from_blob: Vec<u8> = row.get(0).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let to_blob: Vec<u8> = row.get(1).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let prev_blob: Option<Vec<u8>> = row.get(2).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let contact_id_blob: Vec<u8> = row.get(3).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let status: i64 = row.get(4).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let audio_url: Option<String> = row.get(5).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let duration: f64 = row.get(6).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let text: Option<String> = row.get(7).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let client_text: Option<String> = row.get(8).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let gpt_text: Option<String> = row.get(9).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let server_text: Option<String> = row.get(10).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let translated_text: Option<String> = row.get(11).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let language: Option<String> = row.get(12).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let error: Option<String> = row.get(13).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let created_at: i64 = row.get(14).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let updated_at: i64 = row.get(15).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;
            let try_count: i64 = row.get(16).map_err(|e| MessageRepoError::SqlError(e.to_string()))?;

            Ok(Some(Message {
                id,
                from: Self::to_uuid(from_blob),
                to: Self::to_uuid(to_blob),
                prev: prev_blob.map(Self::to_uuid), // Корректно обрабатываем Option
                contact_id: Self::to_uuid(contact_id_blob),
                status,
                audio_url,
                duration,
                text,
                client_text,
                gpt_text,
                server_text,
                translated_text,
                language,
                error,
                created_at,
                updated_at,
                try_count,
            }))
        } else {
            Ok(None)
        }
    }
    
    fn to_uuid(bytes: Vec<u8>) -> Uuid {
        if bytes.len() == 16 {
            Uuid::from_slice(&bytes).unwrap_or(Uuid::nil()) // Возвращаем nil UUID если ошибка
        } else {
            Uuid::nil() // Возвращаем nil UUID, если длина неправильная
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, Result};
    use uuid::Timestamp;

    // Функция для создания тестовой базы данных
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap(); // Используем in-memory базу
        create_message_table(&conn).unwrap();
        conn
    }

    // Тестируем создание новой записи
    #[test]
    fn test_add_incoming_message() {
        let conn = setup_test_db();
        let repo = MessageRepo::new(&conn);

        let json_input = r#"{
            "id": "123e4567-e89b-12d3-a456-426614174000",
            "from": "123e4567-e89b-12d3-a456-426614174001",
            "first_name": "John",
            "last_name": "Doe",
            "picture_url": "http://example.com/pic.jpg",
            "to": "123e4567-e89b-12d3-a456-426614174002",
            "status": 1,
            "text": "Hello",
            "ip": "192.168.1.1",
            "created_at": 1625078000,
            "updated_at": 1625078000
        }"#;

        let result = repo.add_incomming_message(json_input);
        println!("{:?}", result);
        assert!(result.is_ok(), "Failed to add incoming message");

        // Проверяем, что запись была добавлена в базу
        let query = "SELECT COUNT(*) FROM message_data";
        let mut stmt = conn.prepare(query).unwrap();
        let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(count, 1, "Expected one message in the database");
    }

    // Тестируем обновление существующей записи
    #[test]
    fn test_update_message_status() {
        let conn = setup_test_db();
        let repo = MessageRepo::new(&conn);

        let message_id = Uuid::now_v7();
        let message = Message {
            id: message_id,
            from: Uuid::now_v7(),
            to: Uuid::now_v7(),
            status: 0,
            ..Default::default()
        };

        let json_input = serde_json::to_string(&message).unwrap();
        repo.add_message_struct(&json_input, true).unwrap();

        // Проверим, что статус был обновлен
        let status = 1;
        let result = repo.update_message_status(&message_id.to_string(), status);
        println!("{:?}", result);
        assert!(result.is_ok(), "Failed to update message status");

        // Получаем обновленный статус
        let mut stmt = conn.prepare("SELECT status FROM message_data WHERE id = ?1").unwrap();
        let updated_status: i64 = stmt.query_row(params![message_id.as_bytes()], |row| row.get(0)).unwrap();
        assert_eq!(updated_status, status, "Status was not updated correctly");
    }

    // Тестируем удаление записи
    #[test]
    fn test_delete_message() {
        let conn = setup_test_db();
        let repo = MessageRepo::new(&conn);

        let message_id = Uuid::now_v7();
        let message = Message {
            id: message_id,
            from: Uuid::now_v7(),
            to: Uuid::now_v7(),
            status: 0,
            ..Default::default()
        };

        let json_input = serde_json::to_string(&message).unwrap();
        repo.add_message_struct(&json_input, true).unwrap();

        // Удаляем сообщение
        let result = repo.delete_message(message_id);
        assert!(result.is_ok(), "Failed to delete message");

        // Проверяем, что запись была удалена
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM message_data WHERE id = ?1").unwrap();
        let count: i64 = stmt.query_row(params![message_id.as_bytes()], |row| row.get(0)).unwrap();
        assert_eq!(count, 0, "Message was not deleted");
    }

    // Тестируем функцию для получения всех непрочитанных сообщений
    #[test]
    fn test_get_unread_messages() {
        let conn = setup_test_db();
        let repo = MessageRepo::new(&conn);

        let user_id = Uuid::now_v7();
        let contact_id = Uuid::now_v7();

        // Добавляем несколько сообщений
        for _ in 0..3 {
            let json_input = serde_json::json!({
            "id": Uuid::now_v7(),
            "from": "123e4567-e89b-12d3-a456-426614174002",
            "first_name": "John",
            "last_name": "Doe",
            "picture_url": "http://example.com/pic.jpg",
            "to": "123e4567-e89b-12d3-a456-426614174003",
            "status": 0,
            "text": "Hello",
            "ip": "192.168.1.1",
            "created_at": 1625078000,
            "updated_at": 1625078000
            }).to_string();

            let _ = repo.add_incomming_message(&*json_input.as_str());
        }

        // Получаем количество непрочитанных сообщений
        let result = repo.get_unread_messages(&*user_id.to_string());
        assert!(result.is_ok(), "Failed to get unread messages");
        assert_eq!(result.unwrap(), 3, "Unexpected unread message count");
    }
}