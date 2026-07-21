//! フォルダ同期(LIST)。メッセージの初回/増分同期(UIDVALIDITY 等)は後続スライス。

use async_imap::types::NameAttribute;
use futures::StreamExt;

use super::client::ImapSession;
use crate::error::{AppError, AppResult};

/// LIST 1 件分の同期用情報。
pub struct FolderInfo {
    /// IMAP 上の完全パス(例 `[Gmail]/Sent Mail`)。
    pub imap_path: String,
    /// 表示名(パス末尾の要素)。
    pub name: String,
    /// 役割(inbox/sent/drafts/trash/junk/archive/starred)。無ければ None。
    pub role: Option<String>,
}

/// サーバのフォルダ(ラベル)一覧を取得する。
pub async fn fetch_folders(session: &mut ImapSession) -> AppResult<Vec<FolderInfo>> {
    let mut out = Vec::new();
    let mut list = session
        .list(Some(""), Some("*"))
        .await
        .map_err(|_| AppError::Imap("フォルダ一覧の取得に失敗しました".into()))?;

    while let Some(item) = list.next().await {
        let name = item.map_err(|_| AppError::Imap("フォルダ情報の解析に失敗しました".into()))?;
        let path = name.name().to_string();
        let role = infer_role(&path, name.attributes()).map(|s| s.to_string());
        out.push(FolderInfo {
            name: display_name(&path),
            imap_path: path,
            role,
        });
    }
    Ok(out)
}

/// SPECIAL-USE 属性を優先し、無ければ名前ヒューリスティックで役割を推定。
fn infer_role(path: &str, attrs: &[NameAttribute]) -> Option<&'static str> {
    for a in attrs {
        match a {
            NameAttribute::Sent => return Some("sent"),
            NameAttribute::Drafts => return Some("drafts"),
            NameAttribute::Trash => return Some("trash"),
            NameAttribute::Junk => return Some("junk"),
            NameAttribute::Archive | NameAttribute::All => return Some("archive"),
            NameAttribute::Flagged => return Some("starred"),
            _ => {}
        }
    }
    if path.eq_ignore_ascii_case("INBOX") {
        return Some("inbox");
    }
    if path.contains("Sent") {
        return Some("sent");
    }
    if path.contains("Draft") {
        return Some("drafts");
    }
    if path.contains("Trash") || path.contains("Deleted") {
        return Some("trash");
    }
    if path.contains("Junk") || path.contains("Spam") {
        return Some("junk");
    }
    if path.contains("Archive") || path.contains("All Mail") {
        return Some("archive");
    }
    None
}

/// パス末尾の要素を表示名とする(区切りは Gmail/iCloud とも `/`)。
/// メールボックス名は IMAP modified UTF-7 なので、表示用にデコードする
/// (`imap_path` は SELECT 等で使うため生のまま保持し、ここでは表示名だけ変換)。
fn display_name(path: &str) -> String {
    if path.eq_ignore_ascii_case("INBOX") {
        return "受信トレイ".to_string();
    }
    let leaf = path.rsplit('/').next().unwrap_or(path).trim();
    crate::util::imap_utf7::decode(leaf)
}
