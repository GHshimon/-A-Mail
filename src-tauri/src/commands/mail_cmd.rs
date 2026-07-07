use tauri::State;

use crate::crypto::keychain;
use crate::error::{AppError, AppResult};
use crate::imap;
use crate::state::AppState;
use crate::store::{self, models::{Folder, MessageHeader}};

/// フォルダ一覧(DB キャッシュ)。実データは sync_folders で反映。
#[tauri::command]
pub fn list_folders(state: State<'_, AppState>, account_id: i64) -> AppResult<Vec<Folder>> {
    let conn = state.db()?;
    store::list_folders(&conn, account_id)
}

/// IMAP へ接続・ログインできるか検証する(フォルダ操作はしない)。
#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>, account_id: i64) -> AppResult<()> {
    let (email, host, port) = {
        let conn = state.db()?;
        store::account_conn(&conn, account_id)?.ok_or(AppError::NotFound)?
    };
    let pass = keychain::account_password(&email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;

    let mut session = imap::client::open_session(&host, port, &email, &pass).await?;
    imap::client::logout(&mut session).await?;
    Ok(())
}

/// IMAP LIST でフォルダ(ラベル)を取得し DB に upsert、最新の一覧を返す。
#[tauri::command]
pub async fn sync_folders(state: State<'_, AppState>, account_id: i64) -> AppResult<Vec<Folder>> {
    let (email, host, port) = {
        let conn = state.db()?;
        store::account_conn(&conn, account_id)?.ok_or(AppError::NotFound)?
    };
    let pass = keychain::account_password(&email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;

    // ネットワーク I/O 中は DB ロックを保持しない(MutexGuard は上のブロックで解放済み)。
    let mut session = imap::client::open_session(&host, port, &email, &pass).await?;
    let infos = imap::sync::fetch_folders(&mut session).await?;
    let _ = imap::client::logout(&mut session).await;

    let folders = {
        let conn = state.db()?;
        for f in &infos {
            store::upsert_folder(&conn, account_id, &f.imap_path, &f.name, f.role.as_deref())?;
        }
        store::list_folders(&conn, account_id)?
    };
    Ok(folders)
}

/// フォルダを IMAP から同期してメッセージヘッダ(直近 `limit` 件)を取得・保存し、
/// 最新の一覧を返す。UIDVALIDITY が変化した場合はキャッシュを作り直す。
#[tauri::command]
pub async fn sync_folder(
    state: State<'_, AppState>,
    folder_id: i64,
    limit: Option<u32>,
) -> AppResult<Vec<MessageHeader>> {
    let info = {
        let conn = state.db()?;
        store::folder_sync_info(&conn, folder_id)?.ok_or(AppError::NotFound)?
    };
    let pass = keychain::account_password(&info.email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;
    let limit = limit.unwrap_or(50).clamp(1, 500);

    // ネットワーク I/O 中は DB ロックを保持しない。
    let mut session =
        imap::client::open_session(&info.imap_host, info.imap_port, &info.email, &pass).await?;
    let (uidvalidity, uidnext, headers) =
        imap::fetch::fetch_recent_headers(&mut session, &info.imap_path, limit).await?;
    let _ = imap::client::logout(&mut session).await;

    let out = {
        let conn = state.db()?;
        // UIDVALIDITY 変化 → UID の意味が変わるのでフォルダのキャッシュを破棄。
        if let Some(prev) = info.uidvalidity {
            if prev != uidvalidity {
                store::clear_folder_messages(&conn, folder_id)?;
            }
        }
        for h in &headers {
            let flags = (h.seen as i64) | ((h.flagged as i64) << 1);
            store::upsert_message_header(
                &conn,
                info.account_id,
                folder_id,
                h.uid,
                &h.from,
                &h.subject,
                h.date,
                flags,
            )?;
        }
        store::set_folder_uidstate(&conn, folder_id, uidvalidity, uidnext)?;
        store::list_messages(&conn, folder_id, 0, limit as i64)?
    };
    Ok(out)
}

/// メッセージヘッダのページング取得(DB キャッシュ)。
#[tauri::command]
pub fn list_messages(
    state: State<'_, AppState>,
    folder_id: i64,
    offset: i64,
    limit: i64,
) -> AppResult<Vec<MessageHeader>> {
    let conn = state.db()?;
    store::list_messages(&conn, folder_id, offset, limit)
}
