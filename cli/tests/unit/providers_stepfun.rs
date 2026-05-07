use super::*;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

// ── Wrapper structs for deserializing Flexible* types in isolation ──

#[derive(Deserialize)]
struct FlexibleNumberWrapper {
    value: FlexibleNumber,
}

#[derive(Deserialize)]
struct FlexibleTimestampWrapper {
    value: FlexibleTimestamp,
}

#[derive(Deserialize)]
struct FlexibleIntOrStringWrapper {
    value: FlexibleIntOrString,
}

// ── FlexibleNumber tests ───────────────────────────────

#[test]
fn test_flexible_number_from_int() {
    let w: FlexibleNumberWrapper = serde_json::from_str(r#"{"value": 1}"#).unwrap();
    assert!((w.value.value - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_flexible_number_from_float() {
    let w: FlexibleNumberWrapper = serde_json::from_str(r#"{"value": 0.85}"#).unwrap();
    assert!((w.value.value - 0.85).abs() < f64::EPSILON);
}

#[test]
fn test_flexible_number_from_string() {
    let w: FlexibleNumberWrapper = serde_json::from_str(r#"{"value": "0.5"}"#).unwrap();
    assert!((w.value.value - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_flexible_number_from_null() {
    let w: FlexibleNumberWrapper = serde_json::from_str(r#"{"value": null}"#).unwrap();
    assert!((w.value.value - 0.0).abs() < f64::EPSILON);
}

// ── FlexibleTimestamp tests ─────────────────────────────

#[test]
fn test_flexible_timestamp_from_int() {
    let w: FlexibleTimestampWrapper = serde_json::from_str(r#"{"value": 1777528800}"#).unwrap();
    assert_eq!(w.value.value, 1777528800);
}

#[test]
fn test_flexible_timestamp_from_string() {
    let w: FlexibleTimestampWrapper =
        serde_json::from_str(r#"{"value": "1777528800"}"#).unwrap();
    assert_eq!(w.value.value, 1777528800);
}

#[test]
fn test_flexible_timestamp_from_null() {
    let w: FlexibleTimestampWrapper = serde_json::from_str(r#"{"value": null}"#).unwrap();
    assert_eq!(w.value.value, 0);
}

// ── FlexibleIntOrString tests ───────────────────────────

#[test]
fn test_flexible_int_or_string_from_int() {
    let w: FlexibleIntOrStringWrapper = serde_json::from_str(r#"{"value": 1}"#).unwrap();
    assert_eq!(w.value.0, 1);
}

#[test]
fn test_flexible_int_or_string_from_string() {
    let w: FlexibleIntOrStringWrapper =
        serde_json::from_str(r#"{"value": "unauthenticated"}"#).unwrap();
    assert_eq!(w.value.0, 0);
}

#[test]
fn test_flexible_int_or_string_from_null() {
    let w: FlexibleIntOrStringWrapper = serde_json::from_str(r#"{"value": null}"#).unwrap();
    assert_eq!(w.value.0, 0);
}

// ── parse_timestamp tests ───────────────────────────────

#[test]
fn test_parse_timestamp_zero() {
    assert_eq!(parse_timestamp(0), "Unknown");
}

#[test]
fn test_parse_timestamp_valid() {
    let result = parse_timestamp(1777528800);
    assert!(
        result.contains("2026"),
        "Expected RFC3339 containing '2026', got: {}",
        result
    );
}

#[test]
fn test_parse_timestamp_epoch() {
    let result = parse_timestamp(1);
    assert!(
        result.contains("1970"),
        "Expected RFC3339 containing '1970', got: {}",
        result
    );
}

// ── build_status tests ──────────────────────────────────

#[test]
fn test_build_status_full() {
    let data: RateLimitResponse = serde_json::from_str(
        r#"{
            "status": 1,
            "five_hour_usage_left_rate": 0.85,
            "weekly_usage_left_rate": 0.92,
            "five_hour_usage_reset_time": 1777528800,
            "weekly_usage_reset_time": 1778049600
        }"#,
    )
    .unwrap();

    let status = build_status(&data, Some("Plus")).unwrap();

    assert_eq!(status.provider_id, "stepfun");
    assert_eq!(status.provider_name, "StepFun");
    assert!(status.available);
    assert!((status.remaining_percent - 85.0).abs() < f64::EPSILON);
    assert_eq!(
        status.details.get("plan_name").unwrap(),
        &Value::String("Plus".into())
    );
    assert!(
        (status.details["five_hour_usage_left_rate"].as_f64().unwrap() - 0.85).abs()
            < f64::EPSILON
    );
    assert!(
        (status.details["weekly_usage_left_rate"].as_f64().unwrap() - 0.92).abs() < f64::EPSILON
    );
    assert!(status.error.is_none());
}

#[test]
fn test_build_status_no_plan() {
    let data: RateLimitResponse = serde_json::from_str(
        r#"{
            "status": 1,
            "five_hour_usage_left_rate": 0.85,
            "weekly_usage_left_rate": 0.92,
            "five_hour_usage_reset_time": 1777528800,
            "weekly_usage_reset_time": 1778049600
        }"#,
    )
    .unwrap();

    let status = build_status(&data, None).unwrap();

    assert!(
        !status.details.contains_key("plan_name"),
        "plan_name should not be present when None"
    );
}

#[test]
fn test_build_status_zero_rates() {
    let data: RateLimitResponse = serde_json::from_str(
        r#"{
            "status": 1,
            "five_hour_usage_left_rate": 0.0,
            "weekly_usage_left_rate": 0.0,
            "five_hour_usage_reset_time": 0,
            "weekly_usage_reset_time": 0
        }"#,
    )
    .unwrap();

    let status = build_status(&data, None).unwrap();

    assert!((status.remaining_percent - 0.0).abs() < f64::EPSILON);
    assert_eq!(
        status.details.get("five_hour_usage_reset_time").unwrap(),
        &Value::String("Unknown".into())
    );
}

#[test]
fn test_build_status_clamp() {
    let data: RateLimitResponse = serde_json::from_str(
        r#"{
            "status": 1,
            "five_hour_usage_left_rate": 1.5,
            "weekly_usage_left_rate": 1.5
        }"#,
    )
    .unwrap();

    let status = build_status(&data, None).unwrap();

    assert!(
        (status.remaining_percent - 100.0).abs() < f64::EPSILON,
        "remaining_percent should be clamped to 100.0, got: {}",
        status.remaining_percent
    );
}

// ── extract_set_cookie tests ────────────────────────────

#[test]
fn test_extract_set_cookie_single() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_static("INGRESSCOOKIE=abc123; Path=/"),
    );

    let result = extract_set_cookie(&headers, "INGRESSCOOKIE");
    assert_eq!(result, Some("abc123".to_string()));
}

#[test]
fn test_extract_set_cookie_multiple() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_static("OTHER=val; Path=/"),
    );
    headers.append(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_static("INGRESSCOOKIE=xyz789; Path=/"),
    );

    let result = extract_set_cookie(&headers, "INGRESSCOOKIE");
    assert_eq!(result, Some("xyz789".to_string()));
}

#[test]
fn test_extract_set_cookie_missing() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_static("OTHER=val; Path=/"),
    );

    let result = extract_set_cookie(&headers, "INGRESSCOOKIE");
    assert_eq!(result, None);
}

#[test]
fn test_extract_set_cookie_case_insensitive() {
    let mut headers = HeaderMap::new();
    // HeaderMap normalizes header names to lowercase internally,
    // but the lookup is case-insensitive by design.
    headers.insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_static("INGRESSCOOKIE=abc123; Path=/"),
    );

    // Verify that iterating with eq_ignore_ascii_case("set-cookie") works
    let result = extract_set_cookie(&headers, "INGRESSCOOKIE");
    assert_eq!(result, Some("abc123".to_string()));
}

// ── Response deserialization tests ──────────────────────

#[test]
fn test_rate_limit_response_deserialize() {
    let resp: RateLimitResponse = serde_json::from_str(
        r#"{
            "status": 1,
            "code": "unauthenticated",
            "message": "ok",
            "five_hour_usage_left_rate": "0.85",
            "weekly_usage_left_rate": 0.92,
            "five_hour_usage_reset_time": "1777528800",
            "weekly_usage_reset_time": 1778049600
        }"#,
    )
    .unwrap();

    assert_eq!(resp.status, Some(1));
    assert_eq!(resp.message, Some("ok".to_string()));
    assert!(
        (resp.five_hour_usage_left_rate.unwrap().value - 0.85).abs() < f64::EPSILON
    );
    assert!(
        (resp.weekly_usage_left_rate.unwrap().value - 0.92).abs() < f64::EPSILON
    );
    assert_eq!(resp.five_hour_usage_reset_time.unwrap().value, 1777528800);
    assert_eq!(resp.weekly_usage_reset_time.unwrap().value, 1778049600);
}

#[test]
fn test_register_device_response_deserialize() {
    let resp: RegisterDeviceResponse = serde_json::from_str(
        r#"{
            "accessToken": {"raw": "tok1"},
            "refreshToken": {"raw": "tok2"}
        }"#,
    )
    .unwrap();

    assert_eq!(resp.access_token.unwrap().raw, "tok1");
    assert_eq!(resp.refresh_token.unwrap().raw, "tok2");
}

#[test]
fn test_login_response_deserialize() {
    let resp: LoginResponse = serde_json::from_str(
        r#"{
            "accessToken": {"raw": "access_tok"},
            "refreshToken": {"raw": "refresh_tok"}
        }"#,
    )
    .unwrap();

    assert_eq!(resp.access_token.unwrap().raw, "access_tok");
    assert_eq!(resp.refresh_token.unwrap().raw, "refresh_tok");
}

#[test]
fn test_plan_status_response_deserialize() {
    let resp: PlanStatusResponse = serde_json::from_str(
        r#"{
            "status": 1,
            "subscription": {
                "name": "Plus",
                "plan_type": 2,
                "status": 1
            }
        }"#,
    )
    .unwrap();

    assert_eq!(resp.status, Some(1));
    let sub = resp.subscription.unwrap();
    assert_eq!(sub.name, Some("Plus".to_string()));
    assert_eq!(sub.plan_type, Some(2));
    assert_eq!(sub.plan_status, Some(1));
}
