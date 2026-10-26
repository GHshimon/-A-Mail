//! 読み取り専用AIプロジェクトのIPC境界。

use tauri::State;

use crate::ai_projects;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::models::{AiProject, ProjectFileHit};

#[tauri::command]
pub fn list_ai_projects(state: State<'_, AppState>) -> AppResult<Vec<AiProject>> {
    let conn = state.db()?;
    ai_projects::list(&conn)
}

#[tauri::command]
pub async fn add_ai_project(
    state: State<'_, AppState>,
    name: String,
    root_path: String,
) -> AppResult<AiProject> {
    // オフラインの NAS では metadata/canonicalize が長く待つことがあるため、
    // ファイルシステム検証は非同期ランタイムと DB ロックの外で行う。
    let canonical =
        tokio::task::spawn_blocking(move || ai_projects::validate_project_root(&root_path))
            .await
            .map_err(|_| AppError::Db("プロジェクト検証タスクが終了しました".into()))??;
    let conn = state.db()?;
    ai_projects::add(&conn, &name, &canonical)
}

#[tauri::command]
pub fn remove_ai_project(state: State<'_, AppState>, project_id: i64) -> AppResult<()> {
    let conn = state.db()?;
    ai_projects::remove(&conn, project_id)
}

#[tauri::command]
pub async fn search_ai_project_files(
    state: State<'_, AppState>,
    project_ids: Vec<i64>,
    query: String,
    limit: u32,
) -> AppResult<Vec<ProjectFileHit>> {
    let projects = {
        let conn = state.db()?;
        ai_projects::selected(&conn, &project_ids)?
    };
    tokio::task::spawn_blocking(move || ai_projects::search_projects(&projects, &query, limit))
        .await
        .map_err(|_| AppError::Db("プロジェクト検索タスクが終了しました".into()))?
}
