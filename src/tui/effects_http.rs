use std::time::Duration;

use crate::catalog;
use crate::models::parse_models;

use super::overlay::connect::{KeyError, KeyInfo};

pub(super) async fn validate_key(
    provider: &str,
    key: &str,
    base_url: &str,
) -> Result<KeyInfo, KeyError> {
    let info = catalog::find(provider);
    let url = match info {
        Some(info) => catalog::validate_url(info, Some(base_url)),
        None => format!("{}/models", base_url.trim_end_matches('/')),
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| KeyError {
            status: None,
            message: err.to_string(),
            network: true,
        })?;
    let key = Some(key).filter(|key| !key.is_empty());
    let mut request = client.get(&url);
    if let Some(info) = info {
        for (name, value) in catalog::auth_headers(info, key) {
            request = request.header(name, value);
        }
    } else if let Some(key) = key {
        request = request.header("authorization", format!("Bearer {key}"));
    }
    let response = request.send().await.map_err(|err| KeyError {
        status: None,
        message: err.to_string(),
        network: true,
    })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(KeyError {
            status: Some(status.as_u16()),
            message: error_message(&body),
            network: false,
        });
    }
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let data = value.get("data").unwrap_or(&value);
    Ok(KeyInfo {
        limit_remaining: data.get("limit_remaining").and_then(|v| v.as_f64()),
        limit: data.get("limit").and_then(|v| v.as_f64()),
    })
}

pub(super) async fn fetch_models(
    provider: &str,
    base_override: Option<&str>,
    key: Option<&str>,
) -> Result<Vec<crate::models::ModelInfo>, String> {
    let info = catalog::find(provider);
    let url = match info {
        Some(info) => catalog::models_url(info, base_override, false),
        None => format!(
            "{}/models",
            base_override
                .unwrap_or_else(|| crate::ai::default_base_url(provider))
                .trim_end_matches('/')
        ),
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|err| err.to_string())?;
    let key = key.filter(|key| !key.is_empty());
    let mut request = client.get(&url);
    if let Some(info) = info {
        for (name, value) in catalog::auth_headers(info, key) {
            request = request.header(name, value);
        }
    } else if let Some(key) = key {
        request = request.header("authorization", format!("Bearer {key}"));
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "models: HTTP {} {}",
            status.as_u16(),
            error_message(&body)
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
    Ok(parse_models(&value))
}

fn error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|err| err.get("message"))
                .and_then(|message| message.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(160).collect())
}
