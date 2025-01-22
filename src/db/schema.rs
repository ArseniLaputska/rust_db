pub const SCHEMA_V1: &str = r#"
BEGIN;

-- ContactData:
CREATE TABLE IF NOT EXISTS contact_data (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid           TEXT UNIQUE,
    first_name     TEXT,
    last_name      TEXT,
    language       TEXT,
    picture_data   BLOB,
    picture_url    TEXT,
    last_message_at INTEGER,
    created_at     INTEGER,
    updated_at     INTEGER,
    pro            INTEGER,
    relationship   INTEGER
);

-- MessageData:
CREATE TABLE IF NOT EXISTS message_data (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    from_uuid      TEXT, 
    to_uuid        TEXT,
    prev           TEXT,        -- хранить UUID предыдущего сообщения
    contact_id     INTEGER,     -- связь с contact_data.id
    status         INTEGER,
    audio_url      TEXT,
    duration       REAL,
    text           TEXT,
    client_text    TEXT,
    gpt_text       TEXT,
    server_text    TEXT,
    translated_text TEXT,
    language       TEXT,
    error          TEXT,
    created_at     INTEGER,
    updated_at     INTEGER,
    try_count      INTEGER
);

-- ContactBookData:
CREATE TABLE IF NOT EXISTS contact_book_data (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid            TEXT UNIQUE,
    first_name      TEXT,
    last_name       TEXT,
    nick_name       TEXT,
    phone_number    TEXT,
    email           TEXT,
    picture_url     TEXT,
    picture_data    BLOB,
    created_at      REAL,
    updated_at      REAL
);

-- ContactStatusData:
CREATE TABLE IF NOT EXISTS contact_status_data (
    id  INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT UNIQUE,
    status INTEGER
);

-- ContactSeenAtData:
CREATE TABLE IF NOT EXISTS contact_seen_at_data (
    id  INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT UNIQUE,
    date TEXT   -- будем хранить JSON (NSDictionary) как TEXT
);

------------------------------------------------------------------
-- Устанавливаем user_version = 1
PRAGMA user_version = 1;

COMMIT;
"#;