//! P0 confirmed-draft persistence and boundary checks.
//!
//! This module intentionally has no SMTP, CLI, Keychain, or provider calls.  It
//! makes the locally observable state transitions atomic; a future provider
//! adapter may create a reply run, but cannot bypass these ownership checks.

use crate::ai_projects;
use crate::error::{AppError, AppResult};
use crate::store::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "[]".into())
}
fn vec(v: String) -> Vec<String> {
    serde_json::from_str(&v).unwrap_or_default()
}
fn hash(parts: &[&str]) -> String {
    // JSON array preserves item boundaries, unlike concatenation; SHA-256 is
    // used only as a tamper/change detector, never as a password hash.
    let bytes = serde_json::to_vec(parts).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}
fn account_exists(c: &Connection, id: i64) -> AppResult<()> {
    if c.query_row("SELECT 1 FROM accounts WHERE id=?", [id], |r| {
        r.get::<_, i64>(0)
    })
    .optional()?
    .is_none()
    {
        return Err(AppError::NotFound);
    }
    Ok(())
}
fn workspace_ok(c: &Connection, account: i64, workspace: Option<i64>) -> AppResult<()> {
    if let Some(id) = workspace {
        let got: Option<i64> = c
            .query_row(
                "SELECT account_id FROM assistant_workspaces WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if got != Some(account) {
            return Err(AppError::BadInput(
                "この案件はメールアカウントに属していません".into(),
            ));
        }
    }
    Ok(())
}
fn message_account(c: &Connection, id: i64) -> AppResult<i64> {
    c.query_row("SELECT account_id FROM messages WHERE id=?", [id], |r| {
        r.get(0)
    })
    .map_err(Into::into)
}
fn draft(c: &Connection, id: i64) -> AppResult<DraftRecord> {
    let row: (DraftInputOut,i64,String,i64,Option<i64>,Option<i64>,String,Option<String>,String,Vec<String>) = c.query_row("SELECT d.id,d.account_id,d.to_addrs,d.cc_addrs,d.bcc_addrs,d.subject,d.body_text,d.body_html,d.in_reply_to,d.reference_ids,d.updated_at,m.session_id,m.revision,m.source_message_id,m.workspace_id,m.intent,m.reply_run_id,m.status,m.review_reasons FROM drafts d JOIN assistant_draft_meta m ON m.draft_id=d.id WHERE d.id=?", [id], |r| {
     Ok((DraftInputOut { id:r.get(0)?, account_id:r.get(1)?, to:vec(r.get(2)?), cc:vec(r.get(3)?), bcc:vec(r.get(4)?) , subject:r.get(5)?, body_text:r.get(6)?, body_html:r.get(7)?, in_reply_to:r.get(8)?, references:r.get::<_, String>(9)?.split_whitespace().map(str::to_string).collect() }, r.get::<_,i64>(10)?, r.get::<_,String>(11)?, r.get::<_,i64>(12)?, r.get(13)?, r.get(14)?, r.get(15)?, r.get(16)?, r.get(17)?, vec(r.get(18)?)))
 })?;
    let reply = match &row.7 {
        Some(run_id) => load_run(c, run_id)?,
        None => None,
    };
    Ok(DraftRecord {
        draft: row.0,
        updated_at: row.1,
        session_id: row.2,
        revision: row.3,
        source_message_id: row.4,
        workspace_id: row.5,
        intent: row.6,
        reply_run_id: row.7,
        reply,
        status: row.8,
        review_reasons: row.9,
    })
}
fn load_run(c: &Connection, id: &str) -> AppResult<Option<ReplyRun>> {
    c.query_row("SELECT id,draft_id,source_message_id,workspace_id,context_version,profile_version,snapshot_hash,generated_text,questions_json,sources_json,review_reasons,status,truncated,provider,model,input_chars,estimated_input_tokens,actual_input_tokens,actual_output_tokens,created_at FROM assistant_reply_runs WHERE id=?",[id],|r| Ok(ReplyRun{id:r.get(0)?,draft_id:r.get(1)?,source_message_id:r.get(2)?,workspace_id:r.get(3)?,context_version:r.get(4)?,profile_version:r.get(5)?,snapshot_hash:r.get(6)?,generated_text:r.get(7)?,questions:serde_json::from_str(&r.get::<_,String>(8)?).unwrap_or_default(),sources:serde_json::from_str(&r.get::<_,String>(9)?).unwrap_or_default(),review_reasons:vec(r.get(10)?),status:r.get(11)?,truncated:r.get::<_,i64>(12)?!=0,provider:r.get(13)?,model:r.get(14)?,input_chars:r.get(15)?,estimated_input_tokens:r.get(16)?,actual_input_tokens:r.get(17)?,actual_output_tokens:r.get(18)?,created_at:r.get(19)?})).optional().map_err(Into::into)
}

/// Re-read every saved source before it can change or send a draft. Stored
/// snippets are never proof that a source is still permitted or unchanged.
pub fn validate_run_sources(c: &Connection, draft: &DraftRecord, run: &ReplyRun) -> AppResult<()> {
    if run.draft_id != draft.draft.id || run.workspace_id != draft.workspace_id {
        return Err(AppError::BadInput(
            "生成候補の案件境界が一致しません".into(),
        ));
    }
    let allowed = scopes(c, draft.draft.account_id)?;
    for source in &run.sources {
        if source.kind == "message" {
            let source_id = source
                .id
                .strip_prefix("message:")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or_else(|| AppError::BadInput("メール出典IDが不正です".into()))?;
            let row: (String, Option<String>, Option<String>, Option<String>) = c.query_row("SELECT subject,body_text,message_id,in_reply_to FROM messages WHERE id=? AND account_id=?", params![source_id,draft.draft.account_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|_| AppError::BadInput("元メールが削除または変更されました".into()))?;
            let current = hash(&[
                &row.0,
                row.1.as_deref().unwrap_or(""),
                row.2.as_deref().unwrap_or(""),
                row.3.as_deref().unwrap_or(""),
            ]);
            if current != source.version {
                return Err(AppError::BadInput(
                    "元メールが変更されたため再生成してください".into(),
                ));
            }
            if Some(source_id) != draft.source_message_id {
                let boundary: Option<(Option<i64>, String)> = c
                    .query_row(
                        "SELECT workspace_id,status FROM assistant_contexts WHERE source_message_id=? AND account_id=?",
                        params![source_id, draft.draft.account_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let permitted = boundary.is_some_and(|(workspace, status)| {
                    workspace == draft.workspace_id
                        && (status == "confirmed" || status == "common_only")
                });
                if !permitted {
                    return Err(AppError::BadInput(
                        "メール出典が現在の顧客コンテキストに属していません".into(),
                    ));
                }
            }
        } else if source.kind == "project_file" {
            let rest = source
                .id
                .strip_prefix("project:")
                .ok_or_else(|| AppError::BadInput("資料出典IDが不正です".into()))?;
            let (project_id, path) = rest
                .split_once(':')
                .ok_or_else(|| AppError::BadInput("資料出典IDが不正です".into()))?;
            let project_id: i64 = project_id
                .parse()
                .map_err(|_| AppError::BadInput("資料出典IDが不正です".into()))?;
            let scope = allowed
                .iter()
                .find(|s| {
                    s.project_id == project_id
                        && s.confirmed
                        && (s.scope == "common"
                            || (s.scope == "workspace" && s.workspace_id == draft.workspace_id))
                })
                .ok_or_else(|| AppError::BadInput("資料の利用許可が失効しました".into()))?;
            if scope.scope == "workspace" {
                let recipients = json(&(
                    draft.draft.to.clone(),
                    draft.draft.cc.clone(),
                    draft.draft.bcc.clone(),
                ));
                let ok: Option<i64> = c.query_row("SELECT 1 FROM assistant_recipient_confirmations WHERE draft_id=? AND recipients_hash=? AND workspace_id IS ?", params![draft.draft.id,hash(&[&recipients]),draft.workspace_id], |r|r.get(0)).optional()?;
                if ok.is_none() {
                    return Err(AppError::BadInput("案件資料の宛先確認が必要です".into()));
                }
            }
            let project = ai_projects::selected(c, &[project_id])?
                .into_iter()
                .next()
                .ok_or_else(|| AppError::BadInput("資料プロジェクトが利用できません".into()))?;
            let current = ai_projects::read_project_source(&project, path)?;
            if current.version != source.version {
                return Err(AppError::BadInput(
                    "資料が変更されたため再生成してください".into(),
                ));
            }
        } else {
            return Err(AppError::BadInput("出典種別が不正です".into()));
        }
    }
    Ok(())
}

pub fn list_workspaces(c: &Connection, account: i64) -> AppResult<Vec<Workspace>> {
    account_exists(c, account)?;
    let mut s=c.prepare("SELECT w.id,w.account_id,w.customer_id,cu.name,w.name,w.context_version FROM assistant_workspaces w JOIN assistant_customers cu ON cu.id=w.customer_id WHERE w.account_id=? ORDER BY w.updated_at DESC")?;
    let result = s
        .query_map([account], |r| {
            Ok(Workspace {
                id: r.get(0)?,
                account_id: r.get(1)?,
                customer_id: r.get(2)?,
                customer_name: r.get(3)?,
                name: r.get(4)?,
                context_version: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(result)
}
pub fn create_workspace(
    c: &Connection,
    account: i64,
    customer: &str,
    name: &str,
) -> AppResult<Workspace> {
    account_exists(c, account)?;
    if customer.trim().is_empty() || name.trim().is_empty() {
        return Err(AppError::BadInput(
            "顧客名と案件名を入力してください".into(),
        ));
    };
    let t = now();
    let tx = c.unchecked_transaction()?;
    tx.execute("INSERT INTO assistant_customers(account_id,name,created_at) VALUES(?,?,?) ON CONFLICT(account_id,name) DO NOTHING",params![account,customer.trim(),t])?;
    let cid: i64 = tx.query_row(
        "SELECT id FROM assistant_customers WHERE account_id=? AND name=?",
        params![account, customer.trim()],
        |r| r.get(0),
    )?;
    tx.execute(
        "INSERT INTO assistant_workspaces(account_id,customer_id,name,updated_at) VALUES(?,?,?,?)",
        params![account, cid, name.trim(), t],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(Workspace {
        id,
        account_id: account,
        customer_id: cid,
        customer_name: customer.trim().into(),
        name: name.trim().into(),
        context_version: 1,
    })
}

fn context_row(c: &Connection, source: i64) -> AppResult<Option<ReplyContext>> {
    let row: Option<(i64, i64, Option<i64>, String, Vec<String>)> = c
        .query_row(
            "SELECT source_message_id,thread_id,workspace_id,status,review_reasons FROM assistant_contexts WHERE source_message_id=?",
            [source],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, vec(r.get(4)?))),
        )
        .optional()?;
    let Some(row) = row else { return Ok(None) };
    let context_version = row
        .2
        .and_then(|workspace| {
            c.query_row(
                "SELECT context_version FROM assistant_workspaces WHERE id=?",
                [workspace],
                |r| r.get(0),
            )
            .ok()
        })
        .unwrap_or(0);
    Ok(Some(ReplyContext {
        source_message_id: row.0,
        thread_id: row.1,
        workspace_id: row.2,
        context_version,
        status: row.3,
        review_reasons: row.4,
    }))
}

pub fn get_context(c: &Connection, source: i64) -> AppResult<ReplyContext> {
    let account = message_account(c, source)?;
    if let Some(existing) = context_row(c, source)? {
        return Ok(existing);
    }

    // Only RFC reply headers may suggest inheritance. Subject/from matching is
    // deliberately excluded because it can join unrelated customers.
    let headers: (Option<String>, String) = c.query_row(
        "SELECT in_reply_to,coalesce(reference_ids,'') FROM messages WHERE id=? AND account_id=?",
        params![source, account],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut references = Vec::new();
    if let Some(parent) = headers.0 {
        if !parent.trim().is_empty() {
            references.push(parent.trim().to_string());
        }
    }
    for reference in headers.1.split_whitespace() {
        if !references.iter().any(|known| known == reference) {
            references.push(reference.to_string());
        }
    }

    let mut candidates = Vec::new();
    for reference in references {
        let mut statement = c.prepare(
            "SELECT ac.thread_id,ac.workspace_id,ac.status
             FROM messages m JOIN assistant_contexts ac ON ac.source_message_id=m.id
             WHERE m.account_id=? AND m.message_id=?",
        )?;
        for candidate in statement.query_map(params![account, reference], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let candidate = candidate?;
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }

    if !candidates.is_empty() {
        let first = candidates[0].clone();
        let conflict = candidates.iter().any(|candidate| {
            candidate.0 != first.0
                || candidate.1 != first.1
                || candidate.2 == "conflict"
                || first.2 == "conflict"
        });
        let (thread_id, workspace_id, status, reasons) = if conflict {
            (
                source,
                None,
                "conflict",
                vec!["返信ヘッダーが異なる顧客コンテキストを参照しています。案件を手動で確認してください。".to_string()],
            )
        } else if first.1.is_some() {
            (
                first.0,
                first.1,
                "needs_review",
                vec!["RFC返信ヘッダーから案件候補を継承しました。宛先・CCを確認して確定してください。".to_string()],
            )
        } else {
            (
                first.0,
                None,
                "common_only",
                vec!["このスレッドは共通資料のみを使用します。".to_string()],
            )
        };
        c.execute(
            "INSERT INTO assistant_contexts(source_message_id,account_id,workspace_id,thread_id,status,review_reasons) VALUES(?,?,?,?,?,?)",
            params![source, account, workspace_id, thread_id, status, json(&reasons)],
        )?;
        return context_row(c, source)?.ok_or(AppError::NotFound);
    }

    Ok(ReplyContext {
        source_message_id: source,
        thread_id: source,
        workspace_id: None,
        context_version: 0,
        status: "unassigned".into(),
        review_reasons: vec!["案件は未確定です。共通資料以外は使えません。".into()],
    })
}
pub fn bind_context(
    c: &Connection,
    source: i64,
    workspace: Option<i64>,
) -> AppResult<ReplyContext> {
    let account = message_account(c, source)?;
    workspace_ok(c, account, workspace)?;
    let resolved = get_context(c, source)?;
    let (status, reasons) = if workspace.is_some() {
        ("confirmed", Vec::<String>::new())
    } else {
        (
            "common_only",
            vec!["共通資料だけを使用します。顧客固有資料は参照しません。".to_string()],
        )
    };
    c.execute("INSERT INTO assistant_contexts(source_message_id,account_id,workspace_id,thread_id,status,review_reasons) VALUES(?,?,?,?,?,?) ON CONFLICT(source_message_id) DO UPDATE SET workspace_id=excluded.workspace_id,status=excluded.status,review_reasons=excluded.review_reasons",params![source,account,workspace,resolved.thread_id,status,json(&reasons)])?;
    get_context(c, source)
}

pub fn save_draft(
    c: &Connection,
    input: &DraftInput,
    session: &str,
    expected: Option<i64>,
    source: Option<i64>,
    workspace: Option<i64>,
    intent: &str,
    reply: Option<String>,
) -> AppResult<DraftRecord> {
    if session.trim().is_empty() {
        return Err(AppError::BadInput("保存セッションがありません".into()));
    }
    account_exists(c, input.account_id)?;
    workspace_ok(c, input.account_id, workspace)?;
    if let Some(s) = source {
        if message_account(c, s)? != input.account_id {
            return Err(AppError::BadInput(
                "元メールのアカウントが一致しません".into(),
            ));
        }
    }
    if let Some(id) = input.id {
        let owned: Option<(i64, String)> = c
            .query_row(
                "SELECT account_id,status FROM assistant_draft_meta WHERE draft_id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match owned {
            Some((account, status))
                if account == input.account_id
                    && !["sending", "sent", "unknown", "discarded"].contains(&status.as_str()) => {}
            Some(_) => return Err(AppError::BadInput("この下書きは保存できません".into())),
            None => {
                return Err(AppError::BadInput(
                    "管理対象でない下書きはこの保存APIで更新できません".into(),
                ))
            }
        }
    }
    let tx = c.unchecked_transaction()?;
    let existing:Option<(i64,i64)>=tx.query_row("SELECT draft_id,revision FROM assistant_draft_meta WHERE account_id=? AND session_id=?",params![input.account_id,session],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let id = match (existing, input.id) {
        (Some((id, rev)), _) => {
            if input.id.is_some() && input.id != Some(id) {
                return Err(AppError::BadInput(
                    "保存セッションと下書きが一致しません".into(),
                ));
            }
            if expected != Some(rev) {
                return Err(AppError::BadInput(
                    "下書きが別の編集で更新されました。再読み込みしてください".into(),
                ));
            }
            id
        }
        (None, Some(id)) => {
            let rev: Option<i64> = tx
                .query_row(
                    "SELECT revision FROM assistant_draft_meta WHERE draft_id=?",
                    [id],
                    |r| r.get(0),
                )
                .optional()?;
            if rev != expected {
                return Err(AppError::BadInput("下書きの版が一致しません".into()));
            }
            id
        }
        (None, None) => {
            tx.execute("INSERT INTO drafts(account_id,to_addrs,cc_addrs,bcc_addrs,subject,body_text,body_html,in_reply_to,reference_ids,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)",params![input.account_id,json(&input.to),json(&input.cc),json(&input.bcc),input.subject,input.body_text,input.body_html,input.in_reply_to,input.references.join(" "),now()])?;
            tx.last_insert_rowid()
        }
    };
    let changed=tx.execute("UPDATE drafts SET account_id=?,to_addrs=?,cc_addrs=?,bcc_addrs=?,subject=?,body_text=?,body_html=?,in_reply_to=?,reference_ids=?,updated_at=? WHERE id=?",params![input.account_id,json(&input.to),json(&input.cc),json(&input.bcc),input.subject,input.body_text,input.body_html,input.in_reply_to,input.references.join(" "),now(),id])?;
    if changed == 0 {
        return Err(AppError::NotFound);
    };
    tx.execute("INSERT INTO assistant_draft_meta(draft_id,account_id,session_id,revision,source_message_id,workspace_id,intent,reply_run_id,status,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(draft_id) DO UPDATE SET session_id=excluded.session_id,revision=assistant_draft_meta.revision+1,source_message_id=excluded.source_message_id,workspace_id=excluded.workspace_id,intent=excluded.intent,reply_run_id=excluded.reply_run_id,status='editing',updated_at=excluded.updated_at",params![id,input.account_id,session,1,source,workspace,intent,reply,"editing",now()])?;
    tx.commit()?;
    draft(c, id)
}
pub fn list_drafts(c: &Connection, account: i64, discarded: bool) -> AppResult<Vec<DraftRecord>> {
    account_exists(c, account)?;
    let mut s=c.prepare("SELECT draft_id FROM assistant_draft_meta WHERE account_id=? AND (? OR status!='discarded') ORDER BY updated_at DESC LIMIT 100")?;
    let ids = s
        .query_map(params![account, discarded as i64], |r| r.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;
    ids.into_iter().map(|x| draft(c, x)).collect()
}
pub fn revision(c: &Connection, id: i64, expect: i64) -> AppResult<DraftRecord> {
    let d = draft(c, id)?;
    if d.revision != expect {
        return Err(AppError::BadInput(
            "下書きが更新されています。再読み込みしてください".into(),
        ));
    }
    Ok(d)
}
pub fn draft_revision(c: &Connection, id: i64) -> AppResult<i64> {
    Ok(draft(c, id)?.revision)
}
pub fn get_draft(c: &Connection, id: i64, expected: i64) -> AppResult<DraftRecord> {
    revision(c, id, expected)
}

pub fn replace_generated_run(
    c: &Connection,
    run_id: &str,
    draft_id: i64,
    generated_text: &str,
    questions: &[ReplyQuestion],
    sources: &[SourceCard],
    reasons: &[String],
    input_chars: i64,
    truncated: bool,
    provider: &str,
    model: &str,
    actual_input_tokens: Option<i64>,
    actual_output_tokens: Option<i64>,
) -> AppResult<DraftRecord> {
    c.execute("UPDATE assistant_reply_runs SET generated_text=?,questions_json=?,sources_json=?,review_reasons=?,input_chars=?,truncated=?,provider=?,model=?,actual_input_tokens=?,actual_output_tokens=? WHERE id=? AND draft_id=?", params![generated_text,json(&questions),json(&sources),json(&reasons),input_chars,truncated as i64,provider,model,actual_input_tokens,actual_output_tokens,run_id,draft_id])?;
    c.execute("UPDATE assistant_draft_meta SET review_reasons=?,status='needs_review',updated_at=? WHERE draft_id=? AND reply_run_id=?",params![json(&reasons),now(),draft_id,run_id])?;
    draft(c, draft_id)
}
pub fn transition(c: &Connection, id: i64, expect: i64, status: &str) -> AppResult<DraftRecord> {
    let d = revision(c, id, expect)?;
    if d.status == "sending" || d.status == "sent" || d.status == "unknown" {
        return Err(AppError::BadInput(
            "送信処理中または送信済みの下書きは変更できません".into(),
        ));
    };
    c.execute("UPDATE assistant_draft_meta SET status=?,revision=revision+1,updated_at=? WHERE draft_id=?",params![status,now(),id])?;
    draft(c, id)
}
pub fn scopes(c: &Connection, account: i64) -> AppResult<Vec<ProjectScope>> {
    account_exists(c, account)?;
    let mut s=c.prepare("SELECT account_id,project_id,workspace_id,scope,confirmed FROM assistant_project_scopes WHERE account_id=?")?;
    let result = s
        .query_map([account], |r| {
            Ok(ProjectScope {
                account_id: r.get(0)?,
                project_id: r.get(1)?,
                workspace_id: r.get(2)?,
                scope: r.get(3)?,
                confirmed: r.get::<_, i64>(4)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(result)
}
pub fn set_scope(c: &Connection, x: &ProjectScope) -> AppResult<ProjectScope> {
    account_exists(c, x.account_id)?;
    workspace_ok(c, x.account_id, x.workspace_id)?;
    if !["disabled", "common", "workspace"].contains(&x.scope.as_str())
        || (x.scope == "workspace") != x.workspace_id.is_some()
    {
        return Err(AppError::BadInput("資料範囲の指定が不正です".into()));
    };
    let exists: Option<i64> = c
        .query_row(
            "SELECT id FROM ai_projects WHERE id=?",
            [x.project_id],
            |r| r.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    };
    c.execute("INSERT INTO assistant_project_scopes(account_id,project_id,workspace_id,scope,confirmed) VALUES(?,?,?,?,?) ON CONFLICT(account_id,project_id) DO UPDATE SET workspace_id=excluded.workspace_id,scope=excluded.scope,confirmed=excluded.confirmed",params![x.account_id,x.project_id,x.workspace_id,x.scope,x.confirmed as i64])?;
    Ok(x.clone())
}
pub fn settings(c: &Connection, account: i64) -> AppResult<AssistantSettings> {
    account_exists(c, account)?;
    c.execute(
        "INSERT OR IGNORE INTO assistant_settings(account_id,quota_day) VALUES(?,?)",
        params![account, utc_day()],
    )?;
    c.query_row(
        "SELECT s.account_id,s.auto_draft_enabled,s.learning_enabled,s.daily_generation_limit,
                s.auto_generated_today,s.quota_day,
                (SELECT p.id FROM assistant_style_profiles p
                 WHERE p.account_id=s.account_id AND p.workspace_id IS NULL AND p.status='active'
                 LIMIT 1),s.subscription_reply_enabled
         FROM assistant_settings s WHERE s.account_id=?",
        [account],
        |r| {
            Ok(AssistantSettings {
                account_id: r.get(0)?,
                auto_draft_enabled: r.get::<_, i64>(1)? != 0,
                subscription_reply_enabled: r.get::<_, i64>(7)? != 0,
                // Kept in the DTO for an old frontend only. The migration and
                // update path force this off; learning now requires an explicit job.
                learning_enabled: false,
                daily_generation_limit: r.get(3)?,
                auto_generated_today: r.get(4)?,
                quota_day: r.get(5)?,
                active_style_profile_id: r.get(6)?,
            })
        },
    )
    .map_err(Into::into)
}
fn utc_day() -> String {
    format!("{}", now().div_euclid(86400))
}
pub fn update_settings(c: &Connection, x: &AssistantSettings) -> AppResult<AssistantSettings> {
    if !(1..=100).contains(&x.daily_generation_limit) {
        return Err(AppError::BadInput("日次上限は1〜100件です".into()));
    };
    let old = settings(c, x.account_id)?;
    c.execute(
        "UPDATE assistant_settings SET auto_draft_enabled=?,learning_enabled=0,subscription_reply_enabled=?,daily_generation_limit=? WHERE account_id=?",
        params![x.auto_draft_enabled as i64, x.subscription_reply_enabled as i64, x.daily_generation_limit, x.account_id],
    )?;
    let _ = old;
    settings(c, x.account_id)
}
pub fn style_rules(c: &Connection, account: i64) -> AppResult<Vec<StyleRule>> {
    account_exists(c, account)?;
    let mut s=c.prepare("SELECT id,account_id,workspace_id,from_phrase,to_phrase,status,created_at FROM assistant_style_rules WHERE account_id=? ORDER BY created_at DESC")?;
    let result = s
        .query_map([account], |r| {
            Ok(StyleRule {
                id: r.get(0)?,
                account_id: r.get(1)?,
                workspace_id: r.get(2)?,
                from_phrase: r.get(3)?,
                to_phrase: r.get(4)?,
                status: r.get(5)?,
                created_at: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(result)
}
pub fn save_rule(c: &Connection, mut x: StyleRule) -> AppResult<StyleRule> {
    account_exists(c, x.account_id)?;
    workspace_ok(c, x.account_id, x.workspace_id)?;
    if x.from_phrase.trim().is_empty()
        || x.to_phrase.trim().is_empty()
        || x.from_phrase.chars().count() > 200
        || x.to_phrase.chars().count() > 200
        || !["pending", "active", "rejected"].contains(&x.status.as_str())
    {
        return Err(AppError::BadInput("文体ルールが不正です".into()));
    };
    if x.id == 0 {
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM assistant_style_rules WHERE account_id=?",
            [x.account_id],
            |r| r.get(0),
        )?;
        if n >= 100 {
            return Err(AppError::BadInput("文体ルールは100件までです".into()));
        };
        x.created_at = now();
        c.execute("INSERT INTO assistant_style_rules(account_id,workspace_id,from_phrase,to_phrase,status,created_at) VALUES(?,?,?,?,?,?)",params![x.account_id,x.workspace_id,x.from_phrase,x.to_phrase,x.status,x.created_at])?;
        x.id = c.last_insert_rowid()
    } else {
        c.execute("UPDATE assistant_style_rules SET workspace_id=?,from_phrase=?,to_phrase=?,status=? WHERE id=? AND account_id=?",params![x.workspace_id,x.from_phrase,x.to_phrase,x.status,x.id,x.account_id])?;
    }
    c.execute("INSERT INTO assistant_style_profile_versions(account_id,version) VALUES(?,1) ON CONFLICT(account_id) DO UPDATE SET version=version+1",[x.account_id])?;
    Ok(x)
}
pub fn delete_rule(c: &Connection, account: i64, id: i64) -> AppResult<()> {
    if c.execute(
        "DELETE FROM assistant_style_rules WHERE id=? AND account_id=?",
        params![id, account],
    )? == 0
    {
        return Err(AppError::NotFound);
    };
    c.execute("INSERT INTO assistant_style_profile_versions(account_id,version) VALUES(?,1) ON CONFLICT(account_id) DO UPDATE SET version=version+1",[account])?;
    Ok(())
}
pub fn generate(c: &Connection, id: i64, expect: i64) -> AppResult<(DraftRecord, ReplyRun, bool)> {
    let d = revision(c, id, expect)?;
    if d.status == "discarded" {
        return Err(AppError::BadInput("破棄済み下書きは生成できません".into()));
    };
    let (body, sourcecard, truncated) = if let Some(s) = d.source_message_id {
        let (subject, b): (String, Option<String>) = c.query_row(
            "SELECT subject,body_text FROM messages WHERE id=? AND account_id=?",
            params![s, d.draft.account_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let raw = b.unwrap_or_default();
        let trunc = raw.chars().count() > 8000;
        let shown: String = raw.chars().take(8000).collect();
        (
            shown.clone(),
            Some(SourceCard {
                id: format!("message:{s}"),
                kind: "message".into(),
                label: subject,
                relative_path: None,
                locator: None,
                snippet: shown.chars().take(500).collect(),
                version: hash(&[&shown]),
                workspace_id: d.workspace_id,
                fact_status: "unknown".into(),
            }),
            trunc,
        )
    } else {
        ("".into(), None, false)
    };
    let mut reasons = vec!["生成案は送信前に内容と根拠を確認してください。".into()];
    if d.workspace_id.is_none() {
        reasons.push("案件が未確定のため、顧客固有の資料・履歴は使っていません。".into())
    };
    if truncated {
        reasons.push("元メール本文が上限で省略されています。末尾を確認してください。".into())
    };
    let text = if d.intent.trim().is_empty() {
        "ご連絡ありがとうございます。内容を確認のうえ、改めてご返信いたします。".into()
    } else {
        format!(
            "ご連絡ありがとうございます。{}について、内容を確認のうえご返信いたします。",
            d.intent.trim()
        )
    };
    let run = ReplyRun {
        id: Uuid::new_v4().to_string(),
        draft_id: id,
        source_message_id: d.source_message_id,
        workspace_id: d.workspace_id,
        context_version: d
            .workspace_id
            .and_then(|w| {
                c.query_row(
                    "SELECT context_version FROM assistant_workspaces WHERE id=?",
                    [w],
                    |r| r.get(0),
                )
                .ok()
            })
            .unwrap_or(0),
        profile_version: c
            .query_row(
                "SELECT version FROM assistant_style_profile_versions WHERE account_id=?",
                [d.draft.account_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0),
        snapshot_hash: hash(&[&d.draft.subject, &d.draft.body_text, &d.intent]),
        generated_text: text,
        questions: vec![ReplyQuestion {
            id: "q1".into(),
            text: "送信前に回答内容を確認してください".into(),
            status: "needs_input".into(),
            source_ids: sourcecard
                .as_ref()
                .map(|s| vec![s.id.clone()])
                .unwrap_or_default(),
        }],
        sources: sourcecard.into_iter().collect(),
        review_reasons: reasons.clone(),
        status: "needs_review".into(),
        truncated,
        provider: "local-p0".into(),
        model: "review-required".into(),
        input_chars: body.chars().count() as i64,
        estimated_input_tokens: 0,
        actual_input_tokens: None,
        actual_output_tokens: None,
        created_at: now(),
    };
    c.execute(
        "INSERT INTO assistant_reply_runs(
            id,draft_id,source_message_id,workspace_id,context_version,profile_version,
            snapshot_hash,generated_text,questions_json,sources_json,review_reasons,status,
            truncated,provider,model,input_chars,estimated_input_tokens,actual_input_tokens,
            actual_output_tokens,created_at
         ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        params![
            run.id,
            run.draft_id,
            run.source_message_id,
            run.workspace_id,
            run.context_version,
            run.profile_version,
            run.snapshot_hash,
            run.generated_text,
            json(&run.questions),
            json(&run.sources),
            json(&run.review_reasons),
            run.status,
            run.truncated as i64,
            run.provider,
            run.model,
            run.input_chars,
            run.estimated_input_tokens,
            run.actual_input_tokens,
            run.actual_output_tokens,
            run.created_at
        ],
    )?;
    c.execute("UPDATE assistant_draft_meta SET reply_run_id=?,status='needs_review',review_reasons=?,revision=revision+1,updated_at=? WHERE draft_id=?",params![run.id,json(&reasons),now(),id])?;
    let fresh = draft(c, id)?;
    Ok((fresh, run, false))
}
pub fn apply_reply(c: &Connection, id: i64, run_id: &str, expect: i64) -> AppResult<DraftRecord> {
    let d = revision(c, id, expect)?;
    let run = load_run(c, run_id)?.ok_or(AppError::NotFound)?;
    if run.draft_id != id
        || run.snapshot_hash != hash(&[&d.draft.subject, &d.draft.body_text, &d.intent])
    {
        return Err(AppError::BadInput(
            "生成候補が現在の下書きに一致しません".into(),
        ));
    };
    validate_run_sources(c, &d, &run)?;
    c.execute(
        "UPDATE drafts SET body_text=?,updated_at=? WHERE id=?",
        params![run.generated_text, now(), id],
    )?;
    c.execute("UPDATE assistant_draft_meta SET status='needs_review',revision=revision+1,updated_at=? WHERE draft_id=?",params![now(),id])?;
    draft(c, id)
}
/// Explicitly records the recipient/account/workspace combination that the
/// user reviewed. A warning acknowledgement never creates this record.
pub fn confirm_recipients(c: &Connection, id: i64, expect: i64) -> AppResult<DraftRecord> {
    let d = revision(c, id, expect)?;
    if d.status == "discarded"
        || d.status == "sending"
        || d.status == "sent"
        || d.status == "unknown"
    {
        return Err(AppError::BadInput(
            "この下書きの宛先は確認できません".into(),
        ));
    }
    let recipients = json(&(d.draft.to.clone(), d.draft.cc.clone(), d.draft.bcc.clone()));
    c.execute("INSERT INTO assistant_recipient_confirmations(draft_id,revision,recipients_hash,workspace_id,confirmed_at) VALUES(?,?,?,?,?) ON CONFLICT(draft_id) DO UPDATE SET revision=excluded.revision,recipients_hash=excluded.recipients_hash,workspace_id=excluded.workspace_id,confirmed_at=excluded.confirmed_at", params![id,d.revision,hash(&[&recipients]),d.workspace_id,now()])?;
    Ok(d)
}

pub fn prepare(c: &Connection, id: i64, expect: i64) -> AppResult<(String, Vec<String>, bool)> {
    let d = revision(c, id, expect)?;
    if let Some(run_id) = &d.reply_run_id {
        let run = load_run(c, run_id)?.ok_or(AppError::NotFound)?;
        validate_run_sources(c, &d, &run)?;
    }
    if d.status == "discarded" || d.status == "sending" || d.status == "sent" {
        return Err(AppError::BadInput("この下書きは送信準備できません".into()));
    };
    if d.workspace_id.is_some() {
        let recipients = json(&(d.draft.to.clone(), d.draft.cc.clone(), d.draft.bcc.clone()));
        let confirmed: Option<(String, Option<i64>)> = c.query_row("SELECT recipients_hash,workspace_id FROM assistant_recipient_confirmations WHERE draft_id=?", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        if confirmed != Some((hash(&[&recipients]), d.workspace_id)) {
            return Err(AppError::BadInput(
                "顧客情報を使う宛先・CC・BCCを確認してください".into(),
            ));
        }
    }
    // Re-check the source is still in this account. Deleted/moved source must
    // not silently turn into a different message or a sendable stale draft.
    if let Some(source) = d.source_message_id {
        if message_account(c, source)? != d.draft.account_id {
            return Err(AppError::BadInput("元メールの所有者が変わりました".into()));
        }
    }
    let context_version = d
        .workspace_id
        .and_then(|w| {
            c.query_row(
                "SELECT context_version FROM assistant_workspaces WHERE id=? AND account_id=?",
                params![w, d.draft.account_id],
                |r| r.get(0),
            )
            .ok()
        })
        .unwrap_or(0);
    let snap = hash(&[
        &d.draft.subject,
        &d.draft.body_text,
        &json(&d.draft.to),
        &json(&d.draft.cc),
        &json(&d.draft.bcc),
        &d.workspace_id.unwrap_or_default().to_string(),
        &d.source_message_id.unwrap_or_default().to_string(),
        &context_version.to_string(),
        &d.revision.to_string(),
    ]);
    let token = Uuid::new_v4().to_string();
    c.execute(
        "UPDATE assistant_send_attempts SET status='expired' WHERE draft_id=? AND status='prepared' AND expires_at<?",
        params![id, now()],
    )?;
    let live: Option<i64> = c.query_row("SELECT 1 FROM assistant_send_attempts WHERE draft_id=? AND status IN ('prepared','sending')", [id], |r| r.get(0)).optional()?;
    if live.is_some() {
        return Err(AppError::BadInput("この下書きは既に送信確認中です".into()));
    }
    c.execute("INSERT INTO assistant_send_attempts(token,draft_id,revision,snapshot_hash,expires_at,warnings) VALUES(?,?,?,?,?,?)",params![token,id,d.revision,snap,now()+600,json(&d.review_reasons)])?;
    Ok((
        token,
        d.review_reasons.clone(),
        !d.review_reasons.is_empty(),
    ))
}
pub fn claim_send(
    c: &Connection,
    token: &str,
    confirm: bool,
) -> AppResult<(i64, String, Vec<String>, bool)> {
    let row:Option<(i64,i64,String,i64,String,String)>=c.query_row("SELECT draft_id,revision,snapshot_hash,expires_at,status,warnings FROM assistant_send_attempts WHERE token=?",[token],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
    let (id, rev, snap, expiry, status, warnings) = row.ok_or(AppError::NotFound)?;
    if status != "prepared" {
        return Ok((id, status, vec(warnings), false));
    };
    if expiry < now() {
        c.execute(
            "UPDATE assistant_send_attempts SET status='expired' WHERE token=? AND status='prepared'",
            [token],
        )?;
        return Err(AppError::BadInput(
            "送信確認の有効期限が切れました。もう一度確認してください".into(),
        ));
    };
    let d = revision(c, id, rev)?;
    if let Some(run_id) = &d.reply_run_id {
        let run = load_run(c, run_id)?.ok_or(AppError::NotFound)?;
        validate_run_sources(c, &d, &run)?;
    }
    let context_version = d
        .workspace_id
        .and_then(|w| {
            c.query_row(
                "SELECT context_version FROM assistant_workspaces WHERE id=? AND account_id=?",
                params![w, d.draft.account_id],
                |r| r.get(0),
            )
            .ok()
        })
        .unwrap_or(0);
    let current = hash(&[
        &d.draft.subject,
        &d.draft.body_text,
        &json(&d.draft.to),
        &json(&d.draft.cc),
        &json(&d.draft.bcc),
        &d.workspace_id.unwrap_or_default().to_string(),
        &d.source_message_id.unwrap_or_default().to_string(),
        &context_version.to_string(),
        &d.revision.to_string(),
    ]);
    if current != snap {
        return Err(AppError::BadInput(
            "下書きが変更されたため送信確認をやり直してください".into(),
        ));
    };
    let ws = vec(warnings);
    if !ws.is_empty() && !confirm {
        return Err(AppError::BadInput("要確認事項への確認が必要です".into()));
    };
    c.execute(
        "UPDATE assistant_send_attempts SET status='sending' WHERE token=? AND status='prepared'",
        [token],
    )?;
    c.execute(
        "UPDATE assistant_draft_meta SET status='sending' WHERE draft_id=?",
        [id],
    )?;
    Ok((id, "sending".into(), ws, true))
}

pub fn send_draft_input(c: &Connection, draft_id: i64) -> AppResult<DraftInputOut> {
    Ok(draft(c, draft_id)?.draft)
}
pub fn finish_send(
    c: &Connection,
    token: &str,
    draft_id: i64,
    state: &str,
    message_id: Option<&str>,
) -> AppResult<()> {
    if !["sent", "unknown"].contains(&state) {
        return Err(AppError::BadInput("送信完了状態が不正です".into()));
    }
    let tx = c.unchecked_transaction()?;
    if tx.execute(
        "UPDATE assistant_send_attempts SET status=?,message_id=? WHERE token=? AND draft_id=? AND status='sending'",
        params![state, message_id, token, draft_id],
    )? != 1 {
        return Err(AppError::BadInput("送信試行の状態が一致しません".into()));
    }
    tx.execute(
        "UPDATE assistant_draft_meta SET status=?,updated_at=? WHERE draft_id=?",
        params![state, now(), draft_id],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::Provider;
    use crate::store::insert_account;
    use crate::store::migrations;
    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        migrations::apply(&c).unwrap();
        insert_account(&c, "a@example.test", Provider::Gmail, "A").unwrap();
        c
    }
    fn input() -> DraftInput {
        DraftInput {
            id: None,
            account_id: 1,
            to: vec!["b@example.test".into()],
            cc: vec![],
            bcc: vec![],
            subject: "件名".into(),
            body_text: "本文".into(),
            body_html: None,
            in_reply_to: None,
            references: vec![],
        }
    }
    fn message(
        c: &Connection,
        uid: i64,
        message_id: &str,
        in_reply_to: Option<&str>,
        references: &str,
        subject: &str,
        body: &str,
    ) -> i64 {
        c.execute(
            "INSERT OR IGNORE INTO folders(account_id,name,imap_path,role) VALUES(1,'Inbox','INBOX','inbox')",
            [],
        )
        .unwrap();
        let folder: i64 = c
            .query_row("SELECT id FROM folders WHERE account_id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        c.execute(
            "INSERT INTO messages(account_id,folder_id,uid,message_id,in_reply_to,reference_ids,from_addr,subject,date,body_fetched,body_text) VALUES(1,?,?,?,?,?,'sender@example.test',?,?,1,?)",
            params![folder, uid, message_id, in_reply_to, references, subject, uid, body],
        )
        .unwrap();
        c.last_insert_rowid()
    }
    fn project_run(d: &DraftRecord, project_id: i64, path: &str, version: &str) -> ReplyRun {
        ReplyRun {
            id: format!("run-{}-{project_id}", d.draft.id),
            draft_id: d.draft.id,
            source_message_id: d.source_message_id,
            workspace_id: d.workspace_id,
            context_version: 1,
            profile_version: 1,
            snapshot_hash: hash(&[&d.draft.subject, &d.draft.body_text, &d.intent]),
            generated_text: "返信候補".into(),
            questions: vec![],
            sources: vec![SourceCard {
                id: format!("project:{project_id}:{path}"),
                kind: "project_file".into(),
                label: path.into(),
                relative_path: Some(path.into()),
                locator: None,
                snippet: "架空資料".into(),
                version: version.into(),
                workspace_id: d.workspace_id,
                fact_status: "confirmed".into(),
            }],
            review_reasons: vec![],
            status: "needs_review".into(),
            truncated: false,
            provider: "test".into(),
            model: "test".into(),
            input_chars: 0,
            estimated_input_tokens: 0,
            actual_input_tokens: None,
            actual_output_tokens: None,
            created_at: 0,
        }
    }
    #[test]
    fn session_is_idempotent_and_stale_save_is_rejected() {
        let c = db();
        let a = save_draft(&c, &input(), "s", None, None, None, "", None).unwrap();
        let mut i = input();
        i.body_text = "変更".into();
        i.id = Some(a.draft.id);
        let b = save_draft(&c, &i, "s", Some(a.revision), None, None, "", None).unwrap();
        assert_eq!(a.draft.id, b.draft.id);
        assert!(save_draft(&c, &i, "s", Some(a.revision), None, None, "", None).is_err());
    }
    #[test]
    fn token_cannot_be_claimed_twice() {
        let c = db();
        let a = save_draft(&c, &input(), "s", None, None, None, "", None).unwrap();
        let (t, _, _) = prepare(&c, a.draft.id, a.revision).unwrap();
        assert!(claim_send(&c, &t, true).unwrap().3);
        assert!(!claim_send(&c, &t, true).unwrap().3);
    }
    #[test]
    fn structured_snapshot_and_live_prepare_are_not_ambiguous() {
        assert_ne!(hash(&["ab", "c"]), hash(&["a", "bc"]));
        let c = db();
        let d = save_draft(&c, &input(), "s", None, None, None, "", None).unwrap();
        prepare(&c, d.draft.id, d.revision).unwrap();
        assert!(prepare(&c, d.draft.id, d.revision).is_err());
    }

    #[test]
    fn rfc_headers_inherit_only_as_review_candidate_and_subject_never_merges() {
        let c = db();
        let workspace = create_workspace(&c, 1, "顧客A", "案件A").unwrap();
        let parent = message(&c, 1, "<parent@test>", None, "", "同じ件名", "A");
        bind_context(&c, parent, Some(workspace.id)).unwrap();
        let reply = message(
            &c,
            2,
            "<reply@test>",
            Some("<parent@test>"),
            "<parent@test>",
            "同じ件名",
            "返信",
        );
        let inherited = get_context(&c, reply).unwrap();
        assert_eq!(inherited.workspace_id, Some(workspace.id));
        assert_eq!(inherited.thread_id, parent);
        assert_eq!(inherited.status, "needs_review");

        let unrelated = message(&c, 3, "<unrelated@test>", None, "", "同じ件名", "別件");
        let context = get_context(&c, unrelated).unwrap();
        assert_eq!(context.status, "unassigned");
        assert_eq!(context.workspace_id, None);
        assert_eq!(context.thread_id, unrelated);
    }

    #[test]
    fn conflicting_rfc_references_never_choose_a_customer() {
        let c = db();
        let a = create_workspace(&c, 1, "顧客A", "案件").unwrap();
        let b = create_workspace(&c, 1, "顧客B", "案件").unwrap();
        let ma = message(&c, 1, "<a@test>", None, "", "A", "A");
        let mb = message(&c, 2, "<b@test>", None, "", "B", "B");
        bind_context(&c, ma, Some(a.id)).unwrap();
        bind_context(&c, mb, Some(b.id)).unwrap();
        let mixed = message(
            &c,
            3,
            "<mixed@test>",
            Some("<a@test>"),
            "<a@test> <b@test>",
            "混在",
            "混在",
        );
        let context = get_context(&c, mixed).unwrap();
        assert_eq!(context.status, "conflict");
        assert_eq!(context.workspace_id, None);
    }

    #[test]
    fn workspace_project_scopes_isolate_customer_a_and_b() {
        let c = db();
        let a = create_workspace(&c, 1, "顧客A", "案件").unwrap();
        let b = create_workspace(&c, 1, "顧客B", "案件").unwrap();
        let root = std::env::temp_dir().join(format!("a-mail-scope-{}", Uuid::new_v4()));
        let root_a = root.join("a");
        let root_b = root.join("b");
        std::fs::create_dir_all(&root_a).unwrap();
        std::fs::create_dir_all(&root_b).unwrap();
        std::fs::write(root_a.join("facts.txt"), "顧客Aの架空情報").unwrap();
        std::fs::write(root_b.join("facts.txt"), "顧客Bの架空情報").unwrap();
        let pa = ai_projects::add(&c, "A", &std::fs::canonicalize(&root_a).unwrap()).unwrap();
        let pb = ai_projects::add(&c, "B", &std::fs::canonicalize(&root_b).unwrap()).unwrap();
        set_scope(
            &c,
            &ProjectScope {
                account_id: 1,
                project_id: pa.id,
                workspace_id: Some(a.id),
                scope: "workspace".into(),
                confirmed: true,
            },
        )
        .unwrap();
        set_scope(
            &c,
            &ProjectScope {
                account_id: 1,
                project_id: pb.id,
                workspace_id: Some(b.id),
                scope: "workspace".into(),
                confirmed: true,
            },
        )
        .unwrap();
        let d = save_draft(&c, &input(), "scope", None, None, Some(a.id), "", None).unwrap();
        c.execute(
            "INSERT INTO assistant_recipient_confirmations(draft_id,revision,recipients_hash,workspace_id,confirmed_at) VALUES(?,?,?,?,0)",
            params![d.draft.id, d.revision, hash(&[&json(&(d.draft.to.clone(), d.draft.cc.clone(), d.draft.bcc.clone()))]), a.id],
        )
        .unwrap();
        let va = ai_projects::read_project_source(&pa, "facts.txt")
            .unwrap()
            .version;
        let vb = ai_projects::read_project_source(&pb, "facts.txt")
            .unwrap()
            .version;
        assert!(validate_run_sources(&c, &d, &project_run(&d, pa.id, "facts.txt", &va)).is_ok());
        assert!(validate_run_sources(&c, &d, &project_run(&d, pb.id, "facts.txt", &vb)).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_source_rejects_applying_generated_candidate() {
        let c = db();
        let source = message(&c, 1, "<source@test>", None, "", "質問", "変更前");
        let d = save_draft(&c, &input(), "source", None, Some(source), None, "", None).unwrap();
        let (generated, run, _) = generate(&c, d.draft.id, d.revision).unwrap();
        c.execute(
            "UPDATE messages SET body_text='変更後' WHERE id=?",
            [source],
        )
        .unwrap();
        assert!(apply_reply(&c, generated.draft.id, &run.id, generated.revision).is_err());
    }

    #[test]
    fn edited_draft_rejects_old_candidate_even_at_current_revision() {
        let c = db();
        let d = save_draft(&c, &input(), "edit", None, None, None, "", None).unwrap();
        let (generated, run, _) = generate(&c, d.draft.id, d.revision).unwrap();
        let mut edited = input();
        edited.id = Some(d.draft.id);
        edited.body_text = "本人が編集した本文".into();
        let current = save_draft(
            &c,
            &edited,
            "edit",
            Some(generated.revision),
            None,
            None,
            "",
            Some(run.id.clone()),
        )
        .unwrap();
        assert!(apply_reply(&c, current.draft.id, &run.id, current.revision).is_err());
    }

    #[test]
    fn generated_run_records_provider_usage_when_available() {
        let c = db();
        let d = save_draft(&c, &input(), "usage", None, None, None, "", None).unwrap();
        let (_, run, _) = generate(&c, d.draft.id, d.revision).unwrap();
        let updated = replace_generated_run(
            &c,
            &run.id,
            d.draft.id,
            "架空の返信候補",
            &run.questions,
            &run.sources,
            &run.review_reasons,
            120,
            false,
            "codex_subscription",
            "test-model",
            Some(42),
            Some(9),
        )
        .unwrap();
        let saved = updated.reply.unwrap();
        assert_eq!(saved.actual_input_tokens, Some(42));
        assert_eq!(saved.actual_output_tokens, Some(9));
        assert_eq!(saved.provider, "codex_subscription");
    }

    #[test]
    fn recipient_confirmation_survives_body_edit_but_not_recipient_change() {
        let c = db();
        let workspace = create_workspace(&c, 1, "顧客A", "案件").unwrap();
        let d = save_draft(
            &c,
            &input(),
            "confirm",
            None,
            None,
            Some(workspace.id),
            "",
            None,
        )
        .unwrap();
        confirm_recipients(&c, d.draft.id, d.revision).unwrap();
        let mut body_edit = input();
        body_edit.id = Some(d.draft.id);
        body_edit.body_text = "本人が確認後に編集した本文".into();
        let edited = save_draft(
            &c,
            &body_edit,
            "confirm",
            Some(d.revision),
            None,
            Some(workspace.id),
            "",
            None,
        )
        .unwrap();
        let (token, _, _) = prepare(&c, edited.draft.id, edited.revision).unwrap();

        let mut recipient_edit = body_edit;
        recipient_edit.to = vec!["other@example.test".into()];
        let changed = save_draft(
            &c,
            &recipient_edit,
            "confirm",
            Some(edited.revision),
            None,
            Some(workspace.id),
            "",
            None,
        )
        .unwrap();
        assert!(claim_send(&c, &token, true).is_err());
        assert!(prepare(&c, changed.draft.id, changed.revision).is_err());
    }
}
