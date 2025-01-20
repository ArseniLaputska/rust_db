use rusqlite::{Connection, params, Result};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub relationship: i64,
    pub username: Option<String>,
    pub language: Option<String>,
    pub picture_data: Option<Vec<u8>>,
    pub picture_url: Option<String>,
    pub last_message_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_pro: i64,
}

impl Default for Contact {
    fn default() -> Self {
        Self {
            id: Uuid::now_v7(),
            first_name: String::new(),
            last_name: String::new(),
            relationship: 0,
            username: None,
            language: None,
            picture_data: None,
            picture_url: None,
            last_message_at: None,
            created_at: Self::current_timestamp(),
            updated_at: 0,
            is_pro: 0,
        }
    }
}

impl Contact {
    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
}

pub fn create_contact_table(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS contact_data (
            id BLOB PRIMARY KEY,
            first_name TEXT NOT NULL,
            last_name TEXT NOT NULL,
            relationship INTEGER NOT NULL,
            username TEXT,
            language TEXT,
            picture_data BLOB,
            picture_url TEXT,
            last_message_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            is_pro INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    Ok(())
}

pub struct ContactRepo<'a> {
    pub conn: &'a Connection,
}

#[derive(Debug)]
pub enum ContactError {
    NoContact(Uuid),
    InvalidUuid,
    Other(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContactInput {
    pub id: Uuid,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub relationship: i64,
    pub username: Option<String>,
    pub language: Option<String>,
    pub picture_data: Option<Vec<u8>>,
    pub picture_url: Option<String>,
    pub last_message_at: Option<i64>,
    pub pro: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(i64)]
pub enum ContactRelationship {
    Unspecified = 0,
    Friend = 1,
    Muted = 2,
    Ban = 3,
    Other(i64),
}

impl ContactRelationship {
    pub fn to_i64(&self) -> i64 {
        match self {
            ContactRelationship::Unspecified => 0,
            ContactRelationship::Friend => 1,
            ContactRelationship::Muted => 2,
            ContactRelationship::Ban => 3,
            ContactRelationship::Other(val) => *val, // Возвращаем значение для Other
        }
    }
}

impl std::fmt::Display for ContactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContactError::NoContact(id) => write!(f, "No contact with id={}", id),
            ContactError::InvalidUuid => write!(f, "Invalid UUID"),
            ContactError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ContactError {}

impl<'a> ContactRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn add_contact_pb_json(&self, json_input: &str) -> Result<(), ContactError> {
        // Десериализация JSON в ContactInput
        let input: ContactInput = serde_json::from_str(json_input)
            .map_err(|e| ContactError::Other(e.to_string()))?;
        
        self.add_contact_pb(input)
    }

    // Аналогично для других методов, требующих ContactInput
    pub fn add_contact_struct_json(&self, json_input: &str) -> Result<(), ContactError> {
        let input: ContactInput = serde_json::from_str(json_input)
            .map_err(|e| ContactError::Other(e.to_string()))?;
        
        self.add_contact_struct(input)
    }

    /// Обновляет или вставляет контакт на основе входных данных
    pub fn add_contact_pb(&self, input: ContactInput) -> Result<(), ContactError> {
        let existing = self.get_contact(input.id)?;

        if existing.is_some() {
            // Обновляем
            self.update_contact(input)?;
        } else {
            // Вставляем
            self.insert_contact(input)?;
        }
        Ok(())
    }

    /// Вставляет новый контакт
    fn insert_contact(&self, input: ContactInput) -> Result<(), ContactError> {
        let now = Contact::current_timestamp();
        self.conn.execute(
            r#"
            INSERT INTO contact_data (
                id, first_name, last_name, relationship, 
                username, language, picture_data, picture_url, 
                last_message_at, created_at, updated_at, is_pro
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                &input.id.as_bytes(),
                input.first_name,
                input.last_name,
                input.relationship,
                input.username,
                input.language,
                input.picture_data,
                input.picture_url,
                input.last_message_at,
                now,
                now,
                input.pro.unwrap_or(0)
            ],
        )
        .map_err(|e| ContactError::Other(e.to_string()))?;
        Ok(())
    }

    /// Обновляет существующий контакт
    fn update_contact(&self, input: ContactInput) -> Result<(), ContactError> {
        let now = Contact::current_timestamp();
        self.conn.execute(
            r#"
            UPDATE contact_data
            SET first_name = COALESCE(?1, first_name),
                last_name = COALESCE(?2, last_name),
                relationship = ?3,
                username = ?4,
                language = ?5,
                picture_data = ?6,
                picture_url = ?7,
                last_message_at = ?8,
                updated_at = ?9,
                is_pro = COALESCE(?10, is_pro)
            WHERE id = ?11
            "#,
            params![
                input.first_name,
                input.last_name,
                input.relationship,
                input.username,
                input.language,
                input.picture_data,
                input.picture_url,
                input.last_message_at,
                now,
                input.pro,
                &input.id.as_bytes()
            ],
        )
        .map_err(|e| ContactError::Other(e.to_string()))?;
        Ok(())
    }

    pub fn delete_contact(&self, id: Uuid) -> Result<(), ContactError> {
        self.conn.execute(
            "DELETE FROM contact_data WHERE id = ?1",
            params![&id.as_bytes()],
        ).map_err(|e| ContactError::Other(e.to_string()))?;
        Ok(())
    }

    /// Метод для обновления отношения контакта с возможностью вызова callback
    pub fn update_contact_relationship(
        &self,
        id: Uuid,
        rel: i64,
        on_muted: Option<Box<dyn Fn(Uuid) -> Result<(), ContactError>>>,
    ) -> Result<(), ContactError> {
        let now = Contact::current_timestamp();

        self.conn.execute(
            r#"
            UPDATE contact_data
            SET relationship = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
            params![rel, now, &id.as_bytes()],
        )
        .map_err(|e| ContactError::Other(e.to_string()))?;

        // Вызов callback, если отношение установлено на Muted и callback предоставлен
        if rel == 2 {
            if let Some(callback) = on_muted {
                callback(id)?;
            }
        }

        Ok(())
    }

    // Остальные методы репозитория остаются без изменений,
    // только параметры изменены на ContactInput или соответствующие типы

    pub fn add_contact_struct(
        &self,
        input: ContactInput,
    ) -> Result<(), ContactError> {
        // Проверяем наличие контакта
        let existing = self.get_contact(input.id)?;

        // Обновляем или вставляем
        match existing {
            Some(_) => {
                // Обновляем
                self.update_contact(input)?;
            }
            None => {
                // Вставляем
                self.insert_contact(input)?;
            }
        }
        Ok(())
    }

    // Другие методы, такие как update_contact_with_description,
    // update_contact_last_message_at, update_contact_language и т.д.,
    // также могут быть адаптированы аналогично, принимая ContactInput

    pub fn get_image_data(&self, id: Uuid) -> Result<Option<Vec<u8>>, ContactError> {
        let mut stmt = self.conn.prepare(
            "SELECT picture_data FROM contact_data WHERE id = ?1"
        ).map_err(|e| ContactError::Other(e.to_string()))?;

        let mut rows = stmt.query(params![&id.as_bytes()])
            .map_err(|e| ContactError::Other(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| ContactError::Other(e.to_string()))? {
            let data: Option<Vec<u8>> = row.get(0).map_err(|e| ContactError::Other(e.to_string()))?;
            Ok(data)
        } else {
            Ok(None)
        }
    }

    pub fn get_contact(&self, id: Uuid) -> Result<Option<String>, ContactError> {
        let sql = r#"
        SELECT 
            first_name,
            last_name,
            relationship,
            username,
            language,
            picture_data,
            picture_url,
            last_message_at,
            created_at,
            updated_at,
            is_pro
        FROM contact_data
        WHERE id = ?1
        "#;
        let mut stmt = self.conn
            .prepare(sql)
            .map_err(|e| ContactError::Other(e.to_string()))?;

        let mut rows = stmt.query(params![&id.as_bytes()])
            .map_err(|e| ContactError::Other(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| ContactError::Other(e.to_string()))? {
            let first_name: String = row.get(0).map_err(|e| ContactError::Other(e.to_string()))?;
            let last_name: String = row.get(1).map_err(|e| ContactError::Other(e.to_string()))?;
            let relationship: i64 = row.get(2).map_err(|e| ContactError::Other(e.to_string()))?;
            let username: Option<String> = row.get(3).map_err(|e| ContactError::Other(e.to_string()))?;
            let language: Option<String> = row.get(4).map_err(|e| ContactError::Other(e.to_string()))?;
            let picture_data: Option<Vec<u8>> = row.get(5).map_err(|e| ContactError::Other(e.to_string()))?;
            let picture_url: Option<String> = row.get(6).map_err(|e| ContactError::Other(e.to_string()))?;
            let last_message_at: Option<i64> = row.get(7).map_err(|e| ContactError::Other(e.to_string()))?;
            let created_at: i64 = row.get(8).map_err(|e| ContactError::Other(e.to_string()))?;
            let updated_at: i64 = row.get(9).map_err(|e| ContactError::Other(e.to_string()))?;
            let is_pro: i64 = row.get(10).map_err(|e| ContactError::Other(e.to_string()))?;

            let contact = Contact {
                id,
                first_name,
                last_name,
                relationship,
                username,
                language,
                picture_data,
                picture_url,
                last_message_at,
                created_at,
                updated_at,
                is_pro,
            };

            // Сериализация структуры Contact в JSON
            let json = serde_json::to_string(&contact)
                .map_err(|e| ContactError::Other(e.to_string()))?;

            Ok(Some(json))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use super::*;
    use rusqlite::{Connection};
    use uuid::Uuid;

    // Функция для создания временной базы данных в памяти и инициализации таблицы контактов
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("Failed to open in-memory database");
        create_contact_table(&conn).expect("Failed to create contact table");
        conn
    }

    #[test]
    fn test_insert_contact() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        let contact_id = Uuid::now_v7();
        let input = ContactInput {
            id: contact_id,
            first_name: Some("John".to_string()),
            last_name: Some("Doe".to_string()),
            relationship: ContactRelationship::Friend.to_i64(),
            username: Some("johndoe".to_string()),
            language: Some("en".to_string()),
            picture_data: Some(vec![1, 2, 3]),
            picture_url: Some("http://example.com/pic.jpg".to_string()),
            last_message_at: Some(1625077800),
            pro: Some(1),
        };

        // Добавляем контакт
        repo.add_contact_pb(input).expect("Failed to add contact");

        // Проверяем, что контакт был добавлен
        let fetched = repo.get_contact(contact_id).expect("Failed to fetch contact");
        assert!(fetched.is_some());

        let fetched_contact: Contact = serde_json::from_str(&fetched.unwrap()).expect("Failed to deserialize contact");
        assert_eq!(fetched_contact.id, contact_id);
        assert_eq!(fetched_contact.first_name, "John");
        assert_eq!(fetched_contact.last_name, "Doe");
        assert_eq!(fetched_contact.relationship, ContactRelationship::Friend.to_i64());
        assert_eq!(fetched_contact.username, Some("johndoe".to_string()));
        assert_eq!(fetched_contact.language, Some("en".to_string()));
        assert_eq!(fetched_contact.picture_data, Some(vec![1, 2, 3]));
        assert_eq!(fetched_contact.picture_url, Some("http://example.com/pic.jpg".to_string()));
        assert_eq!(fetched_contact.last_message_at, Some(1625077800));
        assert_eq!(fetched_contact.is_pro, 1);
    }

    #[test]
    fn test_update_contact() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        let contact_id = Uuid::now_v7();
        let input = ContactInput {
            id: contact_id,
            first_name: Some("Jane".to_string()),
            last_name: Some("Smith".to_string()),
            relationship: ContactRelationship::Friend.to_i64(),
            username: Some("janesmith".to_string()),
            language: Some("en".to_string()),
            picture_data: None,
            picture_url: None,
            last_message_at: None,
            pro: Some(0),
        };

        // Добавляем контакт
        repo.add_contact_pb(input).expect("Failed to add contact");

        // Обновляем контакт
        let updated_input = ContactInput {
            id: contact_id,
            first_name: Some("Janet".to_string()),
            last_name: None, // Не меняем фамилию
            relationship: ContactRelationship::Muted.to_i64(),
            username: None, // Не меняем username
            language: Some("fr".to_string()),
            picture_data: Some(vec![4, 5, 6]),
            picture_url: None,
            last_message_at: Some(1625077900),
            pro: Some(1),
        };

        repo.add_contact_pb(updated_input).expect("Failed to update contact");

        // Проверяем обновленные данные
        let fetched = repo.get_contact(contact_id).expect("Failed to fetch contact");
        assert!(fetched.is_some());

        let fetched_contact: Contact = serde_json::from_str(&fetched.unwrap()).expect("Failed to deserialize contact");
        assert_eq!(fetched_contact.first_name, "Janet");
        assert_eq!(fetched_contact.last_name, "Smith"); // Не должно измениться
        assert_eq!(fetched_contact.relationship, ContactRelationship::Muted.to_i64());
        assert_ne!(fetched_contact.username, Some("janesmith".to_string())); // Не должно измениться
        assert_eq!(fetched_contact.language, Some("fr".to_string()));
        assert_eq!(fetched_contact.picture_data, Some(vec![4, 5, 6]));
        assert_eq!(fetched_contact.last_message_at, Some(1625077900));
        assert_eq!(fetched_contact.is_pro, 1);
    }

    #[test]
    fn test_delete_contact() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        let contact_id = Uuid::now_v7();
        let input = ContactInput {
            id: contact_id,
            first_name: Some("Alice".to_string()),
            last_name: Some("Wonderland".to_string()),
            relationship: ContactRelationship::Friend.to_i64(),
            username: None,
            language: None,
            picture_data: None,
            picture_url: None,
            last_message_at: None,
            pro: None,
        };

        // Добавляем контакт
        repo.add_contact_pb(input).expect("Failed to add contact");

        // Удаляем контакт
        repo.delete_contact(contact_id).expect("Failed to delete contact");

        // Проверяем, что контакт удален
        let fetched = repo.get_contact(contact_id).expect("Failed to fetch contact");
        assert!(fetched.is_none());
    }

    #[test]
    fn test_get_image_data() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        let contact_id = Uuid::now_v7();
        let picture = vec![10, 20, 30, 40, 50];
        let input = ContactInput {
            id: contact_id,
            first_name: Some("Bob".to_string()),
            last_name: Some("Builder".to_string()),
            relationship: ContactRelationship::Friend.to_i64(),
            username: None,
            language: None,
            picture_data: Some(picture.clone()),
            picture_url: None,
            last_message_at: None,
            pro: None,
        };

        // Добавляем контакт с изображением
        repo.add_contact_pb(input).expect("Failed to add contact");

        // Получаем данные изображения
        let image_data = repo.get_image_data(contact_id).expect("Failed to get image data");
        assert!(image_data.is_some());
        assert_eq!(image_data.unwrap(), picture);
    }

    #[test]
    fn test_update_contact_relationship_with_callback() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        let contact_id = Uuid::now_v7();
        let input = ContactInput {
            id: contact_id,
            first_name: Some("Charlie".to_string()),
            last_name: Some("Brown".to_string()),
            relationship: ContactRelationship::Friend.to_i64(),
            username: None,
            language: None,
            picture_data: None,
            picture_url: None,
            last_message_at: None,
            pro: None,
        };

        // Добавляем контакт
        repo.add_contact_pb(input).expect("Failed to add contact");

        // Флаг для проверки вызова callback
        let callback_called = Arc::new(Mutex::new(false));

        // Определяем callback с использованием Arc<Mutex<bool>>
        let callback_called_clone = Arc::clone(&callback_called);
        let callback = move |id: Uuid| -> Result<(), ContactError> {
            assert_eq!(id, contact_id);
            let mut flag = callback_called_clone.lock().unwrap();
            *flag = true;
            Ok(())
        };

        let relationship = ContactRelationship::Muted.to_i64();

        // Обновляем отношение на Muted и передаем callback
        repo.update_contact_relationship(contact_id, relationship, Some(Box::new(callback)))
            .expect("Failed to update contact relationship");

        // Проверяем, что callback был вызван
        let flag = callback_called.lock().unwrap();
        assert!(*flag);
    }

    #[test]
    fn test_json_deserialization() {
        let contact_id = Uuid::now_v7(); // Генерация UUID

        // Пример валидного JSON, который должен десериализоваться в структуру ContactInput
        let json_input = serde_json::json!({
            "id": contact_id,
            "first_name": "Daisy",
            "last_name": "Duck",
            "relationship": ContactRelationship::Friend.to_i64(),
            "username": "daisyduck",
            "language": "en",
            "picture_data": [255, 254, 253],
            "picture_url": "http://example.com/daisy.jpg",
            "last_message_at": 1625078000,
            "pro": 1
        }).to_string();

        // Десериализация JSON в ContactInput
        let input: ContactInput = serde_json::from_str(&json_input)
            .expect("Failed to deserialize JSON");

        // Проверка, что десериализация прошла успешно и значения правильные
        assert_eq!(input.id, contact_id);
        assert_eq!(input.first_name, Some("Daisy".to_string()));
        assert_eq!(input.last_name, Some("Duck".to_string()));
        assert_eq!(input.relationship, ContactRelationship::Friend.to_i64());
        assert_eq!(input.username, Some("daisyduck".to_string()));
        assert_eq!(input.language, Some("en".to_string()));
        assert_eq!(input.picture_data, Some(vec![255, 254, 253]));
        assert_eq!(input.picture_url, Some("http://example.com/daisy.jpg".to_string()));
        assert_eq!(input.last_message_at, Some(1625078000));
        assert_eq!(input.pro, Some(1));
    }

    #[test]
    fn test_add_contact_pb_json() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        let contact_id = Uuid::now_v7();
        let json_input = serde_json::json!({
            "id": contact_id,
            "first_name": "Daisy",
            "last_name": "Duck",
            "relationship": ContactRelationship::Friend.to_i64(),
            "username": "daisyduck",
            "language": "en",
            "picture_data": [255, 254, 253],
            "picture_url": "http://example.com/daisy.jpg",
            "last_message_at": 1625078000,
            "pro": 1
        }).to_string();

        // Добавляем контакт через JSON
        ContactRepo::add_contact_pb_json(&repo, &json_input).expect("Failed to add contact via JSON");

        // Проверяем, что контакт был добавлен
        let fetched = repo.get_contact(contact_id).expect("Failed to fetch contact");
        assert!(fetched.is_some());

        let fetched_contact: Contact = serde_json::from_str(&fetched.unwrap()).expect("Failed to deserialize contact");
        assert_eq!(fetched_contact.id, contact_id);
        assert_eq!(fetched_contact.first_name, "Daisy");
        assert_eq!(fetched_contact.last_name, "Duck");
        assert_eq!(fetched_contact.relationship, ContactRelationship::Friend.to_i64());
        assert_eq!(fetched_contact.username, Some("daisyduck".to_string()));
        assert_eq!(fetched_contact.language, Some("en".to_string()));
        assert_eq!(fetched_contact.picture_data, Some(vec![255, 254, 253]));
        assert_eq!(fetched_contact.picture_url, Some("http://example.com/daisy.jpg".to_string()));
        assert_eq!(fetched_contact.last_message_at, Some(1625078000));
        assert_eq!(fetched_contact.is_pro, 1);
    }

    #[test]
    fn test_add_invalid_contact() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        // Некорректный JSON
        let invalid_json = "{ invalid json }".to_string();

        // Пытаемся добавить контакт через некорректный JSON
        let result = repo.add_contact_pb_json(&invalid_json);
        assert!(result.is_err());

        if let Err(ContactError::Other(msg)) = result {
            assert!(msg != "");
        } else {
            panic!("Expected ContactError::Other");
        }
    }

    #[test]
    fn test_delete_nonexistent_contact() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        let non_existent_id = Uuid::now_v7();

        // Пытаемся удалить несуществующий контакт
        let result = repo.delete_contact(non_existent_id);
        assert!(result.is_ok()); // Удаление несуществующего контакта не должно вызывать ошибку
    }

    #[test]
    fn test_get_nonexistent_contact() {
        let conn = setup_test_db();
        let repo = ContactRepo::new(&conn);

        let non_existent_id = Uuid::now_v7();

        // Пытаемся получить несуществующий контакт
        let fetched = repo.get_contact(non_existent_id).expect("Failed to fetch contact");
        assert!(fetched.is_none());
    }
}
