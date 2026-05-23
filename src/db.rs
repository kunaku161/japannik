use rusqlite::Connection;
use crate::frequency::FREQ_COLS;

pub fn open(path: &str) -> Connection {
    Connection::open(path).expect("failed to open DB")
}

/// Creates tables and runs all migrations. Safe to call on every startup — fully idempotent.
pub fn ensure_schema(path: &str) {
    let conn = open(path);

    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         CREATE TABLE IF NOT EXISTS words (
             id        INTEGER PRIMARY KEY,
             word      TEXT NOT NULL,
             word_kana TEXT NOT NULL,
             pos       TEXT,
             is_known  BOOLEAN NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS glosses (
             id       INTEGER PRIMARY KEY AUTOINCREMENT,
             language TEXT NOT NULL,
             meaning  TEXT NOT NULL,
             word_id  INTEGER NOT NULL,
             FOREIGN KEY (word_id) REFERENCES words(id)
         );",
    )
    .expect("failed to create tables");

    // Column migrations — silently ignored when already present
    let _ = conn.execute_batch(
        "ALTER TABLE words ADD COLUMN is_known BOOLEAN NOT NULL DEFAULT 0",
    );
    for col in FREQ_COLS {
        let _ = conn.execute_batch(&format!(
            "ALTER TABLE words ADD COLUMN {col} INTEGER NOT NULL DEFAULT 0"
        ));
    }

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_glosses_word_id ON glosses(word_id);
         CREATE INDEX IF NOT EXISTS idx_words_word ON words(word);",
    )
    .expect("failed to create indexes");
}
