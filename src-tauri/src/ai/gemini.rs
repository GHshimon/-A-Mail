//! Gemini REST 呼び出し(generateContent)。M3(§7)。
//!
//! - 認証はヘッダ `x-goog-api-key`(URL クエリに載せない=ログ混入回避)。
//! - リクエスト本文(メール本文を含む)や API キーは **ログに出さない**。
//! - エラーはステータス種別のみに丸める(応答本文にプロンプトが反映され得るため載せない)。
//! - ストリーミング(:streamGenerateContent)は将来対応。まずは非ストリームで A/B/C を実現。

use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

const BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// 生成パラメータ。
pub struct GenConfig {
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub stop: Vec<String>,
    pub timeout: Duration,
}

impl GenConfig {
    /// A: 予測入力(短く・低温)。
    pub fn completion() -> Self {
        Self {
            temperature: 0.2,
            max_output_tokens: 48,
            stop: vec!["\n\n".into()],
            timeout: Duration::from_secs(6),
        }
    }
    /// B/C: 要約・返信(やや長め)。
    pub fn generation() -> Self {
        Self {
            temperature: 0.4,
            max_output_tokens: 512,
            stop: vec![],
            timeout: Duration::from_secs(20),
        }
    }
}

/// `generateContent` を呼び、最初の候補のテキストを返す。
pub async fn generate_content(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    cfg: &GenConfig,
) -> AppResult<String> {
    let url = format!("{BASE}/{model}:generateContent");

    let mut generation_config = json!({
        "temperature": cfg.temperature,
        "maxOutputTokens": cfg.max_output_tokens,
    });
    if !cfg.stop.is_empty() {
        generation_config["stopSequences"] = json!(cfg.stop);
    }

    let body = json!({
        "systemInstruction": { "parts": [{ "text": system }] },
        "contents": [{ "role": "user", "parts": [{ "text": user }] }],
        "generationConfig": generation_config,
    });

    let resp = client
        .post(&url)
        .header("x-goog-api-key", api_key)
        .timeout(cfg.timeout)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let detail = if e.is_timeout() {
                "タイムアウトしました"
            } else {
                "Gemini への接続に失敗しました"
            };
            AppError::Ai(detail.into())
        })?;

    let status = resp.status();
    if !status.is_success() {
        // 応答本文はプロンプトを含み得るため載せない。ステータスのみ。
        tracing::warn!(status = status.as_u16(), "Gemini API エラー応答");
        let msg = match status.as_u16() {
            401 | 403 => "API キーが無効か権限がありません",
            429 => "レート制限に達しました。しばらく待って再試行してください",
            _ => "Gemini API がエラーを返しました",
        };
        return Err(AppError::Ai(msg.into()));
    }

    let v: Value = resp
        .json()
        .await
        .map_err(|_| AppError::Ai("Gemini 応答の解析に失敗しました".into()))?;

    // candidates[0].content.parts[*].text を連結。
    let text = v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<String>()
        })
        .unwrap_or_default();

    Ok(text)
}
