use std::collections::HashMap;

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::{Provider, ProviderConfig, ProviderStatus};

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

// ── Base HTTP helpers ───────────────────────────────────

fn base_headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(reqwest::header::CONTENT_TYPE, "application/json".parse().unwrap());
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
        .header(
            "Cookie",
            format!("INGRESSCOOKIE={}", ingress_cookie),
        )
        .body("{}")
        .send()
        .await?;

    let body_text = reg_resp.text().await?;
    log::debug!("RegisterDevice response: {}", body_text);

    let reg: RegisterDeviceResponse = serde_json::from_str(&body_text)
        .map_err(|e| anyhow::anyhow!("RegisterDevice parse error: {}. Body: {}", e, body_text))?;

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

    let body_text = login_resp.text().await?;
    log::debug!("SignIn response: {}", body_text);

    let login: LoginResponse = serde_json::from_str(&body_text)
        .map_err(|e| anyhow::anyhow!("SignIn parse error: {}. Body: {}", e, body_text))?;

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
async fn query_plan_status(
    client: &reqwest::Client,
    token: &str,
) -> Option<String> {
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
            let body_text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    log::debug!("Plan status response read failed: {}", e);
                    return None;
                }
            };
            log::debug!("Plan status response: {}", body_text);

            let plan: PlanStatusResponse = match serde_json::from_str(&body_text) {
                Ok(p) => p,
                Err(e) => {
                    log::debug!("Plan status parse failed: {}. Body: {}", e, body_text);
                    return None;
                }
            };

            if plan.status != Some(1) {
                log::debug!("Plan status API returned non-success status: {:?}", plan.status);
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

    let body_text = resp.text().await?;
    log::debug!("Rate limit response: {}", body_text);

    let r: RateLimitResponse = serde_json::from_str(&body_text)
        .map_err(|e| anyhow::anyhow!("Rate limit parse error: {}. Body: {}", e, body_text))?;

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

        let client = reqwest::Client::builder().build()?;

        // Step 1: get INGRESSCOOKIE
        let ingress = get_ingress_cookie(&client).await?;

        // Step 2+3: login → token
        let token = full_login(&client, username, password, &ingress).await?;

        // Step 4: query usage
        let data = query_usage(&client, &token).await?;

        // Step 5: query plan status (graceful degradation on failure)
        let plan_name = query_plan_status(&client, &token).await;

        // Check for auth errors
        if data.status != Some(1) {
            let msg = data.message.unwrap_or_else(|| "Unknown error".into());
            // If auth error, re-login and retry once
            if msg.contains("unauthenticated") || msg.contains("embuzzled") || msg.contains("embezzled") {
                log::warn!("Token invalid, re-logging in...");
                let ingress = get_ingress_cookie(&client).await?;
                let token = full_login(&client, username, password, &ingress).await?;
                let data = query_usage(&client, &token).await?;
                if data.status != Some(1) {
                    return Ok(ProviderStatus {
                        provider_id: "stepfun".into(),
                        provider_name: "StepFun".into(),
                        available: false,
                        remaining_percent: 0.0,
                        details: HashMap::new(),
                        error: Some(data.message.unwrap_or_else(|| "Unknown error".into())),
                    });
                }
                // Retry plan status on re-login
                let plan_name = query_plan_status(&client, &token).await;
                return build_status(&data, plan_name.as_deref());
            }
            return Ok(ProviderStatus {
                provider_id: "stepfun".into(),
                provider_name: "StepFun".into(),
                available: false,
                remaining_percent: 0.0,
                details: HashMap::new(),
                error: Some(msg),
            });
        }

        build_status(&data, plan_name.as_deref())
    }
}

fn build_status(data: &RateLimitResponse, plan_name: Option<&str>) -> Result<ProviderStatus, anyhow::Error> {
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
    details.insert("five_hour_usage_left_rate".into(), Value::from(five_hour_left));
    details.insert("weekly_usage_left_rate".into(), Value::from(weekly_left));
    details.insert(
        "five_hour_usage_reset_time".into(),
        Value::String(five_hour_reset),
    );
    details.insert("weekly_usage_reset_time".into(), Value::String(weekly_reset));
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
