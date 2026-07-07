//! FTS5 検索クエリの組み立てと実行(B の土台)。

use rusqlite::Connection;

use crate::error::AppResult;
use crate::store::models::MessageHeader;

/// ユーザー入力を FTS5 の安全な MATCH 文字列へ変換する。
///
/// 各トークンをダブルクオートで囲み、末尾に `*` を付けて前方一致に。
/// これで FTS の演算子(AND/OR/NEAR や記号)による構文エラー・意図せぬ挙動を防ぐ。
fn build_match(query: &str) -> String {
    query
        .split_whitespace()
        .map(|t| {
            let escaped = t.replace('"', "\"\"");
            format!("\"{escaped}\"*")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 全文検索。関連度(rank)順で上位 `limit` 件のヘッダを返す。
pub fn search_messages(
    conn: &Connection,
    account_id: i64,
    query: &str,
    limit: u32,
) -> AppResult<Vec<MessageHeader>> {
    let match_expr = build_match(query);
    if match_expr.is_empty() {
        return Ok(Vec::new());
    }

    let limit = limit.min(200) as i64;
    let mut stmt = conn.prepare(
        "SELECT m.id, m.uid, m.subject, m.from_addr, m.date, m.snippet,
                m.flags, m.has_attachments
         FROM messages_fts
         JOIN messages m ON m.id = messages_fts.rowid
         WHERE messages_fts MATCH ?1 AND m.account_id = ?2
         ORDER BY rank
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![match_expr, account_id, limit],
        |r| {
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
        },
    )?;
    let mut out = Vec::new();
    for m in rows {
        out.push(m?);
    }
    Ok(out)
}
