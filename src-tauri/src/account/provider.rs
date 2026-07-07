//! Gmail / iCloud の接続プリセット(IMAP/SMTP ホスト・ポート)。

use super::Provider;

/// プロバイダ既定の接続設定。
#[derive(Debug, Clone, Copy)]
pub struct ConnPreset {
    pub imap_host: &'static str,
    pub imap_port: u16,
    pub smtp_host: &'static str,
    pub smtp_port: u16,
    pub smtp_starttls: bool,
}

impl Provider {
    /// SPEC 3 の接続プリセットを返す。
    pub fn preset(self) -> ConnPreset {
        match self {
            Provider::Gmail => ConnPreset {
                imap_host: "imap.gmail.com",
                imap_port: 993,
                smtp_host: "smtp.gmail.com",
                smtp_port: 465,
                smtp_starttls: false,
            },
            Provider::Icloud => ConnPreset {
                imap_host: "imap.mail.me.com",
                imap_port: 993,
                smtp_host: "smtp.mail.me.com",
                smtp_port: 587,
                smtp_starttls: true,
            },
        }
    }
}
