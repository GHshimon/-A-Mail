//! IPC adapter for the P0 assistant. Every public command deliberately takes
//! exactly one `{ req: ... }` argument so callers cannot accidentally rely on
//! positional/legacy fields.
use crate::store::models::*;
use crate::{
    ai, ai_projects, assistant_automation, assistant_quality, codex_subscription, crypto::keychain,
    error::AppError, store, style_profile,
};
use crate::{assistant, error::AppResult, state::AppState};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use sha2::Digest;
use std::time::Duration;
use tauri::State;

#[derive(Deserialize)]
pub struct AccountReq {
    pub account_id: i64,
}
#[derive(Deserialize)]
pub struct WorkspaceCreateReq {
    pub account_id: i64,
    pub customer_name: String,
    pub name: String,
}
#[derive(Deserialize)]
pub struct ContextReq {
    pub source_message_id: i64,
}
#[derive(Deserialize)]
pub struct BindReq {
    pub source_message_id: i64,
    pub workspace_id: Option<i64>,
}
#[derive(Deserialize)]
pub struct DraftListReq {
    pub account_id: i64,
    #[serde(default)]
    pub include_discarded: bool,
}
#[derive(Deserialize)]
pub struct SaveReq {
    pub draft: DraftInput,
    pub session_id: String,
    pub expected_revision: Option<i64>,
    pub source_message_id: Option<i64>,
    pub workspace_id: Option<i64>,
    #[serde(default)]
    pub intent: String,
    pub reply_run_id: Option<String>,
}
#[derive(Deserialize)]
pub struct RevisionReq {
    pub draft_id: i64,
    pub expected_revision: i64,
}
#[derive(Deserialize)]
pub struct DraftIdReq {
    pub draft_id: i64,
}
#[derive(Deserialize)]
pub struct GenerateReq {
    pub draft_id: i64,
    pub expected_revision: i64,
    #[serde(default)]
    pub force: bool,
}
#[derive(Deserialize)]
pub struct ApplyReq {
    pub draft_id: i64,
    pub run_id: String,
    pub expected_revision: i64,
}
#[derive(Deserialize)]
pub struct SendReq {
    pub token: String,
    pub confirm_warnings: bool,
}
#[derive(Deserialize)]
pub struct SettingsReq {
    pub account_id: i64,
    pub auto_draft_enabled: bool,
    #[serde(default)]
    pub subscription_reply_enabled: bool,
    /// Accepted for compatibility with the former settings UI, but ignored.
    #[serde(default)]
    pub learning_enabled: bool,
    pub daily_generation_limit: i64,
}
#[derive(Deserialize)]
pub struct ScopeReq {
    pub account_id: i64,
    pub project_id: i64,
    pub workspace_id: Option<i64>,
    pub scope: String,
    pub confirmed: bool,
}
#[derive(Deserialize)]
pub struct RuleReq {
    pub account_id: i64,
    pub id: Option<i64>,
    pub workspace_id: Option<i64>,
    pub from_phrase: String,
    pub to_phrase: String,
    pub status: String,
}
#[derive(Deserialize)]
pub struct DeleteRuleReq {
    pub account_id: i64,
    pub id: i64,
}
#[derive(Deserialize)]
pub struct StyleLearningPreviewReq {
    pub account_id: i64,
    pub requested_count: i64,
}
#[derive(Deserialize)]
pub struct StyleLearningStartReq {
    pub account_id: i64,
    pub requested_count: i64,
    pub consent_version: String,
    pub preview_hash: String,
}
#[derive(Deserialize)]
pub struct StyleProfileIdReq {
    pub account_id: i64,
    pub profile_id: i64,
}
#[derive(Deserialize)]
pub struct StyleProfileUpdateReq {
    pub account_id: i64,
    pub profile_id: i64,
    pub expected_updated_at: i64,
    pub content: StyleProfileContent,
}
#[derive(Deserialize)]
pub struct StyleProfileJobReq {
    pub account_id: i64,
    pub job_id: String,
}
#[derive(Deserialize)]
pub struct StyleProfilePreviewReq {
    pub account_id: i64,
    pub profile_id: i64,
    pub scenario_id: String,
}
#[derive(serde::Serialize)]
pub struct GenerateOut {
    pub draft: DraftRecord,
    pub run: ReplyRun,
    pub applied: bool,
}
#[derive(serde::Serialize)]
pub struct PrepareOut {
    pub token: String,
    pub review_reasons: Vec<String>,
    pub needs_confirmation: bool,
}
#[derive(serde::Serialize)]
pub struct SendOut {
    pub draft_id: i64,
    pub state: String,
    pub message_id: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(serde::Serialize)]
pub struct QueueOut {
    pub processed: i64,
    pub pending: i64,
    pub reason: Option<String>,
}

#[tauri::command]
pub fn assistant_list_workspaces(
    state: State<'_, AppState>,
    req: AccountReq,
) -> AppResult<Vec<Workspace>> {
    assistant::list_workspaces(&*state.db()?, req.account_id)
}
#[tauri::command]
pub fn assistant_create_workspace(
    state: State<'_, AppState>,
    req: WorkspaceCreateReq,
) -> AppResult<Workspace> {
    assistant::create_workspace(&*state.db()?, req.account_id, &req.customer_name, &req.name)
}
#[tauri::command]
pub fn assistant_get_context(
    state: State<'_, AppState>,
    req: ContextReq,
) -> AppResult<ReplyContext> {
    assistant::get_context(&*state.db()?, req.source_message_id)
}
#[tauri::command]
pub fn assistant_bind_context(state: State<'_, AppState>, req: BindReq) -> AppResult<ReplyContext> {
    assistant::bind_context(&*state.db()?, req.source_message_id, req.workspace_id)
}
#[tauri::command]
pub fn assistant_list_project_scopes(
    state: State<'_, AppState>,
    req: AccountReq,
) -> AppResult<Vec<ProjectScope>> {
    assistant::scopes(&*state.db()?, req.account_id)
}
#[tauri::command]
pub fn assistant_set_project_scope(
    state: State<'_, AppState>,
    req: ScopeReq,
) -> AppResult<ProjectScope> {
    assistant::set_scope(
        &*state.db()?,
        &ProjectScope {
            account_id: req.account_id,
            project_id: req.project_id,
            workspace_id: req.workspace_id,
            scope: req.scope,
            confirmed: req.confirmed,
        },
    )
}
#[tauri::command]
pub fn assistant_list_drafts(
    state: State<'_, AppState>,
    req: DraftListReq,
) -> AppResult<Vec<DraftRecord>> {
    assistant::list_drafts(&*state.db()?, req.account_id, req.include_discarded)
}
#[tauri::command]
pub fn assistant_get_draft(state: State<'_, AppState>, req: DraftIdReq) -> AppResult<DraftRecord> {
    let conn = state.db()?;
    let revision = assistant::draft_revision(&conn, req.draft_id)?;
    assistant::revision(&conn, req.draft_id, revision)
}
#[tauri::command]
pub fn assistant_save_draft(state: State<'_, AppState>, req: SaveReq) -> AppResult<DraftRecord> {
    assistant::save_draft(
        &*state.db()?,
        &req.draft,
        &req.session_id,
        req.expected_revision,
        req.source_message_id,
        req.workspace_id,
        &req.intent,
        req.reply_run_id,
    )
}
#[tauri::command]
pub fn assistant_discard_draft(
    state: State<'_, AppState>,
    req: RevisionReq,
) -> AppResult<DraftRecord> {
    assistant::transition(
        &*state.db()?,
        req.draft_id,
        req.expected_revision,
        "discarded",
    )
}
#[tauri::command]
pub fn assistant_restore_draft(
    state: State<'_, AppState>,
    req: RevisionReq,
) -> AppResult<DraftRecord> {
    assistant::transition(
        &*state.db()?,
        req.draft_id,
        req.expected_revision,
        "editing",
    )
}
async fn generate_reply_impl(state: &AppState, req: GenerateReq) -> AppResult<GenerateOut> {
    let (draft_before, source_text, source_card, projects, provider, key, model) = {
        let conn = state.db()?;
        let draft = assistant::get_draft(&conn, req.draft_id, req.expected_revision)?;
        let source_id = draft
            .source_message_id
            .ok_or_else(|| AppError::BadInput("新規メールはP0ではAI返信生成できません".into()))?;
        let (subject, body): (String, Option<String>) = conn.query_row(
            "SELECT subject,body_text FROM messages WHERE id=? AND account_id=?",
            rusqlite::params![source_id, draft.draft.account_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let body = body.unwrap_or_default();
        let msgid: Option<String> = conn.query_row(
            "SELECT message_id FROM messages WHERE id=?",
            [source_id],
            |r| r.get(0),
        )?;
        let irt: Option<String> = conn.query_row(
            "SELECT in_reply_to FROM messages WHERE id=?",
            [source_id],
            |r| r.get(0),
        )?;
        let version = format!(
            "{:x}",
            sha2::Sha256::digest(
                serde_json::to_vec(&[
                    subject.as_str(),
                    body.as_str(),
                    msgid.as_deref().unwrap_or(""),
                    irt.as_deref().unwrap_or("")
                ])
                .unwrap_or_default()
            )
        );
        let card = SourceCard {
            id: format!("message:{source_id}"),
            kind: "message".into(),
            label: subject,
            relative_path: None,
            locator: None,
            snippet: body.chars().take(8000).collect(),
            version,
            workspace_id: draft.workspace_id,
            fact_status: "unknown".into(),
        };
        let settings = store::get_settings(&conn)?;
        if !settings.ai_enabled || !settings.ai_send_body {
            return Err(AppError::Ai("disabled".into()));
        }
        let subscription_reply_enabled =
            assistant::settings(&conn, draft.draft.account_id)?.subscription_reply_enabled;
        let (provider, key, model) = if subscription_reply_enabled {
            if !codex_subscription::release_enabled() {
                return Err(AppError::Ai(
                    "ChatGPT返信は隔離受入前のため、このビルドでは無効です".into(),
                ));
            }
            (
                "codex_subscription".to_string(),
                None,
                codex_subscription::SUBSCRIPTION_MODEL.to_string(),
            )
        } else {
            let model = if settings.ai_provider == "openai" {
                settings.openai_model
            } else {
                settings.model
            };
            let key = if settings.ai_provider == "openai" {
                keychain::openai_key()?
            } else {
                keychain::gemini_key()?
            }
            .ok_or_else(|| AppError::Ai("APIキーが未登録です".into()))?;
            (settings.ai_provider, Some(key), model)
        };
        // Only confirmed common scopes, plus this workspace's confirmed scope
        // after a current recipient confirmation, may cross the AI boundary.
        let recipient_ok: Option<i64> = conn.query_row("SELECT 1 FROM assistant_recipient_confirmations WHERE draft_id=? AND revision=? AND workspace_id IS ?", rusqlite::params![draft.draft.id,draft.revision,draft.workspace_id], |r| r.get(0)).optional()?;
        let allowed = assistant::scopes(&conn, draft.draft.account_id)?
            .into_iter()
            .filter(|s| {
                s.confirmed
                    && (s.scope == "common"
                        || (recipient_ok.is_some()
                            && s.scope == "workspace"
                            && s.workspace_id == draft.workspace_id))
            })
            .map(|s| s.project_id)
            .collect::<Vec<_>>();
        let projects = ai_projects::selected(&conn, &allowed)?;
        (draft, body, card, projects, provider, key, model)
    };
    let (rules, profile) = {
        let conn = state.db()?;
        (
            assistant::style_rules(&conn, draft_before.draft.account_id)?
                .into_iter()
                .filter(|r| {
                    r.status == "active"
                        && (r.workspace_id.is_none() || r.workspace_id == draft_before.workspace_id)
                })
                .collect::<Vec<_>>(),
            style_profile::active_profile(&conn, draft_before.draft.account_id)?,
        )
    };
    let query = format!(
        "{} {}",
        draft_before.draft.subject,
        source_text.chars().take(240).collect::<String>()
    );
    let report = tokio::task::spawn_blocking(move || {
        ai_projects::search_projects_report(&projects, &query, 8)
    })
    .await
    .map_err(|_| AppError::Ai("資料検索タスクが終了しました".into()))??;
    let mut sources = vec![source_card];
    for hit in report.hits {
        let project = {
            let conn = state.db()?;
            ai_projects::selected(&conn, &[hit.project_id])?
                .into_iter()
                .next()
                .ok_or(AppError::NotFound)?
        };
        let read_path = hit.relative_path.clone();
        let extracted = tokio::task::spawn_blocking(move || {
            ai_projects::read_project_source(&project, &read_path)
        })
        .await
        .map_err(|_| AppError::Ai("資料読取タスクが終了しました".into()))??;
        sources.push(SourceCard {
            id: format!("project:{}:{}", hit.project_id, hit.relative_path),
            kind: "project_file".into(),
            label: hit.project_name,
            relative_path: Some(hit.relative_path),
            locator: hit
                .line_number
                .map(|n| format!("{} {}", if extracted.is_docx { "段落" } else { "行" }, n)),
            snippet: hit.snippet,
            version: extracted.version,
            workspace_id: draft_before.workspace_id,
            fact_status: "confirmed".into(),
        });
    }
    let prepared = assistant_quality::prepare_reply(
        &draft_before.draft.subject,
        &source_text,
        &draft_before.intent,
        &sources,
        &rules,
        profile.as_ref().map(|profile| &profile.content),
    );
    let cfg = ai::gemini::GenConfig {
        temperature: 0.2,
        max_output_tokens: 1500,
        stop: vec![],
        timeout: Duration::from_secs(45),
    };
    let (text, provider, model, actual_input_tokens, actual_output_tokens) =
        if provider == "codex_subscription" {
            let generated = state
                .codex_subscription
                .generate(&prepared.system, &prepared.input)
                .await?;
            (
                generated.text,
                generated.provider,
                generated.model,
                generated.input_tokens,
                generated.output_tokens,
            )
        } else {
            let key = key
                .as_deref()
                .ok_or_else(|| AppError::Ai("APIキーが未登録です".into()))?;
            let text = ai::generate_content(
                &state.http,
                &provider,
                key,
                &model,
                &prepared.system,
                &prepared.input,
                &cfg,
            )
            .await?;
            (text, provider, model, None, None)
        };
    let parsed = assistant_quality::parse_reply(&text, &sources)?;
    let (_draft, run, _) = {
        let conn = state.db()?;
        assistant::generate(&conn, req.draft_id, req.expected_revision)?
    };
    let mut reasons = parsed.review_reasons;
    reasons.extend(report.review_reasons);
    if report.truncated || prepared.truncated {
        reasons.push("資料または入力が上限で省略されています".into());
    }
    let draft = {
        let conn = state.db()?;
        assistant::replace_generated_run(
            &conn,
            &run.id,
            req.draft_id,
            &parsed.generated_text,
            &parsed.questions,
            &sources,
            &reasons,
            prepared.input_chars as i64,
            prepared.truncated || report.truncated,
            &provider,
            &model,
            actual_input_tokens,
            actual_output_tokens,
        )?
    };
    let run = draft
        .reply
        .clone()
        .ok_or_else(|| AppError::Db("生成候補の保存に失敗しました".into()))?;
    Ok(GenerateOut {
        draft,
        run,
        applied: false,
    })
}
#[tauri::command]
pub async fn assistant_generate_reply(
    state: State<'_, AppState>,
    req: GenerateReq,
) -> AppResult<GenerateOut> {
    generate_reply_impl(&state, req).await
}
#[tauri::command]
pub fn assistant_apply_reply(state: State<'_, AppState>, req: ApplyReq) -> AppResult<DraftRecord> {
    assistant::apply_reply(
        &*state.db()?,
        req.draft_id,
        &req.run_id,
        req.expected_revision,
    )
}
#[tauri::command]
pub fn assistant_prepare_send(
    state: State<'_, AppState>,
    req: RevisionReq,
) -> AppResult<PrepareOut> {
    let (token, review_reasons, needs_confirmation) =
        assistant::prepare(&*state.db()?, req.draft_id, req.expected_revision)?;
    Ok(PrepareOut {
        token,
        review_reasons,
        needs_confirmation,
    })
}
#[tauri::command]
pub fn assistant_confirm_recipients(
    state: State<'_, AppState>,
    req: RevisionReq,
) -> AppResult<DraftRecord> {
    assistant::confirm_recipients(&*state.db()?, req.draft_id, req.expected_revision)
}
#[tauri::command]
pub async fn assistant_send_draft(state: State<'_, AppState>, req: SendReq) -> AppResult<SendOut> {
    let (id, status, warnings, draft) = {
        let conn = state.db()?;
        let (id, status, warnings, newly_claimed) =
            assistant::claim_send(&conn, &req.token, req.confirm_warnings)?;
        let draft = if newly_claimed {
            Some(assistant::send_draft_input(&conn, id)?)
        } else {
            None
        };
        (id, status, warnings, draft)
    };
    // A repeated token never reaches SMTP again. This is intentionally the only
    // assistant path that calls the existing manual SMTP implementation.
    if let Some(d) = draft {
        let message = crate::store::models::OutgoingMessage {
            account_id: d.account_id,
            to: d.to,
            cc: d.cc,
            bcc: d.bcc,
            subject: d.subject,
            body_text: d.body_text,
            body_html: d.body_html,
            in_reply_to: d.in_reply_to,
            references: d.references,
            draft_id: None,
        };
        match crate::commands::send_message(state.clone(), message).await {
            Ok(sent) => {
                let conn = state.db()?;
                assistant::finish_send(&conn, &req.token, id, "sent", Some(&sent.message_id))?;
                let revision = assistant::draft_revision(&conn, id)?;
                // Sending never triggers style learning. The explicit 100/200
                // message learning flow is the only profile creation path.
                let _record = assistant::revision(&conn, id, revision)?;
                return Ok(SendOut {
                    draft_id: id,
                    state: "sent".into(),
                    message_id: Some(sent.message_id),
                    warnings,
                });
            }
            Err(_) => {
                assistant::finish_send(&*state.db()?, &req.token, id, "unknown", None)?;
                return Ok(SendOut {
                    draft_id: id,
                    state: "unknown".into(),
                    message_id: None,
                    warnings,
                });
            }
        }
    }
    Ok(SendOut {
        draft_id: id,
        state: status,
        message_id: None,
        warnings,
    })
}
#[tauri::command]
pub fn assistant_get_settings(
    state: State<'_, AppState>,
    req: AccountReq,
) -> AppResult<AssistantSettings> {
    assistant::settings(&*state.db()?, req.account_id)
}
#[tauri::command]
pub async fn assistant_update_settings(
    state: State<'_, AppState>,
    req: SettingsReq,
) -> AppResult<AssistantSettings> {
    if req.subscription_reply_enabled && !codex_subscription::release_enabled() {
        return Err(AppError::Ai(
            "ChatGPT返信は隔離受入前のため、このビルドでは有効にできません".into(),
        ));
    }
    if req.subscription_reply_enabled
        && state.codex_subscription.status().await.connection != "connected"
    {
        return Err(AppError::Ai(
            "ChatGPTに接続してから返信への利用を有効にしてください".into(),
        ));
    }
    let conn = state.db()?;
    assistant_automation::set_auto_draft_enabled(&conn, req.account_id, req.auto_draft_enabled)?;
    assistant::update_settings(
        &conn,
        &AssistantSettings {
            account_id: req.account_id,
            auto_draft_enabled: req.auto_draft_enabled,
            subscription_reply_enabled: req.subscription_reply_enabled,
            learning_enabled: false,
            active_style_profile_id: None,
            daily_generation_limit: req.daily_generation_limit,
            auto_generated_today: 0,
            quota_day: String::new(),
        },
    )
}
#[tauri::command]
pub async fn assistant_process_queue(
    state: State<'_, AppState>,
    req: AccountReq,
) -> AppResult<QueueOut> {
    process_queue_impl(&state, req.account_id).await
}

pub(crate) async fn process_queue_impl(state: &AppState, account_id: i64) -> AppResult<QueueOut> {
    let job_and_draft = {
        let conn = state.db()?;
        let s = assistant::settings(&conn, account_id)?;
        if !s.auto_draft_enabled {
            return Ok(QueueOut {
                processed: 0,
                pending: 0,
                reason: Some("自動下書きはオフです".into()),
            });
        }
        let app = store::get_settings(&conn)?;
        if !app.ai_enabled || !app.ai_send_body {
            return Ok(QueueOut {
                processed: 0,
                pending: assistant_automation::pending_count(&conn, account_id)?,
                reason: Some("AIまたは本文共有がオフです".into()),
            });
        }
        assistant_automation::enqueue_new(&conn, account_id)?;
        let Some(job) = assistant_automation::claim_next(&conn, account_id)? else {
            let pending = assistant_automation::pending_count(&conn, account_id)?;
            return Ok(QueueOut {
                processed: 0,
                pending,
                reason: Some(
                    if pending == 0 {
                        "待機中の新着メールはありません"
                    } else {
                        "日次上限または別の生成処理を待っています"
                    }
                    .into(),
                ),
            });
        };
        let (from, subject, message_id, references): (String, String, Option<String>, Option<String>) = conn.query_row(
            "SELECT from_addr,subject,message_id,reference_ids FROM messages WHERE id=? AND account_id=? AND body_fetched=1",
            rusqlite::params![job.source_message_id, job.account_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let reply_subject = if subject.to_ascii_lowercase().starts_with("re:") {
            subject
        } else {
            format!("Re: {subject}")
        };
        let mut refs = references
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        if let Some(id) = message_id.as_ref().filter(|id| !id.trim().is_empty()) {
            refs.push(id.clone());
        }
        refs.sort();
        refs.dedup();
        let input = DraftInput {
            id: None,
            account_id: job.account_id,
            to: vec![from],
            cc: vec![],
            bcc: vec![],
            subject: reply_subject,
            body_text: String::new(),
            body_html: None,
            in_reply_to: message_id,
            references: refs,
        };
        let session = format!("auto:{}:{}", job.account_id, job.rfc_identity);
        let draft = assistant::save_draft(
            &conn,
            &input,
            &session,
            None,
            Some(job.source_message_id),
            None,
            "",
            None,
        )?;
        (job, draft)
    };
    let (job, draft) = job_and_draft;
    match generate_reply_impl(
        state,
        GenerateReq {
            draft_id: draft.draft.id,
            expected_revision: draft.revision,
            force: false,
        },
    )
    .await
    {
        Ok(_) => {
            let conn = state.db()?;
            assistant_automation::finish_job(&conn, job.id, "completed")?;
            Ok(QueueOut {
                processed: 1,
                pending: assistant_automation::pending_count(&conn, account_id)?,
                reason: Some("下書き候補を1件生成しました。内容を確認してください".into()),
            })
        }
        Err(error) => {
            let conn = state.db()?;
            let _ = assistant_automation::finish_job(&conn, job.id, "failed");
            let pending = assistant_automation::pending_count(&conn, account_id)?;
            Err(match error {
                AppError::Auth(_) | AppError::Keychain(_) => error,
                _ => AppError::Ai(format!(
                    "下書き候補を生成できませんでした（待機 {pending} 件）"
                )),
            })
        }
    }
}
#[tauri::command]
pub fn assistant_list_style_rules(
    state: State<'_, AppState>,
    req: AccountReq,
) -> AppResult<Vec<StyleRule>> {
    assistant::style_rules(&*state.db()?, req.account_id)
}
#[tauri::command]
pub fn assistant_save_style_rule(state: State<'_, AppState>, req: RuleReq) -> AppResult<StyleRule> {
    assistant::save_rule(
        &*state.db()?,
        StyleRule {
            id: req.id.unwrap_or(0),
            account_id: req.account_id,
            workspace_id: req.workspace_id,
            from_phrase: req.from_phrase,
            to_phrase: req.to_phrase,
            status: req.status,
            created_at: 0,
        },
    )
}
#[tauri::command]
pub fn assistant_delete_style_rule(
    state: State<'_, AppState>,
    req: DeleteRuleReq,
) -> AppResult<()> {
    assistant::delete_rule(&*state.db()?, req.account_id, req.id)
}

#[tauri::command]
pub fn assistant_style_learning_preview(
    state: State<'_, AppState>,
    req: StyleLearningPreviewReq,
) -> AppResult<StyleLearningPreview> {
    style_profile::learning_preview(&*state.db()?, req.account_id, req.requested_count)
}

#[tauri::command]
pub fn assistant_start_style_learning(
    state: State<'_, AppState>,
    req: StyleLearningStartReq,
) -> AppResult<StyleLearningResult> {
    style_profile::start_learning(
        &*state.db()?,
        req.account_id,
        req.requested_count,
        &req.consent_version,
        &req.preview_hash,
    )
}

#[tauri::command]
pub fn assistant_get_style_learning_job(
    state: State<'_, AppState>,
    req: StyleProfileJobReq,
) -> AppResult<StyleProfileJob> {
    style_profile::get_job(&*state.db()?, req.account_id, &req.job_id)
}

#[tauri::command]
pub fn assistant_cancel_style_learning(
    state: State<'_, AppState>,
    req: StyleProfileJobReq,
) -> AppResult<StyleProfileJob> {
    style_profile::cancel_job(&*state.db()?, req.account_id, &req.job_id)
}

#[tauri::command]
pub fn assistant_list_style_profiles(
    state: State<'_, AppState>,
    req: AccountReq,
) -> AppResult<Vec<StyleProfile>> {
    style_profile::list_profiles(&*state.db()?, req.account_id)
}

#[tauri::command]
pub fn assistant_update_style_profile(
    state: State<'_, AppState>,
    req: StyleProfileUpdateReq,
) -> AppResult<StyleProfile> {
    style_profile::update_profile(
        &*state.db()?,
        req.account_id,
        req.profile_id,
        req.expected_updated_at,
        &req.content,
    )
}

#[tauri::command]
pub fn assistant_activate_style_profile(
    state: State<'_, AppState>,
    req: StyleProfileIdReq,
) -> AppResult<StyleProfile> {
    style_profile::activate_profile(&*state.db()?, req.account_id, req.profile_id)
}

#[tauri::command]
pub fn assistant_delete_style_profile(
    state: State<'_, AppState>,
    req: StyleProfileIdReq,
) -> AppResult<()> {
    style_profile::delete_profile(&*state.db()?, req.account_id, req.profile_id)
}

#[tauri::command]
pub fn assistant_preview_style_profile(
    state: State<'_, AppState>,
    req: StyleProfilePreviewReq,
) -> AppResult<StyleProfilePreviewText> {
    style_profile::preview_profile(
        &*state.db()?,
        req.account_id,
        req.profile_id,
        &req.scenario_id,
    )
}
