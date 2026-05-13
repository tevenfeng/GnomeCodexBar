use super::*;

#[test]
fn test_normalize_workspace_id() {
    assert_eq!(
        normalize_workspace_id(Some("wrk_abc123")),
        Some("wrk_abc123".into())
    );
    assert_eq!(
        normalize_workspace_id(Some("https://opencode.ai/workspace/wrk_xyz789/go")),
        Some("wrk_xyz789".into())
    );
    assert_eq!(
        normalize_workspace_id(Some("workspace=wrk_fromtext")),
        Some("wrk_fromtext".into())
    );
    assert_eq!(normalize_workspace_id(Some("no workspace")), None);
    assert_eq!(normalize_workspace_id(None), None);
}

#[test]
fn test_discovered_workspace_id_cache_helpers() {
    let key = cache_key_for_cookie("sid=abc");
    {
        let mut cached = DISCOVERED_WORKSPACE_IDS.lock().unwrap();
        cached.insert(key, "wrk_cached".into());
    }

    let cached = DISCOVERED_WORKSPACE_IDS.lock().unwrap().get(&key).cloned();
    assert_eq!(cached, Some("wrk_cached".into()));

    DISCOVERED_WORKSPACE_IDS.lock().unwrap().clear();
}

#[test]
fn test_workspace_id_cache_key_differs_by_cookie() {
    assert_ne!(
        cache_key_for_cookie("sid=abc"),
        cache_key_for_cookie("sid=def")
    );
}

#[test]
fn test_request_cookie_header() {
    assert_eq!(
        request_cookie_header("sid=abc; other=1"),
        Some("sid=abc; other=1".into())
    );
    assert_eq!(
        request_cookie_header("Cookie: sid=abc"),
        Some("sid=abc".into())
    );
    assert_eq!(request_cookie_header("   "), None);
}

#[test]
fn test_percent_used_to_left_rate() {
    assert!((percent_used_to_left_rate(0.0) - 1.0).abs() < f64::EPSILON);
    assert!((percent_used_to_left_rate(40.0) - 0.6).abs() < f64::EPSILON);
    assert!((percent_used_to_left_rate(100.0) - 0.0).abs() < f64::EPSILON);
    assert!((percent_used_to_left_rate(120.0) - 0.0).abs() < f64::EPSILON);
    assert!((percent_used_to_left_rate(-10.0) - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_parse_subscription_json_with_monthly() {
    let text = r#"
    {
      "usage": {
        "rollingUsage": { "usagePercent": 25, "resetInSec": 3600 },
        "weeklyUsage": { "usagePercent": "50", "resetInSec": "7200" },
        "monthlyUsage": { "used": 25, "limit": 100, "resetInSec": 86400 }
      }
    }
    "#;
    let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let snapshot = parse_subscription(text, now).unwrap();

    assert_eq!(snapshot.rolling_usage_percent, 25.0);
    assert_eq!(snapshot.weekly_usage_percent, 50.0);
    assert_eq!(snapshot.monthly_usage_percent, Some(25.0));
    assert_eq!(snapshot.rolling_reset_in_sec, 3600);
    assert_eq!(snapshot.weekly_reset_in_sec, 7200);
    assert_eq!(snapshot.monthly_reset_in_sec, Some(86400));
}

#[test]
fn test_parse_subscription_text_fallback() {
    let text = r#"
    rollingUsage: { usagePercent: 10, resetInSec: 60 }
    weeklyUsage: { usagePercent: 20, resetInSec: 120 }
    monthlyUsage: { usagePercent: 30, resetInSec: 180 }
    "#;
    let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let snapshot = parse_subscription(text, now).unwrap();

    assert_eq!(snapshot.rolling_usage_percent, 10.0);
    assert_eq!(snapshot.weekly_usage_percent, 20.0);
    assert_eq!(snapshot.monthly_usage_percent, Some(30.0));
}

#[test]
fn test_parse_subscription_quoted_text_fallback() {
    let text = r#"
    <script>
    window.__DATA__ = {
      "rollingUsage": { "usagePercent": "10", "resetInSec": "60" },
      "weeklyUsage": { "usagePercent": 20, "resetInSec": 120 },
      "monthlyUsage": { "usagePercent": 30, "resetInSec": 180 }
    };
    </script>
    "#;
    let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let snapshot = parse_subscription(text, now).unwrap();

    assert_eq!(snapshot.rolling_usage_percent, 10.0);
    assert_eq!(snapshot.weekly_usage_percent, 20.0);
    assert_eq!(snapshot.monthly_usage_percent, Some(30.0));
}

#[test]
fn test_build_status_contract_with_monthly() {
    let snapshot = UsageSnapshot {
        rolling_usage_percent: 25.0,
        weekly_usage_percent: 40.0,
        monthly_usage_percent: Some(60.0),
        rolling_reset_in_sec: 3600,
        weekly_reset_in_sec: 7200,
        monthly_reset_in_sec: Some(10_800),
    };
    let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let status = build_status(&snapshot, now).unwrap();

    assert_eq!(status.provider_id, "opencodego");
    assert_eq!(status.provider_name, "OpenCode Go");
    assert!(status.available);
    assert_eq!(status.remaining_percent, 75.0);
    assert_eq!(status.details["plan_name"], "OpenCode Go");
    assert_eq!(status.details["five_hour_usage_left_rate"], 0.75);
    assert_eq!(status.details["weekly_usage_left_rate"], 0.6);
    assert_eq!(status.details["monthly_usage_left_rate"], 0.4);
    assert!(status.details["five_hour_usage_reset_time"]
        .as_str()
        .unwrap()
        .starts_with("2023-11-14T23:13:20"));
    assert!(status.details["weekly_usage_reset_time"]
        .as_str()
        .unwrap()
        .starts_with("2023-11-15T00:13:20"));
    assert!(status.details["monthly_usage_reset_time"]
        .as_str()
        .unwrap()
        .starts_with("2023-11-15T01:13:20"));
}
