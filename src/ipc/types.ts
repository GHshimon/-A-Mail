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
  model: string;
  theme: ThemePref;
  poll_interval_sec: number;
}

export type ThemePref = "system" | "light" | "dark";

export interface SettingsPatch {
  ai_enabled?: boolean;
  ai_scope_related_count?: number;
  ai_send_body?: boolean;
  model?: string;
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
}

/** C: 返信ドラフト生成の要求。 */
export interface ReplyReq {
  source_message_id: number;
  tone?: string;
  length?: string;
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
