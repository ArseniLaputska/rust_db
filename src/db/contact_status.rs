use tokio_rusqlite::{Connection, params};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use rusqlite::Transaction;

/// CREATE TABLE IF NOT EXISTS ...
pub async fn create_contact_status_table(conn: &Connection) -> Result<(), ContactStatusError> {
    // Вызываем conn.call(...) и внутри closure вызываем синхронный API.
    conn.call(|conn| {
        conn.execute(r#"
            CREATE TABLE IF NOT EXISTS contact_status (
                id BLOB PRIMARY KEY,
                status INTEGER
            )
        "#, [])?;
        Ok(())
    })
        .await
        .map_err(|e| ContactStatusError::Sql(e.to_string()))?;

    Ok(())
}

#[derive(Debug)]
pub enum ContactStatusError {
    Sql(String),
    Json(String),
    InvalidUuid(String),
    Other(String),
}
impl Display for ContactStatusError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ContactStatusError::Sql(e) => write!(f, "SqlError: {e}"),
            ContactStatusError::Json(e) => write!(f, "JsonError: {e}"),
            ContactStatusError::InvalidUuid(u) => write!(f, "Invalid UUID: {u}"),
            ContactStatusError::Other(o) => write!(f, "Other: {o}"),
        }
    }
}
impl Error for ContactStatusError {}

#[derive(Debug, Clone)]
pub struct ContactStatusData {
    pub id: Uuid,
    pub status: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContactStatusJsonIn {
    pub id: String,
    pub status: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContactStatusJsonOut {
    pub id: String,
    pub status: i64,
}

/// Асинхронный репозиторий для работы с contact_status.
///
/// - Храним `Arc<Connection>` (или ссылку, но обычно `Arc` удобнее).
/// - Все методы — `async fn`.
pub struct ContactStatusRepo {
    conn: std::sync::Arc<Connection>,
}

impl ContactStatusRepo {
    pub fn new(conn: std::sync::Arc<Connection>) -> Self {
        Self { conn }
    }

    /// Добавить/обновить статус по JSON + вернуть итоговое состояние как JSON.
    pub async fn add_status_json(&self, json_input: &str) -> Result<String, ContactStatusError> {
        // 1) Парсим JSON.
        let incoming: ContactStatusJsonIn = serde_json::from_str(json_input)
            .map_err(|e| ContactStatusError::Json(e.to_string()))?;

        // 2) Парсим UUID.
        let parsed_id = Uuid::parse_str(&incoming.id)
            .map_err(|_| ContactStatusError::InvalidUuid(incoming.id.clone()))?;

        // 3) Выполняем транзакцию внутри closure.
        //
        // conn.call(...) даст нам блокирующий &rusqlite::Connection => мы можем вызвать .unchecked_transaction().
        // Возвращаем финальный JSON.
        let final_json = self.conn.call(move |conn| {
            // --- Начало синхронного closure ---
            let tx = conn.unchecked_transaction()?;

            // SELECT
            let mut stmt = tx.prepare("SELECT status FROM contact_status WHERE id=?1")?;
            let mut rows = stmt.query(params![parsed_id.as_bytes()])?;
            let existing: Option<i64> = if let Some(row) = rows.next()? {
                Some(row.get::<_, i64>(0)?)
            } else {
                None
            };
            drop(stmt);

            // INSERT or UPDATE
            if let Some(_old_status) = existing {
                // UPDATE
                tx.execute(
                    "UPDATE contact_status SET status=?1 WHERE id=?2",
                    params![incoming.status, parsed_id.as_bytes()],
                )?;
            } else {
                // INSERT
                tx.execute(
                    "INSERT INTO contact_status (id, status) VALUES (?1, ?2)",
                    params![parsed_id.as_bytes(), incoming.status],
                )?;
            }

            tx.commit()?;

            // Возвращаем финальное состояние (читаем ещё раз).
            let mut stmt2 = conn.prepare("SELECT status FROM contact_status WHERE id=?1")?;
            let mut rows2 = stmt2.query(params![parsed_id.as_bytes()])?;
            if let Some(row2) = rows2.next()? {
                let st: i64 = row2.get(0)?;
                let out_obj = ContactStatusJsonOut {
                    id: parsed_id.to_string(),
                    status: st,
                };
                // сериализуем
                let out = serde_json::to_string(&out_obj)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                Ok(out) // возвращаем Ok(String)
            } else {
                // если не нашли => вернём "{}"
                Ok("{}".to_string())
            }
            // --- Конец синхронного closure ---
        })
            .await // дожидаемся Future
            .map_err(|e| ContactStatusError::Sql(e.to_string()))?;

        Ok(final_json)
    }

    /// Вернуть все статус‑записи одним JSON‑массивом
    pub async fn all_contacts_status_json(&self) -> Result<String, ContactStatusError> {
        let json_str = self.conn.call(|conn| {
            // Синхронный код:
            let mut stmt = conn.prepare("SELECT id, status FROM contact_status")?;
            let mut rows = stmt.query(params![])?;

            let mut results = Vec::new();
            while let Some(row) = rows.next()? {
                let blob: Vec<u8> = row.get(0)?;
                let st: i64 = row.get(1)?;
                if blob.len() == 16 {
                    if let Ok(uid) = Uuid::from_slice(&blob) {
                        results.push(ContactStatusJsonOut {
                            id: uid.to_string(),
                            status: st,
                        });
                    }
                }
            }

            // Сериализуем
            let out_json = serde_json::to_string(&results)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
            Ok(out_json)
        })
            .await
            .map_err(|e| ContactStatusError::Sql(e.to_string()))?;

        Ok(json_str)
    }
}
