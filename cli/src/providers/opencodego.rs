use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use serde_json::Value;

use super::{Provider, ProviderConfig, ProviderStatus};

const BASE_URL: &str = "https://opencode.ai";
const SERVER_URL: &str = "https://opencode.ai/_server";
const WORKSPACES_SERVER_ID: &str =
    "def39973159c7f0483d8793a822b8dbb10d067e12c65455fcb4608459ba0234f";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

const PERCENT_KEYS: &[&str] = &[
    "usagePercent",
    "usedPercent",
    "percentUsed",
    "percent",
    "usage_percent",
    "used_percent",
    "utilization",
    "utilizationPercent",
    "utilization_percent",
    "usage",
];
const RESET_IN_KEYS: &[&str] = &[
    "resetInSec",
    "resetInSeconds",
    "resetSeconds",
    "reset_sec",
    "reset_in_sec",
    "resetsInSec",
    "resetsInSeconds",
    "resetIn",
    "resetSec",
];
const RESET_AT_KEYS: &[&str] = &[
    "resetAt",
    "resetsAt",
    "reset_at",
    "resets_at",
    "nextReset",
    "next_reset",
    "renewAt",
    "renew_at",
];

pub struct OpenCodeGoProvider;

impl OpenCodeGoProvider {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone)]
struct UsageSnapshot {
    rolling_usage_percent: f64,
    weekly_usage_percent: f64,
    monthly_usage_percent: Option<f64>,
    rolling_reset_in_sec: i64,
    weekly_reset_in_sec: i64,
    monthly_reset_in_sec: Option<i64>,
}

#[derive(Debug, Clone)]
struct WindowUsage {
    percent: f64,
    reset_in_sec: i64,
    path_lower: String,
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
        let cookie_header = resolve_cookie_header(config)?;
        let client = reqwest::Client::builder().build()?;
        let workspace_id = match normalize_workspace_id(config.workspace_id.as_deref())
            .or_else(|| env_workspace_id().and_then(|id| normalize_workspace_id(Some(&id))))
        {
            Some(id) => id,
            None => fetch_workspace_id(&client, &cookie_header).await?,
        };
        let text = fetch_usage_page(&client, &workspace_id, &cookie_header).await?;
        let snapshot = parse_subscription(&text, Utc::now())?;
        build_status(&snapshot, Utc::now())
    }
}

fn resolve_cookie_header(config: &ProviderConfig) -> Result<String, anyhow::Error> {
    let raw = config
        .cookie_header
        .clone()
        .or_else(env_cookie_header)
        .ok_or_else(|| anyhow::anyhow!("OpenCode Go cookie_header not configured"))?;
    request_cookie_header(&raw)
        .ok_or_else(|| anyhow::anyhow!("OpenCode Go cookie is invalid or empty"))
}

fn env_cookie_header() -> Option<String> {
    std::env::var("CODEXBAR_OPENCODEGO_COOKIE_HEADER")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env_workspace_id() -> Option<String> {
    std::env::var("CODEXBAR_OPENCODEGO_WORKSPACE_ID")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn request_cookie_header(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(cookie) = trimmed.strip_prefix("Cookie:") {
        let cookie = cookie.trim();
        return (!cookie.is_empty()).then(|| cookie.to_string());
    }
    Some(trimmed.to_string())
}

fn normalize_workspace_id(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.starts_with("wrk_") && trimmed.len() > 4 {
        return Some(trimmed.to_string());
    }
    let re = Regex::new(r"wrk_[A-Za-z0-9]+").ok()?;
    re.find(trimmed).map(|m| m.as_str().to_string())
}

async fn fetch_workspace_id(
    client: &reqwest::Client,
    cookie_header: &str,
) -> Result<String, anyhow::Error> {
    let text = fetch_server_text(client, cookie_header, "GET", None).await?;
    if looks_signed_out(&text) {
        return Err(anyhow::anyhow!(
            "OpenCode Go session cookie is invalid or expired"
        ));
    }
    let mut ids = parse_workspace_ids(&text);
    if ids.is_empty() {
        ids = parse_workspace_ids_from_json(&text);
    }
    if ids.is_empty() {
        let fallback = fetch_server_text(client, cookie_header, "POST", Some("[]")).await?;
        if looks_signed_out(&fallback) {
            return Err(anyhow::anyhow!(
                "OpenCode Go session cookie is invalid or expired"
            ));
        }
        ids = parse_workspace_ids(&fallback);
        if ids.is_empty() {
            ids = parse_workspace_ids_from_json(&fallback);
        }
    }
    ids.into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("OpenCode Go workspace id not found"))
}

async fn fetch_server_text(
    client: &reqwest::Client,
    cookie_header: &str,
    method: &str,
    args: Option<&str>,
) -> Result<String, anyhow::Error> {
    let mut request = if method == "GET" {
        let mut url = format!("{}?id={}", SERVER_URL, WORKSPACES_SERVER_ID);
        if let Some(args) = args {
            url.push_str("&args=");
            url.push_str(args);
        }
        client.get(url)
    } else {
        client.post(SERVER_URL)
    };
    request = request
        .header("Cookie", cookie_header)
        .header("X-Server-Id", WORKSPACES_SERVER_ID)
        .header(
            "X-Server-Instance",
            format!(
                "server-fn:{}",
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
        )
        .header("User-Agent", USER_AGENT)
        .header("Origin", BASE_URL)
        .header("Referer", BASE_URL)
        .header(
            "Accept",
            "text/javascript, application/json;q=0.9, */*;q=0.8",
        );
    if method != "GET" {
        request = request
            .header("Content-Type", "application/json")
            .body(args.unwrap_or("[]").to_string());
    }
    let resp = request.send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        if status.as_u16() == 401 || status.as_u16() == 403 || looks_signed_out(&text) {
            return Err(anyhow::anyhow!(
                "OpenCode Go session cookie is invalid or expired"
            ));
        }
        return Err(anyhow::anyhow!(
            "OpenCode Go API error: HTTP {}",
            status.as_u16()
        ));
    }
    Ok(text)
}

async fn fetch_usage_page(
    client: &reqwest::Client,
    workspace_id: &str,
    cookie_header: &str,
) -> Result<String, anyhow::Error> {
    let url = format!("{}/workspace/{}/go", BASE_URL, workspace_id);
    let resp = client
        .get(url)
        .header("Cookie", cookie_header)
        .header("User-Agent", USER_AGENT)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        if status.as_u16() == 401 || status.as_u16() == 403 || looks_signed_out(&text) {
            return Err(anyhow::anyhow!(
                "OpenCode Go session cookie is invalid or expired"
            ));
        }
        return Err(anyhow::anyhow!(
            "OpenCode Go API error: HTTP {}",
            status.as_u16()
        ));
    }
    if looks_signed_out(&text) {
        return Err(anyhow::anyhow!(
            "OpenCode Go session cookie is invalid or expired"
        ));
    }
    Ok(text)
}

fn looks_signed_out(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("login")
        || lower.contains("sign in")
        || lower.contains("auth/authorize")
        || lower.contains("not associated with an account")
        || lower.contains("actor of type \"public\"")
}

fn parse_workspace_ids(text: &str) -> Vec<String> {
    Regex::new(r#"id\s*:\s*\"(wrk_[^\"]+)\""#)
        .ok()
        .map(|re| {
            re.captures_iter(text)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_workspace_ids_from_json(text: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    collect_workspace_ids(&value, &mut ids);
    ids
}

fn collect_workspace_ids(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::String(s) if s.starts_with("wrk_") && !ids.contains(s) => ids.push(s.clone()),
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_workspace_ids(item, ids)),
        Value::Object(map) => map
            .values()
            .for_each(|item| collect_workspace_ids(item, ids)),
        _ => {}
    }
}

fn parse_subscription(text: &str, now: DateTime<Utc>) -> Result<UsageSnapshot, anyhow::Error> {
    if let Some(snapshot) = parse_subscription_json(text, now) {
        return Ok(snapshot);
    }

    let rolling_percent = extract_double(
        r#"[\"']?rollingUsage[\"']?[^}]*?[\"']?usagePercent[\"']?\s*:\s*[\"']?([0-9]+(?:\.[0-9]+)?)[\"']?"#,
        text,
    )
    .ok_or_else(|| anyhow::anyhow!("OpenCode Go usage parse error: missing rolling usage"))?;
    let rolling_reset = extract_i64(
        r#"[\"']?rollingUsage[\"']?[^}]*?[\"']?resetInSec[\"']?\s*:\s*[\"']?([0-9]+)[\"']?"#,
        text,
    )
    .ok_or_else(|| anyhow::anyhow!("OpenCode Go usage parse error: missing rolling reset"))?;
    let weekly_percent = extract_double(
        r#"[\"']?weeklyUsage[\"']?[^}]*?[\"']?usagePercent[\"']?\s*:\s*[\"']?([0-9]+(?:\.[0-9]+)?)[\"']?"#,
        text,
    )
    .ok_or_else(|| anyhow::anyhow!("OpenCode Go usage parse error: missing weekly usage"))?;
    let weekly_reset = extract_i64(
        r#"[\"']?weeklyUsage[\"']?[^}]*?[\"']?resetInSec[\"']?\s*:\s*[\"']?([0-9]+)[\"']?"#,
        text,
    )
    .ok_or_else(|| anyhow::anyhow!("OpenCode Go usage parse error: missing weekly reset"))?;
    let monthly_percent = extract_double(
        r#"[\"']?monthlyUsage[\"']?[^}]*?[\"']?usagePercent[\"']?\s*:\s*[\"']?([0-9]+(?:\.[0-9]+)?)[\"']?"#,
        text,
    );
    let monthly_reset = extract_i64(
        r#"[\"']?monthlyUsage[\"']?[^}]*?[\"']?resetInSec[\"']?\s*:\s*[\"']?([0-9]+)[\"']?"#,
        text,
    );

    Ok(UsageSnapshot {
        rolling_usage_percent: clamp_percent(rolling_percent),
        weekly_usage_percent: clamp_percent(weekly_percent),
        monthly_usage_percent: monthly_percent.map(clamp_percent),
        rolling_reset_in_sec: rolling_reset.max(0),
        weekly_reset_in_sec: weekly_reset.max(0),
        monthly_reset_in_sec: monthly_reset.map(|v| v.max(0)),
    })
}

fn parse_subscription_json(text: &str, now: DateTime<Utc>) -> Option<UsageSnapshot> {
    let value: Value = serde_json::from_str(text).ok()?;
    parse_usage_dictionary(&value, now).or_else(|| parse_usage_from_candidates(&value, now))
}

fn parse_usage_dictionary(value: &Value, now: DateTime<Utc>) -> Option<UsageSnapshot> {
    let obj = value.as_object()?;
    if let Some(usage) = obj.get("usage") {
        if let Some(snapshot) = parse_usage_dictionary(usage, now) {
            return Some(snapshot);
        }
    }
    for key in ["data", "result", "billing", "payload"] {
        if let Some(nested) = obj.get(key).and_then(|v| parse_usage_dictionary(v, now)) {
            return Some(nested);
        }
    }
    let rolling = first_object(
        value,
        &[
            "rollingUsage",
            "rolling",
            "rolling_usage",
            "rollingWindow",
            "rolling_window",
        ],
    );
    let weekly = first_object(
        value,
        &[
            "weeklyUsage",
            "weekly",
            "weekly_usage",
            "weeklyWindow",
            "weekly_window",
        ],
    );
    let monthly = first_object(
        value,
        &[
            "monthlyUsage",
            "monthly",
            "monthly_usage",
            "monthlyWindow",
            "monthly_window",
        ],
    );
    if let (Some(rolling), Some(weekly)) = (rolling, weekly) {
        return build_snapshot_from_windows(rolling, weekly, monthly, now);
    }
    parse_usage_nested(value, now, 0)
}

fn parse_usage_nested(value: &Value, now: DateTime<Utc>, depth: usize) -> Option<UsageSnapshot> {
    if depth > 3 {
        return None;
    }
    let obj = value.as_object()?;
    let mut rolling = None;
    let mut weekly = None;
    let mut monthly = None;
    for (key, child) in obj {
        if !child.is_object() {
            continue;
        }
        let lower = key.to_lowercase();
        if lower.contains("rolling")
            || lower.contains("hour")
            || lower.contains("5h")
            || lower.contains("5-hour")
        {
            rolling = Some(child);
        } else if lower.contains("weekly") || lower.contains("week") {
            weekly = Some(child);
        } else if lower.contains("monthly") || lower.contains("month") {
            monthly = Some(child);
        }
    }
    if let (Some(rolling), Some(weekly)) = (rolling, weekly) {
        if let Some(snapshot) = build_snapshot_from_windows(rolling, weekly, monthly, now) {
            return Some(snapshot);
        }
    }
    obj.values().find_map(|child| {
        child
            .is_object()
            .then(|| parse_usage_nested(child, now, depth + 1))
            .flatten()
    })
}

fn parse_usage_from_candidates(value: &Value, now: DateTime<Utc>) -> Option<UsageSnapshot> {
    let mut candidates = Vec::new();
    collect_window_candidates(value, now, String::new(), &mut candidates);
    if candidates.is_empty() {
        return None;
    }
    let rolling = pick_window(
        &candidates,
        |p| p.contains("rolling") || p.contains("hour") || p.contains("5h") || p.contains("5-hour"),
        true,
        &[],
    )?;
    let weekly = pick_window(
        &candidates,
        |p| p.contains("weekly") || p.contains("week"),
        false,
        &[rolling.path_lower.as_str()],
    )?;
    let monthly = pick_window(
        &candidates,
        |p| p.contains("monthly") || p.contains("month"),
        false,
        &[rolling.path_lower.as_str(), weekly.path_lower.as_str()],
    );
    Some(UsageSnapshot {
        rolling_usage_percent: rolling.percent,
        weekly_usage_percent: weekly.percent,
        monthly_usage_percent: monthly.as_ref().map(|w| w.percent),
        rolling_reset_in_sec: rolling.reset_in_sec,
        weekly_reset_in_sec: weekly.reset_in_sec,
        monthly_reset_in_sec: monthly.map(|w| w.reset_in_sec),
    })
}

fn first_object<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let obj = value.as_object()?;
    keys.iter()
        .find_map(|key| obj.get(*key).filter(|v| v.is_object()))
}

fn build_snapshot_from_windows(
    rolling: &Value,
    weekly: &Value,
    monthly: Option<&Value>,
    now: DateTime<Utc>,
) -> Option<UsageSnapshot> {
    let rolling = parse_window(rolling, now)?;
    let weekly = parse_window(weekly, now)?;
    let monthly = monthly.and_then(|v| parse_window(v, now));
    Some(UsageSnapshot {
        rolling_usage_percent: rolling.percent,
        weekly_usage_percent: weekly.percent,
        monthly_usage_percent: monthly.as_ref().map(|w| w.percent),
        rolling_reset_in_sec: rolling.reset_in_sec,
        weekly_reset_in_sec: weekly.reset_in_sec,
        monthly_reset_in_sec: monthly.map(|w| w.reset_in_sec),
    })
}

fn parse_window(value: &Value, now: DateTime<Utc>) -> Option<WindowUsage> {
    let obj = value.as_object()?;
    let mut percent = PERCENT_KEYS
        .iter()
        .find_map(|key| double_value(obj.get(*key)));
    if percent.is_none() {
        let used = ["used", "usage", "consumed", "count", "usedTokens"]
            .iter()
            .find_map(|key| double_value(obj.get(*key)));
        let limit = ["limit", "total", "quota", "max", "cap", "tokenLimit"]
            .iter()
            .find_map(|key| double_value(obj.get(*key)));
        if let (Some(used), Some(limit)) = (used, limit) {
            if limit > 0.0 {
                percent = Some((used / limit) * 100.0);
            }
        }
    }
    let mut percent = percent?;
    if (0.0..=1.0).contains(&percent) {
        percent *= 100.0;
    }
    let reset_in_sec = RESET_IN_KEYS
        .iter()
        .find_map(|key| i64_value(obj.get(*key)))
        .or_else(|| {
            RESET_AT_KEYS
                .iter()
                .find_map(|key| date_value(obj.get(*key)).map(|d| (d - now).num_seconds().max(0)))
        })
        .unwrap_or(0)
        .max(0);
    Some(WindowUsage {
        percent: clamp_percent(percent),
        reset_in_sec,
        path_lower: String::new(),
    })
}

fn collect_window_candidates(
    value: &Value,
    now: DateTime<Utc>,
    path: String,
    out: &mut Vec<WindowUsage>,
) {
    match value {
        Value::Object(map) => {
            if let Some(mut window) = parse_window(value, now) {
                window.path_lower = path.to_lowercase();
                out.push(window);
            }
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                collect_window_candidates(child, now, child_path, out);
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                collect_window_candidates(child, now, format!("{}[{}]", path, idx), out);
            }
        }
        _ => {}
    }
}

fn pick_window<F>(
    candidates: &[WindowUsage],
    predicate: F,
    pick_shorter: bool,
    exclude_paths: &[&str],
) -> Option<WindowUsage>
where
    F: Fn(&str) -> bool,
{
    let mut filtered: Vec<_> = candidates
        .iter()
        .filter(|c| !exclude_paths.contains(&c.path_lower.as_str()) && predicate(&c.path_lower))
        .cloned()
        .collect();
    if filtered.is_empty() {
        filtered = candidates
            .iter()
            .filter(|c| !exclude_paths.contains(&c.path_lower.as_str()))
            .cloned()
            .collect();
    }
    filtered.into_iter().min_by(|lhs, rhs| {
        if pick_shorter {
            lhs.reset_in_sec
                .cmp(&rhs.reset_in_sec)
                .then_with(|| rhs.percent.total_cmp(&lhs.percent))
        } else {
            rhs.reset_in_sec
                .cmp(&lhs.reset_in_sec)
                .then_with(|| rhs.percent.total_cmp(&lhs.percent))
        }
    })
}

fn extract_double(pattern: &str, text: &str) -> Option<f64> {
    let re = Regex::new(pattern).ok()?;
    re.captures(text)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
}

fn extract_i64(pattern: &str, text: &str) -> Option<i64> {
    let re = Regex::new(pattern).ok()?;
    re.captures(text)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

fn double_value(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn i64_value(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|v| v as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn date_value(value: Option<&Value>) -> Option<DateTime<Utc>> {
    match value? {
        Value::Number(n) => date_from_number(n.as_f64()?),
        Value::String(s) => {
            if let Ok(number) = s.trim().parse::<f64>() {
                date_from_number(number)
            } else {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }
        }
        _ => None,
    }
}

fn date_from_number(number: f64) -> Option<DateTime<Utc>> {
    let seconds = if number > 1_000_000_000_000.0 {
        number / 1000.0
    } else {
        number
    };
    DateTime::<Utc>::from_timestamp(seconds as i64, 0)
}

fn clamp_percent(percent: f64) -> f64 {
    percent.clamp(0.0, 100.0)
}

fn percent_used_to_left_rate(percent: f64) -> f64 {
    (1.0 - clamp_percent(percent) / 100.0).clamp(0.0, 1.0)
}

fn reset_time(now: DateTime<Utc>, seconds: i64) -> String {
    (now + Duration::seconds(seconds.max(0))).to_rfc3339()
}

fn build_status(
    snapshot: &UsageSnapshot,
    now: DateTime<Utc>,
) -> Result<ProviderStatus, anyhow::Error> {
    let five_hour_left = percent_used_to_left_rate(snapshot.rolling_usage_percent);
    let weekly_left = percent_used_to_left_rate(snapshot.weekly_usage_percent);
    let monthly_left = snapshot
        .monthly_usage_percent
        .map(percent_used_to_left_rate);
    let remaining_percent = (five_hour_left * 100.0).clamp(0.0, 100.0);

    let mut details = HashMap::new();
    details.insert("plan_name".into(), Value::String("OpenCode Go".into()));
    details.insert(
        "five_hour_usage_left_rate".into(),
        Value::from(five_hour_left),
    );
    details.insert("weekly_usage_left_rate".into(), Value::from(weekly_left));
    details.insert(
        "five_hour_usage_reset_time".into(),
        Value::String(reset_time(now, snapshot.rolling_reset_in_sec)),
    );
    details.insert(
        "weekly_usage_reset_time".into(),
        Value::String(reset_time(now, snapshot.weekly_reset_in_sec)),
    );
    if let Some(monthly_left) = monthly_left {
        details.insert("monthly_usage_left_rate".into(), Value::from(monthly_left));
        details.insert(
            "monthly_usage_reset_time".into(),
            Value::String(reset_time(now, snapshot.monthly_reset_in_sec.unwrap_or(0))),
        );
    }

    Ok(ProviderStatus {
        provider_id: "opencodego".into(),
        provider_name: "OpenCode Go".into(),
        available: true,
        remaining_percent,
        details,
        error: None,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/providers_opencodego.rs"]
mod tests;
