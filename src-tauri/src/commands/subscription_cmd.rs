//! IPC boundary for the optional ChatGPT-managed Codex provider.
//!
//! These commands expose status and explicit user-driven account actions only.
//! They never generate or send mail.

use crate::codex_subscription::{SubscriptionLoginPrompt, SubscriptionStatus};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use serde::Deserialize;
#[cfg(target_os = "macos")]
use std::process::Stdio;
use tauri::State;
#[cfg(target_os = "macos")]
use tokio::process::Command;
#[cfg(target_os = "macos")]
use tokio::time::{timeout, Duration};

#[derive(Deserialize)]
pub struct SubscriptionStatusReq {}

#[derive(Deserialize)]
pub struct StartChatGptLoginReq {
    pub request_id: String,
}

#[derive(Deserialize)]
pub struct CancelChatGptLoginReq {
    pub request_id: String,
    pub login_id: Option<String>,
}

#[derive(Deserialize)]
pub struct DisconnectSubscriptionReq {
    pub confirm_shared_logout: bool,
}

#[tauri::command]
pub async fn assistant_get_subscription_status(
    state: State<'_, AppState>,
    _req: SubscriptionStatusReq,
) -> AppResult<SubscriptionStatus> {
    Ok(state.codex_subscription.status().await)
}

#[tauri::command]
pub async fn assistant_start_chatgpt_login(
    state: State<'_, AppState>,
    req: StartChatGptLoginReq,
) -> AppResult<SubscriptionLoginPrompt> {
    let prompt = state.codex_subscription.start_login(req.request_id).await?;
    // The URL was HTTPS/host allowlisted before it reached this command. On
    // macOS, use LaunchServices rather than navigating the mail WebView away.
    // Failure is non-fatal because the UI always exposes the same safe link.
    open_system_auth_url(&prompt.auth_url).await;
    Ok(prompt)
}

#[cfg(target_os = "macos")]
async fn open_system_auth_url(auth_url: &str) {
    let mut command = Command::new("/usr/bin/open");
    command
        .arg(auth_url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Ok(mut child) = command.spawn() {
        if timeout(Duration::from_secs(5), child.wait()).await.is_err() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
}

#[cfg(not(target_os = "macos"))]
async fn open_system_auth_url(_auth_url: &str) {}

#[tauri::command]
pub async fn assistant_cancel_chatgpt_login(
    state: State<'_, AppState>,
    req: CancelChatGptLoginReq,
) -> AppResult<()> {
    state
        .codex_subscription
        .cancel_login(&req.request_id, req.login_id.as_deref())
        .await
}

#[tauri::command]
pub async fn assistant_disconnect_subscription(
    state: State<'_, AppState>,
    req: DisconnectSubscriptionReq,
) -> AppResult<SubscriptionStatus> {
    if !req.confirm_shared_logout {
        return Err(AppError::BadInput(
            "このMacのCodex CLIなどもChatGPTからログアウトします。確認が必要です".into(),
        ));
    }
    let status = state.codex_subscription.disconnect().await?;
    state.db()?.execute(
        "UPDATE assistant_settings SET subscription_reply_enabled=0 WHERE subscription_reply_enabled!=0",
        [],
    )?;
    Ok(status)
}
