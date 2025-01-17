mod db;

use rusqlite::{Connection, Result};

fn main() -> Result<()> {
    let conn = Connection::open("db.sqlite3")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS person (
                  id    INTEGER PRIMARY KEY,
                  name  TEXT NOT NULL,
                  age   INTEGER
                  )",
        [],
    )?;

    println!("Таблица успешно создана!");
    Ok(())
}
