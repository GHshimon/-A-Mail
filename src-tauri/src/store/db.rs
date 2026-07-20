//! SQLite 接続の生成とマイグレーション適用。

use std::path::Path;

use rusqlite::Connection;

use super::migrations;
use crate::error::AppResult;

/// データディレクトリ配下に DB を開き、PRAGMA 設定 + マイグレーションを適用する。
pub fn open(data_dir: &Path) -> AppResult<Connection> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join("a-mail.sqlite");

    let conn = Connection::open(path)?;
    // WAL で読み並行、外部キー有効化。
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;

    migrations::apply(&conn)?;
    Ok(conn)
}
