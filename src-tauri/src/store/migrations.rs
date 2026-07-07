//! `PRAGMA user_version` ベースの逐次マイグレーション。

use rusqlite::Connection;

use crate::error::AppResult;

/// v1: 初期スキーマ(accounts / folders / messages / FTS5 / attachments / drafts / settings)。
const V1: &str = r#"
CREATE TABLE accounts (
  id            INTEGER PRIMARY KEY,
  email         TEXT NOT NULL UNIQUE,
  provider      TEXT NOT NULL CHECK(provider IN ('gmail','icloud')),
  display_name  TEXT NOT NULL DEFAULT '',
  imap_host     TEXT NOT NULL, imap_port INTEGER NOT NULL,
  smtp_host     TEXT NOT NULL, smtp_port INTEGER NOT NULL,
  smtp_starttls INTEGER NOT NULL DEFAULT 1,
  created_at    INTEGER NOT NULL
);

CREATE TABLE folders (
  id             INTEGER PRIMARY KEY,
  account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  imap_path      TEXT NOT NULL,
  role           TEXT,
  uidvalidity    INTEGER,
  uidnext        INTEGER,
  highest_modseq INTEGER,
  last_synced    INTEGER,
  UNIQUE(account_id, imap_path)
);

CREATE TABLE messages (
  id              INTEGER PRIMARY KEY,
  account_id      INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  folder_id       INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  uid             INTEGER NOT NULL,
  message_id      TEXT,
  in_reply_to     TEXT,
  reference_ids   TEXT,
  from_addr       TEXT NOT NULL DEFAULT '',
  to_addrs        TEXT NOT NULL DEFAULT '',
  cc_addrs        TEXT NOT NULL DEFAULT '',
  subject         TEXT NOT NULL DEFAULT '',
  date            INTEGER NOT NULL,
  snippet         TEXT NOT NULL DEFAULT '',
  flags           INTEGER NOT NULL DEFAULT 0,
  has_attachments INTEGER NOT NULL DEFAULT 0,
  body_fetched    INTEGER NOT NULL DEFAULT 0,
  body_text       TEXT,
  body_html       TEXT,
  UNIQUE(folder_id, uid)
);
CREATE INDEX idx_messages_folder_date ON messages(folder_id, date DESC);
CREATE INDEX idx_messages_uid         ON messages(folder_id, uid);
CREATE INDEX idx_messages_msgid       ON messages(message_id);

CREATE VIRTUAL TABLE messages_fts USING fts5(
  subject, from_addr, to_addrs, body_text,
  content='messages', content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, subject, from_addr, to_addrs, body_text)
  VALUES (new.id, new.subject, new.from_addr, new.to_addrs, coalesce(new.body_text,''));
END;
CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, subject, from_addr, to_addrs, body_text)
  VALUES('delete', old.id, old.subject, old.from_addr, old.to_addrs, coalesce(old.body_text,''));
END;
CREATE TRIGGER messages_au AFTER UPDATE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, subject, from_addr, to_addrs, body_text)
  VALUES('delete', old.id, old.subject, old.from_addr, old.to_addrs, coalesce(old.body_text,''));
  INSERT INTO messages_fts(rowid, subject, from_addr, to_addrs, body_text)
  VALUES (new.id, new.subject, new.from_addr, new.to_addrs, coalesce(new.body_text,''));
END;

CREATE TABLE attachments (
  id          INTEGER PRIMARY KEY,
  message_id  INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  filename    TEXT NOT NULL,
  mime        TEXT NOT NULL,
  size        INTEGER NOT NULL DEFAULT 0,
  content_id  TEXT,
  local_path  TEXT
);
CREATE INDEX idx_attachments_msg ON attachments(message_id);

CREATE TABLE drafts (
  id            INTEGER PRIMARY KEY,
  account_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  to_addrs      TEXT NOT NULL DEFAULT '',
  cc_addrs      TEXT NOT NULL DEFAULT '',
  bcc_addrs     TEXT NOT NULL DEFAULT '',
  subject       TEXT NOT NULL DEFAULT '',
  body_text     TEXT NOT NULL DEFAULT '',
  body_html     TEXT,
  in_reply_to   TEXT,
  reference_ids TEXT,
  updated_at    INTEGER NOT NULL
);

CREATE TABLE settings (
  id                     INTEGER PRIMARY KEY CHECK(id=1),
  ai_enabled             INTEGER NOT NULL DEFAULT 0,
  ai_scope_related_count INTEGER NOT NULL DEFAULT 5,
  ai_send_body           INTEGER NOT NULL DEFAULT 1,
  model                  TEXT    NOT NULL DEFAULT 'gemini-2.5-flash',
  theme                  TEXT    NOT NULL DEFAULT 'system',
  poll_interval_sec      INTEGER NOT NULL DEFAULT 120
);
INSERT OR IGNORE INTO settings (id) VALUES (1);
"#;

/// 現在のスキーマ版。マイグレーションを追加したらインクリメントする。
const LATEST: i64 = 1;

pub fn apply(conn: &Connection) -> AppResult<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if version < 1 {
        conn.execute_batch(V1)?;
    }

    // 将来: if version < 2 { conn.execute_batch(V2)?; } ...

    if version < LATEST {
        // user_version はバインド不可のため直接埋め込む(内部定数のみ)。
        conn.execute_batch(&format!("PRAGMA user_version = {LATEST};"))?;
    }
    Ok(())
}
