// db.rs (или src/db/mod.rs)
//
// Здесь:
// 1) включаем нужные модули из std, rusqlite и т.д.
// 2) создаём функцию init_db
// 3) регистрируем hooks
// 4) создаём тест
//
// Для удобства всё в одном файле; в реальном проекте обычно разбиваем на несколько модулей.

pub mod contact;
pub mod message;
pub mod contact_book;
pub mod contact_status;
pub mod contact_seen_at;
pub mod monitor;
pub mod schema;
pub mod migrations;

use rusqlite::{
    hooks::{Action, AuthAction, AuthContext, Authorization, TransactionOperation},
    Connection, Result,
};
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub fn init_db(conn: &Connection) -> Result<()> {
    // Пример создания одной таблицы (для наглядности):
    conn.execute(
        "CREATE TABLE IF NOT EXISTS contact_data (
            id BLOB PRIMARY KEY,
            first_name TEXT,
            last_name TEXT,
            created_at INTEGER
        )",
        [],
    )?;
    Ok(())
}

// Включаем "update_hook" из rusqlite::hooks.
// Он позволяет подписаться на события INSERT, UPDATE, DELETE.
// commit_hook / rollback_hook — отдельные методы.
pub fn register_hooks(conn: &Connection) {
    // Hook на commit (просто выводим в консоль)
    conn.commit_hook(Some(|| {
        println!("::HOOK:: Commit detected");
        // Если вернуть "true", то commit будет превращён в rollback,
        // но нам это не нужно, поэтому возвращаем false
        false
    }));

    conn.rollback_hook(Some(|| {
        println!("::ROLLBACK:: Rollback detected");
    }));

    // Hook на UPDATE/INSERT/DELETE
    conn.update_hook(Some(|action_code: Action, db: &str, table: &str, rowid: i64 | {
        // Пример: выводим информацию о произошедшем событии
        let action = match action_code {
            Action::UNKNOWN => {"Unknown"},
            Action::SQLITE_DELETE => {"Delete"},
            Action::SQLITE_INSERT => {"Insert"},
            Action::SQLITE_UPDATE => {"Update"},
            _ => {"Unknown"},
        };
        println!("::HOOK:: {action} on table '{table}' in DB '{db}', rowid: {rowid}");
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn test_hooks() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        init_db(&conn)?;
        register_hooks(&conn);

        // Допустим, создаём новую запись
        let new_id = Uuid::now_v7();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO contact_data (id, first_name, last_name, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![&new_id.as_bytes(), "John", "Doe", now],
        )?;

        // Обновим фамилию
        conn.execute(
            "UPDATE contact_data SET last_name = ?1 WHERE id = ?2",
            params!["Smith", &new_id.as_bytes()],
        )?;

        // Откатимся транзакцией, чтоб проверить rollback_hook
        {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "DELETE FROM contact_data WHERE id = ?1",
                params![&new_id.as_bytes()],
            )?;
            tx.rollback()?;
        }

        // Закоммитим что-то (для commit_hook)
        conn.execute(
            "INSERT INTO contact_data (id, first_name, last_name, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![&Uuid::now_v7().as_bytes(), "Jane", "Roe", now],
        )?;

        // Можно посмотреть, что реально осталось в базе
        let mut stmt = conn.prepare("SELECT id, first_name, last_name, created_at FROM contact_data")?;
        let rows = stmt.query_map(
            params![],
            |row| {
                let blob: Vec<u8> = row.get(0)?;
                let first_name: String = row.get(1)?;
                let last_name: String = row.get(2)?;
                let created_at: i64 = row.get(3)?;
                Ok((blob, first_name, last_name, created_at))
            })?;

        for r in rows {
            let (blob, fname, lname, created) = r?;
            println!(
                "Row: id={:?}, first_name={}, last_name={}, created_at={}",
                blob, fname, lname, created
            );
        }

        Ok(())
    }
}