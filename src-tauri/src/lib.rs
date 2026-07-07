//! -A-Mail Rust コア。`main.rs` はこの `run()` を呼ぶだけ。

pub mod account;
pub mod ai;
pub mod commands;
pub mod crypto;
pub mod error;
pub mod imap;
pub mod search;
pub mod smtp;
pub mod state;
pub mod store;
pub mod util;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ログ初期化。既定は info。秘密情報(パスワード/APIキー/本文)はログに出さない。
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .setup(|app| {
            // AppState(DB + HTTP)を初期化して注入。
            let state = AppState::new(app.handle())?;
            app.manage(state);

            // macOS: サイドバー vibrancy を適用(半透明)。
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                if let Some(window) = app.get_webview_window("main") {
                    let _ = apply_vibrancy(
                        &window,
                        NSVisualEffectMaterial::Sidebar,
                        None,
                        None,
                    );
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::set_gemini_key,
            commands::has_gemini_key,
            commands::list_accounts,
            commands::add_account,
            commands::remove_account,
            commands::list_folders,
            commands::list_messages,
            commands::search_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
