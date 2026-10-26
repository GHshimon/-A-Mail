//! OpenAI Responses API 呼び出し。
//!
//! API キーとメール本文はログに出さず、応答本文もエラー時には保持しない。

use serde_json::{json, Value};

use crate::ai::gemini::GenConfig;
use crate::error::{AppError, AppResult};

const RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

pub async fn generate_content(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    cfg: &GenConfig,
) -> AppResult<String> {
    let reasoning_effort = if cfg.max_output_tokens <= 64 {
        "none"
    } else {
        "low"
    };
    let body = json!({
        "model": model,
        "instructions": system,
        "input": user,
        "max_output_tokens": cfg.max_output_tokens,
        "reasoning": { "effort": reasoning_effort },
        "text": { "verbosity": "low" },
        "store": false
    });

    let resp = client
        .post(RESPONSES_URL)
        .bearer_auth(api_key)
        .timeout(cfg.timeout)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let detail = if e.is_timeout() {
                "タイムアウトしました"
            } else {
                "OpenAI への接続に失敗しました"
            };
            AppError::Ai(detail.into())
        })?;

    let status = resp.status();
    if !status.is_success() {
        tracing::warn!(status = status.as_u16(), "OpenAI API エラー応答");
        let msg = match status.as_u16() {
            401 | 403 => "API キーが無効か権限がありません",
            429 => "レート制限に達しました。しばらく待って再試行してください",
            _ => "OpenAI API がエラーを返しました",
        };
        return Err(AppError::Ai(msg.into()));
    }

    let value: Value = resp
        .json()
        .await
        .map_err(|_| AppError::Ai("OpenAI 応答の解析に失敗しました".into()))?;

    let text = value["output"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["type"].as_str() == Some("message"))
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .filter(|part| part["type"].as_str() == Some("output_text"))
        .filter_map(|part| part["text"].as_str())
        .collect::<String>();

    if text.is_empty() {
        return Err(AppError::Ai("OpenAI から本文が返りませんでした".into()));
    }
    Ok(text)
}
