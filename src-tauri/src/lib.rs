//! -A-Mail Rust コア。`main.rs` はこの `run()` を呼ぶだけ。

pub mod account;
pub mod ai;
pub mod ai_projects;
pub mod assistant;
pub mod assistant_automation;
pub mod assistant_quality;
pub mod cli_jobs;
pub mod codex_subscription;
pub mod commands;
pub mod crypto;
pub mod error;
pub mod imap;
pub mod search;
pub mod smtp;
pub mod state;
pub mod store;
pub mod style_profile;
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
                    let _ = apply_vibrancy(&window, NSVisualEffectMaterial::Sidebar, None, None);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::set_gemini_key,
            commands::has_gemini_key,
            commands::set_openai_key,
            commands::has_openai_key,
            commands::list_accounts,
            commands::add_account,
            commands::remove_account,
            commands::test_connection,
            commands::sync_folders,
            commands::sync_folder,
            commands::list_folders,
            commands::list_messages,
            commands::load_more_messages,
            commands::get_message,
            commands::search_messages,
            commands::send_message,
            commands::save_draft,
            commands::list_drafts,
            commands::delete_draft,
            commands::ai_complete,
            commands::ai_context_sidebar,
            commands::ai_generate_reply,
            commands::list_ai_projects,
            commands::add_ai_project,
            commands::remove_ai_project,
            commands::search_ai_project_files,
            commands::get_cli_status,
            commands::start_cli_job,
            commands::get_cli_job,
            commands::cancel_cli_job,
            commands::assistant_list_workspaces,
            commands::assistant_create_workspace,
            commands::assistant_get_context,
            commands::assistant_bind_context,
            commands::assistant_list_project_scopes,
            commands::assistant_set_project_scope,
            commands::assistant_list_drafts,
            commands::assistant_get_draft,
            commands::assistant_save_draft,
            commands::assistant_discard_draft,
            commands::assistant_restore_draft,
            commands::assistant_generate_reply,
            commands::assistant_apply_reply,
            commands::assistant_confirm_recipients,
            commands::assistant_prepare_send,
            commands::assistant_send_draft,
            commands::assistant_get_settings,
            commands::assistant_update_settings,
            commands::assistant_process_queue,
            commands::assistant_list_style_rules,
            commands::assistant_save_style_rule,
            commands::assistant_delete_style_rule,
            commands::assistant_style_learning_preview,
            commands::assistant_start_style_learning,
            commands::assistant_get_style_learning_job,
            commands::assistant_cancel_style_learning,
            commands::assistant_list_style_profiles,
            commands::assistant_update_style_profile,
            commands::assistant_activate_style_profile,
            commands::assistant_delete_style_profile,
            commands::assistant_preview_style_profile,
            commands::assistant_get_subscription_status,
            commands::assistant_start_chatgpt_login,
            commands::assistant_cancel_chatgpt_login,
            commands::assistant_disconnect_subscription,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
