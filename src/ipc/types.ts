// Rust 側(src-tauri)の型と 1:1 対応する TypeScript 型。
// 将来 ts-rs による自動生成に置き換え可能な想定で、まずは手書きで定義する。
// フィールド名は Rust の serde 既定(snake_case のまま)に合わせる。

export type Provider = "gmail" | "icloud";

export interface Account {
  id: number;
  email: string;
  provider: Provider;
  display_name: string;
  imap_host: string;
  imap_port: number;
  smtp_host: string;
  smtp_port: number;
  smtp_starttls: boolean;
}

/** サイドバーのフォルダ役割(アイコン割り当て等に使用)。 */
export type FolderRole =
  | "inbox"
  | "sent"
  | "drafts"
  | "trash"
  | "archive"
  | "starred"
  | "junk"
  | "custom";

export interface Folder {
  id: number;
  account_id: number;
  name: string;
  imap_path: string;
  role: FolderRole | null;
  unread: number;
}

/** 一覧表示用の軽量ヘッダ DTO。 */
export interface MessageHeader {
  id: number;
  uid: number;
  subject: string;
  from: string;
  date: number; // epoch 秒
  snippet: string;
  seen: boolean;
  flagged: boolean;
  has_attachments: boolean;
}

export interface AttachmentMeta {
  id: number;
  filename: string;
  mime: string;
  size: number;
  content_id: string | null;
}

/** 本文表示用のフル DTO。 */
export interface MessageFull {
  header: MessageHeader;
  to: string[];
  cc: string[];
  message_id: string;
  in_reply_to: string | null;
  references: string[];
  body_text: string | null;
  body_html: string | null;
  attachments: AttachmentMeta[];
}

/** アプリ設定(値本体=APIキー等は含まない。Keychain 管理)。 */
export interface Settings {
  ai_enabled: boolean;
  ai_scope_related_count: number;
  ai_send_body: boolean;
  ai_provider: AiProvider;
  model: string;
  openai_model: string;
  theme: ThemePref;
  poll_interval_sec: number;
}

export type AiProvider = "gemini" | "openai";

export type ThemePref = "system" | "light" | "dark";

export interface SettingsPatch {
  ai_enabled?: boolean;
  ai_scope_related_count?: number;
  ai_send_body?: boolean;
  ai_provider?: AiProvider;
  model?: string;
  openai_model?: string;
  theme?: ThemePref;
  poll_interval_sec?: number;
}

/** 送信するメッセージ(send_message の引数)。フィールドは Rust の serde 既定 = snake_case。 */
export interface OutgoingMessage {
  account_id: number;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body_text: string;
  body_html?: string | null;
  in_reply_to?: string | null;
  references: string[];
  /** 送信成功後に削除する下書き ID。 */
  draft_id?: number | null;
}

/** 送信結果。 */
export interface SentInfo {
  message_id: string;
  appended_to_sent: boolean;
}

/** 下書きの保存入力(save_draft の引数)。`id` があれば更新。 */
export interface DraftInput {
  id?: number | null;
  account_id: number;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body_text: string;
  body_html?: string | null;
  in_reply_to?: string | null;
  references: string[];
}

/** 下書き一覧の軽量 DTO。 */
export interface DraftHeader {
  id: number;
  account_id: number;
  subject: string;
  to: string[];
  snippet: string;
  updated_at: number;
}

// ---- Confirmed draft assistant (P0) ----
// `assistant_*` commands always receive `{ req: ... }`; these DTO keys deliberately
// mirror the Rust serde names and must remain snake_case.
export type DraftStatus = "editing" | "queued" | "generating" | "unreviewed" | "needs_review" | "stale" | "sending" | "sent" | "failed" | "unknown" | "discarded";
export type ContextStatus = "unassigned" | "confirmed" | "common_only" | "conflict" | "needs_review";
export type FactStatus = "confirmed" | "unknown" | "stale";
export interface Workspace { id: number; account_id: number; customer_id: number; customer_name: string; name: string; context_version: number; }
export interface ReplyContext { source_message_id: number; thread_id: number; workspace_id: number | null; context_version: number; status: ContextStatus; review_reasons: string[]; }
export interface SourceCard { id: string; kind: "message" | "project_file"; label: string; relative_path: string | null; locator: string | null; snippet: string; version: string; workspace_id: number | null; fact_status: FactStatus; }
export interface ReplyQuestion { id: string; text: string; status: "answered" | "needs_input"; source_ids: string[]; }
export interface ReplyRun { id: string; draft_id: number; source_message_id: number | null; workspace_id: number | null; context_version: number; profile_version: number; snapshot_hash: string; generated_text: string; questions: ReplyQuestion[]; sources: SourceCard[]; review_reasons: string[]; status: string; truncated: boolean; provider: string; model: string; input_chars: number; estimated_input_tokens: number; actual_input_tokens: number | null; actual_output_tokens: number | null; created_at: number; }
export interface DraftRecord { draft: DraftInput & { id: number }; session_id: string; revision: number; source_message_id: number | null; workspace_id: number | null; intent: string; reply_run_id: string | null; reply: ReplyRun | null; status: DraftStatus; review_reasons: string[]; updated_at: number; }
export interface AssistantSettings { account_id: number; auto_draft_enabled: boolean; subscription_reply_enabled: boolean; /** @deprecated Continuous learning is disabled. */ learning_enabled: boolean; active_style_profile_id: number | null; daily_generation_limit: number; auto_generated_today: number; quota_day: string; }
export type SubscriptionConnection = "unavailable" | "disconnected" | "connecting" | "connected" | "error";
export interface SubscriptionRateWindow { used_percent: number | null; window_duration_mins: number | null; resets_at: number | null; }
export interface SubscriptionLoginPrompt { request_id: string; login_id: string; auth_url: string; expires_at: number; }
export interface SubscriptionStatus { runtime_available: boolean; runtime_version: string | null; connection: SubscriptionConnection; plan_type: string | null; primary: SubscriptionRateWindow | null; secondary: SubscriptionRateWindow | null; rate_limit_reached: boolean; release_enabled: boolean; login: SubscriptionLoginPrompt | null; message: string | null; }
export interface ProjectScope { account_id: number; project_id: number; workspace_id: number | null; scope: "disabled" | "common" | "workspace"; confirmed: boolean; }
export interface StyleRule { id: number; account_id: number; workspace_id: number | null; from_phrase: string; to_phrase: string; status: "pending" | "active" | "rejected"; created_at: number; }
export interface StylePattern { pattern: string; usage: string; }
export interface StyleRange { median_chars: number; preferred_range: [number, number]; }
export interface StyleParagraphs { median_count: number; blank_line_between: boolean; notes: string[]; }
export interface StyleSignature { enabled: boolean; text: string; }
export interface StylePhraseGroups { request: string[]; acknowledgement: string[]; apology: string[]; decline: string[]; scheduling: string[]; follow_up: string[]; }
export interface StyleProfileContent { schema_version: number; greetings: StylePattern[]; closings: StylePattern[]; signature: StyleSignature; honorific_level: "plain" | "polite" | "very_polite"; tone_notes: string[]; sentence_length: StyleRange; message_length: StyleRange; paragraphs: StyleParagraphs; line_break_rules: string[]; punctuation_rules: string[]; bullet_rules: string[]; phrase_groups: StylePhraseGroups; avoid_phrases: string[]; freeform_instructions: string; }
export interface StyleLearningExclusions { body_not_cached: number; sender_mismatch: number; quoted_or_empty: number; too_short: number; duplicate: number; }
export interface StyleLearningPreview { account_id: number; requested_count: 100 | 200; candidate_count: number; eligible_count: number; selected_count: number; source_range_start: number | null; source_range_end: number | null; exclusions: StyleLearningExclusions; locally_aggregated_count: number; ai_sample_count: number; ai_input_chars: number; preview_hash: string; consent_version: string; notice: string; }
export interface StyleProfile { id: number; account_id: number; workspace_id: number | null; version: number; status: "draft" | "active" | "archived"; source_count: number; source_candidate_count: number; source_range_start: number | null; source_range_end: number | null; source_set_hash: string; content: StyleProfileContent; provider: string; model: string; input_chars: number; estimated_input_tokens: number; actual_input_tokens: number | null; actual_output_tokens: number | null; consent_version: string; created_at: number; updated_at: number; activated_at: number | null; }
export interface StyleProfileJob { id: string; account_id: number; requested_count: 100 | 200; status: "queued" | "collecting" | "analyzing" | "awaiting_review" | "completed" | "failed" | "cancelled" | "interrupted"; progress_current: number; progress_total: number; error_kind: string | null; result_profile_id: number | null; created_at: number; started_at: number | null; finished_at: number | null; }
export interface StyleLearningResult { job: StyleProfileJob; profile: StyleProfile; }
export interface StyleProfilePreviewText { scenario_id: string; subject: string; body_text: string; notice: string; }

// ---- AI(M3) ----

/** A: 予測入力の要求。 */
export interface CompleteReq {
  account_id: number;
  subject: string;
  to: string[];
  body_prefix: string;
}

/** B: 関連情報サイドバーの要求。 */
export interface ContextReq {
  account_id: number;
  to: string[];
  subject: string;
  body: string;
  /** AI が読み取り専用で参照する登録済みプロジェクト。 */
  project_ids: number[];
}

/** B: 要点 1 項目。category は history/commit/todo(想定外もあり得るので string)。 */
export interface ContextPoint {
  category: string;
  text: string;
}

/** B: 要点 + 関連過去メール。 */
export interface ContextResult {
  points: ContextPoint[];
  related: MessageHeader[];
  project_sources: ProjectFileHit[];
}

/** AI 文脈として参照できるローカル/NAS プロジェクト。 */
export interface AiProject {
  id: number;
  name: string;
  root_path: string;
  enabled: boolean;
  created_at: number;
}

/** プロジェクト内検索で採用された出典。パスはルートからの相対パス。 */
export interface ProjectFileHit {
  project_id: number;
  project_name: string;
  relative_path: string;
  line_number?: number | null;
  snippet: string;
}

// ---- Codex CLI (Phase 3) ----

/** アプリが許可する読み取り専用 CLI タスク。任意プロンプトは受け付けない。 */
export type CliTaskKind =
  | "project_brief"
  | "spec_gap"
  | "git_history"
  | "attachment_candidates";

export type CliJobStatus =
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

export interface CliStatus {
  available: boolean;
  version?: string | null;
  message?: string | null;
}

export interface CliJob {
  id: string;
  project_id: number;
  task_kind: CliTaskKind;
  status: CliJobStatus;
  progress: string;
  output?: string | null;
  error?: string | null;
  started_at?: number | null;
  completed_at?: number | null;
}

export interface StartCliJobReq {
  project_id: number;
  task_kind: CliTaskKind;
}

/** C: 返信ドラフト生成の要求。 */
export interface ReplyReq {
  source_message_id: number;
  tone?: string;
  length?: string;
  project_ids: number[];
}

/** C: 返信本文と、その生成に実際に使われたプロジェクト出典。 */
export interface ReplyResult {
  text: string;
  project_sources: ProjectFileHit[];
}

/** Rust の AppError(#[serde(tag = "kind", content = "message")])に対応。 */
export type AppErrorKind =
  | "Auth"
  | "Network"
  | "Imap"
  | "Smtp"
  | "Db"
  | "Keychain"
  | "Ai"
  | "BadInput"
  | "NotFound";

export interface AppError {
  kind: AppErrorKind;
  message: string;
}
