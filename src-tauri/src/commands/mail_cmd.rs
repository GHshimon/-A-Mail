use tauri::State;

use crate::error::AppResult;
use crate::state::AppState;
use crate::store::{self, models::{Folder, MessageHeader}};

/// フォルダ一覧(DB キャッシュ)。IMAP LIST 反映は M1。
#[tauri::command]
pub fn list_folders(state: State<'_, AppState>, account_id: i64) -> AppResult<Vec<Folder>> {
    let conn = state.db()?;
    store::list_folders(&conn, account_id)
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
