//! Database-only state machine for automatic draft candidates.
//!
//! Provider calls, SMTP, and sending are intentionally outside this module.  A
//! `running` job is never retried: process-start recovery marks it `unknown`.

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{
    error::{AppError, AppResult},
    store::models::DraftRecord,
};

/// V6 migration, to be registered by the integrating owner after V5.
pub const SCHEMA: &str = r#"
ALTER TABLE assistant_settings ADD COLUMN auto_draft_initialized INTEGER NOT NULL DEFAULT 0;
CREATE TABLE assistant_auto_candidates (
  source_message_id INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  rfc_identity TEXT NOT NULL,
  observed_at INTEGER NOT NULL,
  UNIQUE(account_id, rfc_identity)
);
ALTER TABLE assistant_generation_jobs ADD COLUMN source_message_id INTEGER REFERENCES messages(id) ON DELETE SET NULL;
ALTER TABLE assistant_generation_jobs ADD COLUMN rfc_identity TEXT;
ALTER TABLE assistant_generation_jobs ADD COLUMN quota_day TEXT NOT NULL DEFAULT '';
ALTER TABLE assistant_generation_jobs ADD COLUMN claimed_at INTEGER;
ALTER TABLE assistant_generation_jobs ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;
CREATE UNIQUE INDEX idx_assistant_jobs_account_identity
  ON assistant_generation_jobs(account_id, rfc_identity);
CREATE INDEX idx_assistant_jobs_claim ON assistant_generation_jobs(account_id, status, created_at);
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: i64,
    pub account_id: i64,
    pub source_message_id: i64,
    pub rfc_identity: String,
    pub status: String,
    pub quota_day: String,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn utc_day() -> String {
    // Match the existing assistant_settings representation.  It is an explicit
    // UTC Unix-day number, not a locale-dependent calendar date.
    now().div_euclid(86_400).to_string()
}

fn rfc_identity(message_id: &str, from: &str, date: i64, subject: &str) -> String {
    let bytes = serde_json::to_vec(&(message_id, from, date, subject)).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

fn ensure_settings(c: &Connection, account: i64) -> AppResult<()> {
    let exists: Option<i64> = c
        .query_row("SELECT id FROM accounts WHERE id=?", [account], |row| {
            row.get(0)
        })
        .optional()?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    c.execute(
        "INSERT OR IGNORE INTO assistant_settings(account_id,quota_day) VALUES(?,?)",
        params![account, utc_day()],
    )?;
    Ok(())
}

/// Changes only the automatic-draft opt-in.  On first enable, all mail already
/// present receives a high-watermark and can never become an automatic candidate.
pub fn set_auto_draft_enabled(c: &Connection, account: i64, enabled: bool) -> AppResult<()> {
    ensure_settings(c, account)?;
    let tx = c.unchecked_transaction()?;
    let (was_enabled, _initialized): (i64, i64) = tx.query_row(
        "SELECT auto_draft_enabled,auto_draft_initialized FROM assistant_settings WHERE account_id=?",
        [account],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    // Every off -> on transition starts from the rows visible at that moment.
    // This prevents mail accumulated while off from becoming an implicit backfill.
    if enabled && was_enabled == 0 {
        let high_watermark: i64 = tx.query_row(
            "SELECT COALESCE(MAX(id),0) FROM messages WHERE account_id=?",
            [account],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE assistant_settings SET auto_draft_enabled=1, auto_draft_initialized=1, high_watermark=? WHERE account_id=?",
            params![high_watermark, account],
        )?;
    } else {
        tx.execute(
            "UPDATE assistant_settings SET auto_draft_enabled=? WHERE account_id=?",
            params![enabled as i64, account],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Records new inbox rows beyond the opt-in watermark.  Rows without a body are
/// kept as candidates and become jobs only after a later call finds body_fetched=1.
pub fn enqueue_new(c: &Connection, account: i64) -> AppResult<usize> {
    ensure_settings(c, account)?;
    let tx = c.unchecked_transaction()?;
    let setting: (i64, i64, Option<i64>) = tx.query_row(
        "SELECT auto_draft_enabled,auto_draft_initialized,high_watermark FROM assistant_settings WHERE account_id=?",
        [account],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if setting.0 == 0 || setting.1 == 0 {
        tx.commit()?;
        return Ok(0);
    }
    let high = setting.2.unwrap_or(0);
    let mut statement = tx.prepare(
        "SELECT m.id,COALESCE(m.message_id,''),m.from_addr,m.date,m.subject
           FROM messages m JOIN folders f ON f.id=m.folder_id
          WHERE m.account_id=? AND m.id>? AND f.role='inbox'",
    )?;
    let rows = statement
        .query_map(params![account, high], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for (source, message_id, from, date, subject) in rows {
        // Message-ID alone is not globally unique in real mail.  The stable
        // envelope fingerprint dedupes mirrored copies while letting a later
        // re-use of the same RFC ID be a distinct message.
        let identity = rfc_identity(&message_id, &from, date, &subject);
        tx.execute(
            "INSERT OR IGNORE INTO assistant_auto_candidates(source_message_id,account_id,rfc_identity,observed_at) VALUES(?,?,?,?)",
            params![source, account, identity, now()],
        )?;
    }
    let made = tx.execute(
        "INSERT OR IGNORE INTO assistant_generation_jobs(account_id,dedupe_key,status,created_at,source_message_id,rfc_identity,quota_day,attempts)
         SELECT ac.account_id, 'auto:' || ac.account_id || ':' || ac.rfc_identity, 'pending', ?, ac.source_message_id, ac.rfc_identity, '', 0
           FROM assistant_auto_candidates ac JOIN messages m ON m.id=ac.source_message_id
          WHERE ac.account_id=? AND m.body_fetched=1",
        params![now(), account],
    )?;
    tx.commit()?;
    Ok(made)
}

/// Atomically reserves at most one queued job.  The quota is reset by UTC day
/// immediately before the reservation, so an interrupted job still consumes its
/// daily attempt and cannot silently be re-run.
pub fn claim_next(c: &Connection, account: i64) -> AppResult<Option<Job>> {
    ensure_settings(c, account)?;
    let tx = c.unchecked_transaction()?;
    let (enabled, initialized, limit, quota_day, used): (i64, i64, i64, String, i64) = tx.query_row(
        "SELECT auto_draft_enabled,auto_draft_initialized,daily_generation_limit,quota_day,auto_generated_today FROM assistant_settings WHERE account_id=?",
        [account],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )?;
    if enabled == 0 || initialized == 0 {
        tx.commit()?;
        return Ok(None);
    }
    if !(1..=100).contains(&limit) {
        return Err(AppError::BadInput("日次上限は1〜100件です".into()));
    }
    let today = utc_day();
    let used = if quota_day == today {
        used
    } else {
        tx.execute(
            "UPDATE assistant_settings SET quota_day=?,auto_generated_today=0 WHERE account_id=?",
            params![today, account],
        )?;
        0
    };
    if used >= limit {
        tx.commit()?;
        return Ok(None);
    }
    // One application has one automatic generation at a time.  A recovered
    // `unknown` job intentionally blocks no slot but is never made pending again.
    let running: i64 = tx.query_row(
        "SELECT COUNT(*) FROM assistant_generation_jobs WHERE status='running'",
        [],
        |row| row.get(0),
    )?;
    if running != 0 {
        tx.commit()?;
        return Ok(None);
    }
    let next: Option<Job> = tx
        .query_row(
            "SELECT id,account_id,source_message_id,rfc_identity,status,quota_day
           FROM assistant_generation_jobs WHERE account_id=? AND status='pending'
           ORDER BY created_at,id LIMIT 1",
            [account],
            |row| {
                Ok(Job {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    source_message_id: row.get(2)?,
                    rfc_identity: row.get(3)?,
                    status: row.get(4)?,
                    quota_day: row.get(5)?,
                })
            },
        )
        .optional()?;
    let Some(mut job) = next else {
        tx.commit()?;
        return Ok(None);
    };
    if tx.execute(
        "UPDATE assistant_generation_jobs SET status='running',claimed_at=?,attempts=attempts+1,quota_day=? WHERE id=? AND status='pending'",
        params![now(), today, job.id],
    )? != 1 {
        return Err(AppError::BadInput("自動下書きジョブの予約に失敗しました".into()));
    }
    tx.execute(
        "UPDATE assistant_settings SET auto_generated_today=auto_generated_today+1 WHERE account_id=?",
        [account],
    )?;
    job.status = "running".into();
    job.quota_day = today;
    tx.commit()?;
    Ok(Some(job))
}

/// Completes a claimed job.  Failed and unknown jobs are terminal in P0.
pub fn finish_job(c: &Connection, id: i64, status: &str) -> AppResult<()> {
    if !["completed", "failed", "unknown"].contains(&status) {
        return Err(AppError::BadInput("ジョブ完了状態が不正です".into()));
    }
    if c.execute(
        "UPDATE assistant_generation_jobs SET status=? WHERE id=? AND status='running'",
        params![status, id],
    )? != 1
    {
        return Err(AppError::BadInput(
            "実行中でないジョブは完了できません".into(),
        ));
    }
    Ok(())
}

/// Call during application startup, before queue processing.  There is no retry
/// because a crashed process may have reached an external provider.
pub fn recover_inflight(c: &Connection, account: i64) -> AppResult<usize> {
    ensure_settings(c, account)?;
    Ok(c.execute(
        "UPDATE assistant_generation_jobs SET status='unknown' WHERE account_id=? AND status='running'",
        [account],
    )?)
}

/// Includes body-not-yet-fetched new candidates so the UI can explain waiting.
pub fn pending_count(c: &Connection, account: i64) -> AppResult<i64> {
    ensure_settings(c, account)?;
    c.query_row(
        "SELECT
          (SELECT COUNT(*) FROM assistant_auto_candidates ac
            WHERE ac.account_id=? AND NOT EXISTS (
              SELECT 1 FROM assistant_generation_jobs j WHERE j.source_message_id=ac.source_message_id
            ))
          +
          (SELECT COUNT(*) FROM assistant_generation_jobs WHERE account_id=? AND status='pending')",
        params![account, account],
        |row| row.get::<_, i64>(0),
    ).map_err(Into::into)
}

/// Legacy compatibility shim. Continuous post-send learning is intentionally
/// disabled; only `style_profile::start_learning` may create a new profile.
pub fn record_sent_style_candidates(_c: &Connection, _draft: &DraftRecord) -> AppResult<usize> {
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::Provider,
        store::models::{DraftInputOut, ReplyRun},
        store::{insert_account, migrations},
    };

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        migrations::apply(&c).unwrap();
        insert_account(&c, "one@example.test", Provider::Gmail, "One").unwrap();
        insert_account(&c, "two@example.test", Provider::Gmail, "Two").unwrap();
        for account in 1..=2 {
            c.execute(
                "INSERT INTO folders(account_id,name,imap_path,role) VALUES(?,?,?, 'inbox')",
                params![account, "Inbox", format!("INBOX-{account}")],
            )
            .unwrap();
        }
        c
    }

    fn message(c: &Connection, account: i64, uid: i64, body_fetched: bool) -> i64 {
        let folder: i64 = c
            .query_row(
                "SELECT id FROM folders WHERE account_id=?",
                [account],
                |row| row.get(0),
            )
            .unwrap();
        c.execute(
            "INSERT INTO messages(account_id,folder_id,uid,message_id,from_addr,subject,date,body_fetched,body_text)
             VALUES(?,?,?,?,?,?,?,?,?)",
            params![account, folder, uid, format!("<m-{account}-{uid}@test>"), "sender@test", "質問", uid, body_fetched as i64, if body_fetched { Some("本文") } else { None::<&str> }],
        ).unwrap();
        c.last_insert_rowid()
    }

    fn sent_record(account: i64, workspace: Option<i64>) -> DraftRecord {
        DraftRecord {
            draft: DraftInputOut {
                id: 9,
                account_id: account,
                to: vec![],
                cc: vec![],
                bcc: vec![],
                subject: "s".into(),
                body_text: "よろしくお願いします。".into(),
                body_html: None,
                in_reply_to: None,
                references: vec![],
            },
            session_id: "session".into(),
            revision: 1,
            source_message_id: Some(3),
            workspace_id: workspace,
            intent: "".into(),
            reply_run_id: Some("run".into()),
            status: "sent".into(),
            review_reasons: vec![],
            updated_at: 0,
            reply: Some(ReplyRun {
                id: "run".into(),
                draft_id: 9,
                source_message_id: Some(3),
                workspace_id: workspace,
                context_version: 0,
                profile_version: 1,
                snapshot_hash: "hash".into(),
                generated_text: "何卒よろしくお願い申し上げます。".into(),
                questions: vec![],
                sources: vec![],
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
            }),
        }
    }

    #[test]
    fn default_off_and_first_enable_exclude_existing_mail() {
        let c = db();
        message(&c, 1, 1, true);
        assert_eq!(enqueue_new(&c, 1).unwrap(), 0);
        set_auto_draft_enabled(&c, 1, true).unwrap();
        assert_eq!(enqueue_new(&c, 1).unwrap(), 0);
        message(&c, 1, 2, true);
        assert_eq!(enqueue_new(&c, 1).unwrap(), 1);
        assert!(claim_next(&c, 1).unwrap().is_some());
    }

    #[test]
    fn unfetched_new_mail_waits_then_is_enqueued_once() {
        let c = db();
        set_auto_draft_enabled(&c, 1, true).unwrap();
        let id = message(&c, 1, 2, false);
        assert_eq!(enqueue_new(&c, 1).unwrap(), 0);
        assert_eq!(pending_count(&c, 1).unwrap(), 1);
        c.execute(
            "UPDATE messages SET body_fetched=1,body_text='本文' WHERE id=?",
            [id],
        )
        .unwrap();
        assert_eq!(enqueue_new(&c, 1).unwrap(), 1);
        assert_eq!(enqueue_new(&c, 1).unwrap(), 0);
    }

    #[test]
    fn quota_is_per_account_and_failed_job_is_not_retried() {
        let c = db();
        set_auto_draft_enabled(&c, 1, true).unwrap();
        set_auto_draft_enabled(&c, 2, true).unwrap();
        c.execute(
            "UPDATE assistant_settings SET daily_generation_limit=1 WHERE account_id=1",
            [],
        )
        .unwrap();
        message(&c, 1, 1, true);
        message(&c, 1, 2, true);
        message(&c, 2, 1, true);
        enqueue_new(&c, 1).unwrap();
        enqueue_new(&c, 2).unwrap();
        let first = claim_next(&c, 1).unwrap().unwrap();
        // A running automatic generation occupies the one app-wide slot.
        assert!(claim_next(&c, 2).unwrap().is_none());
        finish_job(&c, first.id, "failed").unwrap();
        assert!(claim_next(&c, 1).unwrap().is_none());
        assert!(claim_next(&c, 2).unwrap().is_some());
        assert!(claim_next(&c, 1).unwrap().is_none());
    }

    #[test]
    fn rfc_identity_is_account_scoped_and_differentiates_reuse() {
        let c = db();
        set_auto_draft_enabled(&c, 1, true).unwrap();
        set_auto_draft_enabled(&c, 2, true).unwrap();
        let one_a = message(&c, 1, 1, true);
        let one_b = message(&c, 1, 2, true);
        let two = message(&c, 2, 1, true);
        c.execute(
            "UPDATE messages SET message_id='<same@test>' WHERE id IN (?,?,?)",
            params![one_a, one_b, two],
        )
        .unwrap();
        // A reused RFC ID with a changed envelope is a new message; the same ID
        // in another account is also independent.
        c.execute(
            "UPDATE messages SET from_addr='other@test' WHERE id=?",
            [one_b],
        )
        .unwrap();
        assert_eq!(enqueue_new(&c, 1).unwrap(), 2);
        assert_eq!(enqueue_new(&c, 2).unwrap(), 1);
    }

    #[test]
    fn reused_local_source_id_does_not_reuse_old_job_identity() {
        let c = db();
        set_auto_draft_enabled(&c, 1, true).unwrap();
        let old = message(&c, 1, 1, true);
        enqueue_new(&c, 1).unwrap();
        c.execute("DELETE FROM messages WHERE id=?", [old]).unwrap();
        let replacement = message(&c, 1, 2, true);
        assert_eq!(replacement, old, "SQLite may reuse a deleted rowid");
        assert_eq!(enqueue_new(&c, 1).unwrap(), 1);
    }

    #[test]
    fn recovery_marks_running_unknown_without_requeueing() {
        let c = db();
        set_auto_draft_enabled(&c, 1, true).unwrap();
        message(&c, 1, 1, true);
        enqueue_new(&c, 1).unwrap();
        claim_next(&c, 1).unwrap().unwrap();
        assert_eq!(recover_inflight(&c, 1).unwrap(), 1);
        assert!(claim_next(&c, 1).unwrap().is_none());
    }

    #[test]
    fn sending_never_creates_style_learning_candidates() {
        let c = db();
        let one = sent_record(1, None);
        assert_eq!(record_sent_style_candidates(&c, &one).unwrap(), 0);
        set_auto_draft_enabled(&c, 1, false).unwrap();
        c.execute(
            "UPDATE assistant_settings SET learning_enabled=1 WHERE account_id=1",
            [],
        )
        .unwrap();
        assert_eq!(record_sent_style_candidates(&c, &one).unwrap(), 0);
        let rules: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM assistant_style_rules WHERE account_id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rules, 0);
        let two = sent_record(2, None);
        assert_eq!(record_sent_style_candidates(&c, &two).unwrap(), 0);
        let second: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM assistant_style_rules WHERE account_id=2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(second, 0);
    }
}
