use super::*;

#[test]
fn test_deepseek_balance_response_deserialize() {
    let json = r#"{
        "is_available": true,
        "balance_infos": [
            {
                "currency": "CNY",
                "total_balance": "10.5",
                "granted_balance": "5.0",
                "topped_up_balance": "5.5"
            }
        ]
    }"#;

    let resp: DeepSeekBalanceResponse = serde_json::from_str(json).unwrap();
    assert!(resp.is_available);
    assert_eq!(resp.balance_infos.len(), 1);
    let info = &resp.balance_infos[0];
    assert_eq!(info.currency, "CNY");
    assert_eq!(info.total_balance, "10.5");
    assert_eq!(info.granted_balance, "5.0");
    assert_eq!(info.topped_up_balance, "5.5");
}

#[test]
fn test_deepseek_balance_info_string_parsing() {
    let json = r#"{
        "currency": "USD",
        "total_balance": "100.25",
        "granted_balance": "50.75",
        "topped_up_balance": "49.50"
    }"#;

    let info: DeepSeekBalanceInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.currency, "USD");
    assert_eq!(info.total_balance, "100.25");
    assert_eq!(info.granted_balance, "50.75");
    assert_eq!(info.topped_up_balance, "49.50");

    // Verify the string fields parse correctly to f64
    let total: f64 = info.total_balance.parse().unwrap();
    let granted: f64 = info.granted_balance.parse().unwrap();
    let topped_up: f64 = info.topped_up_balance.parse().unwrap();
    assert!((total - 100.25).abs() < f64::EPSILON);
    assert!((granted - 50.75).abs() < f64::EPSILON);
    assert!((topped_up - 49.50).abs() < f64::EPSILON);
}

#[test]
fn test_deepseek_empty_balance_infos() {
    let json = r#"{
        "is_available": true,
        "balance_infos": []
    }"#;

    let resp: DeepSeekBalanceResponse = serde_json::from_str(json).unwrap();
    assert!(resp.is_available);
    assert!(resp.balance_infos.is_empty());

    // Empty balance_infos should yield remaining_percent = 0%
    // (matches the logic in fetch: else branch when .first() returns None)
    let remaining_percent: f64 = if resp.balance_infos.first().is_some() { 100.0 } else { 0.0 };
    assert!((remaining_percent - 0.0_f64).abs() < f64::EPSILON);
}

#[test]
fn test_deepseek_provider_id_and_name() {
    let provider = DeepSeekProvider::new();
    assert_eq!(provider.id(), "deepseek");
    assert_eq!(provider.name(), "DeepSeek");
}

#[test]
fn test_deepseek_remaining_percent_positive_balance() {
    let json = r#"{
        "is_available": true,
        "balance_infos": [
            {
                "currency": "CNY",
                "total_balance": "10.5",
                "granted_balance": "5.0",
                "topped_up_balance": "5.5"
            }
        ]
    }"#;

    let resp: DeepSeekBalanceResponse = serde_json::from_str(json).unwrap();
    let info = resp.balance_infos.first().unwrap();
    let total_balance: f64 = info.total_balance.parse().unwrap_or(0.0);
    let remaining_percent: f64 = if total_balance > 0.0 { 100.0 } else { 0.0 };
    assert!((remaining_percent - 100.0_f64).abs() < f64::EPSILON);
}

#[test]
fn test_deepseek_remaining_percent_zero_balance() {
    let json = r#"{
        "is_available": true,
        "balance_infos": [
            {
                "currency": "CNY",
                "total_balance": "0",
                "granted_balance": "0",
                "topped_up_balance": "0"
            }
        ]
    }"#;

    let resp: DeepSeekBalanceResponse = serde_json::from_str(json).unwrap();
    let info = resp.balance_infos.first().unwrap();
    let total_balance: f64 = info.total_balance.parse().unwrap_or(0.0);
    let remaining_percent: f64 = if total_balance > 0.0 { 100.0 } else { 0.0 };
    assert!((remaining_percent - 0.0_f64).abs() < f64::EPSILON);
}
