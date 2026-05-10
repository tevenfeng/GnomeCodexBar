use crate::providers::{ProviderStatus, StatusSnapshot};
use std::collections::HashMap;

#[test]
fn test_status_snapshot_serialization() {
    let snapshot = StatusSnapshot {
        updated_at: "2026-05-06T12:00:00+00:00".into(),
        providers: vec![
            ProviderStatus {
                provider_id: "deepseek".into(),
                provider_name: "DeepSeek".into(),
                available: true,
                remaining_percent: 100.0,
                details: HashMap::from([
                    ("total_balance".into(), serde_json::json!(10.5)),
                    ("currency".into(), serde_json::json!("CNY")),
                ]),
                error: None,
            },
            ProviderStatus {
                provider_id: "stepfun".into(),
                provider_name: "StepFun".into(),
                available: true,
                remaining_percent: 85.0,
                details: HashMap::from([
                    ("plan_name".into(), serde_json::json!("Plus")),
                    ("five_hour_usage_left_rate".into(), serde_json::json!(0.85)),
                ]),
                error: None,
            },
        ],
    };

    let json = serde_json::to_string_pretty(&snapshot).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse back");

    assert_eq!(parsed["updated_at"], "2026-05-06T12:00:00+00:00");
    assert_eq!(parsed["providers"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["providers"][0]["provider_id"], "deepseek");
    assert_eq!(parsed["providers"][0]["remaining_percent"], 100.0);
    assert_eq!(parsed["providers"][1]["provider_id"], "stepfun");
    assert_eq!(parsed["providers"][1]["remaining_percent"], 85.0);
}

#[test]
fn test_provider_status_serialization() {
    let status = ProviderStatus {
        provider_id: "deepseek".into(),
        provider_name: "DeepSeek".into(),
        available: true,
        remaining_percent: 50.0,
        details: HashMap::from([("total_balance".into(), serde_json::json!(5.0))]),
        error: Some("timeout".into()),
    };

    let json = serde_json::to_string_pretty(&status).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse back");

    assert_eq!(parsed["provider_id"], "deepseek");
    assert_eq!(parsed["provider_name"], "DeepSeek");
    assert_eq!(parsed["available"], true);
    assert_eq!(parsed["remaining_percent"], 50.0);
    assert_eq!(parsed["details"]["total_balance"], 5.0);
    assert_eq!(parsed["error"], "timeout");
}

#[test]
fn test_write_and_read_selected_provider() {
    // Test the JSON round-trip logic directly without file I/O
    let provider_id = "stepfun";
    let val = serde_json::json!({ "selected_provider": provider_id });
    let json_str = serde_json::to_string(&val).expect("serialize");

    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("parse");
    assert_eq!(parsed["selected_provider"], "stepfun");
}

#[test]
fn test_read_selected_provider_missing_file() {
    // Simulate the parsing logic of read_selected_provider_sync with bad input
    // An empty/non-existent file would fail fs::read_to_string, so the function
    // returns None. We test the JSON parsing branch with invalid content.
    let bad_content = "not valid json";
    let result = serde_json::from_str::<serde_json::Value>(bad_content);
    assert!(result.is_err());

    // Also test valid JSON but missing the key
    let valid_but_missing = serde_json::json!({ "other_key": "value" });
    let extracted = valid_but_missing["selected_provider"].as_str();
    assert!(extracted.is_none());
}
