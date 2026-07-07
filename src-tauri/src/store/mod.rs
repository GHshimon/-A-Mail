//! SQLite ストア: 接続・マイグレーション・CRUD。

pub mod db;
pub mod migrations;
pub mod models;

use rusqlite::Connection;

use crate::account::{Account, Provider};
use crate::error::{AppError, AppResult};
use models::{Folder, MessageHeader, Settings, SettingsPatch};

/// 設定(単一行 id=1)を取得。
pub fn get_settings(conn: &Connection) -> AppResult<Settings> {
    let s = conn.query_row(
        "SELECT ai_enabled, ai_scope_related_count, ai_send_body, model, theme, poll_interval_sec
         FROM settings WHERE id = 1",
        [],
        |r| {
            Ok(Settings {
                ai_enabled: r.get::<_, i64>(0)? != 0,
                ai_scope_related_count: r.get(1)?,
                ai_send_body: r.get::<_, i64>(2)? != 0,
                model: r.get(3)?,
                theme: r.get(4)?,
                poll_interval_sec: r.get(5)?,
            })
        },
    )?;
    Ok(s)
}

/// 設定の部分更新。指定フィールドのみ UPDATE し、更新後の全体を返す。
pub fn update_settings(conn: &Connection, patch: &SettingsPatch) -> AppResult<Settings> {
    if let Some(v) = patch.ai_enabled {
        conn.execute("UPDATE settings SET ai_enabled = ?1 WHERE id = 1", [v as i64])?;
    }
    if let Some(v) = patch.ai_scope_related_count {
        conn.execute(
            "UPDATE settings SET ai_scope_related_count = ?1 WHERE id = 1",
            [v],
        )?;
    }
    if let Some(v) = patch.ai_send_body {
        conn.execute("UPDATE settings SET ai_send_body = ?1 WHERE id = 1", [v as i64])?;
    }
    if let Some(v) = &patch.model {
        conn.execute("UPDATE settings SET model = ?1 WHERE id = 1", [v])?;
    }
    if let Some(v) = &patch.theme {
        conn.execute("UPDATE settings SET theme = ?1 WHERE id = 1", [v])?;
    }
    if let Some(v) = patch.poll_interval_sec {
        conn.execute("UPDATE settings SET poll_interval_sec = ?1 WHERE id = 1", [v])?;
    }
    get_settings(conn)
}

/// 登録済みアカウント一覧。
pub fn list_accounts(conn: &Connection) -> AppResult<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, email, provider, display_name, imap_host, imap_port,
                smtp_host, smtp_port, smtp_starttls
         FROM accounts ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        let provider_str: String = r.get(2)?;
        Ok(Account {
            id: r.get(0)?,
            email: r.get(1)?,
            provider: Provider::from_db_str(&provider_str)
                .unwrap_or(Provider::Gmail),
            display_name: r.get(3)?,
            imap_host: r.get(4)?,
            imap_port: r.get(5)?,
            smtp_host: r.get(6)?,
            smtp_port: r.get(7)?,
            smtp_starttls: r.get::<_, i64>(8)? != 0,
        })
    })?;
    let mut out = Vec::new();
    for a in rows {
        out.push(a?);
    }
    Ok(out)
}

/// あるアカウントのフォルダ一覧(未読件数つき)。
pub fn list_folders(conn: &Connection, account_id: i64) -> AppResult<Vec<Folder>> {
    // flags のビット 0 を \Seen とみなし、未読を集計。
    let mut stmt = conn.prepare(
        "SELECT f.id, f.account_id, f.name, f.imap_path, f.role,
                (SELECT COUNT(*) FROM messages m
                   WHERE m.folder_id = f.id AND (m.flags & 1) = 0) AS unread
         FROM folders f
         WHERE f.account_id = ?1
         ORDER BY f.id",
    )?;
    let rows = stmt.query_map([account_id], |r| {
        Ok(Folder {
            id: r.get(0)?,
            account_id: r.get(1)?,
            name: r.get(2)?,
            imap_path: r.get(3)?,
            role: r.get(4)?,
            unread: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for f in rows {
        out.push(f?);
    }
    Ok(out)
}

/// フォルダ内メッセージのヘッダをページング取得(新しい順)。
pub fn list_messages(
    conn: &Connection,
    folder_id: i64,
    offset: i64,
    limit: i64,
) -> AppResult<Vec<MessageHeader>> {
    if !(0..=500).contains(&limit) {
        return Err(AppError::BadInput("limit は 0〜500".into()));
    }
    let mut stmt = conn.prepare(
        "SELECT id, uid, subject, from_addr, date, snippet, flags, has_attachments
         FROM messages
         WHERE folder_id = ?1
         ORDER BY date DESC
         LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt.query_map([folder_id, limit, offset], |r| {
        let flags: i64 = r.get(6)?;
        Ok(MessageHeader {
            id: r.get(0)?,
            uid: r.get(1)?,
            subject: r.get(2)?,
            from: r.get(3)?,
            date: r.get(4)?,
            snippet: r.get(5)?,
            seen: (flags & 1) != 0,
            flagged: (flags & 2) != 0,
            has_attachments: r.get::<_, i64>(7)? != 0,
        })
    })?;
    let mut out = Vec::new();
    for m in rows {
        out.push(m?);
    }
    Ok(out)
}
