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
    pub model: String,
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
}

/// C: 返信ドラフト生成の要求。
#[derive(Debug, Clone, Deserialize)]
pub struct ReplyReq {
    pub source_message_id: i64,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub length: Option<String>,
}

/// 設定の部分更新。未指定フィールドは変更しない。
#[derive(Debug, Default, Deserialize)]
pub struct SettingsPatch {
    pub ai_enabled: Option<bool>,
    pub ai_scope_related_count: Option<i64>,
    pub ai_send_body: Option<bool>,
    pub model: Option<String>,
    pub theme: Option<String>,
    pub poll_interval_sec: Option<i64>,
}
