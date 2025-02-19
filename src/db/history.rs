use tokio_rusqlite::{Connection, Result as SqlResult};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Insert = 0,
    Update = 1,
    Delete = 2,
    Unknown = 3,
}

impl TryFrom<i64> for ChangeType {
    type Error = String;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChangeType::Insert),
            1 => Ok(ChangeType::Update),
            2 => Ok(ChangeType::Delete),
            3 => Ok(ChangeType::Unknown),
            _ => Err(format!("Invalid ChangeType value: {}", value)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: Option<i64>,
    pub entity_name: String,
    pub entity_id: Uuid,
    pub change_type: ChangeType,
    pub author: String,
    pub created_at: f64,
    pub sync_status: i64,
    pub try_count: i64,
}

pub struct PersistentHistory {
    conn: Arc<Connection>,
}

impl PersistentHistory {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
    pub async fn add_record(&self, record: HistoryRecord) -> SqlResult<i64> {
        let tx = self.conn.call(|conn| {
           conn.transaction().map_err(|e| e.into())
        }).await?;

        let entity_id_bytes = record.entity_id.as_bytes().to_vec();
        let change_type_int = record.change_type.clone() as i64;
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        tx.execute(
            r#"INSERT INTO history (
                entity_name,
                entity_id,
                change_type,
                author,
                created_at,
                sync_status,
                try_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            rusqlite::params![
                record.entity_name,
                entity_id_bytes,
                change_type_int,
                record.author,
                created_at,
                record.sync_status,
                record.try_count
            ],
        ).await?;

        let last_id = tx.last_insert_rowid();
        tx.commit().await?;
        Ok(last_id)
    }

    pub async fn get_records_after(&self, after_ts: f64) -> SqlResult<Vec<HistoryRecord>> {
        let mut stmt = self.conn.call(|conn| {
            conn.prepare(
                r#"SELECT
                id,
                entity_name,
                entity_id,
                change_type,
                author,
                created_at,
                sync_status,
                try_count
             FROM history
             WHERE created_at > ?1
             ORDER BY created_at ASC"#
            ).map_err(|e| e.into())
        }).await?;

        let rows = stmt.query_map([after_ts], |row| {
            let id: i64 = row.get(0)?;
            let entity_name: String = row.get(1)?;
            let entity_id_bytes: Vec<u8> = row.get(2)?;
            let change_type_int: i64 = row.get(3)?;
            let author: String = row.get(4)?;
            let created_at: f64 = row.get(5)?;
            let sync_status: i64 = row.get(6)?;
            let try_count: i64 = row.get(7)?;

            let entity_id = Uuid::from_slice(&entity_id_bytes)
                .unwrap_or(Uuid::nil());
            let change_type = ChangeType::try_from(change_type_int)
                .unwrap_or(ChangeType::Unknown);

            Ok(HistoryRecord {
                id: Some(id),
                entity_name,
                entity_id,
                change_type,
                author,
                created_at,
                sync_status,
                try_count
            })
        }).await?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub async fn update_sync_status(&self, record_id: i64, status: i64) -> SqlResult<()> {
        self.conn.call(|conn| {
            conn.execute(
                "UPDATE history SET sync_status = ?1, try_count = try_count + 1 WHERE id = ?2",
                rusqlite::params![status, record_id],
            ).map_err(|e| e.into())
        }).await?;
        Ok(())
    }
}