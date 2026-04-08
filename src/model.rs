use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use bytes::Bytes;
use reqwest::Client;
use serde_json::{Value, json};

use crate::models::StoredModelSettings;

pub async fn describe_image(
    http: &Client,
    settings: &StoredModelSettings,
    content_type: &str,
    bytes: &Bytes,
) -> Result<String> {
    let api_key = settings
        .api_key
        .clone()
        .ok_or_else(|| anyhow!("model API key is not configured"))?;
    let data_url = format!(
        "data:{content_type};base64,{}",
        BASE64.encode(bytes.as_ref())
    );
    let payload = json!({
        "model": settings.model.clone(),
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": "Describe this image for knowledge graph indexing. Return a compact paragraph containing the main scene, any visible text, and notable objects."
                },
                {
                    "type": "image_url",
                    "image_url": { "url": data_url }
                }
            ]
        }],
        "temperature": 0.2
    });

    let response = http
        .post(completion_url(&settings.base_url))
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .context("send image description request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("image description failed with {status}: {body}"));
    }

    let value = response
        .json::<Value>()
        .await
        .context("decode image response")?;
    extract_text_from_response_message(&value)
        .ok_or_else(|| anyhow!("image description response contained no text"))
}

fn extract_text_from_response_message(value: &Value) -> Option<String> {
    let message = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))?;
    extract_text_from_content(message.get("content"))
}

fn extract_text_from_content(content: Option<&Value>) -> Option<String> {
    match content {
        Some(Value::String(text)) => Some(text.trim().to_string()),
        Some(Value::Array(parts)) => {
            let text = parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        _ => None,
    }
}

fn completion_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}
