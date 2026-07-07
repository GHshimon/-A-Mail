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
