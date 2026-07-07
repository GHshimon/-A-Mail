//! Keychain(macOS)ラッパ。`keyring` crate 経由でパスワード / API キーを保管。
//!
//! 秘密の値はここでのみ扱い、コマンド戻り値やログには絶対に出さない。
//! サービス名は SPEC 10.1 の `com.a-mail.app`。

use keyring::Entry;

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.a-mail.app";
const GEMINI_ACCOUNT: &str = "gemini_api_key";

fn entry(account: &str) -> AppResult<Entry> {
    Entry::new(SERVICE, account).map_err(AppError::from)
}

/// 任意アカウントキーに対する保存(例: `imap:{email}`)。
pub fn set_password(account: &str, secret: &str) -> AppResult<()> {
    entry(account)?.set_password(secret).map_err(AppError::from)
}

/// 取得。未登録なら `None`。
pub fn get_password(account: &str) -> AppResult<Option<String>> {
    match entry(account)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// 削除。未登録でも成功扱い。
pub fn delete_password(account: &str) -> AppResult<()> {
    match entry(account)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::from(e)),
    }
}

// ---- Gemini API キー専用ヘルパ ----

pub fn set_gemini_key(key: &str) -> AppResult<()> {
    set_password(GEMINI_ACCOUNT, key)
}

/// 登録有無のみ(値は返さない)。
pub fn has_gemini_key() -> AppResult<bool> {
    Ok(get_password(GEMINI_ACCOUNT)?.is_some())
}

/// Rust 内部でのみ使用(AI 呼び出し時。フロントには渡さない)。
pub fn gemini_key() -> AppResult<Option<String>> {
    get_password(GEMINI_ACCOUNT)
}
