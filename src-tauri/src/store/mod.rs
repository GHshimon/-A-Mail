//! SQLite ストア: 接続・マイグレーション・CRUD。

pub mod db;
pub mod migrations;
pub mod models;

use rusqlite::{params, Connection};

use crate::account::{Account, Provider};
use crate::error::{AppError, AppResult};
use models::{
    AttachmentMeta, DraftHeader, DraftInput, Folder, MessageFull, MessageHeader, Settings,
    SettingsPatch,
};

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

/// アカウントを追加(接続設定はプロバイダのプリセットから解決)。
/// パスワードは保存しない(呼び出し側で Keychain 管理)。
pub fn insert_account(
    conn: &Connection,
    email: &str,
    provider: Provider,
    display_name: &str,
) -> AppResult<Account> {
    // 重複は分かりやすい入力エラーに変換(UNIQUE 制約より前に判定)。
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM accounts WHERE email = ?1",
        [email],
        |r| r.get(0),
    )?;
    if exists > 0 {
        return Err(AppError::BadInput(
            "このメールアドレスは既に登録済みです".into(),
        ));
    }

    let p = provider.preset();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    conn.execute(
        "INSERT INTO accounts
           (email, provider, display_name, imap_host, imap_port,
            smtp_host, smtp_port, smtp_starttls, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            email,
            provider.as_db_str(),
            display_name,
            p.imap_host,
            p.imap_port,
            p.smtp_host,
            p.smtp_port,
            p.smtp_starttls as i64,
            now,
        ],
    )?;

    Ok(Account {
        id: conn.last_insert_rowid(),
        email: email.to_string(),
        provider,
        display_name: display_name.to_string(),
        imap_host: p.imap_host.to_string(),
        imap_port: p.imap_port,
        smtp_host: p.smtp_host.to_string(),
        smtp_port: p.smtp_port,
        smtp_starttls: p.smtp_starttls,
    })
}

/// アカウントのメールアドレスを引く(Keychain 削除用)。
pub fn account_email(conn: &Connection, account_id: i64) -> AppResult<Option<String>> {
    match conn.query_row(
        "SELECT email FROM accounts WHERE id = ?1",
        [account_id],
        |r| r.get::<_, String>(0),
    ) {
        Ok(email) => Ok(Some(email)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// アカウント削除(folders / messages は FK の ON DELETE CASCADE で連鎖削除)。
pub fn delete_account(conn: &Connection, account_id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM accounts WHERE id = ?1", [account_id])?;
    Ok(())
}

/// メールアドレスからアカウントを引く(再登録=パスワード更新の判定用)。
pub fn account_by_email(conn: &Connection, email: &str) -> AppResult<Option<Account>> {
    match conn.query_row(
        "SELECT id, email, provider, display_name, imap_host, imap_port,
                smtp_host, smtp_port, smtp_starttls
         FROM accounts WHERE email = ?1",
        [email],
        |r| {
            let provider_str: String = r.get(2)?;
            Ok(Account {
                id: r.get(0)?,
                email: r.get(1)?,
                provider: Provider::from_db_str(&provider_str).unwrap_or(Provider::Gmail),
                display_name: r.get(3)?,
                imap_host: r.get(4)?,
                imap_port: r.get(5)?,
                smtp_host: r.get(6)?,
                smtp_port: r.get(7)?,
                smtp_starttls: r.get::<_, i64>(8)? != 0,
            })
        },
    ) {
        Ok(a) => Ok(Some(a)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// 表示名を更新(再登録時に表示名を上書きできるように)。
pub fn update_account_display_name(
    conn: &Connection,
    account_id: i64,
    display_name: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE accounts SET display_name = ?1 WHERE id = ?2",
        params![display_name, account_id],
    )?;
    Ok(())
}

/// IMAP 接続に必要な情報 (email, imap_host, imap_port) を引く。
pub fn account_conn(conn: &Connection, account_id: i64) -> AppResult<Option<(String, String, u16)>> {
    match conn.query_row(
        "SELECT email, imap_host, imap_port FROM accounts WHERE id = ?1",
        [account_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, u16>(2)?)),
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// SMTP 送信に必要なアカウント情報。
pub struct SmtpConn {
    pub email: String,
    pub display_name: String,
    pub provider: Provider,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_starttls: bool,
}

/// SMTP 送信に必要な情報を引く(パスワードは Keychain 側)。
pub fn account_smtp_conn(conn: &Connection, account_id: i64) -> AppResult<Option<SmtpConn>> {
    match conn.query_row(
        "SELECT email, display_name, provider, smtp_host, smtp_port, smtp_starttls
         FROM accounts WHERE id = ?1",
        [account_id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, u16>(4)?,
                r.get::<_, i64>(5)? != 0,
            ))
        },
    ) {
        Ok((email, display_name, provider_str, smtp_host, smtp_port, smtp_starttls)) => {
            Ok(Some(SmtpConn {
                email,
                display_name,
                provider: Provider::from_db_str(&provider_str).unwrap_or(Provider::Gmail),
                smtp_host,
                smtp_port,
                smtp_starttls,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// IMAP 操作(APPEND / STORE)の宛先(フォルダ + 接続)。
pub struct ImapTarget {
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_path: String,
}

/// アカウントの Sent 相当フォルダ(role='sent')の IMAP ターゲットを引く。
pub fn sent_folder_target(conn: &Connection, account_id: i64) -> AppResult<Option<ImapTarget>> {
    match conn.query_row(
        "SELECT a.email, a.imap_host, a.imap_port, f.imap_path
         FROM folders f JOIN accounts a ON a.id = f.account_id
         WHERE f.account_id = ?1 AND f.role = 'sent'
         ORDER BY f.id LIMIT 1",
        [account_id],
        |r| {
            Ok(ImapTarget {
                email: r.get(0)?,
                imap_host: r.get(1)?,
                imap_port: r.get(2)?,
                imap_path: r.get(3)?,
            })
        },
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// Message-ID から元メッセージの IMAP ターゲット + UID を引く(\Answered STORE 用)。
pub fn message_target_by_msgid(
    conn: &Connection,
    account_id: i64,
    msgid: &str,
) -> AppResult<Option<(ImapTarget, u32)>> {
    match conn.query_row(
        "SELECT a.email, a.imap_host, a.imap_port, f.imap_path, m.uid
         FROM messages m
         JOIN folders f  ON f.id = m.folder_id
         JOIN accounts a ON a.id = m.account_id
         WHERE m.account_id = ?1 AND m.message_id = ?2
         ORDER BY m.id LIMIT 1",
        params![account_id, msgid],
        |r| {
            Ok((
                ImapTarget {
                    email: r.get(0)?,
                    imap_host: r.get(1)?,
                    imap_port: r.get(2)?,
                    imap_path: r.get(3)?,
                },
                r.get::<_, i64>(4)? as u32,
            ))
        },
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// 元メッセージ(Message-ID 一致)のローカル flags に \Answered(ビット2)を立てる。
pub fn mark_answered_local(conn: &Connection, account_id: i64, msgid: &str) -> AppResult<()> {
    // ビット割当は upsert_message_header と合わせる(0:seen,1:flagged,2:answered)。
    conn.execute(
        "UPDATE messages SET flags = flags | 4
         WHERE account_id = ?1 AND message_id = ?2",
        params![account_id, msgid],
    )?;
    Ok(())
}

/// 下書きを保存(id あれば更新、無ければ挿入)。draft_id を返す。
pub fn upsert_draft(conn: &Connection, d: &DraftInput) -> AppResult<i64> {
    let to_json = serde_json::to_string(&d.to).unwrap_or_else(|_| "[]".into());
    let cc_json = serde_json::to_string(&d.cc).unwrap_or_else(|_| "[]".into());
    let bcc_json = serde_json::to_string(&d.bcc).unwrap_or_else(|_| "[]".into());
    let refs = d.references.join(" ");
    let now = now_secs();

    match d.id {
        Some(id) => {
            let n = conn.execute(
                "UPDATE drafts SET
                   account_id = ?1, to_addrs = ?2, cc_addrs = ?3, bcc_addrs = ?4,
                   subject = ?5, body_text = ?6, body_html = ?7,
                   in_reply_to = ?8, reference_ids = ?9, updated_at = ?10
                 WHERE id = ?11",
                params![
                    d.account_id, to_json, cc_json, bcc_json, d.subject, d.body_text,
                    d.body_html, d.in_reply_to, refs, now, id,
                ],
            )?;
            if n == 0 {
                return Err(AppError::NotFound);
            }
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO drafts
                   (account_id, to_addrs, cc_addrs, bcc_addrs, subject, body_text,
                    body_html, in_reply_to, reference_ids, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    d.account_id, to_json, cc_json, bcc_json, d.subject, d.body_text,
                    d.body_html, d.in_reply_to, refs, now,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// アカウントの下書き一覧(更新の新しい順)。
pub fn list_drafts(conn: &Connection, account_id: i64) -> AppResult<Vec<DraftHeader>> {
    let mut stmt = conn.prepare(
        "SELECT id, account_id, subject, to_addrs, body_text, updated_at
         FROM drafts WHERE account_id = ?1 ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([account_id], |r| {
        let to_json: String = r.get(3)?;
        let body: String = r.get(4)?;
        Ok(DraftHeader {
            id: r.get(0)?,
            account_id: r.get(1)?,
            subject: r.get(2)?,
            to: serde_json::from_str(&to_json).unwrap_or_default(),
            snippet: body.chars().take(80).collect(),
            updated_at: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for d in rows {
        out.push(d?);
    }
    Ok(out)
}

/// 下書きを削除(未存在でも成功扱い)。
pub fn delete_draft(conn: &Connection, draft_id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM drafts WHERE id = ?1", [draft_id])?;
    Ok(())
}

/// フォルダ同期に必要な情報(account + 接続 + 現在の UIDVALIDITY)。
pub struct FolderSyncInfo {
    pub account_id: i64,
    pub imap_path: String,
    pub uidvalidity: Option<i64>,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
}

/// フォルダ ID からアカウント接続情報 + UIDVALIDITY を引く。
pub fn folder_sync_info(conn: &Connection, folder_id: i64) -> AppResult<Option<FolderSyncInfo>> {
    match conn.query_row(
        "SELECT f.account_id, f.imap_path, f.uidvalidity, a.email, a.imap_host, a.imap_port
         FROM folders f JOIN accounts a ON a.id = f.account_id
         WHERE f.id = ?1",
        [folder_id],
        |r| {
            Ok(FolderSyncInfo {
                account_id: r.get(0)?,
                imap_path: r.get(1)?,
                uidvalidity: r.get(2)?,
                email: r.get(3)?,
                imap_host: r.get(4)?,
                imap_port: r.get(5)?,
            })
        },
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// フォルダのメッセージキャッシュを全消去(UIDVALIDITY 変化時)。
pub fn clear_folder_messages(conn: &Connection, folder_id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM messages WHERE folder_id = ?1", [folder_id])?;
    Ok(())
}

/// フォルダの UIDVALIDITY / UIDNEXT / 最終同期時刻を更新。
pub fn set_folder_uidstate(
    conn: &Connection,
    folder_id: i64,
    uidvalidity: i64,
    uidnext: i64,
) -> AppResult<()> {
    let now = now_secs();
    conn.execute(
        "UPDATE folders SET uidvalidity = ?1, uidnext = ?2, last_synced = ?3 WHERE id = ?4",
        params![uidvalidity, uidnext, now, folder_id],
    )?;
    Ok(())
}

/// メッセージヘッダを upsert。既存は flags のみ更新(本文/スニペットは保持)。
#[allow(clippy::too_many_arguments)]
pub fn upsert_message_header(
    conn: &Connection,
    account_id: i64,
    folder_id: i64,
    uid: u32,
    from_addr: &str,
    subject: &str,
    date: i64,
    flags: i64,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO messages
           (account_id, folder_id, uid, from_addr, subject, date, snippet, flags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', ?7)
         ON CONFLICT(folder_id, uid) DO UPDATE SET flags = excluded.flags",
        params![account_id, folder_id, uid as i64, from_addr, subject, date, flags],
    )?;
    Ok(())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// メッセージの所在(フォルダ / UID / 本文取得済みか)を引く。
pub fn message_locate(conn: &Connection, message_id: i64) -> AppResult<Option<(i64, u32, bool)>> {
    match conn.query_row(
        "SELECT folder_id, uid, body_fetched FROM messages WHERE id = ?1",
        [message_id],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)? as u32,
                r.get::<_, i64>(2)? != 0,
            ))
        },
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// 取得した本文・宛先・添付を保存し、body_fetched=1 にする。
#[allow(clippy::too_many_arguments)]
pub fn save_message_body(
    conn: &Connection,
    message_id: i64,
    to_addrs: &[String],
    cc_addrs: &[String],
    message_id_hdr: &str,
    in_reply_to: Option<&str>,
    references: &[String],
    body_text: Option<&str>,
    body_html: Option<&str>,
    attachments: &[(String, String, i64, Option<String>)],
) -> AppResult<()> {
    let to_json = serde_json::to_string(to_addrs).unwrap_or_else(|_| "[]".into());
    let cc_json = serde_json::to_string(cc_addrs).unwrap_or_else(|_| "[]".into());
    let refs = references.join(" ");
    let has_att = !attachments.is_empty();

    conn.execute(
        "UPDATE messages SET
           to_addrs = ?1, cc_addrs = ?2, message_id = ?3, in_reply_to = ?4,
           reference_ids = ?5, body_text = ?6, body_html = ?7,
           has_attachments = ?8, body_fetched = 1
         WHERE id = ?9",
        params![
            to_json,
            cc_json,
            message_id_hdr,
            in_reply_to,
            refs,
            body_text,
            body_html,
            has_att as i64,
            message_id,
        ],
    )?;

    conn.execute("DELETE FROM attachments WHERE message_id = ?1", [message_id])?;
    for (filename, mime, size, cid) in attachments {
        conn.execute(
            "INSERT INTO attachments (message_id, filename, mime, size, content_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![message_id, filename, mime, size, cid],
        )?;
    }
    Ok(())
}

/// 本文込みのフル DTO を組み立てる。
pub fn get_message_full(conn: &Connection, message_id: i64) -> AppResult<MessageFull> {
    let (header, to_json, cc_json, msgid, in_reply_to, refs, body_text, body_html) = conn
        .query_row(
            "SELECT id, uid, subject, from_addr, date, snippet, flags, has_attachments,
                    to_addrs, cc_addrs, message_id, in_reply_to, reference_ids, body_text, body_html
             FROM messages WHERE id = ?1",
            [message_id],
            |r| {
                let flags: i64 = r.get(6)?;
                let header = MessageHeader {
                    id: r.get(0)?,
                    uid: r.get(1)?,
                    subject: r.get(2)?,
                    from: r.get(3)?,
                    date: r.get(4)?,
                    snippet: r.get(5)?,
                    seen: (flags & 1) != 0,
                    flagged: (flags & 2) != 0,
                    has_attachments: r.get::<_, i64>(7)? != 0,
                };
                Ok((
                    header,
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<String>>(12)?,
                    r.get::<_, Option<String>>(13)?,
                    r.get::<_, Option<String>>(14)?,
                ))
            },
        )?;

    let to: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
    let cc: Vec<String> = serde_json::from_str(&cc_json).unwrap_or_default();
    let references: Vec<String> = refs
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();

    let mut stmt = conn.prepare(
        "SELECT id, filename, mime, size, content_id FROM attachments
         WHERE message_id = ?1 ORDER BY id",
    )?;
    let atts = stmt
        .query_map([message_id], |r| {
            Ok(AttachmentMeta {
                id: r.get(0)?,
                filename: r.get(1)?,
                mime: r.get(2)?,
                size: r.get(3)?,
                content_id: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MessageFull {
        header,
        to,
        cc,
        message_id: msgid.unwrap_or_default(),
        in_reply_to,
        references,
        body_text,
        body_html,
        attachments: atts,
    })
}

/// フォルダを upsert(IMAP LIST 反映)。既存は name / role を更新。
pub fn upsert_folder(
    conn: &Connection,
    account_id: i64,
    imap_path: &str,
    name: &str,
    role: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO folders (account_id, name, imap_path, role)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(account_id, imap_path)
         DO UPDATE SET name = excluded.name, role = excluded.role",
        params![account_id, name, imap_path, role],
    )?;
    Ok(())
}
