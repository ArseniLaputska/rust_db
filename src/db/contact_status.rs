use rusqlite::{Connection, Result, params, Transaction};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::error::Error;

// Внутренняя структура для БД
#[derive(Debug, Clone)]
pub struct ContactStatusData {
    pub id: Uuid,
    pub status: i64, // Int64
}

// CREATE TABLE
pub fn create_contact_status_table(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS contact_status (
            id BLOB PRIMARY KEY,
            status INTEGER
        )
        "#,
        [],
    )?;
    Ok(())
}

// Ошибки
#[derive(Debug)]
pub enum ContactStatusError {
    Sql(String),
    Json(String),
    InvalidUuid(String),
    Other(String),
}

impl std::fmt::Display for ContactStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContactStatusError::Sql(e) => write!(f, "SqlError: {e}"),
            ContactStatusError::Json(e) => write!(f, "JsonError: {e}"),
            ContactStatusError::InvalidUuid(u) => write!(f, "Invalid UUID: {u}"),
            ContactStatusError::Other(o) => write!(f, "Other: {o}"),
        }
    }
}
impl Error for ContactStatusError {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContactStatusJsonIn {
    pub id: String,      // UUID в строке
    pub status: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContactStatusJsonOut {
    pub id: String,
    pub status: i64,
}

pub struct ContactStatusRepo<'a> {
    conn: &'a Connection,
}

impl<'a> ContactStatusRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    // add_status_json:
    // Аналог Swift-кода:
    //   if let model = try? await modelContext.get(status: id) { update } else { insert }
    //   возвращаем JSON
    pub fn add_status_json(&self, json_input: &str) -> Result<String, ContactStatusError> {
        // 1) Парсим JSON
        let incoming: ContactStatusJsonIn = serde_json::from_str(json_input)
            .map_err(|e| ContactStatusError::Json(e.to_string()))?;

        // 2) UUID
        let parsed_id = Uuid::parse_str(&incoming.id)
            .map_err(|_| ContactStatusError::InvalidUuid(incoming.id.clone()))?;

        // 3) Транзакция
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| ContactStatusError::Sql(e.to_string()))?;

        // 4) Проверяем наличие
        let existing = self.select_inner_tx(&tx, parsed_id)?;
        if let Some(mut old) = existing {
            // update
            old.status = incoming.status;
            self.update_inner_tx(&tx, &old)?;
        } else {
            // insert
            let new_data = ContactStatusData {
                id: parsed_id,
                status: incoming.status,
            };
            self.insert_inner_tx(&tx, &new_data)?;
        }

        tx.commit().map_err(|e| ContactStatusError::Sql(e.to_string()))?;

        // возвращаем финальное состояние
        let final_data = self.select_inner(parsed_id)?;
        if let Some(fd) = final_data {
            let out_json = serde_json::to_string(&ContactStatusJsonOut {
                id: fd.id.to_string(),
                status: fd.status
            }).map_err(|e| ContactStatusError::Json(e.to_string()))?;
            Ok(out_json)
        } else {
            Ok("{}".to_string())
        }
    }

    // all_contacts_status -> возвращает массив JSON
    // аналог Swift: allContactsStatus() -> [ContactStatusStruct]
    pub fn all_contacts_status_json(&self) -> Result<String, ContactStatusError> {
        // SELECT * FROM contact_status
        let mut stmt = self.conn.prepare(
            "SELECT id, status FROM contact_status"
        ).map_err(|e| ContactStatusError::Sql(e.to_string()))?;

        let mut rows = stmt.query([])
            .map_err(|e| ContactStatusError::Sql(e.to_string()))?;

        let mut results: Vec<ContactStatusJsonOut> = Vec::new();
        while let Some(row) = rows.next().map_err(|e| ContactStatusError::Sql(e.to_string()))? {
            let blob: Vec<u8> = row.get(0).map_err(|e| ContactStatusError::Sql(e.to_string()))?;
            let uuid = if blob.len() == 16 {
                Uuid::from_slice(&blob).ok()
            } else {
                None
            };
            let status: i64 = row.get(1).map_err(|e| ContactStatusError::Sql(e.to_string()))?;

            if let Some(uid) = uuid {
                results.push(ContactStatusJsonOut {
                    id: uid.to_string(),
                    status
                });
            }
        }

        let out_json = serde_json::to_string(&results)
            .map_err(|e| ContactStatusError::Json(e.to_string()))?;
        Ok(out_json)
    }

    // Вспомогательные SELECT/INSERT/UPDATE (внутренние)
    fn select_inner_tx(&self, tx: &Transaction, id: Uuid) -> Result<Option<ContactStatusData>, ContactStatusError> {
        let mut stmt = tx.prepare("SELECT status FROM contact_status WHERE id=?1")
            .map_err(|e| ContactStatusError::Sql(e.to_string()))?;
        let mut rows = stmt.query(params![&id.as_bytes()])
            .map_err(|e| ContactStatusError::Sql(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| ContactStatusError::Sql(e.to_string()))? {
            let status: i64 = row.get(0).map_err(|e| ContactStatusError::Sql(e.to_string()))?;
            Ok(Some(ContactStatusData { id, status }))
        } else {
            Ok(None)
        }
    }

    fn select_inner(&self, id: Uuid) -> Result<Option<ContactStatusData>, ContactStatusError> {
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| ContactStatusError::Sql(e.to_string()))?;
        let result = self.select_inner_tx(&tx, id)?;
        tx.commit().ok();
        Ok(result)
    }

    fn insert_inner_tx(&self, tx: &Transaction, data: &ContactStatusData) -> Result<(), ContactStatusError> {
        tx.execute(
            "INSERT INTO contact_status (id, status) VALUES (?1, ?2)",
            params![&data.id.as_bytes(), data.status],
        ).map_err(|e| ContactStatusError::Sql(e.to_string()))?;
        Ok(())
    }

    fn update_inner_tx(&self, tx: &Transaction, data: &ContactStatusData) -> Result<(), ContactStatusError> {
        tx.execute(
            "UPDATE contact_status SET status=?1 WHERE id=?2",
            params![data.status, &data.id.as_bytes()],
        ).map_err(|e| ContactStatusError::Sql(e.to_string()))?;
        Ok(())
    }
}

// ТЕСТ
#[cfg(test)]
mod test_contact_status {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_contact_status_repo() -> Result<(), Box<dyn std::error::Error>> {
        let conn = Connection::open_in_memory()?;
        create_contact_status_table(&conn)?;
        let repo = ContactStatusRepo::new(&conn);

        // 1) Add
        let input = r#"{"id":"3a6fa8e1-ce94-4975-86ab-2f5e37bcc85e","status":123}"#;
        let out = repo.add_status_json(input)?;
        println!("add_status_json -> {out}");

        // 2) get all
        let all_json = repo.all_contacts_status_json()?;
        println!("all_contacts_status_json -> {all_json}");

        Ok(())
    }
}