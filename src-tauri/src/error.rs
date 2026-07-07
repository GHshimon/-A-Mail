//! 統一エラー型。
//!
//! フロントは `kind` で分岐する({kind, message} 形)。`message` はフロントへ
//! シリアライズされる(adjacently-tagged)ため、**構築サイトの責務**として
//! パスワード・API キー・メール本文を絶対に `message` へ入れない。
//!
//! - 秘密を運ぶ可能性のある変換(`reqwest` / `keyring`)は種別のみの一般化文言に丸める。
//! - `rusqlite` / `io` / `tauri` は診断のため詳細を透過するが、これらに秘密は含まれない
//!   (DB パス等の環境情報のみ。個人利用の自分の Mac 上に限る)。
//! - `Auth` / `Imap` / `Smtp` / `Ai` を手で構築する M1 以降は、資格情報を含めないこと。

#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("認証に失敗しました")]
    Auth(String),
    #[error("ネットワークエラー")]
    Network(String),
    #[error("IMAP エラー")]
    Imap(String),
    #[error("SMTP エラー")]
    Smtp(String),
    #[error("DB エラー")]
    Db(String),
    #[error("Keychain エラー")]
    Keychain(String),
    #[error("AI エラー")]
    Ai(String),
    #[error("不正な入力")]
    BadInput(String),
    #[error("未検出")]
    NotFound,
}

pub type AppResult<T> = Result<T, AppError>;

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        // 秘密(APIキー等)混入を避けるため、種別のみの一般化した文言にする。
        let detail = if e.is_timeout() {
            "タイムアウトしました"
        } else if e.is_connect() {
            "接続に失敗しました"
        } else {
            "通信に失敗しました"
        };
        AppError::Network(detail.into())
    }
}

// keyring エラーは詳細を伏せる(アカウント名等の混入を避ける)。
impl From<keyring::Error> for AppError {
    fn from(_e: keyring::Error) -> Self {
        AppError::Keychain("キーチェーン操作に失敗しました".into())
    }
}
