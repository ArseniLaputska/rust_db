use tokio_rusqlite::{Connection, Result};
use crate::db::schema::SCHEMA_V1;

pub fn setup_migrations(conn: &Connection) -> Result<()> {
    // Узнаём текущую версию схемы
    let ver: i32 = conn.query_row("PRAGMA user_version;", [], |r| r.get(0))?;

    // Если 0 -> выполняем SCHEMA_V1
    if ver < 1 {
        conn.execute_batch(SCHEMA_V1)?;
    }

    // Если в будущем мы решим добавить вторую версию (SCHEMA_V2),
    // то тут появятся проверка `ver < 2 { ... }`

    Ok(())
}