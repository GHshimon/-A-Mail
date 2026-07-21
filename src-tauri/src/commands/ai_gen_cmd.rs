//! AI 生成コマンド A/B/C(Gemini)。M3(§7)。
//!
//! プライバシー: いずれも `ai_enabled` が真かつ Gemini キー登録済みのときだけ実行。
//! A(本文予測)は `ai_send_body` も必須。無効時は AppError::Ai("disabled") を返し、
//! フロントは静かに補助を切る(エラートーストは出さない)。

use tauri::State;

use crate::ai::{gemini, prompts};
use crate::crypto::keychain;
use crate::error::{AppError, AppResult};
use crate::search;
use crate::state::AppState;
use crate::store::{
    self,
    models::{CompleteReq, ContextPoint, ContextReq, ContextResult, ReplyReq},
};

/// AI が使える状態(有効 + キーあり)か確認し、キーとモデルを返す。
fn ensure_ai(state: &AppState) -> AppResult<(String, String)> {
    let model = {
        let conn = state.db()?;
        let settings = store::get_settings(&conn)?;
        if !settings.ai_enabled {
            return Err(AppError::Ai("disabled".into()));
        }
        settings.model
    };
    let key = keychain::gemini_key()?
        .ok_or_else(|| AppError::Ai("Gemini API キーが未登録です".into()))?;
    Ok((key, model))
}

/// epoch 秒 → "YYYY/MM/DD"(chrono 非依存の簡易変換)。
fn ymd(secs: i64) -> String {
    // civil_from_days(Howard Hinnant)。
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}/{m:02}/{d:02}")
}

/// A: 予測入力。カーソル直前までの本文から続きを 1 つ返す。
#[tauri::command]
pub async fn ai_complete(state: State<'_, AppState>, req: CompleteReq) -> AppResult<String> {
    // 本文送信の可否も確認(A は本文を送る)。
    {
        let conn = state.db()?;
        if !store::get_settings(&conn)?.ai_send_body {
            return Err(AppError::Ai("disabled".into()));
        }
    }
    let (key, model) = ensure_ai(&state)?;
    // 続きを書く材料が薄すぎるときは呼ばない(トークン節約・体感)。
    if req.body_prefix.trim().chars().count() < 4 {
        return Ok(String::new());
    }

    let system = prompts::COMPLETE_SYSTEM;
    let input = prompts::complete_input(&req.subject, &req.to, &req.body_prefix);
    let text =
        gemini::generate_content(&state.http, &key, &model, system, &input, &gemini::GenConfig::completion())
            .await?;
    Ok(text.trim_end().to_string())
}

/// B: 関連情報サイドバー。FTS5 で関連過去メールを集め、Gemini が要点化する。
#[tauri::command]
pub async fn ai_context_sidebar(
    state: State<'_, AppState>,
    req: ContextReq,
) -> AppResult<ContextResult> {
    let (key, model) = ensure_ai(&state)?;

    // 1) 関連過去メールを FTS5 で取得(クエリは件名優先、無ければ本文先頭)。
    let query = if !req.subject.trim().is_empty() {
        req.subject.clone()
    } else {
        req.body.chars().take(60).collect()
    };
    let related_count = {
        let conn = state.db()?;
        store::get_settings(&conn)?.ai_scope_related_count.clamp(1, 20) as u32
    };

    let (related, related_input) = {
        let conn = state.db()?;
        let headers = search::search_messages(&conn, req.account_id, &query, related_count)?;
        let mut tuples = Vec::new();
        for h in &headers {
            let body = store::message_body_text(&conn, h.id)?.unwrap_or_else(|| h.snippet.clone());
            tuples.push((ymd(h.date), h.from.clone(), h.subject.clone(), body));
        }
        (headers, tuples)
    };

    // 2) 要点化。関連が無くても作成中メールから拾える経緯があるので呼ぶ。
    let system = prompts::CONTEXT_SYSTEM;
    let input = prompts::context_input(&req.subject, &req.to, &req.body, &related_input);
    let text =
        gemini::generate_content(&state.http, &key, &model, system, &input, &gemini::GenConfig::generation())
            .await?;

    let points = parse_points(&text);
    Ok(ContextResult { points, related })
}

/// C: 返信ドラフト生成。元メールを入力に返信本文を生成する。
#[tauri::command]
pub async fn ai_generate_reply(state: State<'_, AppState>, req: ReplyReq) -> AppResult<String> {
    let (key, model) = ensure_ai(&state)?;

    // 元メールの本文(未取得なら snippet 併用)。
    let full = {
        let conn = state.db()?;
        store::get_message_full(&conn, req.source_message_id)?
    };
    let body = full
        .body_text
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| full.header.snippet.clone());

    let tone = req.tone.as_deref().unwrap_or("standard");
    let length = req.length.as_deref().unwrap_or("mid");
    let system = prompts::reply_system(tone, length);
    let input = prompts::reply_input(&full.header.from, &full.header.subject, &body);
    let text =
        gemini::generate_content(&state.http, &key, &model, &system, &input, &gemini::GenConfig::generation())
            .await?;
    Ok(text.trim().to_string())
}

/// Gemini の JSON 応答を ContextPoint 列へ。コードフェンス等が混じっても頑健に扱う。
fn parse_points(text: &str) -> Vec<ContextPoint> {
    let trimmed = strip_code_fence(text.trim());
    // まずは配列としてパース。
    if let Ok(v) = serde_json::from_str::<Vec<ContextPoint>>(trimmed) {
        return v.into_iter().filter(|p| !p.text.trim().is_empty()).collect();
    }
    // 応答内の最初の [ ... ] を抜き出して再挑戦。
    if let (Some(a), Some(b)) = (trimmed.find('['), trimmed.rfind(']')) {
        if a < b {
            if let Ok(v) = serde_json::from_str::<Vec<ContextPoint>>(&trimmed[a..=b]) {
                return v.into_iter().filter(|p| !p.text.trim().is_empty()).collect();
            }
        }
    }
    // JSON でなければ、全文を 1 つの経緯として扱う(空でなければ)。
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![ContextPoint {
            category: "history".into(),
            text: trimmed.chars().take(300).collect(),
        }]
    }
}

/// ```json ... ``` などのコードフェンスを剥がす。
fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```") {
        // 先頭行(```json 等)を落とし、末尾の ``` も落とす。
        let rest = rest.splitn(2, '\n').nth(1).unwrap_or(rest);
        rest.trim_end().strip_suffix("```").unwrap_or(rest).trim()
    } else {
        s
    }
}
