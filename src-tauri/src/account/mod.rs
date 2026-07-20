//! アカウント・ドメインモデルとプロバイダ判定。

pub mod provider;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub provider: Provider,
    pub display_name: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_starttls: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Gmail,
    Icloud,
}

impl Provider {
    /// DB(TEXT)からの復元。
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "gmail" => Some(Provider::Gmail),
            "icloud" => Some(Provider::Icloud),
            _ => None,
        }
    }

    /// DB へ保存する文字列表現。
    pub fn as_db_str(self) -> &'static str {
        match self {
            Provider::Gmail => "gmail",
            Provider::Icloud => "icloud",
        }
    }
}
