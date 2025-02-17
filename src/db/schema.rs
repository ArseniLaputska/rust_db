pub const SCHEMA_V1: &str = r#"
BEGIN;

-- History:
CREATE TABLE
    IF NOT EXISTS history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        entity_name TEXT NOT NULL,
        entity_id BLOB NOT NULL CHECK (length (entity_id) = 16),
        change_type INTEGER NOT NULL,
        author TEXT NOT NULL,
        created_at REAL NOT NULL,
        sync_status INTEGER NOT NULL,
        try_count INTEGER NOT NULL DEFAULT 0
    );

-- Contact:
CREATE TABLE
    IF NOT EXISTS contact (
        id BLOB PRIMARY KEY CHECK (length (id) = 16),
        first_name TEXT NOT NULL,
        last_name TEXT NOT NULL,
        relationship INTEGER NOT NULL,
        username TEXT,
        language TEXT,
        picture_url TEXT,
        last_message_at REAL,
        created_at REAL NOT NULL,
        updated_at REAL NOT NULL,
        is_pro REAL
    );

-- Message:
-- v7 UUID
-- offset time.now() <-> offset time.now()+offset
-- v4 temp <-> api  f88644f4-eaa7-[1]1ef-aea0-af16be626e57
-- tmp_id + id
CREATE TABLE
    IF NOT EXISTS message (
        id BLOB PRIMARY KEY CHECK (length (id) = 16),
        "from" BLOB NOT NULL CHECK (length ("from") = 16),
        "to" BLOB CHECK (length ("to") = 16),
        prev BLOB CHECK (length (prev) = 16),
        contact_id BLOB CHECK (length (contact_id) = 16),
        status INTEGER,
        audio_url TEXT,
        duration REAL,
        text TEXT,
        client_text TEXT,
        gpt_text TEXT,
        server_text TEXT,
        translated_text TEXT CHECK (
            translated_text IS NULL
            OR json_valid (translated_text)
        ),
        language TEXT,
        error TEXT,
        created_at REAL NOT NULL,
        updated_at REAL NOT NULL
    );

-- ContactBook:
CREATE TABLE
    IF NOT EXISTS contact_book (
        id BLOB PRIMARY KEY CHECK (length (id) = 16),
        first_name TEXT,
        last_name TEXT,
        nick_name TEXT,
        phone_number TEXT,
        email TEXT,
        picture_url TEXT,
        -- picture_data BLOB,
        created_at REAL NOT NULL,
        updated_at REAL NOT NULL
    );

-- ContactStatus:
CREATE TABLE
    IF NOT EXISTS contact_status (
        id BLOB PRIMARY KEY CHECK (length (id) = 16),
        status INTEGER
    );

-- ContactSeenAt:
CREATE TABLE
    IF NOT EXISTS contact_seen_at (
        id BLOB PRIMARY KEY CHECK (length (id) = 16),
        user_id BLOB CHECK (length (user_id) = 16),
        contact_id BLOB CHECK (length (contact_id) = 16),
        date REAL
    );

------------------------------------------------------------------
-- Устанавливаем user_version = 1
PRAGMA user_version = 1;

COMMIT;
"#;