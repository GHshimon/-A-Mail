//! Tauri コマンド(IPC 境界)。
//!
//! commands 層は「引数検証 + ドメイン呼び出し + DTO 変換」だけを担い、
//! 業務ロジックは各ドメインモジュールに置く。

mod account_cmd;
mod ai_cmd;
mod mail_cmd;
mod search_cmd;

pub use account_cmd::{add_account, list_accounts, remove_account};
pub use ai_cmd::{get_settings, has_gemini_key, set_gemini_key, update_settings};
pub use mail_cmd::{
    get_message, list_folders, list_messages, sync_folder, sync_folders, test_connection,
};
pub use search_cmd::search_messages;
