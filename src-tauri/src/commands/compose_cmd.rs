//! 作成 / 送信 / 下書き の Tauri コマンド(M2)。
//!
//! ネットワーク I/O(SMTP 送信・IMAP 書き込み)中は DB ロックを保持しない。
//! 秘密情報(アプリパスワード・本文)は戻り値・ログに載せない。

use tauri::State;
use tracing::warn;

use crate::account::Provider;
use crate::crypto::keychain;
use crate::error::{AppError, AppResult};
use crate::imap;
use crate::smtp;
use crate::state::AppState;
use crate::store::{self, models::{DraftHeader, DraftInput, OutgoingMessage, SentInfo}};

/// メールを送信する。成功後、provider に応じ Sent へ APPEND し、返信元へ \Answered、
/// 下書きがあれば削除する(いずれも best-effort)。
#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    message: OutgoingMessage,
) -> AppResult<SentInfo> {
    // 1) 送信に必要な情報を DB から収集(ロックはこのブロック内だけ)。
    let (smtp, imap_conn, sent_target, answered_target) = {
        let conn = state.db()?;
        let smtp = store::account_smtp_conn(&conn, message.account_id)?
            .ok_or(AppError::NotFound)?;
        let imap_conn = store::account_conn(&conn, message.account_id)?
            .ok_or(AppError::NotFound)?;
        // Gmail は SMTP 送信分を自動で Sent に入れるため APPEND しない(二重化回避)。
        let sent_target = if smtp.provider == Provider::Gmail {
            None
        } else {
            store::sent_folder_target(&conn, message.account_id)?
        };
        // 返信元の Message-ID があればローカルの所在を引く(\Answered STORE 用)。
        let answered_target = match message.in_reply_to.as_deref() {
            Some(irt) if !irt.trim().is_empty() => {
                store::message_target_by_msgid(&conn, message.account_id, irt.trim())?
            }
            _ => None,
        };
        (smtp, imap_conn, sent_target, answered_target)
    };

    let pass = keychain::account_password(&smtp.email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;

    // 2) メッセージ組み立て + 送信。RFC822 バイト列は送信で mail を move する前に取得。
    let from_name = (!smtp.display_name.trim().is_empty()).then_some(smtp.display_name.as_str());
    let (mail, message_id) = smtp::send::build_message(&smtp.email, from_name, &message)?;
    let rfc822 = mail.formatted();
    smtp::send::send_message(
        &smtp.smtp_host,
        smtp.smtp_port,
        smtp.smtp_starttls,
        &smtp.email,
        &pass,
        mail,
    )
    .await?;

    // 3) 送信後処理(IMAP)。失敗しても送信自体は成功しているので致命にしない。
    let (_email, imap_host, imap_port) = imap_conn;
    let mut appended_to_sent = false;
    if sent_target.is_some() || answered_target.is_some() {
        match imap::client::open_session(&imap_host, imap_port, &smtp.email, &pass).await {
            Ok(mut session) => {
                if let Some(t) = &sent_target {
                    match imap::write::append_to_sent(&mut session, &t.imap_path, &rfc822).await {
                        Ok(()) => appended_to_sent = true,
                        Err(_) => warn!("Sent への APPEND に失敗(送信は成功)"),
                    }
                }
                if let Some((t, uid)) = &answered_target {
                    if imap::write::set_answered(&mut session, &t.imap_path, *uid)
                        .await
                        .is_err()
                    {
                        warn!("\\Answered の STORE に失敗(送信は成功)");
                    }
                }
                let _ = imap::client::logout(&mut session).await;
            }
            Err(_) => warn!("送信後処理の IMAP 接続に失敗(送信は成功)"),
        }
    }

    // 4) DB 後処理: 返信元へローカル \Answered、下書き削除。
    {
        let conn = state.db()?;
        if let Some(irt) = message.in_reply_to.as_deref() {
            if !irt.trim().is_empty() {
                let _ = store::mark_answered_local(&conn, message.account_id, irt.trim());
            }
        }
        if let Some(draft_id) = message.draft_id {
            let _ = store::delete_draft(&conn, draft_id);
        }
    }

    Ok(SentInfo {
        message_id,
        appended_to_sent,
    })
}

/// 下書きを保存(`id` があれば更新、無ければ新規)し、draft_id を返す。
#[tauri::command]
pub fn save_draft(state: State<'_, AppState>, draft: DraftInput) -> AppResult<i64> {
    let conn = state.db()?;
    store::upsert_draft(&conn, &draft)
}

/// アカウントの下書き一覧(更新の新しい順)。
#[tauri::command]
pub fn list_drafts(state: State<'_, AppState>, account_id: i64) -> AppResult<Vec<DraftHeader>> {
    let conn = state.db()?;
    store::list_drafts(&conn, account_id)
}

/// 下書きを削除。
#[tauri::command]
pub fn delete_draft(state: State<'_, AppState>, draft_id: i64) -> AppResult<()> {
    let conn = state.db()?;
    store::delete_draft(&conn, draft_id)
}
