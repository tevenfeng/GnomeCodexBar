use std::collections::HashMap;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

pub const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const HTTP_REQUEST_TIMEOUT_SECS: u64 = 60;

/// Shared trait for all provider usage fetchers.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    async fn fetch(&self, config: &ProviderConfig) -> Result<ProviderStatus, anyhow::Error>;
}

/// Per-provider configuration (from config.toml).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    pub enabled: bool,
    // DeepSeek
    pub api_key: Option<String>,
    // StepFun
    pub username: Option<String>,
    pub password: Option<String>,
    // OpenCode Go
    pub cookie_header: Option<String>,
    pub workspace_id: Option<String>,
}

/// Unified status for a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub provider_id: String,
    pub provider_name: String,
    pub available: bool,
    /// Main percentage to display (0-100).
    pub remaining_percent: f64,
    /// Provider-specific detail fields.
    pub details: HashMap<String, serde_json::Value>,
    pub error: Option<String>,
}

/// The full status snapshot written to status.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusSnapshot {
    pub updated_at: String,
    pub providers: Vec<ProviderStatus>,
}

const ERROR_EXCERPT_MAX_CHARS: usize = 240;

/// Redact common credentials from error messages before logging or surfacing them.
pub fn sanitize_error_message(message: &str) -> String {
    let without_query = redact_url_queries(message);
    redact_sensitive_pairs(&without_query)
}

/// Build a short, redacted response excerpt suitable for errors and debug logs.
pub fn sanitize_response_excerpt(body: &str) -> String {
    truncate_chars(&sanitize_error_message(body), ERROR_EXCERPT_MAX_CHARS)
}

/// Build a non-2xx HTTP error message without exposing the full response body.
pub fn sanitized_http_error_message(
    action: &str,
    status: reqwest::StatusCode,
    body: &str,
) -> String {
    format!(
        "{} HTTP {}; body_bytes={}; body_chars={}; excerpt={:?}",
        sanitize_error_message(action),
        status,
        body.len(),
        body.chars().count(),
        sanitize_response_excerpt(body)
    )
}

/// Build a parse error message without exposing the full response body.
pub fn sanitized_parse_error_message(
    action: &str,
    error: &dyn std::fmt::Display,
    body: &str,
) -> String {
    format!(
        "{} parse error: {}; body_bytes={}; body_chars={}; excerpt={:?}",
        sanitize_error_message(action),
        sanitize_error_message(&error.to_string()),
        body.len(),
        body.chars().count(),
        sanitize_response_excerpt(body)
    )
}

fn redact_url_queries(message: &str) -> String {
    let url_re = Regex::new(r#"https?://[^\s\"'<>]+"#).expect("valid url regex");
    url_re
        .replace_all(message, |caps: &Captures| {
            let url = &caps[0];
            match url.split_once('?') {
                Some((base, rest)) => {
                    let fragment = rest.find('#').map(|idx| &rest[idx..]).unwrap_or("");
                    format!("{}?<redacted>{}", base, fragment)
                }
                None => url.to_string(),
            }
        })
        .into_owned()
}

fn redact_sensitive_pairs(message: &str) -> String {
    let mut redacted = message.to_string();
    let patterns = [
        // JSON object/string fields, e.g. "accessToken": {"raw":"..."}
        r#"(?i)"(accessToken|refreshToken|access_token|refresh_token|oasis-token|token|cached_token|cached_ingress_cookie|cookie|set-cookie|cookie_header|password|authorization|api_key)"\s*:\s*(\{[^{}]*\}|"[^"]*"|[^,}\s]+)"#,
        // Header / key-value forms, e.g. Authorization: Bearer ..., Cookie=...
        r#"(?i)\b(accessToken|refreshToken|access_token|refresh_token|oasis-token|token|cached_token|cached_ingress_cookie|cookie|set-cookie|cookie_header|password|authorization|api_key)\b\s*[:=]\s*(Bearer\s+[^,;\s}]+|Basic\s+[^,;\s}]+|"[^"]*"|'[^']*'|[^,;\s}]+)"#,
        // Token-bearing cookie fragments, e.g. Oasis-Token=abc; INGRESSCOOKIE=def
        r#"(?i)\b(Oasis-Token|INGRESSCOOKIE|access_token|refresh_token)\s*=\s*[^,;\s}]+"#,
        // Bearer/basic credentials even when the header name was omitted.
        r#"(?i)\b(Bearer|Basic)\s+[A-Za-z0-9._~+/=-]+"#,
    ];

    for pattern in patterns {
        let re = Regex::new(pattern).expect("valid sensitive-data regex");
        redacted = re
            .replace_all(&redacted, |caps: &Captures| {
                if caps.len() > 2
                    && caps
                        .get(0)
                        .map(|m| m.as_str().contains(':'))
                        .unwrap_or(false)
                {
                    format!("{}: <redacted>", &caps[1])
                } else if caps.len() > 2 {
                    format!("{}=<redacted>", &caps[1])
                } else {
                    "<redacted>".to_string()
                }
            })
            .into_owned();
    }

    redacted
}

fn truncate_chars(message: &str, max_chars: usize) -> String {
    let mut chars = message.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…<truncated>", truncated)
    } else {
        truncated
    }
}

pub mod deepseek;
pub mod opencodego;
pub mod stepfun;

#[cfg(test)]
#[path = "../../tests/unit/providers_mod.rs"]
mod tests;
