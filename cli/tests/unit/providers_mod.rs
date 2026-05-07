use super::*;

#[test]
fn test_provider_config_default() {
    let config = ProviderConfig::default();
    assert!(!config.enabled);
    assert!(config.api_key.is_none());
    assert!(config.username.is_none());
    assert!(config.password.is_none());
}

#[test]
fn test_provider_config_serialization() {
    let config = ProviderConfig {
        enabled: true,
        api_key: Some("sk-test-123".into()),
        username: Some("user@example.com".into()),
        password: Some("secret".into()),
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.enabled, config.enabled);
    assert_eq!(deserialized.api_key, config.api_key);
    assert_eq!(deserialized.username, config.username);
    assert_eq!(deserialized.password, config.password);
}

#[test]
fn test_provider_status_serialization() {
    let mut details = HashMap::new();
    details.insert("currency".into(), serde_json::Value::String("CNY".into()));
    details.insert(
        "total_balance".into(),
        serde_json::Value::Number(serde_json::Number::from_f64(10.5).unwrap()),
    );
    details.insert("is_available".into(), serde_json::Value::Bool(true));

    let status = ProviderStatus {
        provider_id: "deepseek".into(),
        provider_name: "DeepSeek".into(),
        available: true,
        remaining_percent: 100.0,
        details,
        error: None,
    };

    let json = serde_json::to_string(&status).unwrap();
    let deserialized: ProviderStatus = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.provider_id, status.provider_id);
    assert_eq!(deserialized.provider_name, status.provider_name);
    assert_eq!(deserialized.available, status.available);
    assert!((deserialized.remaining_percent - status.remaining_percent).abs() < f64::EPSILON);
    assert_eq!(deserialized.details, status.details);
    assert_eq!(deserialized.error, status.error);
}

#[test]
fn test_status_snapshot_serialization() {
    let status1 = ProviderStatus {
        provider_id: "deepseek".into(),
        provider_name: "DeepSeek".into(),
        available: true,
        remaining_percent: 100.0,
        details: HashMap::new(),
        error: None,
    };
    let status2 = ProviderStatus {
        provider_id: "stepfun".into(),
        provider_name: "StepFun".into(),
        available: true,
        remaining_percent: 85.0,
        details: HashMap::new(),
        error: None,
    };

    let snapshot = StatusSnapshot {
        updated_at: "2026-05-06T12:00:00+00:00".into(),
        providers: vec![status1, status2],
    };

    let json = serde_json::to_string(&snapshot).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(value.get("updated_at").is_some());
    assert_eq!(value["updated_at"], "2026-05-06T12:00:00+00:00");
    assert!(value.get("providers").is_some());
    assert_eq!(value["providers"].as_array().unwrap().len(), 2);
    assert_eq!(value["providers"][0]["provider_id"], "deepseek");
    assert_eq!(value["providers"][1]["provider_id"], "stepfun");
}

#[test]
fn test_provider_status_with_error() {
    let status = ProviderStatus {
        provider_id: "deepseek".into(),
        provider_name: "DeepSeek".into(),
        available: false,
        remaining_percent: 0.0,
        details: HashMap::new(),
        error: Some("HTTP 401: Unauthorized".into()),
    };

    let json = serde_json::to_string(&status).unwrap();
    let deserialized: ProviderStatus = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.error, Some("HTTP 401: Unauthorized".into()));
    assert!(!deserialized.available);
}

#[test]
fn test_provider_status_with_null_error() {
    let status = ProviderStatus {
        provider_id: "deepseek".into(),
        provider_name: "DeepSeek".into(),
        available: true,
        remaining_percent: 100.0,
        details: HashMap::new(),
        error: None,
    };

    let json = serde_json::to_string(&status).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(value["error"].is_null());

    let deserialized: ProviderStatus = serde_json::from_str(&json).unwrap();
    assert!(deserialized.error.is_none());
}
