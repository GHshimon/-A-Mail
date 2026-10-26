//! On-demand, account-scoped writing-style profiles.
//!
//! The selected sent messages are read from the normal mail cache and live only
//! in local variables while this module aggregates structural statistics. No raw
//! body, message ID list, model chunk, or example corpus is persisted here.

use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension, Row};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    store::models::{
        StyleLearningExclusions, StyleLearningPreview, StyleLearningResult, StyleParagraphs,
        StylePattern, StylePhraseGroups, StyleProfile, StyleProfileContent, StyleProfileJob,
        StyleProfilePreviewText, StyleRange, StyleSignature,
    },
};

pub const CONSENT_VERSION: &str = "style-profile-v1";
const SCAN_LIMIT: i64 = 400;
const MIN_AUTHORED_CHARS: usize = 12;
const CONTENT_LIMIT: usize = 20_000;

#[derive(Debug, Clone)]
struct AuthoredMessage {
    id: i64,
    date: i64,
    text: String,
    signature_detected: bool,
}

#[derive(Debug)]
struct Collected {
    account_id: i64,
    requested_count: i64,
    candidate_count: i64,
    eligible_count: i64,
    messages: Vec<AuthoredMessage>,
    exclusions: StyleLearningExclusions,
    source_range_start: Option<i64>,
    source_range_end: Option<i64>,
    source_set_hash: String,
    preview_hash: String,
}

#[derive(Debug)]
struct RawProfile {
    id: i64,
    account_id: i64,
    workspace_id: Option<i64>,
    version: i64,
    status: String,
    source_count: i64,
    source_candidate_count: i64,
    source_range_start: Option<i64>,
    source_range_end: Option<i64>,
    source_set_hash: String,
    content_json: String,
    provider: String,
    model: String,
    input_chars: i64,
    estimated_input_tokens: i64,
    actual_input_tokens: Option<i64>,
    actual_output_tokens: Option<i64>,
    consent_version: String,
    created_at: i64,
    updated_at: i64,
    activated_at: Option<i64>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn validate_requested_count(value: i64) -> AppResult<()> {
    if matches!(value, 100 | 200) {
        Ok(())
    } else {
        Err(AppError::BadInput(
            "文体学習の対象は直近100件または200件です".into(),
        ))
    }
}

fn account_email(conn: &Connection, account_id: i64) -> AppResult<String> {
    conn.query_row(
        "SELECT email FROM accounts WHERE id=?",
        [account_id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or(AppError::NotFound)
}

fn sender_matches(from: &str, account_email: &str) -> bool {
    let expected = account_email.trim().to_ascii_lowercase();
    let value = from.trim().to_ascii_lowercase();
    if value == expected {
        return true;
    }
    value
        .split_once('<')
        .and_then(|(_, rest)| rest.split_once('>'))
        .is_some_and(|(address, _)| address.trim() == expected)
}

fn is_quote_boundary(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("-----original message-----")
        || lower.starts_with("---------- forwarded message")
        || lower.starts_with("from:")
        || lower.starts_with("sent:")
        || lower.starts_with("to:")
        || lower.starts_with("subject:")
        || lower.contains(" wrote:")
        || line.starts_with("差出人:")
        || line.starts_with("差出人：")
        || line.starts_with("送信日時:")
        || line.starts_with("送信日時：")
        || line.starts_with("転送メッセージ")
        || line.starts_with("転送されたメッセージ")
}

fn looks_like_signature_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains('@')
        || lower.starts_with("tel")
        || lower.starts_with("phone")
        || lower.starts_with("mobile")
        || line.starts_with('〒')
}

fn strip_authored_text(raw: &str) -> (String, bool) {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = Vec::new();
    let mut signature_detected = false;
    for raw_line in normalized.lines() {
        let trimmed = raw_line.trim();
        if (trimmed == "--" || trimmed == "-- " || trimmed == "___") && !lines.is_empty() {
            signature_detected = true;
            break;
        }
        if is_quote_boundary(trimmed) && !lines.is_empty() {
            break;
        }
        if trimmed.starts_with('>') {
            continue;
        }
        lines.push(raw_line.trim_end().to_owned());
    }

    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }

    // Common signatures without the RFC delimiter are removed from the first
    // obvious contact line onward. Their contents never become profile text.
    let tail_start = lines.len().saturating_sub(6);
    if let Some(index) =
        (tail_start..lines.len()).find(|index| looks_like_signature_line(&lines[*index]))
    {
        signature_detected = true;
        lines.truncate(index);
        while lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.pop();
        }
    }

    (lines.join("\n"), signature_detected)
}

fn collect(conn: &Connection, account_id: i64, requested_count: i64) -> AppResult<Collected> {
    validate_requested_count(requested_count)?;
    let email = account_email(conn, account_id)?;
    let mut statement = conn.prepare(
        "SELECT m.id,m.date,m.from_addr,m.body_fetched,m.body_text
         FROM messages m
         JOIN folders f ON f.id=m.folder_id AND f.account_id=m.account_id
         WHERE m.account_id=? AND f.role='sent'
         ORDER BY m.date DESC,m.id DESC
         LIMIT ?",
    )?;
    let rows = statement.query_map(params![account_id, SCAN_LIMIT], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut exclusions = StyleLearningExclusions::default();
    let mut eligible = Vec::new();
    let mut seen = HashSet::new();
    let mut candidate_count = 0_i64;
    for row in rows {
        let (id, date, from, body_fetched, body) = row?;
        candidate_count += 1;
        if body_fetched == 0 || body.as_deref().is_none_or(str::is_empty) {
            exclusions.body_not_cached += 1;
            continue;
        }
        if !sender_matches(&from, &email) {
            exclusions.sender_mismatch += 1;
            continue;
        }
        let (text, signature_detected) = strip_authored_text(body.as_deref().unwrap_or_default());
        let authored_chars = text
            .chars()
            .filter(|character| !character.is_whitespace())
            .count();
        if authored_chars == 0 {
            exclusions.quoted_or_empty += 1;
            continue;
        }
        if authored_chars < MIN_AUTHORED_CHARS {
            exclusions.too_short += 1;
            continue;
        }
        let body_hash = format!("{:x}", Sha256::digest(text.as_bytes()));
        if !seen.insert(body_hash) {
            exclusions.duplicate += 1;
            continue;
        }
        eligible.push(AuthoredMessage {
            id,
            date,
            text,
            signature_detected,
        });
    }

    let eligible_count = eligible.len() as i64;
    eligible.truncate(requested_count as usize);
    let source_range_start = eligible.iter().map(|message| message.date).min();
    let source_range_end = eligible.iter().map(|message| message.date).max();
    let mut source_hasher = Sha256::new();
    for message in &eligible {
        source_hasher.update(message.id.to_le_bytes());
        source_hasher.update(message.date.to_le_bytes());
        source_hasher.update(Sha256::digest(message.text.as_bytes()));
    }
    let source_set_hash = format!("{:x}", source_hasher.finalize());
    let mut preview_hasher = Sha256::new();
    preview_hasher.update(account_id.to_le_bytes());
    preview_hasher.update(requested_count.to_le_bytes());
    preview_hasher.update(candidate_count.to_le_bytes());
    preview_hasher.update(eligible_count.to_le_bytes());
    preview_hasher.update(source_set_hash.as_bytes());
    let preview_hash = format!("{:x}", preview_hasher.finalize());

    Ok(Collected {
        account_id,
        requested_count,
        candidate_count,
        eligible_count,
        messages: eligible,
        exclusions,
        source_range_start,
        source_range_end,
        source_set_hash,
        preview_hash,
    })
}

fn as_preview(collected: &Collected) -> StyleLearningPreview {
    StyleLearningPreview {
        account_id: collected.account_id,
        requested_count: collected.requested_count,
        candidate_count: collected.candidate_count,
        eligible_count: collected.eligible_count,
        selected_count: collected.messages.len() as i64,
        source_range_start: collected.source_range_start,
        source_range_end: collected.source_range_end,
        exclusions: collected.exclusions.clone(),
        locally_aggregated_count: collected.messages.len() as i64,
        ai_sample_count: 0,
        ai_input_chars: 0,
        preview_hash: collected.preview_hash.clone(),
        consent_version: CONSENT_VERSION.into(),
        notice: "Phase Aでは外部AIへ送信せず、このMac内で文章構造と安全な定型表現だけを集計します。生メールの学習用複製は保存しません。".into(),
    }
}

pub fn learning_preview(
    conn: &Connection,
    account_id: i64,
    requested_count: i64,
) -> AppResult<StyleLearningPreview> {
    collect(conn, account_id, requested_count).map(|collected| as_preview(&collected))
}

fn sorted_range(mut values: Vec<usize>) -> StyleRange {
    if values.is_empty() {
        return StyleRange::default();
    }
    values.sort_unstable();
    let index = |numerator: usize, denominator: usize| {
        (values.len().saturating_sub(1) * numerator) / denominator
    };
    StyleRange {
        median_chars: values[index(1, 2)] as i64,
        preferred_range: [values[index(1, 4)] as i64, values[index(3, 4)] as i64],
    }
}

fn occurrences(messages: &[AuthoredMessage], needle: &str) -> usize {
    messages
        .iter()
        .filter(|message| message.text.contains(needle))
        .count()
}

fn ranked_patterns(
    messages: &[AuthoredMessage],
    patterns: &[&str],
    limit: usize,
) -> Vec<StylePattern> {
    let mut ranked = patterns
        .iter()
        .map(|pattern| (*pattern, occurrences(messages, pattern)))
        .filter(|(_, count)| *count > 0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    ranked
        .into_iter()
        .take(limit)
        .map(|(pattern, count)| StylePattern {
            pattern: pattern.into(),
            usage: format!("採用メールの{count}件で使用"),
        })
        .collect()
}

fn ranked_phrases(messages: &[AuthoredMessage], patterns: &[&str], limit: usize) -> Vec<String> {
    let mut ranked = patterns
        .iter()
        .map(|pattern| (*pattern, occurrences(messages, pattern)))
        .filter(|(_, count)| *count > 0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    ranked
        .into_iter()
        .take(limit)
        .map(|(pattern, _)| pattern.to_owned())
        .collect()
}

fn build_content(messages: &[AuthoredMessage]) -> StyleProfileContent {
    let message_lengths = messages
        .iter()
        .map(|message| message.text.chars().count())
        .collect::<Vec<_>>();
    let mut sentence_lengths = Vec::new();
    let mut paragraph_counts = Vec::new();
    let mut messages_with_blank_lines = 0_usize;
    let mut messages_with_bullets = 0_usize;
    let mut full_stops = 0_usize;
    let mut exclamations = 0_usize;
    for message in messages {
        sentence_lengths.extend(
            message
                .text
                .split(['。', '！', '？', '!', '?', '\n'])
                .map(str::trim)
                .filter(|sentence| !sentence.is_empty())
                .map(|sentence| sentence.chars().count()),
        );
        let paragraphs = message
            .text
            .split("\n\n")
            .filter(|paragraph| !paragraph.trim().is_empty())
            .count();
        paragraph_counts.push(paragraphs);
        if paragraphs > 1 {
            messages_with_blank_lines += 1;
        }
        if message.text.lines().any(|line| {
            line.trim_start()
                .chars()
                .next()
                .is_some_and(|character| matches!(character, '・' | '●' | '■' | '-' | '*'))
        }) {
            messages_with_bullets += 1;
        }
        full_stops += message.text.matches('。').count();
        exclamations += message.text.matches(['！', '!']).count();
    }

    let message_length = sorted_range(message_lengths);
    let sentence_length = sorted_range(sentence_lengths);
    let paragraph_range = sorted_range(paragraph_counts);
    let blank_line_between =
        !messages.is_empty() && messages_with_blank_lines * 2 >= messages.len();
    let mut tone_notes = Vec::new();
    let very_polite = occurrences(messages, "申し上げ") + occurrences(messages, "いたします");
    let polite = occurrences(messages, "です") + occurrences(messages, "ます");
    let honorific_level = if !messages.is_empty() && very_polite >= messages.len() / 2 {
        tone_notes.push("丁寧語に加えて謙譲表現を使う".into());
        "very_polite"
    } else if polite > 0 {
        tone_notes.push("です・ます調で整える".into());
        "polite"
    } else {
        tone_notes.push("簡潔な常体を基調にする".into());
        "plain"
    };
    if message_length.median_chars > 0 && message_length.median_chars <= 240 {
        tone_notes.push("要点を短くまとめる".into());
    }

    let mut line_break_rules = Vec::new();
    if blank_line_between {
        line_break_rules.push("話題が変わる段落の間に空行を入れる".into());
    } else {
        line_break_rules.push("空行を増やしすぎず、まとまりごとに改行する".into());
    }
    let mut punctuation_rules = Vec::new();
    if full_stops > 0 {
        punctuation_rules.push("文末は句点「。」で整える".into());
    }
    if exclamations * 4 < messages.len().max(1) {
        punctuation_rules.push("感嘆符は多用しない".into());
    }
    let bullet_rules = if !messages.is_empty() && messages_with_bullets * 4 >= messages.len() {
        vec!["複数の確認事項は箇条書きにする".into()]
    } else {
        vec!["短い連絡では箇条書きを多用しない".into()]
    };

    StyleProfileContent {
        schema_version: 1,
        greetings: ranked_patterns(
            messages,
            &[
                "いつもお世話になっております。",
                "お世話になっております。",
                "ご連絡ありがとうございます。",
                "お問い合わせありがとうございます。",
                "ご返信ありがとうございます。",
            ],
            5,
        ),
        closings: ranked_patterns(
            messages,
            &[
                "よろしくお願いいたします。",
                "どうぞよろしくお願いいたします。",
                "引き続きよろしくお願いいたします。",
                "よろしくお願いします。",
                "何卒よろしくお願いいたします。",
            ],
            5,
        ),
        signature: StyleSignature {
            enabled: messages.iter().any(|message| message.signature_detected),
            text: String::new(),
        },
        honorific_level: honorific_level.into(),
        tone_notes,
        sentence_length,
        message_length,
        paragraphs: StyleParagraphs {
            median_count: paragraph_range.median_chars,
            blank_line_between,
            notes: vec!["挨拶・要件・結びを別のまとまりとして扱う".into()],
        },
        line_break_rules,
        punctuation_rules,
        bullet_rules,
        phrase_groups: StylePhraseGroups {
            request: ranked_phrases(
                messages,
                &[
                    "ご確認いただけますでしょうか。",
                    "ご確認をお願いいたします。",
                    "ご教示いただけますと幸いです。",
                ],
                5,
            ),
            acknowledgement: ranked_phrases(
                messages,
                &[
                    "承知いたしました。",
                    "かしこまりました。",
                    "確認いたしました。",
                ],
                5,
            ),
            apology: ranked_phrases(
                messages,
                &[
                    "申し訳ございません。",
                    "失礼いたしました。",
                    "お待たせいたしました。",
                ],
                5,
            ),
            decline: ranked_phrases(
                messages,
                &[
                    "今回は見送らせていただきます。",
                    "ご希望に添えず申し訳ございません。",
                ],
                5,
            ),
            scheduling: ranked_phrases(
                messages,
                &[
                    "ご都合はいかがでしょうか。",
                    "候補日をご共有いただけますでしょうか。",
                ],
                5,
            ),
            follow_up: ranked_phrases(
                messages,
                &["改めてご連絡いたします。", "引き続き確認いたします。"],
                5,
            ),
        },
        avoid_phrases: Vec::new(),
        freeform_instructions: String::new(),
    }
}

fn contains_sensitive_token(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains('@')
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("www.")
    {
        return true;
    }
    let digits = value.chars().filter(char::is_ascii_digit).count();
    let mut consecutive = 0_usize;
    let mut longest = 0_usize;
    for character in value.chars() {
        if character.is_ascii_digit() {
            consecutive += 1;
            longest = longest.max(consecutive);
        } else {
            consecutive = 0;
        }
    }
    digits >= 7 || longest >= 4 || (digits > 0 && (value.contains('円') || value.contains('月')))
}

fn validate_string(value: &str, max: usize) -> AppResult<()> {
    if value.chars().count() > max {
        return Err(AppError::BadInput(
            "文体プロファイルの文字数が上限を超えています".into(),
        ));
    }
    if contains_sensitive_token(value) {
        return Err(AppError::BadInput(
            "文体プロファイルにメール・URL・電話・金額・日付らしい値を保存できません".into(),
        ));
    }
    Ok(())
}

fn validate_strings(values: &[String], max_items: usize, max_chars: usize) -> AppResult<()> {
    if values.len() > max_items {
        return Err(AppError::BadInput(
            "文体プロファイルの項目数が上限を超えています".into(),
        ));
    }
    for value in values {
        validate_string(value, max_chars)?;
    }
    Ok(())
}

pub fn validate_content(content: &StyleProfileContent) -> AppResult<()> {
    if content.schema_version != 1
        || !matches!(
            content.honorific_level.as_str(),
            "plain" | "polite" | "very_polite"
        )
    {
        return Err(AppError::BadInput(
            "文体プロファイルの形式が不正です".into(),
        ));
    }
    for range in [&content.sentence_length, &content.message_length] {
        if range.median_chars < 0
            || range.preferred_range[0] < 0
            || range.preferred_range[1] < range.preferred_range[0]
            || range.preferred_range[1] > 20_000
        {
            return Err(AppError::BadInput(
                "文体プロファイルの文章量が不正です".into(),
            ));
        }
    }
    if content.paragraphs.median_count < 0 || content.paragraphs.median_count > 100 {
        return Err(AppError::BadInput(
            "文体プロファイルの段落数が不正です".into(),
        ));
    }
    if content.greetings.len() > 10 || content.closings.len() > 10 {
        return Err(AppError::BadInput("挨拶・結びは各10件までです".into()));
    }
    for pattern in content.greetings.iter().chain(&content.closings) {
        validate_string(&pattern.pattern, 200)?;
        validate_string(&pattern.usage, 200)?;
    }
    validate_string(&content.signature.text, 500)?;
    validate_strings(&content.tone_notes, 20, 200)?;
    validate_strings(&content.paragraphs.notes, 20, 200)?;
    validate_strings(&content.line_break_rules, 20, 200)?;
    validate_strings(&content.punctuation_rules, 20, 200)?;
    validate_strings(&content.bullet_rules, 20, 200)?;
    validate_strings(&content.phrase_groups.request, 20, 200)?;
    validate_strings(&content.phrase_groups.acknowledgement, 20, 200)?;
    validate_strings(&content.phrase_groups.apology, 20, 200)?;
    validate_strings(&content.phrase_groups.decline, 20, 200)?;
    validate_strings(&content.phrase_groups.scheduling, 20, 200)?;
    validate_strings(&content.phrase_groups.follow_up, 20, 200)?;
    validate_strings(&content.avoid_phrases, 20, 200)?;
    validate_string(&content.freeform_instructions, 1_200)?;
    let serialized = serde_json::to_string(content)
        .map_err(|_| AppError::BadInput("文体プロファイルを保存できません".into()))?;
    if serialized.chars().count() > CONTENT_LIMIT {
        return Err(AppError::BadInput("文体プロファイルが大きすぎます".into()));
    }
    Ok(())
}

fn raw_profile(row: &Row<'_>) -> rusqlite::Result<RawProfile> {
    Ok(RawProfile {
        id: row.get(0)?,
        account_id: row.get(1)?,
        workspace_id: row.get(2)?,
        version: row.get(3)?,
        status: row.get(4)?,
        source_count: row.get(5)?,
        source_candidate_count: row.get(6)?,
        source_range_start: row.get(7)?,
        source_range_end: row.get(8)?,
        source_set_hash: row.get(9)?,
        content_json: row.get(10)?,
        provider: row.get(11)?,
        model: row.get(12)?,
        input_chars: row.get(13)?,
        estimated_input_tokens: row.get(14)?,
        actual_input_tokens: row.get(15)?,
        actual_output_tokens: row.get(16)?,
        consent_version: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
        activated_at: row.get(20)?,
    })
}

fn into_profile(raw: RawProfile) -> AppResult<StyleProfile> {
    let content = serde_json::from_str(&raw.content_json)
        .map_err(|_| AppError::Db("文体プロファイルの保存形式が不正です".into()))?;
    Ok(StyleProfile {
        id: raw.id,
        account_id: raw.account_id,
        workspace_id: raw.workspace_id,
        version: raw.version,
        status: raw.status,
        source_count: raw.source_count,
        source_candidate_count: raw.source_candidate_count,
        source_range_start: raw.source_range_start,
        source_range_end: raw.source_range_end,
        source_set_hash: raw.source_set_hash,
        content,
        provider: raw.provider,
        model: raw.model,
        input_chars: raw.input_chars,
        estimated_input_tokens: raw.estimated_input_tokens,
        actual_input_tokens: raw.actual_input_tokens,
        actual_output_tokens: raw.actual_output_tokens,
        consent_version: raw.consent_version,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
        activated_at: raw.activated_at,
    })
}

const PROFILE_SELECT: &str = "SELECT id,account_id,workspace_id,version,status,source_count,source_candidate_count,source_range_start,source_range_end,source_set_hash,content_json,provider,model,input_chars,estimated_input_tokens,actual_input_tokens,actual_output_tokens,consent_version,created_at,updated_at,activated_at FROM assistant_style_profiles";

pub fn get_profile(conn: &Connection, account_id: i64, profile_id: i64) -> AppResult<StyleProfile> {
    let sql = format!("{PROFILE_SELECT} WHERE id=? AND account_id=? AND status!='deleted'");
    let raw = conn
        .query_row(&sql, params![profile_id, account_id], raw_profile)
        .optional()?
        .ok_or(AppError::NotFound)?;
    into_profile(raw)
}

pub fn list_profiles(conn: &Connection, account_id: i64) -> AppResult<Vec<StyleProfile>> {
    account_email(conn, account_id)?;
    let sql = format!(
        "{PROFILE_SELECT} WHERE account_id=? AND status!='deleted' ORDER BY updated_at DESC,id DESC"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([account_id], raw_profile)?;
    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(into_profile(row?)?);
    }
    Ok(profiles)
}

pub fn active_profile(conn: &Connection, account_id: i64) -> AppResult<Option<StyleProfile>> {
    let sql = format!(
        "{PROFILE_SELECT} WHERE account_id=? AND workspace_id IS NULL AND status='active' LIMIT 1"
    );
    conn.query_row(&sql, [account_id], raw_profile)
        .optional()?
        .map(into_profile)
        .transpose()
}

pub fn start_learning(
    conn: &Connection,
    account_id: i64,
    requested_count: i64,
    consent_version: &str,
    preview_hash: &str,
) -> AppResult<StyleLearningResult> {
    if consent_version != CONSENT_VERSION {
        return Err(AppError::BadInput(
            "文体学習の確認画面を更新してください".into(),
        ));
    }
    let collected = collect(conn, account_id, requested_count)?;
    if collected.preview_hash != preview_hash {
        return Err(AppError::BadInput(
            "対象メールが変わりました。件数と期間をもう一度確認してください".into(),
        ));
    }
    if collected.messages.is_empty() {
        return Err(AppError::BadInput(
            "本文取得済みの送信メールがないため文体を作成できません".into(),
        ));
    }

    let content = build_content(&collected.messages);
    validate_content(&content)?;
    let content_json = serde_json::to_string(&content)
        .map_err(|_| AppError::BadInput("文体プロファイルを保存できません".into()))?;
    let input_chars = collected
        .messages
        .iter()
        .map(|message| message.text.chars().count() as i64)
        .sum::<i64>();
    let timestamp = now();
    let job_id = Uuid::new_v4().to_string();
    let tx = conn.unchecked_transaction()?;
    let running: i64 = tx.query_row(
        "SELECT COUNT(*) FROM assistant_style_profile_jobs WHERE account_id=? AND status IN ('queued','collecting','analyzing')",
        [account_id],
        |row| row.get(0),
    )?;
    if running > 0 {
        return Err(AppError::BadInput(
            "このアカウントの文体学習は実行中です".into(),
        ));
    }
    let version: i64 = tx.query_row(
        "SELECT COALESCE(MAX(version),0)+1 FROM assistant_style_profiles WHERE account_id=? AND workspace_id IS NULL",
        [account_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO assistant_style_profiles(
           account_id,workspace_id,version,status,source_count,source_candidate_count,
           source_range_start,source_range_end,source_set_hash,content_json,provider,model,
           input_chars,estimated_input_tokens,actual_input_tokens,actual_output_tokens,
           consent_version,created_at,updated_at,activated_at
         ) VALUES(?,NULL,?,'draft',?,?,?,?,?,?,'local','deterministic-v1',?,0,NULL,NULL,?,?,?,NULL)",
        params![
            account_id,
            version,
            collected.messages.len() as i64,
            collected.candidate_count,
            collected.source_range_start,
            collected.source_range_end,
            collected.source_set_hash,
            content_json,
            input_chars,
            consent_version,
            timestamp,
            timestamp,
        ],
    )?;
    let profile_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO assistant_style_profile_jobs(
           id,account_id,requested_count,status,progress_current,progress_total,preview_hash,
           result_profile_id,created_at,started_at
         ) VALUES(?,?,?,'awaiting_review',?,?,?,?,?,?)",
        params![
            job_id,
            account_id,
            requested_count,
            collected.messages.len() as i64,
            collected.messages.len() as i64,
            collected.preview_hash,
            profile_id,
            timestamp,
            timestamp,
        ],
    )?;
    tx.commit()?;

    Ok(StyleLearningResult {
        job: get_job(conn, account_id, &job_id)?,
        profile: get_profile(conn, account_id, profile_id)?,
    })
}

pub fn get_job(conn: &Connection, account_id: i64, job_id: &str) -> AppResult<StyleProfileJob> {
    conn.query_row(
        "SELECT id,account_id,requested_count,status,progress_current,progress_total,error_kind,result_profile_id,created_at,started_at,finished_at
         FROM assistant_style_profile_jobs WHERE id=? AND account_id=?",
        params![job_id, account_id],
        |row| {
            Ok(StyleProfileJob {
                id: row.get(0)?,
                account_id: row.get(1)?,
                requested_count: row.get(2)?,
                status: row.get(3)?,
                progress_current: row.get(4)?,
                progress_total: row.get(5)?,
                error_kind: row.get(6)?,
                result_profile_id: row.get(7)?,
                created_at: row.get(8)?,
                started_at: row.get(9)?,
                finished_at: row.get(10)?,
            })
        },
    )
    .optional()?
    .ok_or(AppError::NotFound)
}

pub fn cancel_job(conn: &Connection, account_id: i64, job_id: &str) -> AppResult<StyleProfileJob> {
    let job = get_job(conn, account_id, job_id)?;
    if matches!(
        job.status.as_str(),
        "completed" | "cancelled" | "failed" | "interrupted"
    ) {
        return Err(AppError::BadInput("この文体学習は終了しています".into()));
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE assistant_style_profile_jobs SET status='cancelled',finished_at=? WHERE id=? AND account_id=?",
        params![now(), job_id, account_id],
    )?;
    if let Some(profile_id) = job.result_profile_id {
        tx.execute(
            "UPDATE assistant_style_profiles SET status='deleted',content_json='{}',updated_at=? WHERE id=? AND account_id=? AND status='draft'",
            params![now(), profile_id, account_id],
        )?;
    }
    tx.commit()?;
    get_job(conn, account_id, job_id)
}

fn insert_profile_copy(
    conn: &Connection,
    current: &StyleProfile,
    content: &StyleProfileContent,
    timestamp: i64,
) -> AppResult<i64> {
    let version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version),0)+1 FROM assistant_style_profiles WHERE account_id=? AND workspace_id IS ?",
        params![current.account_id, current.workspace_id],
        |row| row.get(0),
    )?;
    let content_json = serde_json::to_string(content)
        .map_err(|_| AppError::BadInput("文体プロファイルを保存できません".into()))?;
    conn.execute(
        "INSERT INTO assistant_style_profiles(
           account_id,workspace_id,version,status,source_count,source_candidate_count,
           source_range_start,source_range_end,source_set_hash,content_json,provider,model,
           input_chars,estimated_input_tokens,actual_input_tokens,actual_output_tokens,
           consent_version,created_at,updated_at,activated_at
         ) VALUES(?,?,?,'draft',?,?,?,?,?,?,'local','manual-edit',?,?,?,?,?,?,?,NULL)",
        params![
            current.account_id,
            current.workspace_id,
            version,
            current.source_count,
            current.source_candidate_count,
            current.source_range_start,
            current.source_range_end,
            current.source_set_hash,
            content_json,
            current.input_chars,
            current.estimated_input_tokens,
            current.actual_input_tokens,
            current.actual_output_tokens,
            current.consent_version,
            timestamp,
            timestamp,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_profile(
    conn: &Connection,
    account_id: i64,
    profile_id: i64,
    expected_updated_at: i64,
    content: &StyleProfileContent,
) -> AppResult<StyleProfile> {
    validate_content(content)?;
    let current = get_profile(conn, account_id, profile_id)?;
    if current.updated_at != expected_updated_at {
        return Err(AppError::BadInput(
            "文体プロファイルが別の操作で更新されています".into(),
        ));
    }
    let timestamp = now().max(current.updated_at + 1);
    let profile_id = if current.status == "draft" {
        let content_json = serde_json::to_string(content)
            .map_err(|_| AppError::BadInput("文体プロファイルを保存できません".into()))?;
        conn.execute(
            "UPDATE assistant_style_profiles SET content_json=?,provider='local',model='manual-edit',updated_at=?
             WHERE id=? AND account_id=? AND status='draft' AND updated_at=?",
            params![content_json, timestamp, profile_id, account_id, expected_updated_at],
        )?;
        profile_id
    } else {
        insert_profile_copy(conn, &current, content, timestamp)?
    };
    get_profile(conn, account_id, profile_id)
}

pub fn activate_profile(
    conn: &Connection,
    account_id: i64,
    profile_id: i64,
) -> AppResult<StyleProfile> {
    let profile = get_profile(conn, account_id, profile_id)?;
    validate_content(&profile.content)?;
    let timestamp = now().max(profile.updated_at + 1);
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE assistant_style_profiles SET status='archived',updated_at=?
         WHERE account_id=? AND workspace_id IS ? AND status='active' AND id!=?",
        params![timestamp, account_id, profile.workspace_id, profile_id],
    )?;
    if tx.execute(
        "UPDATE assistant_style_profiles SET status='active',activated_at=?,updated_at=?
         WHERE id=? AND account_id=? AND status!='deleted'",
        params![timestamp, timestamp, profile_id, account_id],
    )? == 0
    {
        return Err(AppError::NotFound);
    }
    tx.execute(
        "INSERT INTO assistant_style_profile_versions(account_id,version) VALUES(?,?)
         ON CONFLICT(account_id) DO UPDATE SET version=MAX(version+1,excluded.version)",
        params![account_id, profile.version],
    )?;
    tx.execute(
        "UPDATE assistant_style_profile_jobs SET status='completed',finished_at=?
         WHERE account_id=? AND result_profile_id=? AND status='awaiting_review'",
        params![timestamp, account_id, profile_id],
    )?;
    tx.commit()?;
    get_profile(conn, account_id, profile_id)
}

pub fn delete_profile(conn: &Connection, account_id: i64, profile_id: i64) -> AppResult<()> {
    let profile = get_profile(conn, account_id, profile_id)?;
    let timestamp = now().max(profile.updated_at + 1);
    let tx = conn.unchecked_transaction()?;
    if tx.execute(
        "UPDATE assistant_style_profiles SET status='deleted',content_json='{}',updated_at=?,activated_at=NULL
         WHERE id=? AND account_id=? AND status!='deleted'",
        params![timestamp, profile_id, account_id],
    )? == 0
    {
        return Err(AppError::NotFound);
    }
    tx.execute(
        "INSERT INTO assistant_style_profile_versions(account_id,version) VALUES(?,1)
         ON CONFLICT(account_id) DO UPDATE SET version=version+1",
        [account_id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn preview_profile(
    conn: &Connection,
    account_id: i64,
    profile_id: i64,
    scenario_id: &str,
) -> AppResult<StyleProfilePreviewText> {
    let profile = get_profile(conn, account_id, profile_id)?;
    let (subject, core) = match scenario_id {
        "inquiry" => (
            "【架空プレビュー】お問い合わせへの返信",
            "お問い合わせの内容を確認いたしました。必要事項を確認のうえ、改めてご案内いたします。",
        ),
        "schedule" => (
            "【架空プレビュー】日程のご相談",
            "日程のご連絡ありがとうございます。候補を確認し、都合のよい時間をご連絡いたします。",
        ),
        _ => return Err(AppError::BadInput("未対応の架空プレビューです".into())),
    };
    let greeting = profile
        .content
        .greetings
        .first()
        .map(|pattern| pattern.pattern.as_str())
        .unwrap_or("お世話になっております。");
    let closing = profile
        .content
        .closings
        .first()
        .map(|pattern| pattern.pattern.as_str())
        .unwrap_or("よろしくお願いいたします。");
    let separator = if profile.content.paragraphs.blank_line_between {
        "\n\n"
    } else {
        "\n"
    };
    Ok(StyleProfilePreviewText {
        scenario_id: scenario_id.into(),
        subject: subject.into(),
        body_text: [greeting, core, closing].join(separator),
        notice: "架空データだけを使ったローカルプレビューです。外部AI・SMTPには接続していません。"
            .into(),
    })
}

/// A bounded, fact-free fragment for reply prompting. It deliberately omits
/// source metadata, usage counters, and any raw example mail.
pub fn prompt_content(profile: &StyleProfile) -> AppResult<serde_json::Value> {
    validate_content(&profile.content)?;
    serde_json::to_value(&profile.content)
        .map_err(|_| AppError::BadInput("文体プロファイルを使用できません".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::Provider,
        store::{insert_account, migrations},
    };

    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::apply(&conn).unwrap();
        insert_account(&conn, "owner@example.test", Provider::Gmail, "Owner").unwrap();
        insert_account(&conn, "other@example.test", Provider::Gmail, "Other").unwrap();
        conn.execute(
            "INSERT INTO folders(id,account_id,name,imap_path,role) VALUES(1,1,'Sent','Sent','sent')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO folders(id,account_id,name,imap_path,role) VALUES(2,2,'Sent','Sent-2','sent')",
            [],
        )
        .unwrap();
        conn
    }

    fn mail(conn: &Connection, id: i64, account: i64, from: &str, date: i64, body: &str) {
        conn.execute(
            "INSERT INTO messages(id,account_id,folder_id,uid,from_addr,subject,date,snippet,body_fetched,body_text)
             VALUES(?,?,?,?,?,'テストメール',?,'',1,?)",
            params![id, account, account, id, from, date, body],
        )
        .unwrap();
    }

    #[test]
    fn preview_uses_only_authored_unique_sent_mail_from_the_account() {
        let conn = database();
        let body =
            "お世話になっております。\n\nご確認をお願いいたします。\n\nよろしくお願いいたします。";
        mail(&conn, 1, 1, "Owner <owner@example.test>", 100, body);
        mail(&conn, 2, 1, "owner@example.test", 90, body);
        mail(
            &conn,
            3,
            1,
            "customer@example.test",
            80,
            "別人が書いた十分に長い本文です。",
        );
        mail(
            &conn,
            4,
            1,
            "owner@example.test",
            70,
            "> 引用だけです\n> 返信本文ではありません",
        );
        mail(
            &conn,
            5,
            2,
            "other@example.test",
            60,
            "お世話になっております。別アカウントです。",
        );

        let preview = learning_preview(&conn, 1, 100).unwrap();
        assert_eq!(preview.candidate_count, 4);
        assert_eq!(preview.selected_count, 1);
        assert_eq!(preview.exclusions.duplicate, 1);
        assert_eq!(preview.exclusions.sender_mismatch, 1);
        assert_eq!(preview.exclusions.quoted_or_empty, 1);
        assert_eq!(preview.ai_input_chars, 0);
    }

    #[test]
    fn learning_persists_only_abstract_profile_and_requires_matching_preview() {
        let conn = database();
        let raw = "山田株式会社 御中\n\nお世話になっております。\nご連絡ありがとうございます。\n\nご確認をお願いいたします。\n\nよろしくお願いいたします。\n-- \nOwner\nowner@example.test";
        mail(&conn, 1, 1, "owner@example.test", 100, raw);
        let preview = learning_preview(&conn, 1, 100).unwrap();
        assert!(start_learning(&conn, 1, 100, CONSENT_VERSION, "stale").is_err());

        let result = start_learning(&conn, 1, 100, CONSENT_VERSION, &preview.preview_hash).unwrap();
        assert_eq!(result.profile.status, "draft");
        assert!(result.profile.content.signature.enabled);
        assert!(result
            .profile
            .content
            .greetings
            .iter()
            .any(|item| item.pattern == "お世話になっております。"));
        let stored: String = conn
            .query_row(
                "SELECT content_json FROM assistant_style_profiles WHERE id=?",
                [result.profile.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!stored.contains("山田株式会社"));
        assert!(!stored.contains("owner@example.test"));
        assert!(!stored.contains("ご連絡ありがとうございます。\n\nご確認"));
    }

    #[test]
    fn only_activated_profile_is_returned_and_versions_can_be_reactivated() {
        let conn = database();
        mail(
            &conn,
            1,
            1,
            "owner@example.test",
            100,
            "お世話になっております。\n\n確認いたしました。\n\nよろしくお願いします。",
        );
        let preview = learning_preview(&conn, 1, 100).unwrap();
        let first = start_learning(&conn, 1, 100, CONSENT_VERSION, &preview.preview_hash)
            .unwrap()
            .profile;
        assert!(active_profile(&conn, 1).unwrap().is_none());
        let active = activate_profile(&conn, 1, first.id).unwrap();
        assert_eq!(active_profile(&conn, 1).unwrap().unwrap().id, active.id);

        let mut edited = active.content.clone();
        edited.freeform_instructions = "結論を先に書く".into();
        let second = update_profile(&conn, 1, active.id, active.updated_at, &edited).unwrap();
        assert_eq!(second.status, "draft");
        assert_ne!(second.id, active.id);
        activate_profile(&conn, 1, second.id).unwrap();
        assert_eq!(active_profile(&conn, 1).unwrap().unwrap().id, second.id);
        assert_eq!(get_profile(&conn, 1, active.id).unwrap().status, "archived");
    }

    #[test]
    fn profile_validation_rejects_likely_personal_or_case_specific_values() {
        let mut content = StyleProfileContent::default();
        content.greetings.push(StylePattern {
            pattern: "owner@example.testへ返信".into(),
            usage: "常用".into(),
        });
        assert!(validate_content(&content).is_err());

        content.greetings[0].pattern = "価格は12000円です".into();
        assert!(validate_content(&content).is_err());
    }

    #[test]
    fn invalid_counts_and_cross_account_ids_are_rejected() {
        let conn = database();
        assert!(learning_preview(&conn, 1, 50).is_err());
        mail(
            &conn,
            1,
            1,
            "owner@example.test",
            100,
            "お世話になっております。\n十分に長い本文です。\nよろしくお願いします。",
        );
        let preview = learning_preview(&conn, 1, 100).unwrap();
        let profile = start_learning(&conn, 1, 100, CONSENT_VERSION, &preview.preview_hash)
            .unwrap()
            .profile;
        assert!(get_profile(&conn, 2, profile.id).is_err());
    }
}
