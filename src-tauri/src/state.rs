//! アプリ全体で共有する状態。`tauri::State` で各コマンドへ注入する。

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

use crate::error::{AppError, AppResult};
use crate::store::db;

pub struct AppState {
    /// 単一 writer コネクション。書き込みを直列化(WAL で読みは並行)。
    pub db: Mutex<Connection>,
    /// Gemini など外部 HTTP 用クライアント(M3 で使用)。
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(app: &tauri::AppHandle) -> AppResult<Self> {
        let data_dir = app.path().app_data_dir().map_err(AppError::from)?;
        let conn = db::open(&data_dir)?;
        Ok(Self {
            db: Mutex::new(conn),
            http: reqwest::Client::new(),
        })
    }

    /// DB ロック取得のヘルパ(poison を AppError に変換)。
    pub fn db(&self) -> AppResult<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| AppError::Db("DB ロックの取得に失敗しました".into()))
    }
}
