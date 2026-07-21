//! IMAP 書き込み系(送信後処理)。Sent への APPEND と \Answered の STORE。M2。
//!
//! いずれも失敗しても致命ではない(送信自体は成功済み)。呼び出し側は best-effort
//! で扱い、失敗はログに留める。エラーは種別のみに丸める。

use futures::StreamExt;

use super::client::ImapSession;
use crate::error::{AppError, AppResult};

/// 送信済みメッセージ(RFC822 バイト列)を Sent フォルダへ `\Seen` 付きで APPEND する。
pub async fn append_to_sent(
    session: &mut ImapSession,
    mailbox: &str,
    rfc822: &[u8],
) -> AppResult<()> {
    session
        .append(mailbox, Some("(\\Seen)"), None, rfc822)
        .await
        .map_err(|_| AppError::Imap("Sent への保存に失敗しました".into()))?;
    Ok(())
}

/// 元メッセージ(UID)へ `\Answered` を立てる(返信時)。
pub async fn set_answered(
    session: &mut ImapSession,
    mailbox: &str,
    uid: u32,
) -> AppResult<()> {
    session
        .select(mailbox)
        .await
        .map_err(|_| AppError::Imap("フォルダを開けませんでした".into()))?;

    let mut stream = session
        .uid_store(uid.to_string(), "+FLAGS (\\Answered)")
        .await
        .map_err(|_| AppError::Imap("フラグ更新に失敗しました".into()))?;

    // 応答(更新後の FETCH)を読み切ってコマンドを完了させる。
    while let Some(item) = stream.next().await {
        item.map_err(|_| AppError::Imap("フラグ更新の応答解析に失敗しました".into()))?;
    }
    Ok(())
}
