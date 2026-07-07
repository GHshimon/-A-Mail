use tauri::State;

use crate::error::AppResult;
use crate::search;
use crate::state::AppState;
use crate::store::models::MessageHeader;

/// FTS5 全文検索(B の土台)。M0 は検索土台のみ(キャッシュが空なら空結果)。
#[tauri::command]
pub fn search_messages(
    state: State<'_, AppState>,
    account_id: i64,
    query: String,
    limit: u32,
) -> AppResult<Vec<MessageHeader>> {
    let conn = state.db()?;
    search::search_messages(&conn, account_id, &query, limit)
}
