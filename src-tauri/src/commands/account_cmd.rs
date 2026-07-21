use tauri::State;

use crate::account::{Account, Provider};
use crate::crypto::keychain;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store;

/// 登録済みアカウント一覧。
#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> AppResult<Vec<Account>> {
    let conn = state.db()?;
    store::list_accounts(&conn)
}

/// アカウントを追加する。
///
/// - 接続設定はプロバイダのプリセットから解決(DB へ保存)。
/// - アプリパスワードは **Keychain のみ** に保管し、DB には残さない。
/// - Keychain 保存に失敗した場合は、直前に作った DB 行を戻す(不整合防止)。
#[tauri::command]
pub fn add_account(
    state: State<'_, AppState>,
    email: String,
    app_password: String,
    provider: Provider,
    display_name: String,
) -> AppResult<Account> {
    let email = email.trim().to_string();
    // Gmail 等のアプリパスワードは「abcd efgh ijkl mnop」と空白区切りで表示される。
    // 空白込みで入力されても通るよう、内部を含む全空白を除去する(app pw は英数字のみ)。
    let app_password: String = app_password.chars().filter(|c| !c.is_whitespace()).collect();
    let display_name = display_name.trim().to_string();

    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadInput("メールアドレスが不正です".into()));
    }
    if app_password.is_empty() {
        return Err(AppError::BadInput("アプリパスワードが空です".into()));
    }

    let conn = state.db()?;

    // 既存アドレスなら「パスワードの入れ直し」とみなして更新する
    // (UI に編集画面が無いため、再登録=更新でリカバリできるようにする)。
    if let Some(existing) = store::account_by_email(&conn, &email)? {
        keychain::set_account_password(&email, &app_password)?;
        if !display_name.is_empty() {
            store::update_account_display_name(&conn, existing.id, &display_name)?;
        }
        return store::account_by_email(&conn, &email)?
            .ok_or(AppError::NotFound);
    }

    let account = store::insert_account(&conn, &email, provider, &display_name)?;

    if let Err(e) = keychain::set_account_password(&email, &app_password) {
        // Keychain 保存に失敗 → DB 行を戻してから失敗を返す。
        let _ = store::delete_account(&conn, account.id);
        return Err(e);
    }

    Ok(account)
}

/// アカウントを削除する(DB 行 + Keychain のパスワード)。
#[tauri::command]
pub fn remove_account(state: State<'_, AppState>, account_id: i64) -> AppResult<()> {
    let conn = state.db()?;
    // 先にメールを引いてから削除(削除後は引けないため)。
    if let Some(email) = store::account_email(&conn, account_id)? {
        store::delete_account(&conn, account_id)?;
        // Keychain 側の失敗は致命ではない(DB は消えている)。
        let _ = keychain::delete_account_password(&email);
    }
    Ok(())
}
