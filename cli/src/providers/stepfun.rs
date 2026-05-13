use std::{collections::HashMap, path::PathBuf, time::Duration};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::atomic_write::atomic_write;

use super::{
    sanitize_error_message, sanitize_response_excerpt, sanitized_http_error_message,
    sanitized_parse_error_message, Provider, ProviderConfig, ProviderStatus,
    HTTP_CONNECT_TIMEOUT_SECS, HTTP_REQUEST_TIMEOUT_SECS,
};

// ── Constants (matching C++ stepfun-monitor) ────────────
const WEB_ID: &str = "c8a1002d2c457e758785a9979832217c7c0b884c";
const APP_ID: &str = "10300";
const PLATFORM_URL: &str = "https://platform.stepfun.com";
const API_URL: &str =
    "https://platform.stepfun.com/api/step.openapi.devcenter.Dashboard/QueryStepPlanRateLimit";
const PLAN_STATUS_URL: &str =
    "https://platform.stepfun.com/api/step.openapi.devcenter.Dashboard/GetStepPlanStatus";
const REGISTER_URL: &str =
    "https://platform.stepfun.com/passport/proto.api.passport.v1.PassportService/RegisterDevice";
const LOGIN_URL: &str =
    "https://platform.stepfun.com/passport/proto.api.passport.v1.PassportService/SignInByPassword";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

pub struct StepFunProvider;

impl StepFunProvider {
    pub fn new() -> Self {
        Self
    }
}

// ── Token cache ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StepFunTokenCache {
    token: String,
    updated_at: String,
}

fn stepfun_cache_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gnome-codex-bar")
        .join("stepfun-cache.toml")
}

fn read_token_cache() -> Option<StepFunTokenCache> {
    read_token_cache_from_path(&stepfun_cache_path())
}

fn write_token_cache(token: &str) {
    let cache = StepFunTokenCache {
        token: token.to_string(),
        updated_at: Utc::now().to_rfc3339(),
    };

    if let Err(e) = write_token_cache_to_path(&stepfun_cache_path(), &cache) {
        log::warn!(
            "StepFun token cache write failed: {}",
            sanitize_error_message(&e.to_string())
        );
    }
}

fn clear_token_cache() {
    let path = stepfun_cache_path();
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::debug!("StepFun token cache cleanup failed: {}", e);
        }
    }
}

fn read_token_cache_from_path(path: &std::path::Path) -> Option<StepFunTokenCache> {
    tighten_cache_permissions(path);

    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            log::debug!("StepFun token cache read failed: {}", e);
            return None;
        }
    };

    let cache = match toml::from_str::<StepFunTokenCache>(&contents) {
        Ok(cache) if !cache.token.trim().is_empty() => cache,
        Ok(_) => return None,
        Err(e) => {
            log::debug!("StepFun token cache parse failed: {}", e);
            return None;
        }
    };

    tighten_cache_permissions(path);
    Some(cache)
}

fn write_token_cache_to_path(
    path: &std::path::Path,
    cache: &StepFunTokenCache,
) -> Result<(), anyhow::Error> {
    let contents = toml::to_string(cache)?;
    atomic_write(path, contents)?;
    tighten_cache_permissions(path);
    Ok(())
}

#[cfg(unix)]
fn tighten_cache_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let current = metadata.permissions().mode() & 0o777;
        if current != 0o600 {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

#[cfg(not(unix))]
fn tighten_cache_permissions(_path: &std::path::Path) {}

// ── Base HTTP helpers ───────────────────────────────────

fn base_headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        reqwest::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    h.insert(
        reqwest::header::HeaderName::from_static("oasis-appid"),
        APP_ID.parse().unwrap(),
    );
    h.insert(
        reqwest::header::HeaderName::from_static("oasis-platform"),
        "web".parse().unwrap(),
    );
    h.insert(
        reqwest::header::HeaderName::from_static("oasis-webid"),
        WEB_ID.parse().unwrap(),
    );
    h.insert(reqwest::header::USER_AGENT, UA.parse().unwrap());
    h.insert(
        reqwest::header::HeaderName::from_static("connect-protocol-version"),
        "1".parse().unwrap(),
    );
    h
}

fn get_headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(reqwest::header::USER_AGENT, UA.parse().unwrap());
    h.insert(
        reqwest::header::HeaderName::from_static("oasis-appid"),
        APP_ID.parse().unwrap(),
    );
    h.insert(
        reqwest::header::HeaderName::from_static("oasis-platform"),
        "web".parse().unwrap(),
    );
    h.insert(
        reqwest::header::HeaderName::from_static("oasis-webid"),
        WEB_ID.parse().unwrap(),
    );
    h
}

fn extract_set_cookie(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    for (key, value) in headers.iter() {
        if key.as_str().eq_ignore_ascii_case("set-cookie") {
            if let Ok(cookie_str) = value.to_str() {
                for part in cookie_str.split(';') {
                    let part = part.trim();
                    if let Some(val) = part.strip_prefix(&format!("{}=", name)) {
                        return Some(val.trim().to_string());
                    }
                }
            }
        }
    }
    None
}

async fn read_success_text(resp: reqwest::Response, action: &str) -> Result<String, anyhow::Error> {
    let status = resp.status();
    let body_text = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(sanitized_http_error_message(
            action, status, &body_text
        )));
    }
    Ok(body_text)
}

fn parse_json<T: serde::de::DeserializeOwned>(
    action: &str,
    body_text: &str,
) -> Result<T, anyhow::Error> {
    serde_json::from_str(body_text)
        .map_err(|e| anyhow::anyhow!(sanitized_parse_error_message(action, &e, body_text)))
}

// ── Auth response types ─────────────────────────────────

#[derive(Debug, Deserialize)]
struct TokenPair {
    raw: String,
}

#[derive(Debug, Deserialize)]
struct RegisterDeviceResponse {
    #[serde(rename = "accessToken")]
    access_token: Option<TokenPair>,
    #[serde(rename = "refreshToken")]
    refresh_token: Option<TokenPair>,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    #[serde(rename = "accessToken")]
    access_token: Option<TokenPair>,
    #[serde(rename = "refreshToken")]
    refresh_token: Option<TokenPair>,
}

// ── Plan status response types ───────────────────────────

#[derive(Debug, Deserialize)]
struct PlanStatusSubscription {
    name: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "plan_type")]
    plan_type: Option<i64>,
    #[allow(dead_code)]
    #[serde(rename = "status")]
    plan_status: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PlanStatusResponse {
    status: Option<i32>,
    subscription: Option<PlanStatusSubscription>,
}

// ── Rate limit response types ───────────────────────────

#[derive(Debug, Deserialize)]
struct RateLimitResponse {
    status: Option<i32>,
    #[serde(default)]
    #[allow(dead_code)]
    code: Option<FlexibleIntOrString>,
    #[serde(default)]
    message: Option<String>,
    five_hour_usage_left_rate: Option<FlexibleNumber>,
    weekly_usage_left_rate: Option<FlexibleNumber>,
    five_hour_usage_reset_time: Option<FlexibleTimestamp>,
    weekly_usage_reset_time: Option<FlexibleTimestamp>,
}

/// Accepts both int and string for the `code` field (API returns "unauthenticated" on error).
#[derive(Debug)]
#[allow(dead_code)]
struct FlexibleIntOrString(i64);

impl<'de> Deserialize<'de> for FlexibleIntOrString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v: Value = Deserialize::deserialize(deserializer)?;
        match v {
            Value::Number(n) => Ok(FlexibleIntOrString(n.as_i64().unwrap_or(0))),
            Value::String(_s) => Ok(FlexibleIntOrString(0)),
            _ => Ok(FlexibleIntOrString(0)),
        }
    }
}

/// Accepts both int and float (API returns 1 or 0.99781543).
#[derive(Debug)]
struct FlexibleNumber {
    value: f64,
}

impl<'de> Deserialize<'de> for FlexibleNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v: Value = Deserialize::deserialize(deserializer)?;
        match v {
            Value::Number(n) => Ok(FlexibleNumber {
                value: n.as_f64().unwrap_or(0.0),
            }),
            Value::String(s) => Ok(FlexibleNumber {
                value: s.parse().unwrap_or(0.0),
            }),
            _ => Ok(FlexibleNumber { value: 0.0 }),
        }
    }
}

/// Accepts both string and int timestamps (API returns "1777528800").
#[derive(Debug)]
struct FlexibleTimestamp {
    value: i64,
}

impl<'de> Deserialize<'de> for FlexibleTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v: Value = Deserialize::deserialize(deserializer)?;
        match v {
            Value::Number(n) => Ok(FlexibleTimestamp {
                value: n.as_i64().unwrap_or(0),
            }),
            Value::String(s) => Ok(FlexibleTimestamp {
                value: s.parse().unwrap_or(0),
            }),
            _ => Ok(FlexibleTimestamp { value: 0 }),
        }
    }
}

fn parse_timestamp(ts: i64) -> String {
    if ts == 0 {
        return "Unknown".into();
    }
    // API returns second-level timestamps
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|dt: DateTime<Utc>| dt.to_rfc3339())
        .unwrap_or_else(|| "Unknown".into())
}

// ── Login flow ──────────────────────────────────────────

/// Step 1: GET homepage → extract INGRESSCOOKIE from Set-Cookie.
async fn get_ingress_cookie(client: &reqwest::Client) -> Result<String, anyhow::Error> {
    let resp = client
        .get(PLATFORM_URL)
        .headers(get_headers())
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await?;
        return Err(anyhow::anyhow!(sanitized_http_error_message(
            "StepFun get ingress cookie",
            status,
            &body_text
        )));
    }

    extract_set_cookie(resp.headers(), "INGRESSCOOKIE")
        .ok_or_else(|| anyhow::anyhow!("Failed to get INGRESSCOOKIE from {}", PLATFORM_URL))
}

/// Step 2 + 3: RegisterDevice + SignInByPassword → Oasis-Token.
async fn full_login(
    client: &reqwest::Client,
    username: &str,
    password: &str,
    ingress_cookie: &str,
) -> Result<String, anyhow::Error> {
    // Step 2: RegisterDevice
    let reg_resp = client
        .post(REGISTER_URL)
        .headers(base_headers())
        .header("Cookie", format!("INGRESSCOOKIE={}", ingress_cookie))
        .body("{}")
        .send()
        .await?;

    let body_text = read_success_text(reg_resp, "StepFun RegisterDevice").await?;
    log::debug!(
        "RegisterDevice response excerpt: {}",
        sanitize_response_excerpt(&body_text)
    );

    let reg: RegisterDeviceResponse = parse_json("StepFun RegisterDevice", &body_text)?;

    let access = reg
        .access_token
        .ok_or_else(|| anyhow::anyhow!("RegisterDevice: no accessToken"))?;
    let refresh = reg
        .refresh_token
        .ok_or_else(|| anyhow::anyhow!("RegisterDevice: no refreshToken"))?;
    let anon_token = format!("{}...{}", access.raw, refresh.raw);
    log::info!("Got anonymous token");

    // Step 3: SignInByPassword
    let login_body = serde_json::json!({
        "username": username,
        "password": password,
    });

    let login_cookie = format!(
        "Oasis-Token={}; Oasis-Webid={}; INGRESSCOOKIE={}",
        anon_token, WEB_ID, ingress_cookie
    );

    let login_resp = client
        .post(LOGIN_URL)
        .headers(base_headers())
        .header("Cookie", &login_cookie)
        .json(&login_body)
        .send()
        .await?;

    let body_text = read_success_text(login_resp, "StepFun SignInByPassword").await?;
    log::debug!(
        "SignIn response excerpt: {}",
        sanitize_response_excerpt(&body_text)
    );

    let login: LoginResponse = parse_json("StepFun SignInByPassword", &body_text)?;

    let access = login
        .access_token
        .ok_or_else(|| anyhow::anyhow!("Login failed: no accessToken"))?;
    let refresh = login
        .refresh_token
        .ok_or_else(|| anyhow::anyhow!("Login failed: no refreshToken"))?;
    let token = format!("{}...{}", access.raw, refresh.raw);
    log::info!("Login successful");

    Ok(token)
}

/// Query plan status (subscription name) with Oasis-Token.
async fn query_plan_status(client: &reqwest::Client, token: &str) -> Option<String> {
    let cookie = format!("Oasis-Token={}; Oasis-Webid={}", token, WEB_ID);

    let resp = client
        .post(PLAN_STATUS_URL)
        .headers(base_headers())
        .header("Cookie", &cookie)
        .body("{}")
        .send()
        .await;

    match resp {
        Ok(resp) => {
            let status = resp.status();
            let body_text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    log::debug!("Plan status response read failed: {}", e);
                    return None;
                }
            };
            if !status.is_success() {
                log::debug!(
                    "{}",
                    sanitized_http_error_message("StepFun plan status", status, &body_text)
                );
                return None;
            }
            log::debug!(
                "Plan status response excerpt: {}",
                sanitize_response_excerpt(&body_text)
            );

            let plan: PlanStatusResponse = match parse_json("StepFun plan status", &body_text) {
                Ok(p) => p,
                Err(e) => {
                    log::debug!("Plan status parse failed: {}", e);
                    return None;
                }
            };

            if plan.status != Some(1) {
                log::debug!(
                    "Plan status API returned non-success status: {:?}",
                    plan.status
                );
                return None;
            }

            plan.subscription
                .and_then(|s| s.name)
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
        }
        Err(e) => {
            log::debug!("Plan status request failed: {}", e);
            None
        }
    }
}

/// Query rate limits with Oasis-Token (no INGRESSCOOKIE needed).
async fn query_usage(
    client: &reqwest::Client,
    token: &str,
) -> Result<RateLimitResponse, anyhow::Error> {
    let cookie = format!("Oasis-Token={}; Oasis-Webid={}", token, WEB_ID);

    let resp = client
        .post(API_URL)
        .headers(base_headers())
        .header("Cookie", &cookie)
        .body("{}")
        .send()
        .await?;

    let body_text = read_success_text(resp, "StepFun rate limit").await?;
    log::debug!(
        "Rate limit response excerpt: {}",
        sanitize_response_excerpt(&body_text)
    );

    let r: RateLimitResponse = parse_json("StepFun rate limit", &body_text)?;

    Ok(r)
}

#[async_trait::async_trait]
impl Provider for StepFunProvider {
    fn id(&self) -> &'static str {
        "stepfun"
    }

    fn name(&self) -> &'static str {
        "StepFun"
    }

    async fn fetch(&self, config: &ProviderConfig) -> Result<ProviderStatus, anyhow::Error> {
        let username = config
            .username
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("StepFun username not configured"))?;
        let password = config
            .password
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("StepFun password not configured"))?;

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .build()?;

        if let Some(cache) = read_token_cache() {
            let data = match query_usage(&client, &cache.token).await {
                Ok(data) => data,
                Err(e) if is_auth_transport_error(&e.to_string()) => {
                    log::warn!("StepFun cached token rejected, re-logging in...");
                    clear_token_cache();
                    let token = login_with_fresh_ingress(&client, username, password).await?;
                    let data = query_usage(&client, &token).await?;
                    if data.status == Some(1) {
                        write_token_cache(&token);
                    }
                    return finish_usage_status(&client, &token, data).await;
                }
                Err(e) => return Err(e),
            };
            if data.status == Some(1) {
                let plan_name = query_plan_status(&client, &cache.token).await;
                return build_status(&data, plan_name.as_deref());
            }

            if !is_auth_error(&data) {
                return unavailable_status(rate_limit_message(data));
            }

            log::warn!("StepFun cached token invalid, re-logging in...");
            clear_token_cache();
        }

        let token = login_with_fresh_ingress(&client, username, password).await?;
        let data = query_usage(&client, &token).await?;
        if data.status == Some(1) {
            write_token_cache(&token);
        } else if is_auth_error(&data) {
            clear_token_cache();
        }
        finish_usage_status(&client, &token, data).await
    }
}

async fn finish_usage_status(
    client: &reqwest::Client,
    token: &str,
    data: RateLimitResponse,
) -> Result<ProviderStatus, anyhow::Error> {
    if data.status != Some(1) {
        return unavailable_status(rate_limit_message(data));
    }

    let plan_name = query_plan_status(client, token).await;
    build_status(&data, plan_name.as_deref())
}

async fn login_with_fresh_ingress(
    client: &reqwest::Client,
    username: &str,
    password: &str,
) -> Result<String, anyhow::Error> {
    let ingress = get_ingress_cookie(client).await?;
    full_login(client, username, password, &ingress).await
}

fn is_auth_error(data: &RateLimitResponse) -> bool {
    let message = data.message.as_deref().unwrap_or_default();
    let message = message.to_ascii_lowercase();
    message.contains("unauthenticated")
        || message.contains("embuzzled")
        || message.contains("embezzled")
}

fn is_auth_transport_error(message: &str) -> bool {
    message.contains("HTTP 401") || message.contains("HTTP 403")
}

fn rate_limit_message(data: RateLimitResponse) -> String {
    data.message.unwrap_or_else(|| "Unknown error".into())
}

fn unavailable_status(message: String) -> Result<ProviderStatus, anyhow::Error> {
    Ok(ProviderStatus {
        provider_id: "stepfun".into(),
        provider_name: "StepFun".into(),
        available: false,
        remaining_percent: 0.0,
        details: HashMap::new(),
        error: Some(sanitize_error_message(&message)),
    })
}

fn build_status(
    data: &RateLimitResponse,
    plan_name: Option<&str>,
) -> Result<ProviderStatus, anyhow::Error> {
    let five_hour_left = data
        .five_hour_usage_left_rate
        .as_ref()
        .map(|r| r.value)
        .unwrap_or(1.0);
    let weekly_left = data
        .weekly_usage_left_rate
        .as_ref()
        .map(|r| r.value)
        .unwrap_or(1.0);

    let five_hour_reset = data
        .five_hour_usage_reset_time
        .as_ref()
        .map(|t| t.value)
        .map(parse_timestamp)
        .unwrap_or_default();
    let weekly_reset = data
        .weekly_usage_reset_time
        .as_ref()
        .map(|t| t.value)
        .map(parse_timestamp)
        .unwrap_or_default();

    // C++ uses remaining_percent = left_rate * 100 (how much is LEFT)
    let remaining_percent = (five_hour_left * 100.0).clamp(0.0, 100.0);

    let mut details = HashMap::new();
    details.insert(
        "five_hour_usage_left_rate".into(),
        Value::from(five_hour_left),
    );
    details.insert("weekly_usage_left_rate".into(), Value::from(weekly_left));
    details.insert(
        "five_hour_usage_reset_time".into(),
        Value::String(five_hour_reset),
    );
    details.insert(
        "weekly_usage_reset_time".into(),
        Value::String(weekly_reset),
    );
    if let Some(name) = plan_name {
        details.insert("plan_name".into(), Value::String(name.to_string()));
    }

    Ok(ProviderStatus {
        provider_id: "stepfun".into(),
        provider_name: "StepFun".into(),
        available: true,
        remaining_percent,
        details,
        error: None,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/providers_stepfun.rs"]
mod tests;
