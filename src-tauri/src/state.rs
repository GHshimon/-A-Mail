//! アプリ全体で共有する状態。`tauri::State` で各コマンドへ注入する。

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

use crate::cli_jobs::CliJobManager;
use crate::codex_subscription::CodexSubscriptionManager;
use crate::error::{AppError, AppResult};
use crate::store::db;

pub struct AppState {
    /// 単一 writer コネクション。書き込みを直列化(WAL で読みは並行)。
    pub db: Mutex<Connection>,
    /// Gemini など外部 HTTP 用クライアント(M3 で使用)。
    pub http: reqwest::Client,
    /// Codex CLI は1ジョブだけを実行し、状態とキャンセルをここで直列化する。
    pub cli_jobs: CliJobManager,
    /// ChatGPT managed login と、隔離した一時返信生成をアプリ寿命内だけ管理する。
    pub codex_subscription: CodexSubscriptionManager,
}

impl AppState {
    pub fn new(app: &tauri::AppHandle) -> AppResult<Self> {
        let data_dir = app.path().app_data_dir().map_err(AppError::from)?;
        let conn = db::open(&data_dir)?;
        // A crashed provider call has an unknown outcome. Never retry it on
        // restart; leave the draft visible for human inspection.
        let account_ids = {
            let mut statement = conn.prepare("SELECT id FROM accounts")?;
            let ids = statement
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        for account_id in account_ids {
            crate::assistant_automation::recover_inflight(&conn, account_id)?;
        }
        Ok(Self {
            db: Mutex::new(conn),
            http: reqwest::Client::new(),
            cli_jobs: CliJobManager::new(),
            codex_subscription: CodexSubscriptionManager::new(),
        })
    }

    /// DB ロック取得のヘルパ(poison を AppError に変換)。
    pub fn db(&self) -> AppResult<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| AppError::Db("DB ロックの取得に失敗しました".into()))
    }
}
