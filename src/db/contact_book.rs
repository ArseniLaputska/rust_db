use rusqlite::{Connection, Result, params, Transaction};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::error::Error;

/////////////////////////////////////////////////////////////
// 1) МОДЕЛЬ ДАННЫХ В RUST (ВНУТРЕННЯЯ)
/////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct ContactBookData {
    pub id: Uuid,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub nick_name: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub picture_url: Option<String>,
    pub picture_data: Option<Vec<u8>>,
    pub created_at: f64, // Swift uses Double
    pub updated_at: f64,
}

// Значения по умолчанию
impl Default for ContactBookData {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            first_name: None,
            last_name: None,
            nick_name: None,
            phone_number: None,
            email: None,
            picture_url: None,
            picture_data: None,
            created_at: current_timestamp_f64(),
            updated_at: current_timestamp_f64(),
        }
    }
}

fn current_timestamp_f64() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.as_secs_f64()
}

/////////////////////////////////////////////////////////////
// 2) СОЗДАДИМ ТАБЛИЦУ contact_book_data
/////////////////////////////////////////////////////////////

pub fn create_contact_book_table(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS contact_book_data (
            id BLOB PRIMARY KEY,
            first_name TEXT,
            last_name TEXT,
            nick_name TEXT,
            phone_number TEXT,
            email TEXT,
            picture_url TEXT,
            picture_data BLOB,
            created_at REAL,
            updated_at REAL
        )
        "#,
        [],
    )?;
    Ok(())
}

/////////////////////////////////////////////////////////////
// 3) JSON-СТРУКТУРЫ ДЛЯ SWIFT (ВХОД/ВЫХОД)
/////////////////////////////////////////////////////////////
//
// Swift будет передавать JSON, например:
// {
//   "id": "550e8400-e29b-41d4-a716-446655440000",
//   "firstName": "John",
//   ...
//   "createdAt": 123456789.0,
//   "updatedAt": 987654321.0
// }
// В Rust мы парсим это в ContactBookDataJson.
// Затем сохраняем (INSERT/UPDATE) в contact_book_data.
//
// При возврате тоже отдадим JSON (ContactBookDataJsonOut),
// чтобы Swift мог прочитать результат.
//

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ContactBookDataJson {
    pub id: Option<String>,          // UUID в строке
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub nick_name: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub picture_url: Option<String>,
    /// base64-кодированные бинарные данные (если нужно)
    pub picture_data_base64: Option<String>,
    pub created_at: Option<f64>,
    pub updated_at: Option<f64>,
}

// Для вывода (можно объединить, но часто разделяют)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContactBookDataJsonOut {
    pub id: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub nick_name: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub picture_url: Option<String>,
    pub picture_data_base64: Option<String>,
    pub created_at: f64,
    pub updated_at: f64,
}

/////////////////////////////////////////////////////////////
// 4) ОШИБКИ
/////////////////////////////////////////////////////////////
#[derive(Debug)]
pub enum ContactBookError {
    Sql(String),
    Json(String),
    InvalidUuid(String),
    Other(String),
}

impl std::fmt::Display for ContactBookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContactBookError::Sql(e) => write!(f, "SqlError: {e}"),
            ContactBookError::Json(e) => write!(f, "JsonError: {e}"),
            ContactBookError::InvalidUuid(u) => write!(f, "Invalid UUID: {u}"),
            ContactBookError::Other(o) => write!(f, "Other: {o}"),
        }
    }
}
impl Error for ContactBookError {}

/////////////////////////////////////////////////////////////
// 5) РЕПОЗИТОРИЙ (ContactBookRepo)
/////////////////////////////////////////////////////////////

pub struct ContactBookRepo<'a> {
    conn: &'a Connection,
}

impl<'a> ContactBookRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    //-------------------------------------------------------------
    // add_contact_book_json
    //
    // Аналог "func add(bookContact contacts: [ContactBookStruct])"
    // Но мы делаем версию для одного контакта, используя JSON.
    // При необходимости, можно расширить до массива JSON.
    //
    // Логика:
    // 1) Парсим входной JSON в ContactBookDataJson.
    // 2) Если "id" есть и контакт уже существует -> UPDATE, иначе -> INSERT.
    // 3) Возвращаем финальное состояние в виде JSON.
    //-------------------------------------------------------------
    pub fn add_contact_book_json(&self, json_input: &str) -> Result<String, ContactBookError> {
        let cb_json: ContactBookDataJson = serde_json::from_str(json_input)
            .map_err(|e| ContactBookError::Json(e.to_string()))?;

        // парсим ID (если нет, генерируем)
        let contact_id = if let Some(id_str) = &cb_json.id {
            match Uuid::parse_str(id_str) {
                Ok(u) => u,
                Err(_) => return Err(ContactBookError::InvalidUuid(id_str.clone()))
            }
        } else {
            Uuid::new_v4()
        };

        // Начинаем транзакцию
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| ContactBookError::Sql(e.to_string()))?;

        // SELECT, проверяем, есть ли такой
        let existing = self.select_inner_tx(&tx, contact_id)?;

        if let Some(mut old) = existing {
            // UPDATE
            old.first_name = cb_json.first_name.clone();
            old.last_name = cb_json.last_name.clone();
            old.nick_name = cb_json.nick_name.clone();
            old.phone_number = cb_json.phone_number.clone();
            old.email = cb_json.email.clone();
            old.picture_url = cb_json.picture_url.clone();
            // picture_data_base64 -> decode
            if let Some(b64) = &cb_json.picture_data_base64 {
                if let Ok(bin) = base64::decode(b64) {
                    old.picture_data = Some(bin);
                }
            }
            // updated_at
            old.updated_at = cb_json.updated_at.unwrap_or_else(|| current_timestamp_f64());

            self.update_inner_tx(&tx, &old)?;
        } else {
            // INSERT
            let new_data = ContactBookData {
                id: contact_id,
                first_name: cb_json.first_name.clone(),
                last_name: cb_json.last_name.clone(),
                nick_name: cb_json.nick_name.clone(),
                phone_number: cb_json.phone_number.clone(),
                email: cb_json.email.clone(),
                picture_url: cb_json.picture_url.clone(),
                picture_data: if let Some(b64) = &cb_json.picture_data_base64 {
                    base64::decode(b64).ok()
                } else {
                    None
                },
                created_at: cb_json.created_at.unwrap_or_else(|| current_timestamp_f64()),
                updated_at: cb_json.updated_at.unwrap_or_else(|| current_timestamp_f64()),
            };
            self.insert_inner_tx(&tx, &new_data)?;
        }

        tx.commit().map_err(|e| ContactBookError::Sql(e.to_string()))?;

        // Возвращаем итоговое состояние
        let final_data = self.select_inner(contact_id)?;
        if let Some(fd) = final_data {
            let out_json = serde_json::to_string(&contact_book_data_to_json_out(&fd))
                .map_err(|e| ContactBookError::Json(e.to_string()))?;
            Ok(out_json)
        } else {
            Ok("{}".to_string()) // если почему-то нет
        }
    }

    //-------------------------------------------------------------
    // delete_contact_book_json
    //-------------------------------------------------------------
    pub fn delete_contact_book_json(&self, id_str: &str) -> Result<String, ContactBookError> {
        let contact_id = Uuid::parse_str(id_str)
            .map_err(|_| ContactBookError::InvalidUuid(id_str.to_string()))?;

        let tx = self.conn.unchecked_transaction()
            .map_err(|e| ContactBookError::Sql(e.to_string()))?;

        // Удаляем
        tx.execute(
            "DELETE FROM contact_book_data WHERE id=?1",
            params![&contact_id.as_bytes()],
        ).map_err(|e| ContactBookError::Sql(e.to_string()))?;

        tx.commit().map_err(|e| ContactBookError::Sql(e.to_string()))?;

        // Можно вернуть пустой JSON или старое значение, если нужно.
        Ok("{}".to_string())
    }

    //-------------------------------------------------------------
    // get_contact_book_json
    // Возвращаем JSON
    //-------------------------------------------------------------
    pub fn get_contact_book_json(&self, id_str: &str) -> Result<String, ContactBookError> {
        let contact_id = Uuid::parse_str(id_str)
            .map_err(|_| ContactBookError::InvalidUuid(id_str.to_string()))?;

        let data_opt = self.select_inner(contact_id)?;
        if let Some(d) = data_opt {
            let out = serde_json::to_string(&contact_book_data_to_json_out(&d))
                .map_err(|e| ContactBookError::Json(e.to_string()))?;
            Ok(out)
        } else {
            Ok("{}".to_string())
        }
    }

    //-------------------------------------------------------------
    // update_contact_book_json
    // Принимаем JSON, парсим, UPDATE частичных полей
    //-------------------------------------------------------------
    pub fn update_contact_book_json(&self, id_str: &str, json_input: &str) -> Result<String, ContactBookError> {
        let contact_id = Uuid::parse_str(id_str)
            .map_err(|_| ContactBookError::InvalidUuid(id_str.to_string()))?;

        let cb_json: ContactBookDataJson = serde_json::from_str(json_input)
            .map_err(|e| ContactBookError::Json(e.to_string()))?;

        let tx = self.conn.unchecked_transaction()
            .map_err(|e| ContactBookError::Sql(e.to_string()))?;

        let mut existing = match self.select_inner_tx(&tx, contact_id)? {
            Some(e) => e,
            None => {
                tx.commit().unwrap_or_default();
                return Ok("{}".to_string());
            }
        };

        // Обновим поля
        if let Some(f) = cb_json.first_name { existing.first_name = Some(f); }
        if let Some(l) = cb_json.last_name { existing.last_name = Some(l); }
        if let Some(n) = cb_json.nick_name { existing.nick_name = Some(n); }
        if let Some(p) = cb_json.phone_number { existing.phone_number = Some(p); }
        if let Some(e) = cb_json.email { existing.email = Some(e); }
        if let Some(url) = cb_json.picture_url { existing.picture_url = Some(url); }
        if let Some(b64) = cb_json.picture_data_base64 {
            if let Ok(bin) = base64::decode(b64) {
                existing.picture_data = Some(bin);
            }
        }
        existing.updated_at = cb_json.updated_at.unwrap_or_else(|| current_timestamp_f64());

        self.update_inner_tx(&tx, &existing)?;

        tx.commit().map_err(|e| ContactBookError::Sql(e.to_string()))?;

        // Возвращаем
        let final_data = self.select_inner(contact_id)?;
        if let Some(fd) = final_data {
            let out_json = serde_json::to_string(&contact_book_data_to_json_out(&fd))
                .map_err(|e| ContactBookError::Json(e.to_string()))?;
            Ok(out_json)
        } else {
            Ok("{}".to_string())
        }
    }

    /////////////////////////////////////////////////
    // ВНУТРЕННИЕ (Tx-версии) INSERT/UPDATE/SELECT
    /////////////////////////////////////////////////

    fn insert_inner_tx(&self, tx: &Transaction, cbd: &ContactBookData) -> Result<(), ContactBookError> {
        tx.execute(
            r#"
            INSERT INTO contact_book_data (
                id, first_name, last_name, nick_name,
                phone_number, email, picture_url, picture_data,
                created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4,
                    ?5, ?6, ?7, ?8,
                    ?9, ?10)
            "#,
            params![
                &cbd.id.as_bytes(),
                &cbd.first_name,
                &cbd.last_name,
                &cbd.nick_name,
                &cbd.phone_number,
                &cbd.email,
                &cbd.picture_url,
                &cbd.picture_data,
                &cbd.created_at,
                &cbd.updated_at
            ],
        ).map_err(|e| ContactBookError::Sql(e.to_string()))?;
        Ok(())
    }

    fn update_inner_tx(&self, tx: &Transaction, cbd: &ContactBookData) -> Result<(), ContactBookError> {
        tx.execute(
            r#"
            UPDATE contact_book_data
            SET first_name=?1,
                last_name=?2,
                nick_name=?3,
                phone_number=?4,
                email=?5,
                picture_url=?6,
                picture_data=?7,
                created_at=?8,
                updated_at=?9
            WHERE id=?10
            "#,
            params![
                &cbd.first_name,
                &cbd.last_name,
                &cbd.nick_name,
                &cbd.phone_number,
                &cbd.email,
                &cbd.picture_url,
                &cbd.picture_data,
                &cbd.created_at,
                &cbd.updated_at,
                &cbd.id.as_bytes()
            ],
        ).map_err(|e| ContactBookError::Sql(e.to_string()))?;
        Ok(())
    }

    fn select_inner_tx(&self, tx: &Transaction, id: Uuid) -> Result<Option<ContactBookData>, ContactBookError> {
        let sql = r#"
        SELECT
            first_name, last_name, nick_name, phone_number,
            email, picture_url, picture_data, created_at, updated_at
        FROM contact_book_data
        WHERE id=?1
        "#;
        let mut stmt = tx.prepare(sql)
            .map_err(|e| ContactBookError::Sql(e.to_string()))?;
        let mut rows = stmt.query(params![&id.as_bytes()])
            .map_err(|e| ContactBookError::Sql(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| ContactBookError::Sql(e.to_string()))? {
            let first_name: Option<String> = row.get(0).map_err(|e| ContactBookError::Sql(e.to_string()))?;
            let last_name: Option<String> = row.get(1).map_err(|e| ContactBookError::Sql(e.to_string()))?;
            let nick_name: Option<String> = row.get(2).map_err(|e| ContactBookError::Sql(e.to_string()))?;
            let phone_number: Option<String> = row.get(3).map_err(|e| ContactBookError::Sql(e.to_string()))?;
            let email: Option<String> = row.get(4).map_err(|e| ContactBookError::Sql(e.to_string()))?;
            let picture_url: Option<String> = row.get(5).map_err(|e| ContactBookError::Sql(e.to_string()))?;
            let picture_data: Option<Vec<u8>> = row.get(6).map_err(|e| ContactBookError::Sql(e.to_string()))?;
            let created_at: f64 = row.get(7).map_err(|e| ContactBookError::Sql(e.to_string()))?;
            let updated_at: f64 = row.get(8).map_err(|e| ContactBookError::Sql(e.to_string()))?;

            Ok(Some(ContactBookData {
                id,
                first_name,
                last_name,
                nick_name,
                phone_number,
                email,
                picture_url,
                picture_data,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    fn select_inner(&self, id: Uuid) -> Result<Option<ContactBookData>, ContactBookError> {
        let tx = self.conn.unchecked_transaction()
            .map_err(|e| ContactBookError::Sql(e.to_string()))?;
        let result = self.select_inner_tx(&tx, id)?;
        tx.commit().ok();
        Ok(result)
    }
}

// Преобразуем внутреннюю ContactBookData в JSON-формат для вывода
fn contact_book_data_to_json_out(c: &ContactBookData) -> ContactBookDataJsonOut {
    ContactBookDataJsonOut {
        id: c.id.to_string(),
        first_name: c.first_name.clone(),
        last_name: c.last_name.clone(),
        nick_name: c.nick_name.clone(),
        phone_number: c.phone_number.clone(),
        email: c.email.clone(),
        picture_url: c.picture_url.clone(),
        picture_data_base64: c.picture_data.as_ref().map(|bin| base64::encode(bin)),
        created_at: c.created_at,
        updated_at: c.updated_at,
    }
}


///////////////////////////////////////////
// ТЕСТ
///////////////////////////////////////////
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_contact_book_repo() -> Result<(), Box<dyn std::error::Error>> {
        let conn = Connection::open_in_memory()?;
        create_contact_book_table(&conn)?;

        let repo = ContactBookRepo::new(&conn);

        // 1) add_contact_book_json
        let input_json = r#"{
            "id": "67f926b7-19ec-4350-8b2e-436f43d613f2",
            "first_name": "Alice",
            "last_name": "Anderson",
            "email": "alice@example.com",
            "phone_number": "123456",
            "picture_data_base64": "SGVsbG8gV29ybGQ=",
            "created_at": 1000.0,
            "updated_at": 1000.0
        }"#;
        let out_json = repo.add_contact_book_json(input_json)?;
        println!("add_contact_book_json -> {out_json}");

        // 2) get_contact_book_json
        let out2 = repo.get_contact_book_json("67f926b7-19ec-4350-8b2e-436f43d613f2")?;
        println!("get_contact_book_json -> {out2}");

        // 3) update_contact_book_json
        let update_json = r#"{
            "nick_name": "Ally"
        }"#;
        let out3 = repo.update_contact_book_json("67f926b7-19ec-4350-8b2e-436f43d613f2", update_json)?;
        println!("update_contact_book_json -> {out3}");

        // 4) delete_contact_book_json
        let out4 = repo.delete_contact_book_json("67f926b7-19ec-4350-8b2e-436f43d613f2")?;
        println!("delete_contact_book_json -> {out4}");

        Ok(())
    }
}
