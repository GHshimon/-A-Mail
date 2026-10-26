//! AI プロバイダー連携(補完 A / 関連情報 B / 返信ドラフト C)。

pub mod gemini;
pub mod openai;
pub mod prompts;

use crate::error::AppResult;
use gemini::GenConfig;

/// 選択中プロバイダーへ共通の生成要求を振り分ける。
pub async fn generate_content(
    client: &reqwest::Client,
    provider: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    cfg: &GenConfig,
) -> AppResult<String> {
    match provider {
        "openai" => openai::generate_content(client, api_key, model, system, user, cfg).await,
        _ => gemini::generate_content(client, api_key, model, system, user, cfg).await,
    }
}
