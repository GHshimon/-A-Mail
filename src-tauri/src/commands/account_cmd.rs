use tauri::State;

use crate::account::Account;
use crate::error::AppResult;
use crate::state::AppState;
use crate::store;

/// 登録済みアカウント一覧。
#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> AppResult<Vec<Account>> {
    let conn = state.db()?;
    store::list_accounts(&conn)
}
