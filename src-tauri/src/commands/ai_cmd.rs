use tauri::State;

use crate::crypto::keychain;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::{self, models::{Settings, SettingsPatch}};

/// 現在の設定。
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    let conn = state.db()?;
    store::get_settings(&conn)
}

/// 設定の部分更新。
#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> AppResult<Settings> {
    let conn = state.db()?;
    store::update_settings(&conn, &patch)
}

/// Gemini API キーを Keychain に保存(DB には保存しない)。
#[tauri::command]
pub fn set_gemini_key(api_key: String) -> AppResult<()> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err(AppError::BadInput("API キーが空です".into()));
    }
    keychain::set_gemini_key(key)
}

/// キー登録有無のみ(値は返さない)。
#[tauri::command]
pub fn has_gemini_key() -> AppResult<bool> {
    keychain::has_gemini_key()
}
