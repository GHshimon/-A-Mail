//! Tauri コマンド(IPC 境界)。
//!
//! commands 層は「引数検証 + ドメイン呼び出し + DTO 変換」だけを担い、
//! 業務ロジックは各ドメインモジュールに置く。

mod account_cmd;
mod ai_cmd;
mod ai_gen_cmd;
mod compose_cmd;
mod mail_cmd;
mod search_cmd;

// glob 再エクスポート必須。`#[tauri::command]` は関数の隣に補助アイテム
// (`__cmd__x` マクロ / `__tauri_command_name_x`)を生成し、`generate_handler!` は
// それらを `commands::` 直下から探す。名前指定の `pub use` は関数だけを運び
// 補助アイテムを取りこぼすため、glob で丸ごと再エクスポートする。
pub use account_cmd::*;
pub use ai_cmd::*;
pub use ai_gen_cmd::*;
pub use compose_cmd::*;
pub use mail_cmd::*;
pub use search_cmd::*;
