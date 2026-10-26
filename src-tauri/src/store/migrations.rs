//! `PRAGMA user_version` ベースの逐次マイグレーション。

use rusqlite::{Connection, OptionalExtension};

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
  model                  TEXT    NOT NULL DEFAULT 'gemini-flash-latest',
  theme                  TEXT    NOT NULL DEFAULT 'system',
  poll_interval_sec      INTEGER NOT NULL DEFAULT 120
);
INSERT OR IGNORE INTO settings (id) VALUES (1);
"#;

/// v2: 既定モデルを更新。`gemini-2.5-flash` は新規ユーザー/キーへ提供終了となり
/// 404("no longer available to new users")を返すため、`gemini-flash-latest`
/// (常に最新の flash を指すエイリアス)へ差し替える。既存インストールの行も更新。
const V2: &str = r#"
UPDATE settings SET model = 'gemini-flash-latest' WHERE model = 'gemini-2.5-flash';
"#;

/// v3: AI の実装を特定ベンダーから分離し、OpenAI Responses API を選択可能にする。
/// `model` は既存 Gemini 利用者との互換性のため Gemini 側の設定として維持する。
const V3_PROVIDER: &str = "ALTER TABLE settings ADD COLUMN ai_provider TEXT NOT NULL DEFAULT 'gemini' CHECK(ai_provider IN ('gemini','openai'));";
const V3_MODEL: &str =
    "ALTER TABLE settings ADD COLUMN openai_model TEXT NOT NULL DEFAULT 'gpt-5.6-luna';";

/// v4: AI が文脈として読むローカル/NASプロジェクトのルートを登録する。
/// ルート配下の内容はDBへ複製せず、検索時に読み取り専用で走査する。
const V4: &str = r#"
CREATE TABLE IF NOT EXISTS ai_projects (
  id         INTEGER PRIMARY KEY,
  name       TEXT    NOT NULL,
  root_path  TEXT    NOT NULL UNIQUE,
  enabled    INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ai_projects_enabled ON ai_projects(enabled, id);
"#;

/// v5: 確認型下書きアシスタント。既存の drafts 本文を正本のまま保ち、
/// 顧客境界・版・送信トークンは別テーブルに置く。
const V5: &str = r#"
CREATE TABLE IF NOT EXISTS assistant_customers (
  id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  name TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(account_id, name)
);
CREATE TABLE IF NOT EXISTS assistant_workspaces (
  id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  customer_id INTEGER NOT NULL REFERENCES assistant_customers(id) ON DELETE CASCADE,
  name TEXT NOT NULL, context_version INTEGER NOT NULL DEFAULT 1, updated_at INTEGER NOT NULL,
  UNIQUE(account_id, customer_id, name)
);
CREATE TABLE IF NOT EXISTS assistant_contexts (
  source_message_id INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  workspace_id INTEGER REFERENCES assistant_workspaces(id) ON DELETE SET NULL,
  thread_id INTEGER NOT NULL, status TEXT NOT NULL DEFAULT 'unassigned', review_reasons TEXT NOT NULL DEFAULT '[]'
);
CREATE TABLE IF NOT EXISTS assistant_draft_meta (
  draft_id INTEGER PRIMARY KEY REFERENCES drafts(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  session_id TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 1,
  source_message_id INTEGER REFERENCES messages(id) ON DELETE SET NULL,
  workspace_id INTEGER REFERENCES assistant_workspaces(id) ON DELETE SET NULL,
  intent TEXT NOT NULL DEFAULT '', reply_run_id TEXT, status TEXT NOT NULL DEFAULT 'editing',
  review_reasons TEXT NOT NULL DEFAULT '[]', updated_at INTEGER NOT NULL,
  UNIQUE(account_id, session_id)
);
CREATE TABLE IF NOT EXISTS assistant_reply_runs (
  id TEXT PRIMARY KEY, draft_id INTEGER NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
  source_message_id INTEGER REFERENCES messages(id) ON DELETE SET NULL,
  workspace_id INTEGER REFERENCES assistant_workspaces(id) ON DELETE SET NULL,
  context_version INTEGER NOT NULL, profile_version INTEGER NOT NULL, snapshot_hash TEXT NOT NULL,
  generated_text TEXT NOT NULL, questions_json TEXT NOT NULL, sources_json TEXT NOT NULL,
  review_reasons TEXT NOT NULL, status TEXT NOT NULL, truncated INTEGER NOT NULL DEFAULT 0,
  provider TEXT NOT NULL, model TEXT NOT NULL, input_chars INTEGER NOT NULL,
  estimated_input_tokens INTEGER NOT NULL, actual_input_tokens INTEGER, actual_output_tokens INTEGER,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS assistant_settings (
  account_id INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  auto_draft_enabled INTEGER NOT NULL DEFAULT 0, learning_enabled INTEGER NOT NULL DEFAULT 0,
  subscription_reply_enabled INTEGER NOT NULL DEFAULT 0,
  daily_generation_limit INTEGER NOT NULL DEFAULT 20 CHECK(daily_generation_limit BETWEEN 1 AND 100),
  auto_generated_today INTEGER NOT NULL DEFAULT 0, quota_day TEXT NOT NULL DEFAULT '', high_watermark INTEGER
);
CREATE TABLE IF NOT EXISTS assistant_style_rules (
  id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  workspace_id INTEGER REFERENCES assistant_workspaces(id) ON DELETE CASCADE,
  from_phrase TEXT NOT NULL, to_phrase TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('pending','active','rejected')), created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS assistant_style_profile_versions (account_id INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE, version INTEGER NOT NULL DEFAULT 1);
CREATE TABLE IF NOT EXISTS assistant_send_attempts (
  token TEXT PRIMARY KEY, draft_id INTEGER NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL, snapshot_hash TEXT NOT NULL, expires_at INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'prepared', message_id TEXT, warnings TEXT NOT NULL DEFAULT '[]'
);
CREATE TABLE IF NOT EXISTS assistant_recipient_confirmations (
  draft_id INTEGER PRIMARY KEY REFERENCES drafts(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL, recipients_hash TEXT NOT NULL, workspace_id INTEGER,
  confirmed_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS assistant_project_scopes (
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  project_id INTEGER NOT NULL REFERENCES ai_projects(id) ON DELETE CASCADE,
  workspace_id INTEGER REFERENCES assistant_workspaces(id) ON DELETE CASCADE,
  scope TEXT NOT NULL CHECK(scope IN ('disabled','common','workspace')), confirmed INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(account_id, project_id)
);
CREATE TABLE IF NOT EXISTS assistant_generation_jobs (
  id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  dedupe_key TEXT NOT NULL UNIQUE, status TEXT NOT NULL, created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_assistant_drafts_account ON assistant_draft_meta(account_id, updated_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assistant_one_live_send_per_draft
  ON assistant_send_attempts(draft_id) WHERE status IN ('prepared','sending');
"#;

/// On-demand style profiles. Source messages remain in the normal mail cache;
/// these tables store only aggregate metadata and abstract profile JSON.
const STYLE_PROFILE_P0: &str = r#"
CREATE TABLE IF NOT EXISTS assistant_style_profiles (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  workspace_id INTEGER REFERENCES assistant_workspaces(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('draft','active','archived','deleted')),
  source_count INTEGER NOT NULL DEFAULT 0,
  source_candidate_count INTEGER NOT NULL DEFAULT 0,
  source_range_start INTEGER,
  source_range_end INTEGER,
  source_set_hash TEXT NOT NULL,
  content_json TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  input_chars INTEGER NOT NULL DEFAULT 0,
  estimated_input_tokens INTEGER NOT NULL DEFAULT 0,
  actual_input_tokens INTEGER,
  actual_output_tokens INTEGER,
  consent_version TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  activated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assistant_style_profile_version
  ON assistant_style_profiles(account_id, IFNULL(workspace_id, 0), version);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assistant_style_profile_active
  ON assistant_style_profiles(account_id, IFNULL(workspace_id, 0)) WHERE status='active';
CREATE INDEX IF NOT EXISTS idx_assistant_style_profile_list
  ON assistant_style_profiles(account_id, updated_at DESC) WHERE status!='deleted';

CREATE TABLE IF NOT EXISTS assistant_style_profile_jobs (
  id TEXT PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  requested_count INTEGER NOT NULL CHECK(requested_count IN (100,200)),
  status TEXT NOT NULL CHECK(status IN ('queued','collecting','analyzing','awaiting_review','completed','failed','cancelled','interrupted')),
  progress_current INTEGER NOT NULL DEFAULT 0,
  progress_total INTEGER NOT NULL DEFAULT 0,
  preview_hash TEXT NOT NULL,
  error_kind TEXT,
  result_profile_id INTEGER REFERENCES assistant_style_profiles(id) ON DELETE SET NULL,
  created_at INTEGER NOT NULL,
  started_at INTEGER,
  finished_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_assistant_style_profile_jobs_account
  ON assistant_style_profile_jobs(account_id, created_at DESC);

UPDATE assistant_style_profile_jobs
SET status='interrupted',finished_at=COALESCE(finished_at,unixepoch())
WHERE status IN ('queued','collecting','analyzing');

-- Continuous post-send learning was an earlier experiment. Explicit jobs are
-- now the only learning entry point, including after upgrades.
UPDATE assistant_settings SET learning_enabled=0 WHERE learning_enabled!=0;
"#;

/// 現在のスキーマ版。マイグレーションを追加したらインクリメントする。
const LATEST: i64 = 9;

const CONTACTS: &str = r#"
CREATE TABLE IF NOT EXISTS contacts (
  id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  address TEXT NOT NULL, name TEXT NOT NULL DEFAULT '', seen_count INTEGER NOT NULL DEFAULT 1,
  last_seen INTEGER NOT NULL, UNIQUE(account_id, address)
);
CREATE INDEX IF NOT EXISTS idx_contacts_lookup ON contacts(account_id, address);
"#;

fn has_table(conn: &Connection, table: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?",
            [table],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> AppResult<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names.iter().any(|name| name == column))
}

fn record(conn: &Connection, name: &str) -> AppResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(name,applied_at) VALUES(?,unixepoch())",
        [name],
    )?;
    Ok(())
}

fn add_column(
    conn: &Connection,
    table: &str,
    column: &str,
    sql: &str,
    step: &str,
) -> AppResult<()> {
    if !has_column(conn, table, column)? {
        conn.execute_batch(sql)?;
    }
    record(conn, step)
}

pub fn apply(conn: &Connection) -> AppResult<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let had_category = has_column(conn, "messages", "category").unwrap_or(false);
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations(name TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
    )?;

    if !has_table(&tx, "messages")? {
        tx.execute_batch(V1)?;
    }
    record(&tx, "base/v1")?;

    // These steps are detected from actual structure. This safely converges both
    // the former Claude v4 lineage and the Codex v5 lineage.
    tx.execute_batch(V2)?;
    record(&tx, "codex/normalize-model")?;
    add_column(
        &tx,
        "settings",
        "ai_provider",
        V3_PROVIDER,
        "codex/settings-provider",
    )?;
    add_column(
        &tx,
        "settings",
        "openai_model",
        V3_MODEL,
        "codex/settings-openai-model",
    )?;
    if !has_table(&tx, "ai_projects")? {
        tx.execute_batch(V4)?;
    }
    record(&tx, "codex/ai-projects")?;

    tx.execute_batch(CONTACTS)?;
    record(&tx, "claude/contacts")?;
    add_column(
        &tx,
        "messages",
        "category",
        "ALTER TABLE messages ADD COLUMN category TEXT;",
        "claude/messages-category",
    )?;
    add_column(
        &tx,
        "messages",
        "category_reason",
        "ALTER TABLE messages ADD COLUMN category_reason TEXT;",
        "claude/messages-category-reason",
    )?;
    tx.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_messages_category ON messages(folder_id,category);",
    )?;
    add_column(
        &tx,
        "settings",
        "ai_classify_enabled",
        "ALTER TABLE settings ADD COLUMN ai_classify_enabled INTEGER NOT NULL DEFAULT 0;",
        "claude/settings-ai-classify",
    )?;
    // Only the old Claude v3 database still needs this one-time repair. A v4
    // database already ran it; a Codex database only just received NULL columns.
    if had_category && version == 3 {
        tx.execute_batch("UPDATE messages SET category=NULL WHERE category='other' AND (category_reason IS NULL OR category_reason='');")?;
    }
    record(&tx, "claude/repair-empty-other")?;

    tx.execute_batch(V5)?;
    record(&tx, "codex/assistant-p0")?;
    add_column(&tx, "assistant_settings", "auto_draft_initialized", "ALTER TABLE assistant_settings ADD COLUMN auto_draft_initialized INTEGER NOT NULL DEFAULT 0;", "assistant/auto-initialized")?;
    add_column(
        &tx,
        "assistant_settings",
        "subscription_reply_enabled",
        "ALTER TABLE assistant_settings ADD COLUMN subscription_reply_enabled INTEGER NOT NULL DEFAULT 0;",
        "assistant/subscription-reply-enabled",
    )?;
    add_column(&tx, "assistant_generation_jobs", "source_message_id", "ALTER TABLE assistant_generation_jobs ADD COLUMN source_message_id INTEGER REFERENCES messages(id) ON DELETE SET NULL;", "assistant/jobs-source")?;
    add_column(
        &tx,
        "assistant_generation_jobs",
        "rfc_identity",
        "ALTER TABLE assistant_generation_jobs ADD COLUMN rfc_identity TEXT;",
        "assistant/jobs-identity",
    )?;
    add_column(
        &tx,
        "assistant_generation_jobs",
        "quota_day",
        "ALTER TABLE assistant_generation_jobs ADD COLUMN quota_day TEXT NOT NULL DEFAULT '';",
        "assistant/jobs-quota-day",
    )?;
    add_column(
        &tx,
        "assistant_generation_jobs",
        "claimed_at",
        "ALTER TABLE assistant_generation_jobs ADD COLUMN claimed_at INTEGER;",
        "assistant/jobs-claimed-at",
    )?;
    add_column(
        &tx,
        "assistant_generation_jobs",
        "attempts",
        "ALTER TABLE assistant_generation_jobs ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;",
        "assistant/jobs-attempts",
    )?;
    tx.execute_batch(
        r#"CREATE TABLE IF NOT EXISTS assistant_auto_candidates (
          source_message_id INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
          account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
          rfc_identity TEXT NOT NULL, observed_at INTEGER NOT NULL,
          UNIQUE(account_id,rfc_identity)
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_assistant_jobs_account_identity ON assistant_generation_jobs(account_id,rfc_identity);
        CREATE INDEX IF NOT EXISTS idx_assistant_jobs_claim ON assistant_generation_jobs(account_id,status,created_at);"#,
    )?;
    record(&tx, "assistant/automation")?;

    tx.execute_batch(STYLE_PROFILE_P0)?;
    record(&tx, "assistant/on-demand-style-profile")?;

    tx.execute_batch(&format!("PRAGMA user_version = {LATEST};"))?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_database_has_provider_settings() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        apply(&conn).expect("migration succeeds");

        let (version, provider, model): (i64, String, String) = conn
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), ai_provider, openai_model
                 FROM settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("settings row");

        assert_eq!(version, LATEST);
        assert_eq!(provider, "gemini");
        assert_eq!(model, "gpt-5.6-luna");

        let project_columns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('ai_projects')",
                [],
                |row| row.get(0),
            )
            .expect("ai_projects schema");
        assert_eq!(project_columns, 5);
        assert!(has_table(&conn, "contacts").unwrap());
        assert!(has_column(&conn, "messages", "category").unwrap());
        assert!(has_column(&conn, "settings", "ai_classify_enabled").unwrap());
        assert!(has_column(&conn, "assistant_settings", "auto_draft_initialized").unwrap());
        assert!(has_column(&conn, "assistant_settings", "subscription_reply_enabled").unwrap());
        conn.execute(
            "INSERT INTO accounts(id,email,provider,display_name,imap_host,imap_port,smtp_host,smtp_port,smtp_starttls,created_at)
             VALUES(99,'subscription@example.test','gmail','','imap',993,'smtp',587,1,0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO assistant_settings(account_id) VALUES(99)", [])
            .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT subscription_reply_enabled FROM assistant_settings WHERE account_id=99",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
        assert!(has_table(&conn, "assistant_auto_candidates").unwrap());
        assert!(has_table(&conn, "assistant_style_profiles").unwrap());
        assert!(has_table(&conn, "assistant_style_profile_jobs").unwrap());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
    }

    #[test]
    fn codex_v3_database_is_upgraded_without_changing_settings() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(V1).expect("v1");
        conn.execute_batch(V2).expect("v2");
        conn.execute_batch(V3_PROVIDER).expect("provider");
        conn.execute_batch(V3_MODEL).expect("model");
        conn.execute_batch("PRAGMA user_version = 3;")
            .expect("version");

        apply(&conn).expect("v4 migration succeeds");

        let (version, provider): (i64, String) = conn
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), ai_provider
                 FROM settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("settings preserved");
        assert_eq!(version, LATEST);
        assert_eq!(provider, "gemini");
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        apply(&conn).unwrap();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        apply(&conn).unwrap();
        let after: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn claude_v4_lineage_converges_without_reclassifying_existing_other() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute_batch(CONTACTS).unwrap();
        conn.execute_batch(
            "ALTER TABLE messages ADD COLUMN category TEXT;
             ALTER TABLE messages ADD COLUMN category_reason TEXT;
             ALTER TABLE settings ADD COLUMN ai_classify_enabled INTEGER NOT NULL DEFAULT 0;
             INSERT INTO accounts(id,email,provider,display_name,imap_host,imap_port,smtp_host,smtp_port,smtp_starttls,created_at)
               VALUES(1,'test@example.test','gmail','','imap',993,'smtp',587,1,0);
             INSERT INTO folders(id,account_id,name,imap_path,role) VALUES(1,1,'Inbox','INBOX','inbox');
             INSERT INTO messages(id,account_id,folder_id,uid,subject,date,category,category_reason)
               VALUES(1,1,1,1,'keep',0,'other',NULL);
             PRAGMA user_version=4;",
        ).unwrap();
        apply(&conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT category FROM messages WHERE id=1", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "other"
        );
        assert!(has_column(&conn, "settings", "ai_provider").unwrap());
        assert!(has_table(&conn, "ai_projects").unwrap());
        assert!(has_table(&conn, "assistant_reply_runs").unwrap());
        assert!(has_table(&conn, "assistant_auto_candidates").unwrap());
    }

    #[test]
    fn codex_v5_lineage_gains_claude_tables_and_automation() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute_batch(V2).unwrap();
        conn.execute_batch(V3_PROVIDER).unwrap();
        conn.execute_batch(V3_MODEL).unwrap();
        conn.execute_batch(V4).unwrap();
        conn.execute_batch(V5).unwrap();
        conn.execute_batch("PRAGMA user_version=5;").unwrap();
        apply(&conn).unwrap();
        assert!(has_table(&conn, "contacts").unwrap());
        assert!(has_column(&conn, "messages", "category_reason").unwrap());
        assert!(has_column(&conn, "settings", "ai_classify_enabled").unwrap());
        assert!(has_column(&conn, "assistant_generation_jobs", "attempts").unwrap());
        assert!(has_table(&conn, "assistant_style_profiles").unwrap());
    }

    #[test]
    fn partial_assistant_schema_is_completed() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute_batch(
            "CREATE TABLE assistant_customers(id INTEGER PRIMARY KEY, account_id INTEGER, name TEXT, created_at INTEGER); PRAGMA user_version=5;",
        ).unwrap();
        apply(&conn).unwrap();
        assert!(has_table(&conn, "assistant_reply_runs").unwrap());
        assert!(has_table(&conn, "assistant_generation_jobs").unwrap());
        assert!(has_table(&conn, "assistant_style_profiles").unwrap());
    }

    #[test]
    fn upgrade_disables_legacy_continuous_learning() {
        let conn = Connection::open_in_memory().unwrap();
        apply(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id,email,provider,display_name,imap_host,imap_port,smtp_host,smtp_port,smtp_starttls,created_at)
             VALUES(1,'style@example.test','gmail','','imap',993,'smtp',587,1,0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO assistant_settings(account_id,learning_enabled) VALUES(1,1)",
            [],
        )
        .unwrap();

        apply(&conn).unwrap();

        assert_eq!(
            conn.query_row(
                "SELECT learning_enabled FROM assistant_settings WHERE account_id=1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
    }
}
