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
