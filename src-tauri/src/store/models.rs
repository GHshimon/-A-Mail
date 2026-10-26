//! DB 行 <-> DTO。フロント(src/ipc/types.ts)と対応する。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub imap_path: String,
    pub role: Option<String>,
    /// 未読件数(一覧表示用に集計)。
    pub unread: i64,
}

/// 一覧表示用の軽量ヘッダ DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub id: i64,
    pub uid: i64,
    pub subject: String,
    pub from: String,
    pub date: i64,
    pub snippet: String,
    pub seen: bool,
    pub flagged: bool,
    pub has_attachments: bool,
}

/// 添付ファイルのメタ情報(本体はまだ保存しない=遅延ダウンロードは後続)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub id: i64,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    pub content_id: Option<String>,
}

/// 本文表示用のフル DTO(get_message が返す)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFull {
    pub header: MessageHeader,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub message_id: String,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<AttachmentMeta>,
}

/// アプリ設定(値本体=API キー等は含めない。Keychain 管理)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub ai_enabled: bool,
    pub ai_scope_related_count: i64,
    pub ai_send_body: bool,
    pub ai_provider: String,
    pub model: String,
    pub openai_model: String,
    pub theme: String,
    pub poll_interval_sec: i64,
}

/// 送信するメッセージ(フロント → send_message)。宛先はメールアドレス文字列。
#[derive(Debug, Clone, Deserialize)]
pub struct OutgoingMessage {
    pub account_id: i64,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body_text: String,
    #[serde(default)]
    pub body_html: Option<String>,
    /// 返信元の Message-ID(`<...>`)。
    #[serde(default)]
    pub in_reply_to: Option<String>,
    /// References ヘッダに連ねる Message-ID 群。
    #[serde(default)]
    pub references: Vec<String>,
    /// 送信成功後に削除する下書き ID。
    #[serde(default)]
    pub draft_id: Option<i64>,
}

/// 送信結果。生成した Message-ID を返す(フロントの表示/スレッド用)。
#[derive(Debug, Clone, Serialize)]
pub struct SentInfo {
    pub message_id: String,
    /// Sent フォルダへ APPEND したか(provider 別に要否が変わる)。
    pub appended_to_sent: bool,
}

/// 下書きの保存入力(フロント → save_draft)。`id` があれば更新。
#[derive(Debug, Clone, Deserialize)]
pub struct DraftInput {
    #[serde(default)]
    pub id: Option<i64>,
    pub account_id: i64,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body_text: String,
    #[serde(default)]
    pub body_html: Option<String>,
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
}

/// 下書き一覧の軽量 DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftHeader {
    pub id: i64,
    pub account_id: i64,
    pub subject: String,
    pub to: Vec<String>,
    pub snippet: String,
    pub updated_at: i64,
}

// ---- Confirmed draft assistant (P0) DTOs ----
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: i64,
    pub account_id: i64,
    pub customer_id: i64,
    pub customer_name: String,
    pub name: String,
    pub context_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyContext {
    pub source_message_id: i64,
    pub thread_id: i64,
    pub workspace_id: Option<i64>,
    pub context_version: i64,
    pub status: String,
    pub review_reasons: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCard {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub relative_path: Option<String>,
    pub locator: Option<String>,
    pub snippet: String,
    pub version: String,
    pub workspace_id: Option<i64>,
    pub fact_status: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyQuestion {
    pub id: String,
    pub text: String,
    pub status: String,
    pub source_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyRun {
    pub id: String,
    pub draft_id: i64,
    pub source_message_id: Option<i64>,
    pub workspace_id: Option<i64>,
    pub context_version: i64,
    pub profile_version: i64,
    pub snapshot_hash: String,
    pub generated_text: String,
    pub questions: Vec<ReplyQuestion>,
    pub sources: Vec<SourceCard>,
    pub review_reasons: Vec<String>,
    pub status: String,
    pub truncated: bool,
    pub provider: String,
    pub model: String,
    pub input_chars: i64,
    pub estimated_input_tokens: i64,
    pub actual_input_tokens: Option<i64>,
    pub actual_output_tokens: Option<i64>,
    pub created_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftRecord {
    pub draft: DraftInputOut,
    pub session_id: String,
    pub revision: i64,
    pub source_message_id: Option<i64>,
    pub workspace_id: Option<i64>,
    pub intent: String,
    pub reply_run_id: Option<String>,
    pub reply: Option<ReplyRun>,
    pub status: String,
    pub review_reasons: Vec<String>,
    pub updated_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftInputOut {
    pub id: i64,
    pub account_id: i64,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantSettings {
    pub account_id: i64,
    pub auto_draft_enabled: bool,
    /// Prefer the user's ChatGPT-managed Codex allowance over an API key.
    pub subscription_reply_enabled: bool,
    /// Legacy compatibility only. Continuous learning is permanently disabled;
    /// style learning is an explicit 100/200-message job.
    pub learning_enabled: bool,
    pub active_style_profile_id: Option<i64>,
    pub daily_generation_limit: i64,
    pub auto_generated_today: i64,
    pub quota_day: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectScope {
    pub account_id: i64,
    pub project_id: i64,
    pub workspace_id: Option<i64>,
    pub scope: String,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleRule {
    pub id: i64,
    pub account_id: i64,
    pub workspace_id: Option<i64>,
    pub from_phrase: String,
    pub to_phrase: String,
    pub status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StylePattern {
    pub pattern: String,
    pub usage: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StyleRange {
    pub median_chars: i64,
    pub preferred_range: [i64; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StyleParagraphs {
    pub median_count: i64,
    pub blank_line_between: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StyleSignature {
    pub enabled: bool,
    /// Phase A never learns signature contents because they often contain PII.
    /// A future explicit signature editor may populate this separately.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StylePhraseGroups {
    pub request: Vec<String>,
    pub acknowledgement: Vec<String>,
    pub apology: Vec<String>,
    pub decline: Vec<String>,
    pub scheduling: Vec<String>,
    pub follow_up: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleProfileContent {
    pub schema_version: i64,
    pub greetings: Vec<StylePattern>,
    pub closings: Vec<StylePattern>,
    pub signature: StyleSignature,
    pub honorific_level: String,
    pub tone_notes: Vec<String>,
    pub sentence_length: StyleRange,
    pub message_length: StyleRange,
    pub paragraphs: StyleParagraphs,
    pub line_break_rules: Vec<String>,
    pub punctuation_rules: Vec<String>,
    pub bullet_rules: Vec<String>,
    pub phrase_groups: StylePhraseGroups,
    pub avoid_phrases: Vec<String>,
    pub freeform_instructions: String,
}

impl Default for StyleProfileContent {
    fn default() -> Self {
        Self {
            schema_version: 1,
            greetings: Vec::new(),
            closings: Vec::new(),
            signature: StyleSignature::default(),
            honorific_level: "polite".into(),
            tone_notes: Vec::new(),
            sentence_length: StyleRange::default(),
            message_length: StyleRange::default(),
            paragraphs: StyleParagraphs::default(),
            line_break_rules: Vec::new(),
            punctuation_rules: Vec::new(),
            bullet_rules: Vec::new(),
            phrase_groups: StylePhraseGroups::default(),
            avoid_phrases: Vec::new(),
            freeform_instructions: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StyleLearningExclusions {
    pub body_not_cached: i64,
    pub sender_mismatch: i64,
    pub quoted_or_empty: i64,
    pub too_short: i64,
    pub duplicate: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleLearningPreview {
    pub account_id: i64,
    pub requested_count: i64,
    pub candidate_count: i64,
    pub eligible_count: i64,
    pub selected_count: i64,
    pub source_range_start: Option<i64>,
    pub source_range_end: Option<i64>,
    pub exclusions: StyleLearningExclusions,
    pub locally_aggregated_count: i64,
    pub ai_sample_count: i64,
    pub ai_input_chars: i64,
    pub preview_hash: String,
    pub consent_version: String,
    pub notice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleProfile {
    pub id: i64,
    pub account_id: i64,
    pub workspace_id: Option<i64>,
    pub version: i64,
    pub status: String,
    pub source_count: i64,
    pub source_candidate_count: i64,
    pub source_range_start: Option<i64>,
    pub source_range_end: Option<i64>,
    pub source_set_hash: String,
    pub content: StyleProfileContent,
    pub provider: String,
    pub model: String,
    pub input_chars: i64,
    pub estimated_input_tokens: i64,
    pub actual_input_tokens: Option<i64>,
    pub actual_output_tokens: Option<i64>,
    pub consent_version: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub activated_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleProfileJob {
    pub id: String,
    pub account_id: i64,
    pub requested_count: i64,
    pub status: String,
    pub progress_current: i64,
    pub progress_total: i64,
    pub error_kind: Option<String>,
    pub result_profile_id: Option<i64>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleLearningResult {
    pub job: StyleProfileJob,
    pub profile: StyleProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleProfilePreviewText {
    pub scenario_id: String,
    pub subject: String,
    pub body_text: String,
    pub notice: String,
}

// ---- AI(M3)DTO ----

/// A: 予測入力の要求。
#[derive(Debug, Clone, Deserialize)]
pub struct CompleteReq {
    pub account_id: i64,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub to: Vec<String>,
    /// カーソル直前までの本文。
    #[serde(default)]
    pub body_prefix: String,
}

/// B: 関連情報サイドバーの要求。
#[derive(Debug, Clone, Deserialize)]
pub struct ContextReq {
    pub account_id: i64,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body: String,
    /// 明示的に選択された読み取り専用プロジェクト。
    #[serde(default)]
    pub project_ids: Vec<i64>,
}

/// AI が参照できる読み取り専用ローカルプロジェクト。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiProject {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    pub enabled: bool,
    pub created_at: i64,
}

/// ローカルプロジェクト内の、クエリに一致したテキスト断片。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFileHit {
    pub project_id: i64,
    pub project_name: String,
    pub relative_path: String,
    pub snippet: String,
    pub line_number: Option<u32>,
}

/// B: 要点 1 項目(フロントの category と対応)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPoint {
    /// history / commit / todo
    pub category: String,
    pub text: String,
}

/// B: 要点 + 関連過去メール。
#[derive(Debug, Clone, Serialize)]
pub struct ContextResult {
    pub points: Vec<ContextPoint>,
    pub related: Vec<MessageHeader>,
    pub project_sources: Vec<ProjectFileHit>,
}

/// C: 返信ドラフト生成の要求。
#[derive(Debug, Clone, Deserialize)]
pub struct ReplyReq {
    pub source_message_id: i64,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub length: Option<String>,
    /// 明示的に選択された読み取り専用プロジェクト。
    #[serde(default)]
    pub project_ids: Vec<i64>,
}

/// C: 生成本文と、その生成に実際に渡したプロジェクト出典。
#[derive(Debug, Clone, Serialize)]
pub struct ReplyResult {
    pub text: String,
    pub project_sources: Vec<ProjectFileHit>,
}

/// 設定の部分更新。未指定フィールドは変更しない。
#[derive(Debug, Default, Deserialize)]
pub struct SettingsPatch {
    pub ai_enabled: Option<bool>,
    pub ai_scope_related_count: Option<i64>,
    pub ai_send_body: Option<bool>,
    pub ai_provider: Option<String>,
    pub model: Option<String>,
    pub openai_model: Option<String>,
    pub theme: Option<String>,
    pub poll_interval_sec: Option<i64>,
}
