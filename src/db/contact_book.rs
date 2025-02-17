// // src/db/contact_book.rs
//
// use tokio_rusqlite::{Connection, Result as SqlResult, Transaction, params};
// use uuid::Uuid;
// use serde::{Deserialize, Serialize};
// use std::sync::Arc;
// use std::str::FromStr;
// use log::{debug, error, info, warn, trace};
// use serde::ser::SerializeStruct;
// use thiserror::Error;
//
// /// Ошибки репозитория контактной книги
// #[derive(Debug, Error)]
// pub enum ContactBookError {
//     #[error("SQL Error: {0}")]
//     SqlError(String),
//
//     #[error("JSON Error: {0}")]
//     JsonError(String),
//
//     #[error("Invalid UUID: {0}")]
//     InvalidUuid(String),
//
//     #[error("Other Error: {0}")]
//     Other(String),
// }
//
// /// Внутренняя модель данных контактной книги
// #[derive(Debug, Clone)]
// pub struct ContactBook {
//     pub id: Uuid,
//     pub first_name: Option<String>,
//     pub last_name: Option<String>,
//     pub nick_name: Option<String>,
//     pub phone_number: Option<String>,
//     pub email: Option<String>,
//     pub picture_url: Option<String>,
//     pub picture_data: Option<Vec<u8>>,
//     pub created_at: f64, // Swift использует Double
//     pub updated_at: f64,
// }
//
// impl Default for ContactBook {
//     fn default() -> Self {
//         Self {
//             id: Uuid::new_v4(),
//             first_name: None,
//             last_name: None,
//             nick_name: None,
//             phone_number: None,
//             email: None,
//             picture_url: None,
//             picture_data: None,
//             created_at: Self::current_timestamp_f64(),
//             updated_at: Self::current_timestamp_f64(),
//         }
//     }
// }
//
// impl ContactBook {
//     /// Получение текущего временного штампа
//     fn current_timestamp_f64() -> f64 {
//         use std::time::{SystemTime, UNIX_EPOCH};
//         let now = SystemTime::now().duration_since(UNIX_EPOCH)
//             .unwrap_or_default();
//         now.as_secs_f64()
//     }
// }
//
// /// Структура для входных данных из JSON (Swift -> Rust)
// #[derive(Serialize, Deserialize, Debug, Clone, Default)]
// pub struct ContactBookJson {
//     pub id: Option<String>,              // UUID в строке
//     pub first_name: Option<String>,
//     pub last_name: Option<String>,
//     pub nick_name: Option<String>,
//     pub phone_number: Option<String>,
//     pub email: Option<String>,
//     pub picture_url: Option<String>,
//     /// base64-кодированные бинарные данные (если нужно)
//     pub picture_data_base64: Option<String>,
//     pub created_at: Option<f64>,
//     pub updated_at: Option<f64>,
// }
//
// /// Структура для вывода данных в JSON (Rust -> Swift)
// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct ContactBookJsonOut {
//     pub id: String,
//     pub first_name: Option<String>,
//     pub last_name: Option<String>,
//     pub nick_name: Option<String>,
//     pub phone_number: Option<String>,
//     pub email: Option<String>,
//     pub picture_url: Option<String>,
//     pub picture_data_base64: Option<String>,
//     pub created_at: f64,
//     pub updated_at: f64,
// }
//
// /// Внутренняя модель данных реализует сериализацию/десериализацию
// impl Serialize for ContactBook {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where S: serde::Serializer {
//         let mut state = serializer.serialize_struct("ContactBook", 10)?;
//         state.serialize_field("id", &self.id.to_string())?;
//         state.serialize_field("first_name", &self.first_name)?;
//         state.serialize_field("last_name", &self.last_name)?;
//         state.serialize_field("nick_name", &self.nick_name)?;
//         state.serialize_field("phone_number", &self.phone_number)?;
//         state.serialize_field("email", &self.email)?;
//         state.serialize_field("picture_url", &self.picture_url)?;
//         if let Some(ref data) = self.picture_data {
//             let b64 = base64::encode(data);
//             state.serialize_field("picture_data_base64", &Some(b64))?;
//         } else {
//             state.serialize_field("picture_data_base64", &None::<String>)?;
//         }
//         state.serialize_field("created_at", &self.created_at)?;
//         state.serialize_field("updated_at", &self.updated_at)?;
//         state.end()
//     }
// }
//
// impl<'de> Deserialize<'de> for ContactBook {
//     fn deserialize<D>(deserializer: D) -> Result<ContactBook, D::Error>
//     where D: serde::Deserializer<'de> {
//         #[derive(Deserialize)]
//         struct ContactBookHelper {
//             id: String,
//             first_name: Option<String>,
//             last_name: Option<String>,
//             nick_name: Option<String>,
//             phone_number: Option<String>,
//             email: Option<String>,
//             picture_url: Option<String>,
//             picture_data_base64: Option<String>,
//             created_at: f64,
//             updated_at: f64,
//         }
//
//         let helper = ContactBookHelper::deserialize(deserializer)?;
//         let picture_data = if let Some(b64) = helper.picture_data_base64 {
//             match base64::decode(&b64) {
//                 Ok(data) => Some(data),
//                 Err(_) => None,
//             }
//         } else {
//             None
//         };
//
//         Ok(ContactBook {
//             id: Uuid::from_str(&helper.id).map_err(serde::de::Error::custom)?,
//             first_name: helper.first_name,
//             last_name: helper.last_name,
//             nick_name: helper.nick_name,
//             phone_number: helper.phone_number,
//             email: helper.email,
//             picture_url: helper.picture_url,
//             picture_data,
//             created_at: helper.created_at,
//             updated_at: helper.updated_at,
//         })
//     }
// }
//
// /// Репозиторий для работы с контактной книгой
// #[derive(Clone)]
// pub struct ContactBookRepo {
//     conn: Arc<Connection>,
// }
//
// impl ContactBookRepo {
//     /// Создаём новый репозиторий
//     pub fn new(conn: Arc<Connection>) -> Self {
//         Self { conn }
//     }
//
//     pub fn create_contact_book_table(conn: &Connection) -> Result<(), Err()> {
//         conn.execute(
//             r#"
//         CREATE TABLE IF NOT EXISTS contact_book (
//             id BLOB PRIMARY KEY,
//             first_name TEXT,
//             last_name TEXT,
//             nick_name TEXT,
//             phone_number TEXT,
//             email TEXT,
//             picture_url TEXT,
//             picture_data BLOB,
//             created_at REAL,
//             updated_at REAL
//         )
//         "#,
//             [],
//         )?;
//         Ok(())
//     }
//
//     /// Добавляем или обновляем контакт из JSON
//     pub async fn add_contact_book_json(&self, json_input: &str) -> Result<String, ContactBookError> {
//         // Десериализация JSON в ContactBookJson
//         let cb_json: ContactBookJson = serde_json::from_str(json_input)
//             .map_err(|e| ContactBookError::JsonError(e.to_string()))?;
//
//         // Парсим ID (если нет, генерируем)
//         let contact_id = if let Some(id_str) = &cb_json.id {
//             match Uuid::parse_str(id_str) {
//                 Ok(u) => u,
//                 Err(_) => return Err(ContactBookError::InvalidUuid(id_str.clone()))
//             }
//         } else {
//             Uuid::new_v4()
//         };
//
//         // Начинаем транзакцию
//         let mut tx = self.conn.await.transaction().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//
//         // Проверяем, существует ли контакт
//         if self.select_inner_tx(&mut tx, contact_id).await? {
//             // Обновляем существующий контакт
//             let mut old = self.select_contact_book_data(&mut tx, contact_id).await?;
//             // Обновляем поля
//             if let Some(f) = cb_json.first_name { old.unwrap().first_name = Some(f); }
//             if let Some(l) = cb_json.last_name { old.unwrap().last_name = Some(l); }
//             if let Some(n) = cb_json.nick_name { old.unwrap().nick_name = Some(n); }
//             if let Some(pn) = cb_json.phone_number { old.unwrap().phone_number = Some(pn); }
//             if let Some(email) = cb_json.email { old.unwrap().email = Some(email); }
//             if let Some(url) = cb_json.picture_url { old.unwrap().picture_url = Some(url); }
//             if let Some(b64) = cb_json.picture_data_base64 {
//                 if let Ok(bin) = base64::decode(b64) {
//                     old.unwrap().picture_data = Some(bin);
//                 }
//             }
//             old.unwrap().updated_at = cb_json.updated_at.unwrap_or_else(|| ContactBook::current_timestamp_f64());
//
//             self.update_inner_tx(&mut tx, &old).await?;
//             debug!("Updated existing contact {}", contact_id);
//         } else {
//             // Вставляем новый контакт
//             let new_data = ContactBook {
//                 id: contact_id,
//                 first_name: cb_json.first_name.clone(),
//                 last_name: cb_json.last_name.clone(),
//                 nick_name: cb_json.nick_name.clone(),
//                 phone_number: cb_json.phone_number.clone(),
//                 email: cb_json.email.clone(),
//                 picture_url: cb_json.picture_url.clone(),
//                 picture_data: if let Some(b64) = &cb_json.picture_data_base64 {
//                     base64::decode(b64).ok()
//                 } else {
//                     None
//                 },
//                 created_at: cb_json.created_at.unwrap_or_else(|| ContactBook::current_timestamp_f64()),
//                 updated_at: cb_json.updated_at.unwrap_or_else(|| ContactBook::current_timestamp_f64()),
//             };
//             self.insert_inner_tx(&mut tx, &new_data).await?;
//             info!("Inserted new contact {}", contact_id);
//         }
//
//         tx.commit().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//
//         // Возвращаем итоговое состояние
//         let final_data = self.select_contact_book_data_inner(contact_id).await?;
//         if let Some(fd) = final_data {
//             let out_json = serde_json::to_string(&Self::contact_book_to_json_out(&fd))
//                 .map_err(|e| ContactBookError::JsonError(e.to_string()))?;
//             Ok(out_json)
//         } else {
//             Ok("{}".to_string()) // если почему-то нет
//         }
//     }
//
//     /// Удаляем контакт по JSON ID
//     pub async fn delete_contact_book_json(&self, id_str: &str) -> Result<String, ContactBookError> {
//         let contact_id = Uuid::parse_str(id_str)
//             .map_err(|_| ContactBookError::InvalidUuid(id_str.to_string()))?;
//
//         let mut tx = self.conn.await.transaction().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//
//         // Удаляем
//         let rows_affected = tx.execute(
//             "DELETE FROM contact_book WHERE id=?1",
//             &[&contact_id.as_bytes()],
//         ).await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//
//         if rows_affected > 0 {
//             debug!("Deleted contact {}", contact_id);
//         } else {
//             info!("Contact {} not found for deletion", contact_id);
//         }
//
//         tx.commit().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//
//         // Возвращаем пустой JSON
//         Ok("{}".to_string())
//     }
//
//     /// Получаем контакт по JSON ID и возвращаем его как JSON строку
//     pub async fn get_contact_book_json(&self, id_str: &str) -> Result<String, ContactBookError> {
//         let contact_id = Uuid::parse_str(id_str)
//             .map_err(|_| ContactBookError::InvalidUuid(id_str.to_string()))?;
//
//         let data_opt = self.select_contact_book_data(contact_id).await?;
//         if let Some(d) = data_opt {
//             let out = serde_json::to_string(&Self::contact_book_to_json_out(&d))
//                 .map_err(|e| ContactBookError::JsonError(e.to_string()))?;
//             Ok(out)
//         } else {
//             Ok("{}".to_string())
//         }
//     }
//
//     /// Обновляем контакт частично на основе JSON
//     pub async fn update_contact_book_json(&self, id_str: &str, json_input: &str) -> Result<String, ContactBookError> {
//         let contact_id = Uuid::parse_str(id_str)
//             .map_err(|_| ContactBookError::InvalidUuid(id_str.to_string()))?;
//
//         let cb_json: ContactBookJson = serde_json::from_str(json_input)
//             .map_err(|e| ContactBookError::JsonError(e.to_string()))?;
//
//         let mut tx = self.conn.await.transaction().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//
//         let mut existing = match self.select_contact_book_data(&mut tx, contact_id).await? {
//             Some(e) => e,
//             None => {
//                 tx.commit().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//                 return Ok("{}".to_string());
//             }
//         };
//
//         // Обновляем поля
//         if let Some(f) = cb_json.first_name { existing.first_name = Some(f); }
//         if let Some(l) = cb_json.last_name { existing.last_name = Some(l); }
//         if let Some(n) = cb_json.nick_name { existing.nick_name = Some(n); }
//         if let Some(pn) = cb_json.phone_number { existing.phone_number = Some(pn); }
//         if let Some(email) = cb_json.email { existing.email = Some(email); }
//         if let Some(url) = cb_json.picture_url { existing.picture_url = Some(url); }
//         if let Some(b64) = cb_json.picture_data_base64 {
//             if let Ok(bin) = base64::decode(b64) {
//                 existing.picture_data = Some(bin);
//             }
//         }
//         existing.updated_at = cb_json.updated_at.unwrap_or_else(|| ContactBook::current_timestamp_f64());
//
//         self.update_inner_tx(&mut tx, &existing).await?;
//         debug!("Updated contact {}", contact_id);
//
//         tx.commit().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//
//         // Возвращаем итоговое состояние
//         let final_data = self.select_contact_book_data_inner(contact_id).await?;
//         if let Some(fd) = final_data {
//             let out_json = serde_json::to_string(&Self::contact_book_to_json_out(&fd))
//                 .map_err(|e| ContactBookError::JsonError(e.to_string()))?;
//             Ok(out_json)
//         } else {
//             Ok("{}".to_string())
//         }
//     }
//
//     /////////////////////////////////////////////////
//     // ВНУТРЕННИЕ (Tx-версии) INSERT/UPDATE/SELECT
//     /////////////////////////////////////////////////
//
//     /// Вставляем контакт внутрь транзакции
//     async fn insert_inner_tx(&self, tx: &mut Transaction<'_>, cbd: &ContactBook) -> Result<(), ContactBookError> {
//         tx.execute(
//             r#"
//             INSERT INTO contact_book (
//                 id, first_name, last_name, nick_name,
//                 phone_number, email, picture_url, picture_data,
//                 created_at, updated_at
//             )
//             VALUES (?1, ?2, ?3, ?4,
//                     ?5, ?6, ?7, ?8,
//                     ?9, ?10)
//             "#,
//             params![
//                 &cbd.id.as_bytes(),
//                 cbd.first_name.as_deref(),
//                 cbd.last_name.as_deref(),
//                 cbd.nick_name.as_deref(),
//                 cbd.phone_number.as_deref(),
//                 cbd.email.as_deref(),
//                 cbd.picture_url.as_deref(),
//                 cbd.picture_data.as_deref(),
//                 &cbd.created_at,
//                 &cbd.updated_at,
//             ],
//         ).await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//         Ok(())
//     }
//
//     /// Обновляем контакт внутрь транзакции
//     async fn update_inner_tx(&self, tx: &mut Transaction<'_>, cbd: &ContactBook) -> Result<(), ContactBookError> {
//         tx.execute(
//             r#"
//             UPDATE contact_book
//             SET first_name=?1,
//                 last_name=?2,
//                 nick_name=?3,
//                 phone_number=?4,
//                 email=?5,
//                 picture_url=?6,
//                 picture_data=?7,
//                 created_at=?8,
//                 updated_at=?9
//             WHERE id=?10
//             "#,
//             params![
//                 cbd.first_name.as_deref(),
//                 cbd.last_name.as_deref(),
//                 cbd.nick_name.as_deref(),
//                 cbd.phone_number.as_deref(),
//                 cbd.email.as_deref(),
//                 cbd.picture_url.as_deref(),
//                 cbd.picture_data.as_deref(),
//                 &cbd.created_at,
//                 &cbd.updated_at,
//                 &cbd.id.as_bytes(),
//             ],
//         ).await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//         Ok(())
//     }
//
//     /// Проверяем, существует ли контакт внутри транзакции
//     async fn select_inner_tx(&self, tx: &mut Transaction<'_>, id: Uuid) -> Result<bool, ContactBookError> {
//         let sql = "SELECT 1 FROM contact_book WHERE id=?1 LIMIT 1";
//         let mut stmt = tx.prepare(sql).await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//         let mut rows = stmt.query(&[&id.as_bytes()]).await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//
//         Ok(rows.next().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?.is_some())
//     }
//
//     /// Получаем данные контакта внутри транзакции
//     async fn select_contact_book_data(&self, tx: &mut Transaction<'_>, id: Uuid) -> Result<Option<ContactBook>, ContactBookError> {
//         let sql = r#"
//         SELECT
//             first_name, last_name, nick_name, phone_number,
//             email, picture_url, picture_data, created_at, updated_at
//         FROM contact_book
//         WHERE id=?1
//         "#;
//         let mut stmt = tx.prepare(sql).await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//         let row = stmt.query_row(&[&id.as_bytes()], |row| {
//             let first_name: Option<String> = row.get(0)?;
//             let last_name: Option<String> = row.get(1)?;
//             let nick_name: Option<String> = row.get(2)?;
//             let phone_number: Option<String> = row.get(3)?;
//             let email: Option<String> = row.get(4)?;
//             let picture_url: Option<String> = row.get(5)?;
//             let picture_data: Option<Vec<u8>> = row.get(6)?;
//             let created_at: f64 = row.get(7)?;
//             let updated_at: f64 = row.get(8)?;
//
//             Ok(ContactBook {
//                 id: Uuid::new_v4(), // Placeholder, заменим позже
//                 first_name,
//                 last_name,
//                 nick_name,
//                 phone_number,
//                 email,
//                 picture_url,
//                 picture_data,
//                 created_at,
//                 updated_at,
//             })
//         }).await;
//
//         match row {
//             Ok(mut cbd) => {
//                 cbd.id = id; // Устанавливаем правильный ID
//                 Ok(Some(cbd))
//             },
//             Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
//             Err(e) => Err(ContactBookError::SqlError(e.to_string())),
//         }
//     }
//
//     /// Получаем данные контакта (вне транзакции)
//     async fn select_contact_book_data_inner(&self, id: Uuid) -> Result<Option<ContactBook>, ContactBookError> {
//         let mut tx = self.conn.await.transaction().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//         let result = self.select_contact_book_data(&mut tx, id).await?;
//         tx.commit().await.map_err(|e| ContactBookError::SqlError(e.to_string()))?;
//         Ok(result)
//     }
//
//     /// Преобразуем внутреннюю ContactBook в JSON-формат для вывода
//     fn contact_book_to_json_out(c: &ContactBook) -> ContactBookJsonOut {
//         ContactBookJsonOut {
//             id: c.id.to_string(),
//             first_name: c.first_name.clone(),
//             last_name: c.last_name.clone(),
//             nick_name: c.nick_name.clone(),
//             phone_number: c.phone_number.clone(),
//             email: c.email.clone(),
//             picture_url: c.picture_url.clone(),
//             picture_data_base64: c.picture_data.as_ref().map(|bin| base64::encode(bin)),
//             created_at: c.created_at,
//             updated_at: c.updated_at,
//         }
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use tokio_rusqlite::Connection;
//     use std::sync::{Arc, Mutex};
//     use uuid::Uuid;
//
//     /// Функция для создания тестовой базы данных
//     async fn setup_test_db() -> Arc<Connection> {
//         let conn = Connection::open_in_memory().await.expect("Failed to open in-memory database");
//         ContactBookRepo::create_contact_book_table(&conn).await.expect("Failed to create contact_book table");
//         Arc::new(conn)
//     }
//
//     /// Тестирование добавления контакта
//     #[tokio::test]
//     async fn test_insert_contact() -> Result<(), ContactBookError> {
//         let conn = setup_test_db().await;
//         let repo = ContactBookRepo::new(conn.clone());
//
//         let contact_id = Uuid::new_v4();
//         let input_json = serde_json::json!({
//             "id": contact_id.to_string(),
//             "first_name": "Alice",
//             "last_name": "Anderson",
//             "nick_name": "Ally",
//             "phone_number": "1234567890",
//             "email": "alice@example.com",
//             "picture_url": "http://example.com/pic.jpg",
//             "picture_data_base64": base64::encode(&[1, 2, 3]),
//             "created_at": 1000.0,
//             "updated_at": 1000.0
//         }).to_string();
//
//         // Добавляем контакт
//         let out_json = repo.add_contact_book_json(&input_json).await?;
//         println!("add_contact_book_json -> {}", out_json);
//
//         // Проверяем, что контакт был добавлен
//         let fetched_json = repo.get_contact_book_json(&contact_id.to_string()).await?;
//         println!("get_contact_book_json -> {}", fetched_json);
//
//         let fetched_contact: ContactBookJsonOut = serde_json::from_str(&fetched_json)
//             .map_err(|e| ContactBookError::JsonError(e.to_string()))?;
//
//         assert_eq!(fetched_contact.id, contact_id.to_string());
//         assert_eq!(fetched_contact.first_name, Some("Alice".to_string()));
//         assert_eq!(fetched_contact.last_name, Some("Anderson".to_string()));
//         assert_eq!(fetched_contact.nick_name, Some("Ally".to_string()));
//         assert_eq!(fetched_contact.phone_number, Some("1234567890".to_string()));
//         assert_eq!(fetched_contact.email, Some("alice@example.com".to_string()));
//         assert_eq!(fetched_contact.picture_url, Some("http://example.com/pic.jpg".to_string()));
//         assert_eq!(fetched_contact.picture_data_base64, Some("AQID".to_string())); // base64(1,2,3)
//         assert_eq!(fetched_contact.created_at, 1000.0);
//         assert_eq!(fetched_contact.updated_at, 1000.0);
//
//         Ok(())
//     }
//
//     /// Тестирование обновления контакта
//     #[tokio::test]
//     async fn test_update_contact() -> Result<(), ContactBookError> {
//         let conn = setup_test_db().await;
//         let repo = ContactBookRepo::new(conn.clone());
//
//         let contact_id = Uuid::new_v4();
//         let input_json = serde_json::json!({
//             "id": contact_id.to_string(),
//             "first_name": "Bob",
//             "last_name": "Builder",
//             "nick_name": "Bobby",
//             "phone_number": "0987654321",
//             "email": "bob@example.com",
//             "picture_url": "http://example.com/bob.jpg",
//             "picture_data_base64": base64::encode(&[4, 5, 6]),
//             "created_at": 2000.0,
//             "updated_at": 2000.0
//         }).to_string();
//
//         // Добавляем контакт
//         repo.add_contact_book_json(&input_json).await?;
//
//         // Обновляем контакт
//         let update_json = serde_json::json!({
//             "nick_name": "Rob",
//             "phone_number": "1122334455",
//             "updated_at": 3000.0
//         }).to_string();
//
//         let out_json = repo.update_contact_book_json(&contact_id.to_string(), &update_json).await?;
//         println!("update_contact_book_json -> {}", out_json);
//
//         // Проверяем обновленные данные
//         let fetched_json = repo.get_contact_book_json(&contact_id.to_string()).await?;
//         println!("get_contact_book_json -> {}", fetched_json);
//
//         let fetched_contact: ContactBookJsonOut = serde_json::from_str(&fetched_json)
//             .map_err(|e| ContactBookError::JsonError(e.to_string()))?;
//
//         assert_eq!(fetched_contact.id, contact_id.to_string());
//         assert_eq!(fetched_contact.first_name, Some("Bob".to_string()));
//         assert_eq!(fetched_contact.last_name, Some("Builder".to_string()));
//         assert_eq!(fetched_contact.nick_name, Some("Rob".to_string())); // Обновлено
//         assert_eq!(fetched_contact.phone_number, Some("1122334455".to_string())); // Обновлено
//         assert_eq!(fetched_contact.email, Some("bob@example.com".to_string()));
//         assert_eq!(fetched_contact.picture_url, Some("http://example.com/bob.jpg".to_string()));
//         assert_eq!(fetched_contact.picture_data_base64, Some("BAUG".to_string())); // base64(4,5,6)
//         assert_eq!(fetched_contact.created_at, 2000.0);
//         assert_eq!(fetched_contact.updated_at, 3000.0); // Обновлено
//
//         Ok(())
//     }
//
//     /// Тестирование удаления контакта
//     #[tokio::test]
//     async fn test_delete_contact() -> Result<(), ContactBookError> {
//         let conn = setup_test_db().await;
//         let repo = ContactBookRepo::new(conn.clone());
//
//         let contact_id = Uuid::new_v4();
//         let input_json = serde_json::json!({
//             "id": contact_id.to_string(),
//             "first_name": "Charlie",
//             "last_name": "Chaplin",
//             "nick_name": "Chuck",
//             "phone_number": "5555555555",
//             "email": "charlie@example.com",
//             "picture_url": "http://example.com/charlie.jpg",
//             "picture_data_base64": base64::encode(&[7, 8, 9]),
//             "created_at": 4000.0,
//             "updated_at": 4000.0
//         }).to_string();
//
//         // Добавляем контакт
//         repo.add_contact_book_json(&input_json).await?;
//
//         // Удаляем контакт
//         let delete_out = repo.delete_contact_book_json(&contact_id.to_string()).await?;
//         println!("delete_contact_book_json -> {}", delete_out);
//
//         // Проверяем, что контакт удалён
//         let fetched_json = repo.get_contact_book_json(&contact_id.to_string()).await?;
//         println!("get_contact_book_json after delete -> {}", fetched_json);
//         assert_eq!(fetched_json, "{}", "Contact was not deleted");
//
//         Ok(())
//     }
//
//     /// Тестирование обновления контакта частично
//     #[tokio::test]
//     async fn test_update_contact_book_partial() -> Result<(), ContactBookError> {
//         let conn = setup_test_db().await;
//         let repo = ContactBookRepo::new(conn.clone());
//
//         let contact_id = Uuid::new_v4();
//         let input_json = serde_json::json!({
//             "id": contact_id.to_string(),
//             "first_name": "Eve",
//             "last_name": "Evans",
//             "nick_name": "Evie",
//             "phone_number": "7777777777",
//             "email": "eve@example.com",
//             "picture_url": "http://example.com/eve.jpg",
//             "picture_data_base64": base64::encode(&[11, 22, 33]),
//             "created_at": 6000.0,
//             "updated_at": 6000.0
//         }).to_string();
//
//         // Добавляем контакт
//         repo.add_contact_book_json(&input_json).await?;
//
//         // Обновляем только email и picture_url
//         let update_json = serde_json::json!({
//             "email": "eve_new@example.com",
//             "picture_url": "http://example.com/eve_new.jpg",
//             "updated_at": 7000.0
//         }).to_string();
//
//         let out_json = repo.update_contact_book_json(&contact_id.to_string(), &update_json).await?;
//         println!("update_contact_book_json -> {}", out_json);
//
//         // Проверяем обновленные данные
//         let fetched_json = repo.get_contact_book_json(&contact_id.to_string()).await?;
//         println!("get_contact_book_json -> {}", fetched_json);
//
//         let fetched_contact: ContactBookJsonOut = serde_json::from_str(&fetched_json)
//             .map_err(|e| ContactBookError::JsonError(e.to_string()))?;
//
//         assert_eq!(fetched_contact.email, Some("eve_new@example.com".to_string()));
//         assert_eq!(fetched_contact.picture_url, Some("http://example.com/eve_new.jpg".to_string()));
//         assert_eq!(fetched_contact.updated_at, 7000.0);
//
//         // Остальные поля должны остаться неизменными
//         assert_eq!(fetched_contact.first_name, Some("Eve".to_string()));
//         assert_eq!(fetched_contact.last_name, Some("Evans".to_string()));
//         assert_eq!(fetched_contact.nick_name, Some("Evie".to_string()));
//         assert_eq!(fetched_contact.phone_number, Some("7777777777".to_string()));
//         assert_eq!(fetched_contact.picture_data_base64, Some("Cxsq".to_string())); // base64(11,22,33)
//
//         Ok(())
//     }
//
//     /// Тестирование добавления контакта с некорректным JSON
//     #[tokio::test]
//     async fn test_add_invalid_contact() -> Result<(), ContactBookError> {
//         let conn = setup_test_db().await;
//         let repo = ContactBookRepo::new(conn.clone());
//
//         // Некорректный JSON
//         let invalid_json = "{ invalid json }".to_string();
//
//         // Пытаемся добавить контакт через некорректный JSON
//         let result = repo.add_contact_book_json(&invalid_json).await;
//         assert!(result.is_err(), "Expected error when adding contact with invalid JSON");
//
//         if let Err(ContactBookError::JsonError(msg)) = result {
//             assert!(!msg.is_empty(), "Error message should not be empty");
//         } else {
//             panic!("Expected ContactBookError::Json");
//         }
//
//         Ok(())
//     }
//
//     /// Тестирование удаления несуществующего контакта
//     #[tokio::test]
//     async fn test_delete_nonexistent_contact() -> Result<(), ContactBookError> {
//         let conn = setup_test_db().await;
//         let repo = ContactBookRepo::new(conn.clone());
//
//         let non_existent_id = Uuid::new_v4();
//
//         // Пытаемся удалить несуществующий контакт
//         let result = repo.delete_contact_book_json(&non_existent_id.to_string()).await;
//         assert!(result.is_ok(), "Deleting non-existent contact should not fail");
//
//         // Получаем пустой JSON
//         let fetched_json = repo.get_contact_book_json(&non_existent_id.to_string()).await?;
//         assert_eq!(fetched_json, "{}", "Non-existent contact should return empty JSON");
//
//         Ok(())
//     }
//
//     /// Тестирование получения несуществующего контакта
//     #[tokio::test]
//     async fn test_get_nonexistent_contact() -> Result<(), ContactBookError> {
//         let conn = setup_test_db().await;
//         let repo = ContactBookRepo::new(conn.clone());
//
//         let non_existent_id = Uuid::new_v4();
//
//         // Пытаемся получить несуществующий контакт
//         let fetched_json = repo.get_contact_book_json(&non_existent_id.to_string()).await?;
//         assert_eq!(fetched_json, "{}", "Non-existent contact should return empty JSON");
//
//         Ok(())
//     }
// }
