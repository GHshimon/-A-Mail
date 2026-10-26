//! ChatGPT-managed Codex connection and reply-only provider.
//!
//! Authentication/status uses the official app-server account API. Reply
//! generation deliberately uses a fresh `codex exec --ephemeral` process so
//! mail prompts are not written to Codex rollout history. The process runs in
//! an empty temporary cwd with user config, rules, MCP, apps, hooks and web
//! disabled. Any observed tool execution makes the result unusable.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{oneshot, watch, Mutex};
use tokio::time::{timeout, Instant};

use crate::error::{AppError, AppResult};

const RPC_TIMEOUT: Duration = Duration::from_secs(8);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const GENERATION_TIMEOUT: Duration = Duration::from_secs(60);
const OUTPUT_LINE_LIMIT: usize = 512 * 1024;
const STDERR_LIMIT: usize = 64 * 1024;
const PROMPT_LIMIT: usize = 32_000;
const FINAL_OUTPUT_LIMIT: usize = 32 * 1024;
pub const SUBSCRIPTION_MODEL: &str = "gpt-5.6-terra";

/// Real-mail use remains compile-time gated until AI04/LC01 are accepted on a
/// packaged build. Browser mocks can still exercise the complete UI flow.
pub const fn release_enabled() -> bool {
    cfg!(feature = "codex-subscription-reply")
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SubscriptionRateWindow {
    pub used_percent: Option<f64>,
    pub window_duration_mins: Option<i64>,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SubscriptionLoginPrompt {
    pub request_id: String,
    pub login_id: String,
    pub auth_url: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SubscriptionStatus {
    pub runtime_available: bool,
    pub runtime_version: Option<String>,
    pub connection: String,
    pub plan_type: Option<String>,
    pub primary: Option<SubscriptionRateWindow>,
    pub secondary: Option<SubscriptionRateWindow>,
    pub rate_limit_reached: bool,
    pub release_enabled: bool,
    pub login: Option<SubscriptionLoginPrompt>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionGeneration {
    pub text: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

#[derive(Debug, Clone)]
struct LoginState {
    request_id: String,
    connection: String,
    prompt: Option<SubscriptionLoginPrompt>,
    plan_type: Option<String>,
    message: Option<String>,
}

#[derive(Default)]
struct RuntimeState {
    login: Option<LoginState>,
    cancel_login: Option<watch::Sender<bool>>,
}

pub struct CodexSubscriptionManager {
    state: Arc<Mutex<RuntimeState>>,
    generation: Mutex<()>,
    shutdown: watch::Sender<bool>,
    drop_cancel: StdMutex<Option<watch::Sender<bool>>>,
}

impl Default for CodexSubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexSubscriptionManager {
    pub fn new() -> Self {
        let (shutdown, _receiver) = watch::channel(false);
        Self {
            state: Arc::new(Mutex::new(RuntimeState::default())),
            generation: Mutex::new(()),
            shutdown: shutdown.clone(),
            drop_cancel: StdMutex::new(Some(shutdown)),
        }
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
        if let Ok(state) = self.drop_cancel.lock() {
            if let Some(cancel) = state.as_ref() {
                let _ = cancel.send(true);
            }
        }
    }

    pub async fn status(&self) -> SubscriptionStatus {
        let login = self.state.lock().await.login.clone();
        if let Some(login) = login.filter(|item| item.connection == "connecting") {
            return SubscriptionStatus {
                runtime_available: find_codex_binary().is_some(),
                runtime_version: codex_version().await,
                connection: login.connection,
                plan_type: login.plan_type,
                primary: None,
                secondary: None,
                rate_limit_reached: false,
                release_enabled: release_enabled(),
                login: login.prompt,
                message: login.message,
            };
        }

        let mut status = if release_enabled() {
            query_app_server_status().await
        } else {
            query_local_status().await
        };
        if let Some(login) = self.state.lock().await.login.clone() {
            if login.connection == "error" && status.connection != "connected" {
                status.connection = "error".into();
                status.message = login.message;
            }
        }
        status
    }

    pub async fn start_login(&self, request_id: String) -> AppResult<SubscriptionLoginPrompt> {
        if !release_enabled() {
            return Err(AppError::Ai(
                "ChatGPT接続は隔離受入前のため、このビルドでは無効です".into(),
            ));
        }
        validate_request_id(&request_id)?;
        {
            let state = self.state.lock().await;
            if let Some(login) = state.login.as_ref() {
                if login.connection == "connecting" {
                    if login.request_id == request_id {
                        if let Some(prompt) = &login.prompt {
                            return Ok(prompt.clone());
                        }
                    }
                    return Err(AppError::Ai("別のChatGPT接続を処理中です".into()));
                }
            }
        }
        let binary =
            find_codex_binary().ok_or_else(|| AppError::Ai("Codex CLIが見つかりません".into()))?;
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (ready_tx, ready_rx) = oneshot::channel();
        {
            let mut state = self.state.lock().await;
            state.login = Some(LoginState {
                request_id: request_id.clone(),
                connection: "connecting".into(),
                prompt: None,
                plan_type: None,
                message: Some("ChatGPTの認証画面を準備しています".into()),
            });
            state.cancel_login = Some(cancel_tx);
        }
        let state = Arc::clone(&self.state);
        let request_for_task = request_id.clone();
        let shutdown_rx = self.shutdown.subscribe();
        tokio::spawn(async move {
            login_task(
                state,
                binary,
                request_for_task,
                cancel_rx,
                shutdown_rx,
                ready_tx,
            )
            .await;
        });

        match timeout(Duration::from_secs(12), ready_rx).await {
            Ok(Ok(Ok(prompt))) => Ok(prompt),
            Ok(Ok(Err(message))) => Err(AppError::Ai(message)),
            _ => {
                self.cancel_login(&request_id, None).await?;
                Err(AppError::Ai(
                    "ChatGPT接続の開始が時間切れになりました".into(),
                ))
            }
        }
    }

    pub async fn cancel_login(&self, request_id: &str, login_id: Option<&str>) -> AppResult<()> {
        validate_request_id(request_id)?;
        let mut state = self.state.lock().await;
        let login = state
            .login
            .as_ref()
            .filter(|item| item.request_id == request_id)
            .ok_or(AppError::NotFound)?;
        if let (Some(expected), Some(prompt)) = (login_id, login.prompt.as_ref()) {
            if expected != prompt.login_id {
                return Err(AppError::BadInput("ChatGPT接続IDが一致しません".into()));
            }
        }
        if let Some(cancel) = state.cancel_login.as_ref() {
            let _ = cancel.send(true);
        }
        let login = state.login.as_mut().ok_or(AppError::NotFound)?;
        login.connection = "disconnected".into();
        login.message = Some("ChatGPT接続をキャンセルしました".into());
        Ok(())
    }

    pub async fn disconnect(&self) -> AppResult<SubscriptionStatus> {
        if !release_enabled() {
            return Err(AppError::Ai(
                "ChatGPT接続は隔離受入前のため、このビルドでは無効です".into(),
            ));
        }
        {
            let state = self.state.lock().await;
            if let Some(cancel) = state.cancel_login.as_ref() {
                let _ = cancel.send(true);
            }
        }
        logout_app_server().await?;
        let mut state = self.state.lock().await;
        state.login = None;
        state.cancel_login = None;
        drop(state);
        Ok(self.status().await)
    }

    pub async fn generate(&self, system: &str, input: &str) -> AppResult<SubscriptionGeneration> {
        if !release_enabled() {
            return Err(AppError::Ai(
                "ChatGPT返信生成は隔離受入前のため、このビルドでは無効です".into(),
            ));
        }
        if system.chars().count().saturating_add(input.chars().count()) > PROMPT_LIMIT {
            return Err(AppError::BadInput("AI入力が上限を超えています".into()));
        }
        let _guard = self
            .generation
            .try_lock()
            .map_err(|_| AppError::Ai("別のChatGPT返信を生成中です".into()))?;
        let local = query_local_status().await;
        if local.connection != "connected" {
            return Err(AppError::Ai(
                "ChatGPTに接続されていません。設定から接続してください".into(),
            ));
        }
        if self.shutdown.borrow().to_owned() {
            return Err(AppError::Ai("A-mailを終了しています".into()));
        }
        run_ephemeral_generation(
            find_codex_binary().ok_or_else(|| AppError::Ai("Codex CLIが見つかりません".into()))?,
            system,
            input,
            self.shutdown.subscribe(),
        )
        .await
    }
}

impl Drop for CodexSubscriptionManager {
    fn drop(&mut self) {
        if let Ok(mut state) = self.drop_cancel.lock() {
            if let Some(cancel) = state.take() {
                let _ = cancel.send(true);
            }
        }
    }
}

fn validate_request_id(value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.len() > 100
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(AppError::BadInput("ChatGPT接続要求IDが不正です".into()));
    }
    Ok(())
}

async fn login_task(
    state: Arc<Mutex<RuntimeState>>,
    binary: PathBuf,
    request_id: String,
    mut cancel_rx: watch::Receiver<bool>,
    mut shutdown_rx: watch::Receiver<bool>,
    ready_tx: oneshot::Sender<Result<SubscriptionLoginPrompt, String>>,
) {
    let mut ready_tx = Some(ready_tx);
    let result = async {
        let mut rpc = RpcSession::start(&binary).await?;
        rpc.initialize().await?;
        let login = rpc
            .request(2, "account/login/start", json!({"type":"chatgpt"}))
            .await?;
        let login_id = login
            .get("loginId")
            .and_then(Value::as_str)
            .ok_or_else(|| "ChatGPT接続IDを取得できませんでした".to_string())?
            .to_string();
        let auth_url = login
            .get("authUrl")
            .and_then(Value::as_str)
            .filter(|url| allowed_auth_url(url))
            .ok_or_else(|| "ChatGPT認証URLを検証できませんでした".to_string())?
            .to_string();
        let prompt = SubscriptionLoginPrompt {
            request_id: request_id.clone(),
            login_id: login_id.clone(),
            auth_url,
            expires_at: now() + LOGIN_TIMEOUT.as_secs() as i64,
        };
        {
            let mut current = state.lock().await;
            current.login = Some(LoginState {
                request_id: request_id.clone(),
                connection: "connecting".into(),
                prompt: Some(prompt.clone()),
                plan_type: None,
                message: Some("ブラウザでChatGPTへの接続を完了してください".into()),
            });
        }
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(Ok(prompt));
        }

        let deadline = Instant::now() + LOGIN_TIMEOUT;
        let outcome = loop {
            tokio::select! {
                _ = cancel_rx.changed() => {
                    let _ = rpc.send_request(9, "account/login/cancel", json!({"loginId":login_id})).await;
                    break Err("ChatGPT接続をキャンセルしました".to_string());
                }
                _ = shutdown_rx.changed() => {
                    let _ = rpc.send_request(9, "account/login/cancel", json!({"loginId":login_id})).await;
                    break Err("A-mailの終了によりChatGPT接続を中断しました".to_string());
                }
                _ = tokio::time::sleep_until(deadline) => {
                    let _ = rpc.send_request(9, "account/login/cancel", json!({"loginId":login_id})).await;
                    break Err("ChatGPT接続が時間切れになりました".to_string());
                }
                message = rpc.next_message() => {
                    let message = message?;
                    let method = message.get("method").and_then(Value::as_str);
                    if method == Some("account/updated") {
                        let plan = message.pointer("/params/planType").and_then(Value::as_str).map(str::to_string);
                        if let Some(current) = state.lock().await.login.as_mut() {
                            current.plan_type = plan;
                        }
                    }
                    if method == Some("account/login/completed")
                        && message.pointer("/params/loginId").and_then(Value::as_str) == Some(login_id.as_str())
                    {
                        let success = message.pointer("/params/success").and_then(Value::as_bool).unwrap_or(false);
                        break if success { Ok(()) } else { Err("ChatGPT接続を完了できませんでした".to_string()) };
                    }
                    if message.get("id").is_some() {
                        break Err("ChatGPT接続中に未許可の要求を受信しました".to_string());
                    }
                }
            }
        };
        rpc.terminate().await;
        outcome
    }
    .await;

    let mut current = state.lock().await;
    current.cancel_login = None;
    match result {
        Ok(()) => {
            if let Some(login) = current.login.as_mut() {
                login.connection = "connected".into();
                login.prompt = None;
                login.message = Some("ChatGPTに接続しました".into());
            }
        }
        Err(message) => {
            if let Some(tx) = ready_tx.take() {
                let _ = tx.send(Err(message.clone()));
            }
            if let Some(login) = current.login.as_mut() {
                login.connection = if message.contains("キャンセル") || message.contains("終了")
                {
                    "disconnected".into()
                } else {
                    "error".into()
                };
                login.prompt = None;
                login.message = Some(message);
            }
        }
    }
}

async fn query_local_status() -> SubscriptionStatus {
    let Some(binary) = find_codex_binary() else {
        return SubscriptionStatus {
            runtime_available: false,
            runtime_version: None,
            connection: "unavailable".into(),
            plan_type: None,
            primary: None,
            secondary: None,
            rate_limit_reached: false,
            release_enabled: release_enabled(),
            login: None,
            message: Some("Codex CLIが見つかりません".into()),
        };
    };
    let version = codex_version().await;
    let mut command = Command::new(binary);
    command
        .arg("login")
        .arg("status")
        .env("PATH", crate::util::cli_env::child_path())
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(4), command.output()).await;
    let (connection, message) = match output {
        Ok(Ok(output)) if output.status.success() => {
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if text.contains("Logged in using ChatGPT") {
                (
                    "connected",
                    (!release_enabled()).then(|| {
                        "ChatGPTログインを検出しました。実メール利用は隔離受入前のため停止中です"
                            .to_string()
                    }),
                )
            } else {
                (
                    "disconnected",
                    Some("ChatGPTサブスクリプションには接続されていません".into()),
                )
            }
        }
        _ => (
            "error",
            Some("Codexのログイン状態を確認できませんでした".into()),
        ),
    };
    SubscriptionStatus {
        runtime_available: true,
        runtime_version: version,
        connection: connection.into(),
        plan_type: None,
        primary: None,
        secondary: None,
        rate_limit_reached: false,
        release_enabled: release_enabled(),
        login: None,
        message,
    }
}

async fn query_app_server_status() -> SubscriptionStatus {
    let mut base = query_local_status().await;
    let Some(binary) = find_codex_binary() else {
        return base;
    };
    let result = async {
        let mut rpc = RpcSession::start(&binary).await?;
        rpc.initialize().await?;
        let account = rpc
            .request(2, "account/read", json!({"refreshToken":false}))
            .await?;
        let account_type = account.pointer("/account/type").and_then(Value::as_str);
        if account_type != Some("chatgpt") {
            rpc.terminate().await;
            return Ok::<_, String>((None, None));
        }
        let limits = rpc
            .request(3, "account/rateLimits/read", Value::Null)
            .await
            .ok();
        rpc.terminate().await;
        Ok((Some(account), limits))
    }
    .await;
    match result {
        Ok((Some(account), limits)) => {
            base.connection = "connected".into();
            base.plan_type = account
                .pointer("/account/planType")
                .and_then(Value::as_str)
                .map(str::to_string);
            base.message = None;
            if let Some(limits) = limits {
                let snapshot = limits
                    .pointer("/rateLimitsByLimitId/codex")
                    .or_else(|| limits.get("rateLimits"));
                if let Some(snapshot) = snapshot {
                    base.primary = parse_rate_window(snapshot.get("primary"));
                    base.secondary = parse_rate_window(snapshot.get("secondary"));
                    base.rate_limit_reached = snapshot
                        .get("rateLimitReachedType")
                        .is_some_and(|value| !value.is_null())
                        || base.primary.as_ref().and_then(|value| value.used_percent)
                            >= Some(100.0)
                        || base.secondary.as_ref().and_then(|value| value.used_percent)
                            >= Some(100.0);
                }
            }
        }
        Ok((None, _)) => {
            base.connection = "disconnected".into();
            base.plan_type = None;
            base.message = Some("ChatGPTサブスクリプションには接続されていません".into());
        }
        Err(_) => {
            base.connection = "error".into();
            base.message = Some("ChatGPTの利用状態を確認できませんでした".into());
        }
    }
    base
}

fn parse_rate_window(value: Option<&Value>) -> Option<SubscriptionRateWindow> {
    let value = value.filter(|value| !value.is_null())?;
    Some(SubscriptionRateWindow {
        used_percent: value.get("usedPercent").and_then(Value::as_f64),
        window_duration_mins: value.get("windowDurationMins").and_then(Value::as_i64),
        resets_at: value.get("resetsAt").and_then(Value::as_i64),
    })
}

async fn logout_app_server() -> AppResult<()> {
    let binary =
        find_codex_binary().ok_or_else(|| AppError::Ai("Codex CLIが見つかりません".into()))?;
    let result = async {
        let mut rpc = RpcSession::start(&binary).await?;
        rpc.initialize().await?;
        rpc.request(2, "account/logout", Value::Null).await?;
        rpc.terminate().await;
        Ok::<_, String>(())
    }
    .await;
    result.map_err(AppError::Ai)
}

fn allowed_auth_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    url.host_str().is_some_and(|host| {
        host == "chatgpt.com"
            || host.ends_with(".chatgpt.com")
            || host == "openai.com"
            || host.ends_with(".openai.com")
    })
}

async fn codex_version() -> Option<String> {
    let binary = find_codex_binary()?;
    match timeout(
        Duration::from_secs(3),
        Command::new(binary)
            .env("PATH", crate::util::cli_env::child_path())
            .arg("--version")
            .output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!value.is_empty()).then_some(value)
        }
        _ => None,
    }
}

fn find_codex_binary() -> Option<PathBuf> {
    [
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
    ]
    .into_iter()
    .find_map(|path| {
        let meta = fs::metadata(&path).ok()?;
        meta.is_file()
            .then(|| fs::canonicalize(&path).ok())
            .flatten()
    })
}

struct RpcSession {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    pid: Option<u32>,
}

impl RpcSession {
    async fn start(binary: &Path) -> Result<Self, String> {
        let mut command = Command::new(binary);
        command
            .arg("app-server")
            .arg("--stdio")
            .arg("--strict-config")
            .arg("-c")
            .arg("mcp_servers={}")
            .arg("-c")
            .arg("features.apps=false")
            .arg("-c")
            .arg("features.hooks=false")
            .env("PATH", crate::util::cli_env::child_path())
            .env_remove("OPENAI_API_KEY")
            .env_remove("OPENAI_BASE_URL")
            .env_remove("OPENAI_API_BASE")
            .env_remove("CODEX_API_KEY")
            .env_remove("CODEX_API_BASE_URL")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }
        let mut child = command
            .spawn()
            .map_err(|_| "Codex App Serverを起動できませんでした".to_string())?;
        let pid = child.id();
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Codex App Serverへ入力できません".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Codex App Serverの応答を読めません".to_string())?;
        Ok(Self {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            pid,
        })
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.request(
            1,
            "initialize",
            json!({
                "clientInfo":{"name":"a_mail","title":"A-mail","version":"0.1.0"},
                "capabilities":{"experimentalApi":false}
            }),
        )
        .await?;
        self.send_value(&json!({"method":"initialized","params":{}}))
            .await
    }

    async fn request(&mut self, id: i64, method: &str, params: Value) -> Result<Value, String> {
        self.send_request(id, method, params).await?;
        let deadline = Instant::now() + RPC_TIMEOUT;
        loop {
            let message = timeout_at(deadline, self.next_message()).await??;
            if message.get("id").and_then(Value::as_i64) != Some(id) {
                if message.get("id").is_some() {
                    return Err("Codex App Serverから未許可の要求を受信しました".into());
                }
                continue;
            }
            if message.get("error").is_some() {
                return Err(format!(
                    "Codex App Serverの処理に失敗しました（request {id}）"
                ));
            }
            return message
                .get("result")
                .cloned()
                .ok_or_else(|| "Codex App Serverの応答が不正です".into());
        }
    }

    async fn send_request(&mut self, id: i64, method: &str, params: Value) -> Result<(), String> {
        self.send_value(&json!({"method":method,"id":id,"params":params}))
            .await
    }

    async fn send_value(&mut self, value: &Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(value)
            .map_err(|_| "Codex App Server要求を作成できません".to_string())?;
        bytes.push(b'\n');
        self.stdin
            .write_all(&bytes)
            .await
            .map_err(|_| "Codex App Serverへ要求を送信できません".to_string())?;
        self.stdin
            .flush()
            .await
            .map_err(|_| "Codex App Serverへ要求を送信できません".to_string())
    }

    async fn next_message(&mut self) -> Result<Value, String> {
        let line = self
            .lines
            .next_line()
            .await
            .map_err(|_| "Codex App Serverの応答を読めません".to_string())?
            .ok_or_else(|| "Codex App Serverが終了しました".to_string())?;
        if line.len() > OUTPUT_LINE_LIMIT {
            return Err("Codex App Serverの応答が上限を超えました".into());
        }
        serde_json::from_str(&line).map_err(|_| "Codex App Serverの応答形式が不正です".to_string())
    }

    async fn terminate(mut self) {
        let _ = self.stdin.shutdown().await;
        if timeout(Duration::from_secs(2), self.child.wait())
            .await
            .is_ok()
        {
            return;
        }
        terminate_process_group(self.pid);
        let _ = self.child.wait().await;
    }
}

impl Drop for RpcSession {
    fn drop(&mut self) {
        // Error paths before the graceful terminator still kill the complete
        // process group; `kill_on_drop` alone only covers the direct child.
        terminate_process_group(self.pid);
        let _ = self.child.start_kill();
    }
}

async fn timeout_at<T>(
    deadline: Instant,
    future: impl std::future::Future<Output = T>,
) -> Result<T, String> {
    tokio::time::timeout_at(deadline, future)
        .await
        .map_err(|_| "Codex App Serverの応答が時間切れになりました".to_string())
}

struct PrivateWorkspace {
    path: PathBuf,
}

impl PrivateWorkspace {
    fn create() -> AppResult<Self> {
        let path = std::env::temp_dir().join(format!("a-mail-reply-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { path })
    }
}

impl Drop for PrivateWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn reply_schema() -> Value {
    json!({
        "type":"object",
        "properties":{
            "generated_text":{"type":"string","minLength":1,"maxLength":8000},
            "questions":{"type":"array","minItems":1,"maxItems":20,"items":{
                "type":"object",
                "properties":{
                    "id":{"type":"string","minLength":1,"maxLength":120},
                    "text":{"type":"string","minLength":1,"maxLength":1000},
                    "status":{"type":"string","enum":["answered","needs_input"]},
                    "source_ids":{"type":"array","maxItems":20,"items":{"type":"string","maxLength":160}}
                },
                "required":["id","text","status","source_ids"],
                "additionalProperties":false
            }},
            "review_reasons":{"type":"array","maxItems":20,"items":{"type":"string","maxLength":1000}}
        },
        "required":["generated_text","questions","review_reasons"],
        "additionalProperties":false
    })
}

fn build_generation_command(binary: &Path, cwd: &Path, schema: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        // Codex の実体は `env node` 起動の JS。GUI 起動の PATH では node を見つけられない。
        .env("PATH", crate::util::cli_env::child_path())
        .arg("--sandbox")
        .arg("read-only")
        .arg("--ask-for-approval")
        .arg("never")
        .arg("exec")
        .arg("--strict-config")
        .arg("--ephemeral")
        .arg("--ignore-user-config")
        .arg("--ignore-rules")
        .arg("--skip-git-repo-check")
        .arg("--color")
        .arg("never")
        .arg("--json")
        .arg("--model")
        .arg(SUBSCRIPTION_MODEL)
        .arg("--output-schema")
        .arg(schema)
        .arg("-C")
        .arg(cwd)
        .arg("-c")
        .arg("model_reasoning_effort=\"low\"")
        .arg("-c")
        .arg("project_doc_max_bytes=0")
        .arg("-c")
        .arg("web_search=\"disabled\"")
        .arg("-c")
        .arg("features.hooks=false")
        .arg("-c")
        .arg("features.multi_agent=false")
        .arg("-c")
        .arg("features.apps=false")
        .arg("-c")
        .arg("mcp_servers={}")
        .arg("-c")
        .arg("shell_environment_policy.inherit=\"core\"")
        .arg("-")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("OPENAI_API_BASE")
        .env_remove("CODEX_API_KEY")
        .env_remove("CODEX_API_BASE_URL")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("GEMINI_API_KEY");
    command
}

#[derive(Default)]
struct GenerationEvents {
    final_text: Option<String>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
}

impl GenerationEvents {
    fn ingest(&mut self, line: &str) -> Result<(), &'static str> {
        if line.len() > OUTPUT_LINE_LIMIT {
            return Err("output_limit");
        }
        let event: Value = serde_json::from_str(line).map_err(|_| "invalid_jsonl")?;
        match event.get("type").and_then(Value::as_str) {
            Some("item.started") | Some("item.updated") | Some("item.completed") => {
                let kind = event.pointer("/item/type").and_then(Value::as_str);
                match kind {
                    Some("agent_message") => {
                        if event.get("type").and_then(Value::as_str) == Some("item.completed") {
                            if let Some(text) = event.pointer("/item/text").and_then(Value::as_str)
                            {
                                if text.len() > FINAL_OUTPUT_LIMIT {
                                    return Err("output_limit");
                                }
                                self.final_text = Some(text.trim().to_string());
                            }
                        }
                    }
                    Some("reasoning") => {}
                    Some(_) | None => return Err("tool_or_unknown_item"),
                }
            }
            Some("turn.completed") => {
                self.input_tokens = event.pointer("/usage/input_tokens").and_then(Value::as_i64);
                self.output_tokens = event
                    .pointer("/usage/output_tokens")
                    .and_then(Value::as_i64);
            }
            Some("thread.started") | Some("turn.started") => {}
            Some("error") | Some("turn.failed") => return Err("provider_error"),
            Some(_) | None => return Err("unknown_event"),
        }
        Ok(())
    }
}

async fn run_ephemeral_generation(
    binary: PathBuf,
    system: &str,
    input: &str,
    mut shutdown_rx: watch::Receiver<bool>,
) -> AppResult<SubscriptionGeneration> {
    let workspace = PrivateWorkspace::create()?;
    let schema_path = workspace.path.join("reply-output.schema.json");
    fs::write(
        &schema_path,
        serde_json::to_vec(&reply_schema())
            .map_err(|_| AppError::Ai("AI出力schemaを作成できません".into()))?,
    )?;
    let mut command = build_generation_command(&binary, &workspace.path, &schema_path);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|_| AppError::Ai("Codex返信生成を起動できませんでした".into()))?;
    let pid = child.id();
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Ai("Codex返信生成へ入力できません".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Ai("Codex返信生成の結果を読めません".into()))?;
    let stderr = child.stderr.take();
    let stderr_task = stderr.map(|stream| tokio::spawn(read_bounded(stream, STDERR_LIMIT)));
    let prompt = format!(
        "文章生成だけを行ってください。ファイル、シェル、Web、MCP、アプリ、外部ツールを一切使用しないでください。\n{system}\n入力JSON:\n{input}"
    );
    stdin
        .write_all(prompt.as_bytes())
        .await
        .map_err(|_| AppError::Ai("Codex返信生成へ入力できません".into()))?;
    stdin
        .shutdown()
        .await
        .map_err(|_| AppError::Ai("Codex返信生成へ入力できません".into()))?;
    drop(stdin);

    let mut lines = BufReader::new(stdout).lines();
    let mut events = GenerationEvents::default();
    let deadline = Instant::now() + GENERATION_TIMEOUT;
    let exit_status = loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                terminate_process_group(pid);
                let _ = child.wait().await;
                return Err(AppError::Ai("A-mailの終了により返信生成を中断しました".into()));
            }
            _ = tokio::time::sleep_until(deadline) => {
                terminate_process_group(pid);
                let _ = child.wait().await;
                return Err(AppError::Ai("ChatGPT返信生成が時間切れになりました".into()));
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if events.ingest(&line).is_err() {
                            terminate_process_group(pid);
                            let _ = child.wait().await;
                            return Err(AppError::Ai("Codexが許可されていない処理を要求したため、候補を破棄しました".into()));
                        }
                    }
                    Ok(None) => break child.wait().await,
                    Err(_) => {
                        terminate_process_group(pid);
                        let _ = child.wait().await;
                        return Err(AppError::Ai("Codex返信生成の結果を読めません".into()));
                    }
                }
            }
            status = child.wait() => break status,
        }
    };
    while let Ok(Ok(Some(line))) = timeout(Duration::from_millis(50), lines.next_line()).await {
        if events.ingest(&line).is_err() {
            return Err(AppError::Ai(
                "Codexが許可されていない処理を要求したため、候補を破棄しました".into(),
            ));
        }
    }
    if let Some(task) = stderr_task {
        match timeout(Duration::from_secs(1), task).await {
            Ok(Ok(Ok(_))) => {}
            _ => {
                return Err(AppError::Ai(
                    "Codex返信生成の診断出力が上限を超えました".into(),
                ))
            }
        }
    }
    if !exit_status.is_ok_and(|status| status.success()) {
        return Err(AppError::Ai("ChatGPT返信生成に失敗しました".into()));
    }
    let text = events
        .final_text
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Ai("ChatGPT返信候補を取得できませんでした".into()))?;
    Ok(SubscriptionGeneration {
        text,
        provider: "codex_subscription".into(),
        model: SUBSCRIPTION_MODEL.into(),
        input_tokens: events.input_tokens,
        output_tokens: events.output_tokens,
    })
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
) -> Result<Vec<u8>, ()> {
    use tokio::io::AsyncReadExt;
    let mut kept = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0u8; 8192];
    loop {
        let count = reader.read(&mut buffer).await.map_err(|_| ())?;
        if count == 0 {
            return Ok(kept);
        }
        if kept.len().saturating_add(count) > limit {
            return Err(());
        }
        kept.extend_from_slice(&buffer[..count]);
    }
}

#[cfg(unix)]
fn terminate_process_group(pid: Option<u32>) {
    if let Some(pid) = pid {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_pid: Option<u32>) {}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_command_is_ephemeral_and_has_no_external_tools() {
        let command = build_generation_command(
            Path::new("/usr/local/bin/codex"),
            Path::new("/tmp/a-mail-empty"),
            Path::new("/tmp/a-mail-empty/schema.json"),
        );
        let debug = format!("{command:?}");
        for expected in [
            "read-only",
            "never",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "web_search=\\\"disabled\\\"",
            "features.hooks=false",
            "features.apps=false",
            "mcp_servers={}",
            "schema.json",
        ] {
            assert!(debug.contains(expected), "missing {expected}: {debug}");
        }
        assert!(!debug.contains("workspace-write"));
        assert!(!debug.contains("danger-full-access"));
        for secret in [
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "CODEX_API_KEY",
            "CODEX_API_BASE_URL",
            "ANTHROPIC_API_KEY",
            "GEMINI_API_KEY",
        ] {
            assert!(debug.contains(&format!("-u {secret}")));
        }
    }

    #[cfg(not(feature = "codex-subscription-reply"))]
    #[tokio::test]
    async fn ordinary_build_refuses_to_start_shared_login() {
        let manager = CodexSubscriptionManager::new();
        assert!(manager
            .start_login("fictional-request".into())
            .await
            .is_err());
    }

    #[test]
    fn output_parser_accepts_only_agent_text_and_usage() {
        let mut events = GenerationEvents::default();
        events
            .ingest(r#"{"type":"thread.started","thread_id":"fictional"}"#)
            .unwrap();
        events
            .ingest(r#"{"type":"item.completed","item":{"type":"agent_message","text":"{\"generated_text\":\"架空返信\",\"questions\":[],\"review_reasons\":[]}"}}"#)
            .unwrap();
        events
            .ingest(r#"{"type":"turn.completed","usage":{"input_tokens":321,"output_tokens":45}}"#)
            .unwrap();
        assert!(events.final_text.as_deref().unwrap().contains("架空返信"));
        assert_eq!(events.input_tokens, Some(321));
        assert_eq!(events.output_tokens, Some(45));
    }

    #[test]
    fn output_parser_rejects_every_tool_or_file_event() {
        for kind in [
            "command_execution",
            "file_change",
            "mcp_tool_call",
            "web_search",
            "dynamic_tool_call",
        ] {
            let mut events = GenerationEvents::default();
            let line = json!({"type":"item.started","item":{"type":kind}}).to_string();
            assert_eq!(events.ingest(&line), Err("tool_or_unknown_item"));
        }
    }

    #[test]
    fn auth_url_is_allowlisted() {
        assert!(allowed_auth_url("https://chatgpt.com/auth/callback?a=1"));
        assert!(allowed_auth_url("https://auth.openai.com/codex"));
        assert!(!allowed_auth_url("http://chatgpt.com/auth"));
        assert!(!allowed_auth_url("https://chatgpt.com.evil.example/auth"));
        assert!(!allowed_auth_url("https://user:pass@chatgpt.com/auth"));
    }

    #[test]
    fn rate_limit_parser_handles_missing_values_without_calling_them_zero() {
        let window = parse_rate_window(Some(&json!({
            "usedPercent":25.5,
            "windowDurationMins":300,
            "resetsAt":12345
        })))
        .unwrap();
        assert_eq!(window.used_percent, Some(25.5));
        assert_eq!(parse_rate_window(Some(&Value::Null)), None);
    }
}
