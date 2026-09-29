//! OpenCode Go provider.
//!
//! OpenCode replaced its web console with a client-rendered SPA, so the usage
//! numbers no longer appear in the served HTML. The console exposes JSON
//! endpoints instead:
//!
//! * `GET /console/api/orgs` — the organizations the session can see; the id is
//!   the same `wrk_…` workspace id used by the old console.
//! * `GET /console/api/go/status` — the current OpenCode Go subscription together
//!   with its metering windows. Requires the `x-org-id` header.
//!
//! Authentication uses the console session cookie (`__Host-console_session`),
//! which can be supplied directly or read from a local browser.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::LazyLock,
    time::Duration as StdDuration,
};

use serde::Deserialize;
use serde_json::Value;

use super::{
    sanitize_error_message, sanitized_http_error_message, sanitized_parse_error_message, Provider,
    ProviderConfig, ProviderStatus, HTTP_CONNECT_TIMEOUT_SECS, HTTP_REQUEST_TIMEOUT_SECS,
};
use crate::browser_cookies;

const CONSOLE_API_BASE: &str = "https://opencode.ai/console/api";
const CONSOLE_HOST: &str = "opencode.ai";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

/// Console session cookie names, most specific first.
const SESSION_COOKIE_NAMES: &[&str] = &["__Host-console_session", "console_session"];

/// Workspace/organization ids always carry this prefix.
const WORKSPACE_PREFIX: &str = "wrk_";

/// Org ids are stable for a given session, so remember them per cookie.
static DISCOVERED_ORGS: LazyLock<std::sync::Mutex<HashMap<u64, String>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

pub struct OpenCodeGoProvider;

impl OpenCodeGoProvider {
    pub fn new() -> Self {
        Self
    }
}

// ── API response models ─────────────────────────────────

/// `GET /console/api/go/status`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoStatus {
    #[serde(default)]
    pub product: Option<String>,
    /// Present as an object, but the published schema says array — accept both.
    #[serde(default)]
    pub access: Option<AccessField>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AccessField {
    Many(Vec<Access>),
    One(Box<Access>),
}

impl AccessField {
    pub fn into_first(self) -> Option<Access> {
        match self {
            AccessField::Many(mut list) => list.drain(..).next(),
            AccessField::One(access) => Some(*access),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Access {
    #[serde(default)]
    pub starts_at: Option<String>,
    #[serde(default)]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub cancel_at_period_end: Option<bool>,
    #[serde(default)]
    pub meters: Option<Meters>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meters {
    #[serde(default)]
    pub five_hour: Option<Meter>,
    #[serde(default)]
    pub week: Option<Meter>,
    #[serde(default)]
    pub month: Option<Meter>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meter {
    #[serde(default)]
    pub resets_at: Option<String>,
    #[serde(default)]
    pub limit_micro_cents: Option<FlexibleAmount>,
    #[serde(default)]
    pub used_micro_cents: Option<FlexibleAmount>,
}

/// Micro-cent amounts arrive as strings (BigInt) but numbers show up too.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FlexibleAmount {
    Text(String),
    Integer(i64),
    Float(f64),
}

impl FlexibleAmount {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            FlexibleAmount::Text(value) => value.trim().parse::<f64>().ok(),
            FlexibleAmount::Integer(value) => Some(*value as f64),
            FlexibleAmount::Float(value) => Some(*value),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Organization {
    id: String,
}

// ── Logic ───────────────────────────────────────────────

/// Fraction of a metering window still available (0.0 – 1.0).
///
/// A window that has not started yet reports `usedMicroCents = 0` and null
/// timestamps, which naturally yields a full window.
pub fn remaining_rate(meter: Option<&Meter>) -> Option<f64> {
    let meter = meter?;
    let limit = meter.limit_micro_cents.as_ref().and_then(|v| v.as_f64())?;
    if limit.is_nan() || limit <= 0.0 {
        return None;
    }
    let used = meter
        .used_micro_cents
        .as_ref()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    Some(((limit - used) / limit).clamp(0.0, 1.0))
}

/// Display name for the subscription product.
pub fn plan_name(product: Option<&str>) -> &'static str {
    match product {
        Some("go-plus") => "OpenCode Go Plus",
        _ => "OpenCode Go",
    }
}

pub fn build_status(status: &GoStatus) -> Result<ProviderStatus, anyhow::Error> {
    let Some(access) = status.access.clone().and_then(|field| field.into_first()) else {
        return Ok(ProviderStatus {
            provider_id: "opencodego".into(),
            provider_name: "OpenCode Go".into(),
            available: false,
            remaining_percent: 0.0,
            details: HashMap::new(),
            error: Some("No active OpenCode Go subscription on this workspace".into()),
        });
    };

    let meters = access.meters.clone().unwrap_or_default();
    let five_hour_left = remaining_rate(meters.five_hour.as_ref());
    let weekly_left = remaining_rate(meters.week.as_ref());
    let monthly_left = remaining_rate(meters.month.as_ref());

    let main_left = five_hour_left.or(weekly_left).or(monthly_left).unwrap_or(0.0);

    let mut details = HashMap::new();
    details.insert(
        "plan_name".into(),
        Value::String(plan_name(status.product.as_deref()).into()),
    );
    if let Some(rate) = five_hour_left {
        details.insert("five_hour_usage_left_rate".into(), Value::from(rate));
        if let Some(reset) = meters
            .five_hour
            .as_ref()
            .and_then(|meter| meter.resets_at.clone())
        {
            details.insert("five_hour_usage_reset_time".into(), Value::String(reset));
        }
    }
    if let Some(rate) = weekly_left {
        details.insert("weekly_usage_left_rate".into(), Value::from(rate));
        if let Some(reset) = meters.week.as_ref().and_then(|meter| meter.resets_at.clone()) {
            details.insert("weekly_usage_reset_time".into(), Value::String(reset));
        }
    }
    if let Some(rate) = monthly_left {
        details.insert("monthly_usage_left_rate".into(), Value::from(rate));
        if let Some(reset) = meters
            .month
            .as_ref()
            .and_then(|meter| meter.resets_at.clone())
        {
            details.insert("monthly_usage_reset_time".into(), Value::String(reset));
        }
    }
    if let Some(ends_at) = access.ends_at.clone() {
        details.insert("access_ends_at".into(), Value::String(ends_at));
    }
    if let Some(starts_at) = access.starts_at.clone() {
        details.insert("access_starts_at".into(), Value::String(starts_at));
    }
    if let Some(cancel) = access.cancel_at_period_end {
        details.insert("cancel_at_period_end".into(), Value::Bool(cancel));
    }

    Ok(ProviderStatus {
        provider_id: "opencodego".into(),
        provider_name: "OpenCode Go".into(),
        available: true,
        remaining_percent: (main_left * 100.0).clamp(0.0, 100.0),
        details,
        error: None,
    })
}

// ── Configuration helpers ───────────────────────────────

/// Strip a leading `Cookie:` label and surrounding whitespace.
pub fn normalize_cookie_header(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let value = match trimmed
        .get(..7)
        .filter(|prefix| prefix.eq_ignore_ascii_case("cookie:"))
    {
        Some(_) => trimmed[7..].trim(),
        None => trimmed,
    };
    (!value.is_empty()).then(|| value.to_string())
}

/// Accept a bare id or something like `wrk_…` embedded in a URL.
pub fn normalize_workspace_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with(WORKSPACE_PREFIX) && trimmed.len() > WORKSPACE_PREFIX.len() {
        return Some(trimmed.to_string());
    }
    trimmed
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .find(|part| part.starts_with(WORKSPACE_PREFIX) && part.len() > WORKSPACE_PREFIX.len())
        .map(|part| part.to_string())
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn cache_key(secret: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    secret.hash(&mut hasher);
    hasher.finish()
}

/// Resolve the console cookie, preferring explicit configuration.
async fn resolve_org_id(
    client: &reqwest::Client,
    cookie_header: &str,
    configured: Option<&str>,
) -> Result<String, anyhow::Error> {
    if let Some(id) = configured.and_then(normalize_workspace_id) {
        return Ok(id);
    }

    let key = cache_key(cookie_header);
    if let Some(cached) = DISCOVERED_ORGS
        .lock()
        .ok()
        .and_then(|cache| cache.get(&key).cloned())
    {
        return Ok(cached);
    }

    let response = client
        .get(format!("{CONSOLE_API_BASE}/orgs"))
        .header("Cookie", cookie_header)
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(console_error("OpenCode Go list organizations", status, &body));
    }

    let organizations: Vec<Organization> =
        serde_json::from_str(&body).map_err(|error| {
            anyhow::anyhow!(sanitized_parse_error_message(
                "OpenCode Go list organizations",
                &error,
                &body
            ))
        })?;

    let id = organizations
        .into_iter()
        .next()
        .map(|org| org.id)
        .ok_or_else(|| anyhow::anyhow!("OpenCode Go account has no workspace"))?;

    if let Ok(mut cache) = DISCOVERED_ORGS.lock() {
        cache.insert(key, id.clone());
    }
    Ok(id)
}

async fn fetch_go_status(
    client: &reqwest::Client,
    cookie_header: &str,
    org_id: &str,
) -> Result<GoStatus, anyhow::Error> {
    let response = client
        .get(format!("{CONSOLE_API_BASE}/go/status"))
        .header("Cookie", cookie_header)
        .header("x-org-id", org_id)
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(console_error("OpenCode Go status", status, &body));
    }

    serde_json::from_str(&body).map_err(|error| {
        anyhow::anyhow!(sanitized_parse_error_message(
            "OpenCode Go status",
            &error,
            &body
        ))
    })
}

/// Turn a console API failure into an actionable, redacted error.
fn console_error(action: &str, status: reqwest::StatusCode, body: &str) -> anyhow::Error {
    let tag = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("_tag").and_then(|tag| tag.as_str()).map(str::to_string));

    let hint = match (status.as_u16(), tag.as_deref()) {
        (401, _) => Some(
            "the OpenCode console session is missing or expired; sign in at \
             https://opencode.ai/console and refresh the stored cookie",
        ),
        (403, Some("OrgRequired")) | (400, Some("OrgRequired")) => {
            Some("the workspace id is missing; set it with `--workspace-id wrk_…`")
        }
        (403, _) => Some("this account cannot read the workspace's Go subscription"),
        _ => None,
    };

    let detail = sanitized_http_error_message(action, status, body);
    let message = match hint {
        Some(hint) => format!("{detail} ({hint})"),
        None => detail,
    };

    anyhow::Error::new(ConsoleApiError {
        status: status.as_u16(),
        message,
    })
}

/// Console API failure that keeps its HTTP status, so the caller can tell a
/// stale session apart from a hard failure.
#[derive(Debug)]
pub struct ConsoleApiError {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for ConsoleApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConsoleApiError {}

/// True when the failure means "the session cookie is not accepted".
pub fn is_unauthorized(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<ConsoleApiError>()
        .map(|error| error.status == 401)
        .unwrap_or(false)
}

// ── Provider ────────────────────────────────────────────

/// Cookie taken from explicit configuration, if any.
fn explicit_cookie(config: &ProviderConfig) -> Option<(String, String)> {
    let raw = config
        .cookie_header
        .clone()
        .or_else(|| env_var("CODEXBAR_OPENCODEGO_COOKIE_HEADER"))?;
    normalize_cookie_header(&raw).map(|header| (header, "config/env cookie_header".to_string()))
}

/// Read the session cookie from a local browser.
async fn browser_cookie(config: &ProviderConfig) -> Result<(String, String), anyhow::Error> {
    let preferred = config
        .cookie_browser
        .clone()
        .or_else(|| env_var("CODEXBAR_OPENCODEGO_COOKIE_BROWSER"));

    match browser_cookies::find_cookie(preferred.as_deref(), CONSOLE_HOST, SESSION_COOKIE_NAMES).await
    {
        Ok(Some(cookie)) => {
            let source = format!("{} profile {}", cookie.browser, cookie.profile.display());
            Ok((cookie.header_value(), source))
        }
        Ok(None) => Err(anyhow::anyhow!(
            "no OpenCode console cookie found: set one with \
             `codex-bar-cli config opencodego --cookie-header '<__Host-console_session=…>'` \
             or sign in to https://opencode.ai/console in a supported browser ({}).",
            browser_cookies::known_browsers().join(", ")
        )),
        Err(error) => Err(anyhow::anyhow!("no OpenCode console cookie found: {error}")),
    }
}

async fn load_status(
    client: &reqwest::Client,
    config: &ProviderConfig,
    cookie_header: &str,
) -> Result<ProviderStatus, anyhow::Error> {
    let org_id = resolve_org_id(client, cookie_header, config.workspace_id.as_deref()).await?;
    let status = fetch_go_status(client, cookie_header, &org_id).await?;
    build_status(&status)
}

#[async_trait::async_trait]
impl Provider for OpenCodeGoProvider {
    fn id(&self) -> &'static str {
        "opencodego"
    }

    fn name(&self) -> &'static str {
        "OpenCode Go"
    }

    async fn fetch(&self, config: &ProviderConfig) -> Result<ProviderStatus, anyhow::Error> {
        let client = reqwest::Client::builder()
            .connect_timeout(StdDuration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(StdDuration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .build()?;

        // An explicitly configured cookie wins, but a session that the console
        // rejects should not mask a perfectly good browser session.
        if let Some((header, source)) = explicit_cookie(config) {
            log::debug!("OpenCode Go using console cookie from {source}");
            match load_status(&client, config, &header).await {
                Ok(status) => return Ok(status),
                Err(error) if is_unauthorized(&error) => {
                    log::warn!(
                        "Configured OpenCode Go cookie was rejected; falling back to a local browser session"
                    );
                }
                Err(error) => return Err(error),
            }
        }

        let (header, source) = browser_cookie(config).await?;
        log::debug!(
            "OpenCode Go using console cookie from {}",
            sanitize_error_message(&source)
        );
        load_status(&client, config, &header).await
    }
}

#[cfg(test)]
#[path = "../../tests/unit/providers_opencodego.rs"]
mod tests;
