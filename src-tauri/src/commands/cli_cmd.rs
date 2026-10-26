//! Codex CLI のIPC境界。フロントから任意パスや任意プロンプトは受け取らない。

use std::path::PathBuf;

use serde::Deserialize;
use tauri::State;

use crate::ai_projects;
use crate::cli_jobs::{self, CliJob, CliStatus, CliTaskKind};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StartCliJobReq {
    pub project_id: i64,
    pub task_kind: CliTaskKind,
}

#[tauri::command]
pub async fn get_cli_status() -> AppResult<CliStatus> {
    Ok(cli_jobs::get_status().await)
}

#[tauri::command]
pub async fn start_cli_job(state: State<'_, AppState>, req: StartCliJobReq) -> AppResult<CliJob> {
    // IPC由来のpath/nameは使わず、登録済みかつ有効な単一IDだけをDBから解決する。
    let project = {
        let conn = state.db()?;
        ai_projects::selected(&conn, &[req.project_id])?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::BadInput("プロジェクトが登録されていません".into()))?
    };

    // 登録後にルートがsymlink化・移動された場合は起動直前に拒否する。
    let registered_root = PathBuf::from(&project.root_path);
    let validation_path = project.root_path.clone();
    let current_root =
        tokio::task::spawn_blocking(move || ai_projects::validate_project_root(&validation_path))
            .await
            .map_err(|_| AppError::Ai("プロジェクト検証を完了できませんでした".into()))??;
    if current_root != registered_root {
        return Err(AppError::BadInput(
            "登録後にプロジェクトの場所が変更されています".into(),
        ));
    }

    state
        .cli_jobs
        .start(project.id, project.name, current_root, req.task_kind)
        .await
        .map_err(AppError::Ai)
}

#[tauri::command]
pub async fn get_cli_job(state: State<'_, AppState>, job_id: String) -> AppResult<CliJob> {
    if job_id.trim().is_empty() || job_id.len() > 100 {
        return Err(AppError::BadInput("CLIジョブIDが不正です".into()));
    }
    match state.cli_jobs.current().await {
        Some(job) if job.id == job_id => Ok(job),
        _ => Err(AppError::NotFound),
    }
}

#[tauri::command]
pub async fn cancel_cli_job(state: State<'_, AppState>, job_id: String) -> AppResult<CliJob> {
    if job_id.trim().is_empty() || job_id.len() > 100 {
        return Err(AppError::BadInput("CLIジョブIDが不正です".into()));
    }
    state.cli_jobs.cancel(&job_id).await.map_err(AppError::Ai)
}
