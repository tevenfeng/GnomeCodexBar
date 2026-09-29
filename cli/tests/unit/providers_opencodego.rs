use super::*;

/// Trimmed-down copy of a real `GET /console/api/go/status` response.
const SAMPLE_STATUS: &str = r#"{
  "subscriberUserId": "acc_example",
  "product": "go",
  "renewalProduct": "go",
  "paymentMethodId": "payment_method_example",
  "paymentMethodKind": "alipay",
  "renewalCurrency": "usd",
  "useBalance": false,
  "cancelAtPeriodEnd": true,
  "renewalPending": false,
  "access": {
    "startsAt": "2026-09-10T13:00:46.000Z",
    "endsAt": "2026-10-10T13:00:46.000Z",
    "cancelAtPeriodEnd": true,
    "meters": {
      "fiveHour": {
        "startsAt": null,
        "resetsAt": null,
        "limitMicroCents": "1200000000",
        "usedMicroCents": "0"
      },
      "week": {
        "startsAt": "2026-09-28T00:00:00.000Z",
        "resetsAt": "2026-10-05T00:00:00.000Z",
        "limitMicroCents": "3000000000",
        "usedMicroCents": "0"
      },
      "month": {
        "resetsAt": "2026-10-10T13:00:46.000Z",
        "limitMicroCents": "6000000000",
        "usedMicroCents": "3017846243"
      }
    }
  },
  "upgradeAuthorizationRequired": true
}"#;

fn parse(text: &str) -> GoStatus {
    serde_json::from_str(text).expect("sample payload should deserialize")
}

#[test]
fn test_normalize_workspace_id() {
    assert_eq!(normalize_workspace_id("wrk_abc123"), Some("wrk_abc123".into()));
    assert_eq!(
        normalize_workspace_id("https://opencode.ai/workspace/wrk_xyz789/go"),
        Some("wrk_xyz789".into())
    );
    assert_eq!(normalize_workspace_id("  wrk_fromtext  "), Some("wrk_fromtext".into()));
    assert_eq!(normalize_workspace_id("no workspace"), None);
    assert_eq!(normalize_workspace_id(""), None);
    assert_eq!(normalize_workspace_id("wrk_"), None);
}

#[test]
fn test_normalize_cookie_header() {
    assert_eq!(
        normalize_cookie_header("__Host-console_session=abc"),
        Some("__Host-console_session=abc".into())
    );
    assert_eq!(
        normalize_cookie_header("Cookie: __Host-console_session=abc"),
        Some("__Host-console_session=abc".into())
    );
    assert_eq!(
        normalize_cookie_header("cookie:   a=1; b=2  "),
        Some("a=1; b=2".into())
    );
    assert_eq!(normalize_cookie_header("   "), None);
    assert_eq!(normalize_cookie_header("Cookie:"), None);
}

#[test]
fn test_cache_key_differs_by_cookie() {
    assert_ne!(cache_key("sid=abc"), cache_key("sid=def"));
    assert_eq!(cache_key("sid=abc"), cache_key("sid=abc"));
}

#[test]
fn test_remaining_rate() {
    let meter = |limit: &str, used: &str| Meter {
        resets_at: None,
        limit_micro_cents: Some(FlexibleAmount::Text(limit.into())),
        used_micro_cents: Some(FlexibleAmount::Text(used.into())),
    };

    assert_eq!(remaining_rate(None), None);
    assert_eq!(remaining_rate(Some(&meter("1200000000", "0"))), Some(1.0));
    assert_eq!(remaining_rate(Some(&meter("1200000000", "300000000"))), Some(0.75));
    assert_eq!(remaining_rate(Some(&meter("1200000000", "1200000000"))), Some(0.0));
    // Overspend is clamped rather than going negative.
    assert_eq!(remaining_rate(Some(&meter("100", "150"))), Some(0.0));
    // A zero or missing limit cannot produce a ratio.
    assert_eq!(remaining_rate(Some(&meter("0", "0"))), None);
    let missing_limit = Meter {
        resets_at: None,
        limit_micro_cents: None,
        used_micro_cents: None,
    };
    assert_eq!(remaining_rate(Some(&missing_limit)), None);
}

#[test]
fn test_flexible_amount_accepts_strings_and_numbers() {
    let payload: GoStatus = parse(
        r#"{"access":{"meters":{"fiveHour":{"limitMicroCents":1200000000,"usedMicroCents":300000000}}}}"#,
    );
    let access = payload.access.unwrap().into_first().unwrap();
    let five_hour = access.meters.unwrap().five_hour.unwrap();
    assert_eq!(remaining_rate(Some(&five_hour)), Some(0.75));

    let floats: GoStatus =
        parse(r#"{"access":{"meters":{"week":{"limitMicroCents":100.0,"usedMicroCents":25.0}}}}"#);
    let access = floats.access.unwrap().into_first().unwrap();
    let week = access.meters.unwrap().week.unwrap();
    assert_eq!(remaining_rate(Some(&week)), Some(0.75));
}

#[test]
fn test_plan_name() {
    assert_eq!(plan_name(Some("go")), "OpenCode Go");
    assert_eq!(plan_name(Some("go-plus")), "OpenCode Go Plus");
    assert_eq!(plan_name(None), "OpenCode Go");
}

#[test]
fn test_build_status_contract_from_live_shape() {
    let status = build_status(&parse(SAMPLE_STATUS)).unwrap();

    assert_eq!(status.provider_id, "opencodego");
    assert_eq!(status.provider_name, "OpenCode Go");
    assert!(status.available);
    assert!(status.error.is_none());

    // The 5h window has not started, so it counts as full and drives the headline.
    assert_eq!(status.remaining_percent, 100.0);
    assert_eq!(status.details["plan_name"], "OpenCode Go");
    assert_eq!(status.details["five_hour_usage_left_rate"], 1.0);
    assert_eq!(status.details["weekly_usage_left_rate"], 1.0);

    // (6_000_000_000 - 3_017_846_243) / 6_000_000_000
    let monthly = status.details["monthly_usage_left_rate"].as_f64().unwrap();
    assert!((monthly - 0.4970256261666667).abs() < 1e-12, "unexpected {monthly}");

    // A window that has not started reports no reset time.
    assert!(!status.details.contains_key("five_hour_usage_reset_time"));
    assert_eq!(
        status.details["weekly_usage_reset_time"],
        "2026-10-05T00:00:00.000Z"
    );
    assert_eq!(
        status.details["monthly_usage_reset_time"],
        "2026-10-10T13:00:46.000Z"
    );
    assert_eq!(status.details["access_ends_at"], "2026-10-10T13:00:46.000Z");
    assert_eq!(status.details["access_starts_at"], "2026-09-10T13:00:46.000Z");
    assert_eq!(status.details["cancel_at_period_end"], true);
}

#[test]
fn test_build_status_uses_five_hour_window_when_started() {
    let status = build_status(&parse(
        r#"{"product":"go","access":{"meters":{"fiveHour":{
             "startsAt":"2026-09-29T10:00:00.000Z",
             "resetsAt":"2026-09-29T15:00:00.000Z",
             "limitMicroCents":"1200000000","usedMicroCents":"900000000"},
           "week":{"limitMicroCents":"3000000000","usedMicroCents":"1500000000"}}}}"#,
    ))
    .unwrap();

    assert_eq!(status.remaining_percent, 25.0);
    assert_eq!(status.details["five_hour_usage_left_rate"], 0.25);
    assert_eq!(status.details["weekly_usage_left_rate"], 0.5);
    assert_eq!(
        status.details["five_hour_usage_reset_time"],
        "2026-09-29T15:00:00.000Z"
    );
}

#[test]
fn test_build_status_accepts_access_as_array() {
    let status = build_status(&parse(
        r#"{"product":"go-plus","access":[{"meters":{"fiveHour":{
             "limitMicroCents":"4800000000","usedMicroCents":"1200000000"}}}]}"#,
    ))
    .unwrap();

    assert!(status.available);
    assert_eq!(status.remaining_percent, 75.0);
    assert_eq!(status.details["plan_name"], "OpenCode Go Plus");
}

#[test]
fn test_build_status_without_subscription_is_unavailable() {
    for payload in [
        r#"{"access":null}"#,
        r#"{"access":[]}"#,
        r#"{"product":"go"}"#,
    ] {
        let status = build_status(&parse(payload)).unwrap();
        assert!(!status.available, "expected unavailable for {payload}");
        assert_eq!(status.remaining_percent, 0.0);
        assert!(status.details.is_empty());
        assert!(status.error.unwrap().contains("No active OpenCode Go subscription"));
    }
}

#[test]
fn test_build_status_tolerates_missing_meters() {
    let status = build_status(&parse(r#"{"access":{}}"#)).unwrap();
    assert!(status.available);
    assert_eq!(status.remaining_percent, 0.0);
    assert_eq!(status.details["plan_name"], "OpenCode Go");
    assert!(!status.details.contains_key("five_hour_usage_left_rate"));
}

#[test]
fn test_console_error_maps_auth_failures_to_actionable_messages() {
    let unauthorized = console_error(
        "OpenCode Go status",
        reqwest::StatusCode::UNAUTHORIZED,
        r#"{"_tag":"Unauthorized"}"#,
    )
    .to_string();
    assert!(unauthorized.contains("console session"), "{unauthorized}");

    let org_required = console_error(
        "OpenCode Go status",
        reqwest::StatusCode::BAD_REQUEST,
        r#"{"_tag":"OrgRequired","message":"x-org-id is required"}"#,
    )
    .to_string();
    assert!(org_required.contains("workspace id"), "{org_required}");

    let forbidden = console_error(
        "OpenCode Go status",
        reqwest::StatusCode::FORBIDDEN,
        r#"{"_tag":"Forbidden"}"#,
    )
    .to_string();
    assert!(forbidden.contains("cannot read"), "{forbidden}");
}

#[test]
fn test_is_unauthorized_only_for_401() {
    let unauthorized = console_error(
        "OpenCode Go status",
        reqwest::StatusCode::UNAUTHORIZED,
        r#"{"_tag":"Unauthorized"}"#,
    );
    assert!(is_unauthorized(&unauthorized));
    // The error keeps its status for callers that need to branch on it.
    assert_eq!(
        unauthorized.downcast_ref::<ConsoleApiError>().map(|e| e.status),
        Some(401)
    );

    let server_error = console_error(
        "OpenCode Go status",
        reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        "boom",
    );
    assert!(!is_unauthorized(&server_error));
    assert!(!is_unauthorized(&anyhow::anyhow!("plain error")));
}
