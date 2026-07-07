//! メッセージ取得(ヘッダ)。本文の遅延取得は後続スライス。
//!
//! ENVELOPE ではなく `BODY.PEEK[HEADER]` を取り、`mail-parser` で復号する。
//! これで RFC2047 エンコードワード(=?ISO-2022-JP?…)の日本語件名/差出人も
//! 正しく表示できる(`BODY.PEEK` なので \Seen は立たない)。

use async_imap::types::Flag;
use futures::StreamExt;
use mail_parser::MessageParser;

use super::client::ImapSession;
use crate::error::{AppError, AppResult};

/// 一覧表示用に取り出したヘッダ 1 件。
pub struct HeaderRow {
    pub uid: u32,
    pub subject: String,
    /// 差出人の表示名(無ければアドレス)。
    pub from: String,
    /// epoch 秒(INTERNALDATE)。
    pub date: i64,
    pub seen: bool,
    pub flagged: bool,
}

/// フォルダを SELECT し、直近 `limit` 件のヘッダを取得する。
/// 返り値: (UIDVALIDITY, UIDNEXT, ヘッダ列)。
pub async fn fetch_recent_headers(
    session: &mut ImapSession,
    mailbox: &str,
    limit: u32,
) -> AppResult<(i64, i64, Vec<HeaderRow>)> {
    let mb = session
        .select(mailbox)
        .await
        .map_err(|_| AppError::Imap("フォルダを開けませんでした".into()))?;
    let uidvalidity = mb.uid_validity.unwrap_or(0) as i64;
    let uidnext = mb.uid_next.unwrap_or(0) as i64;
    let exists = mb.exists;

    let mut out = Vec::new();
    if exists == 0 {
        return Ok((uidvalidity, uidnext, out));
    }

    // 直近 limit 件をシーケンス範囲で取得(UID も同時に得る)。
    let start = exists.saturating_sub(limit.saturating_sub(1)).max(1);
    let range = format!("{start}:{exists}");

    let mut fetches = session
        .fetch(range, "(UID FLAGS INTERNALDATE BODY.PEEK[HEADER])")
        .await
        .map_err(|_| AppError::Imap("メッセージの取得に失敗しました".into()))?;

    while let Some(item) = fetches.next().await {
        let f = item.map_err(|_| AppError::Imap("メッセージの解析に失敗しました".into()))?;
        let uid = f.uid.unwrap_or(0);
        if uid == 0 {
            continue;
        }
        let date = f.internal_date().map(|d| d.timestamp()).unwrap_or(0);
        let seen = f.flags().any(|fl| matches!(fl, Flag::Seen));
        let flagged = f.flags().any(|fl| matches!(fl, Flag::Flagged));

        let mut subject = String::new();
        let mut from = String::new();
        if let Some(bytes) = f.header() {
            if let Some(msg) = MessageParser::default().parse(bytes) {
                subject = msg.subject().unwrap_or("").to_string();
                from = msg
                    .from()
                    .and_then(|a| a.first())
                    .map(|addr| {
                        addr.name()
                            .map(|n| n.to_string())
                            .or_else(|| addr.address().map(|s| s.to_string()))
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
            }
        }
        out.push(HeaderRow {
            uid,
            subject,
            from,
            date,
            seen,
            flagged,
        });
    }
    Ok((uidvalidity, uidnext, out))
}
