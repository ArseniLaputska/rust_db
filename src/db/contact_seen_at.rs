use tokio_rusqlite::{Connection, Result, params, Transaction};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone)]
pub struct ContactSeenAtData {
    pub id: Uuid,
    // храним JSON-строку
    pub date_json: Option<String>,
}

pub fn create_contact_seen_at_table(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS contact_seen_at (
            id BLOB PRIMARY KEY,
            date TEXT
        )
        "#,
        [],
    )?;
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContactSeenAtJsonIn {
    pub id: String,
    // Предположим, Swift передаёт словарь date как JSON в виде строки
    // Либо может быть `{ "userId": 123456.0 }`.
    // Тут два варианта:
    // 1) Либо делаем "date: HashMap<String,f64>",
    // 2) Либо "date_json: serde_json::Value".
    // Для наглядности выберем HashMap.
    pub date: Option<std::collections::HashMap<String, f64>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContactSeenAtJsonOut {
    pub id: String,
    pub date: Option<std::collections::HashMap<String, f64>>,
}

pub struct ContactSeenAtRepo<'a> {
    conn: &'a Connection,
}

#[derive(Debug)]
pub enum ContactSeenAtError {
    Sql(String),
    Json(String),
    InvalidUuid(String),
    Other(String),
}
impl std::fmt::Display for ContactSeenAtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContactSeenAtError::Sql(e) => write!(f, "SqlError: {e}"),
            ContactSeenAtError::Json(e) => write!(f, "JsonError: {e}"),
            ContactSeenAtError::InvalidUuid(u) => write!(f, "InvalidUUID: {u}"),
            ContactSeenAtError::Other(o) => write!(f, "Other: {o}"),
        }
    }
}
impl Error for ContactSeenAtError {}

impl<'a> ContactSeenAtRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    // add_seen_json
    // Аналог: func add(seen seenAt: Tolki_Contact_V1_ContactSeenAt)
    pub fn add_seen_json(&self, json_input: &str) -> Result<String, ContactSeenAtError> {
        let incoming: ContactSeenAtJsonIn = serde_json::from_str(json_input)
            .map_err(|e| ContactSeenAtError::Json(e.to_string()))?;

        let parsed_id = Uuid::parse_str(&incoming.id)
            .map_err(|_| ContactSeenAtError::InvalidUuid(incoming.id.clone()))?;

        // date -> serde_json
        let date_json_str = if let Some(ref map) = incoming.date {
            // превращаем HashMap<String,f64> в JSON-строку
            serde_json::to_string(map)
                .map_err(|e| ContactSeenAtError::Json(e.to_string()))?
        } else {
            "".to_string()
        };

        // Транзакция
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;

        // Проверим, есть ли уже запись
        let existing = self.select_inner_tx(&tx, parsed_id)?;
        if let Some(mut old) = existing {
            // обновим
            // Если хотим "объединять" старый словарь и новый, придётся мержить JSON.
            // В Swift-коде "var olddate: [String:Double] = ... for (k,v) in new { olddate[k] = v }" 
            // В Rust — подобная логика:
            let merged_str = merge_date_json(&old.date_json, &date_json_str)?;
            old.date_json = Some(merged_str);
            self.update_inner_tx(&tx, &old)?;
        } else {
            // вставим
            let new_data = ContactSeenAtData {
                id: parsed_id,
                date_json: Some(date_json_str),
            };
            self.insert_inner_tx(&tx, &new_data)?;
        }

        tx.commit().map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;

        // возвращаем финальное состояние
        let final_data = self.select_inner(parsed_id)?;
        let out_json = match final_data {
            Some(fd) => {
                // date_json -> HashMap<String,f64>
                let map_opt = if let Some(s) = fd.date_json {
                    if s.is_empty() {
                        None
                    } else {
                        serde_json::from_str::<std::collections::HashMap<String,f64>>(&s).ok()
                    }
                } else {
                    None
                };
                let out_struct = ContactSeenAtJsonOut {
                    id: fd.id.to_string(),
                    date: map_opt
                };
                serde_json::to_string(&out_struct)
                    .map_err(|e| ContactSeenAtError::Json(e.to_string()))?
            },
            None => "{}".to_string()
        };
        Ok(out_json)
    }

    // allSeenAt() -> [ContactSeenAtStruct]
    // аналог: func allSeenAt() throws -> [ContactSeenAtStruct]
    pub fn all_seen_json(&self) -> Result<String, ContactSeenAtError> {
        let mut stmt = self.conn.prepare("SELECT id, date FROM contact_seen_at")
            .map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;

        let mut rows = stmt.query([])
            .map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().map_err(|e| ContactSeenAtError::Sql(e.to_string()))? {
            let blob: Vec<u8> = row.get(0).map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;
            let date_str: Option<String> = row.get(1).map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;

            if blob.len() == 16 {
                if let Ok(uid) = Uuid::from_slice(&blob) {
                    let map_opt = if let Some(ds) = date_str {
                        if ds.is_empty() { None } else {
                            serde_json::from_str::<std::collections::HashMap<String,f64>>(&ds).ok()
                        }
                    } else {
                        None
                    };
                    results.push(ContactSeenAtJsonOut {
                        id: uid.to_string(),
                        date: map_opt
                    });
                }
            }
        }

        let out_json = serde_json::to_string(&results)
            .map_err(|e| ContactSeenAtError::Json(e.to_string()))?;
        Ok(out_json)
    }

    // private SELECT/INSERT/UPDATE
    fn select_inner_tx(&self, tx: &Transaction, id: Uuid) -> Result<Option<ContactSeenAtData>, ContactSeenAtError> {
        let mut stmt = tx.prepare("SELECT date FROM contact_seen_at WHERE id=?1")
            .map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;
        let mut rows = stmt.query(params![&id.as_bytes()])
            .map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| ContactSeenAtError::Sql(e.to_string()))? {
            let date_str: Option<String> = row.get(0).map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;
            Ok(Some(ContactSeenAtData { id, date_json: date_str }))
        } else {
            Ok(None)
        }
    }

    fn select_inner(&self, id: Uuid) -> Result<Option<ContactSeenAtData>, ContactSeenAtError> {
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;
        let result = self.select_inner_tx(&tx, id)?;
        tx.commit().ok();
        Ok(result)
    }

    fn insert_inner_tx(&self, tx: &Transaction, data: &ContactSeenAtData) -> Result<(), ContactSeenAtError> {
        tx.execute(
            "INSERT INTO contact_seen_at (id, date) VALUES (?1, ?2)",
            params![&data.id.as_bytes(), &data.date_json],
        ).map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;
        Ok(())
    }

    fn update_inner_tx(&self, tx: &Transaction, data: &ContactSeenAtData) -> Result<(), ContactSeenAtError> {
        tx.execute(
            "UPDATE contact_seen_at SET date=?1 WHERE id=?2",
            params![&data.date_json, &data.id.as_bytes()],
        ).map_err(|e| ContactSeenAtError::Sql(e.to_string()))?;
        Ok(())
    }
}

// Функция, чтобы «слить» старый JSON-словарь и новый
fn merge_date_json(old: &Option<String>, new_s: &str) -> Result<String, ContactSeenAtError> {
    // parse old map
    let old_map: std::collections::HashMap<String,f64> = if let Some(o) = old {
        if !o.is_empty() {
            serde_json::from_str(o).map_err(|e| ContactSeenAtError::Json(e.to_string()))?
        } else {
            std::collections::HashMap::new()
        }
    } else {
        std::collections::HashMap::new()
    };

    // parse new map
    if new_s.is_empty() {
        // тогда нет ничего добавлять
        return Ok(serde_json::to_string(&old_map).unwrap_or_default());
    }
    let mut new_map: std::collections::HashMap<String,f64> = serde_json::from_str(new_s)
        .map_err(|e| ContactSeenAtError::Json(e.to_string()))?;

    // слить
    for (k, v) in old_map {
        // если в new_map нет этого ключа, добавляем
        if !new_map.contains_key(&k) {
            new_map.insert(k, v);
        }
    }

    // сериализуем обратно
    let merged_str = serde_json::to_string(&new_map)
        .map_err(|e| ContactSeenAtError::Json(e.to_string()))?;
    Ok(merged_str)
}

// ТЕСТ
#[cfg(test)]
mod test_seen_at {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_contact_seen_at_repo() -> Result<(), Box<dyn std::error::Error>> {
        let conn = Connection::open_in_memory()?;
        create_contact_seen_at_table(&conn)?;

        let repo = ContactSeenAtRepo::new(&conn);

        // 1) add_seen_json
        // Имитация Swift: "id":"11111111-1111-1111-1111-111111111111"
        //  "date": {"some-user": 167889999.0, "other-user": 1234.567}
        let input = r#"{
            "id": "11111111-1111-1111-1111-111111111111",
            "date": {
                "some-user": 167889999.0,
                "other-user": 1234.567
            }
        }"#;
        let out1 = repo.add_seen_json(input)?;
        println!("add_seen_json -> {out1}");

        // 2) снова добавим с новыми датами => мерж
        let input2 = r#"{
            "id": "11111111-1111-1111-1111-111111111111",
            "date": {
                "some-user": 9999.0,
                "new-user": 5555.0
            }
        }"#;
        let out2 = repo.add_seen_json(input2)?;
        println!("add_seen_json (second) -> {out2}");

        // 3) all_seen_json
        let out3 = repo.all_seen_json()?;
        println!("all_seen_json -> {out3}");

        Ok(())
    }
}
