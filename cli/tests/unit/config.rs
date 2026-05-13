use super::*;

#[test]
fn test_default_config() {
    let config = Config::default();

    // Providers should exist
    assert!(config.providers.contains_key("deepseek"));
    assert!(config.providers.contains_key("stepfun"));
    assert!(config.providers.contains_key("opencodego"));

    // DeepSeek and StepFun should be enabled by default; OpenCode Go requires cookie config.
    assert!(config.providers["deepseek"].enabled);
    assert!(config.providers["stepfun"].enabled);
    assert!(!config.providers["opencodego"].enabled);

    // General config defaults
    assert_eq!(config.general.refresh_interval_secs, 300);
    assert_eq!(config.general.selected_provider, "deepseek");
    assert_eq!(
        config.general.provider_order,
        vec!["deepseek", "stepfun", "opencodego"]
    );
    assert!(config.general.budget_monthly.is_none());
}

#[test]
fn test_refresh_interval_clamp_helper() {
    let mut general = GeneralConfig {
        refresh_interval_secs: 0,
        budget_monthly: None,
        selected_provider: "deepseek".into(),
        provider_order: default_provider_order(),
    };
    assert_eq!(
        general.refresh_interval_secs_clamped(),
        MIN_REFRESH_INTERVAL_SECS
    );

    general.refresh_interval_secs = 60;
    assert_eq!(general.refresh_interval_secs_clamped(), 60);

    general.refresh_interval_secs = MAX_REFRESH_INTERVAL_SECS + 1;
    assert_eq!(
        general.refresh_interval_secs_clamped(),
        MAX_REFRESH_INTERVAL_SECS
    );
}

#[test]
fn test_config_save_serializes_clamped_refresh_interval() {
    let mut config = Config::default();
    config.general.refresh_interval_secs = 0;

    let toml_str = toml::to_string_pretty(&config.clone().clamp_refresh_interval())
        .expect("serialize clamped config");
    let config2: Config = toml::from_str(&toml_str).expect("deserialize from TOML");

    assert_eq!(
        config2.general.refresh_interval_secs,
        MIN_REFRESH_INTERVAL_SECS
    );
}

#[test]
fn test_config_toml_roundtrip() {
    let mut config = Config::default();
    config.providers.get_mut("deepseek").unwrap().api_key = Some("sk-test123".into());
    config.providers.get_mut("stepfun").unwrap().username = Some("user@example.com".into());
    config.providers.get_mut("stepfun").unwrap().password = Some("secret".into());
    config
        .providers
        .get_mut("opencodego")
        .unwrap()
        .cookie_header = Some("sid=test".into());
    config.providers.get_mut("opencodego").unwrap().workspace_id = Some("wrk_test".into());
    config.general.refresh_interval_secs = 60;
    config.general.budget_monthly = Some(100.0);
    config.general.selected_provider = "stepfun".into();
    config.general.provider_order = vec!["opencodego".into(), "stepfun".into(), "deepseek".into()];

    let toml_str = toml::to_string_pretty(&config).expect("serialize to TOML");
    let config2: Config = toml::from_str(&toml_str).expect("deserialize from TOML");

    assert_eq!(
        config2.providers["deepseek"].api_key,
        Some("sk-test123".into())
    );
    assert!(config2.providers["deepseek"].enabled);
    assert_eq!(
        config2.providers["stepfun"].username,
        Some("user@example.com".into())
    );
    assert_eq!(config2.providers["stepfun"].password, Some("secret".into()));
    assert_eq!(
        config2.providers["opencodego"].cookie_header,
        Some("sid=test".into())
    );
    assert_eq!(
        config2.providers["opencodego"].workspace_id,
        Some("wrk_test".into())
    );
    assert_eq!(config2.general.refresh_interval_secs, 60);
    assert_eq!(config2.general.budget_monthly, Some(100.0));
    assert_eq!(config2.general.selected_provider, "stepfun");
    assert_eq!(
        config2.general.provider_order,
        vec!["opencodego", "stepfun", "deepseek"]
    );
}

#[test]
fn test_config_missing_provider_order_uses_default() {
    let toml_str = r#"
[general]
refresh_interval_secs = 60
selected_provider = "stepfun"
"#;

    let config: Config = toml::from_str(toml_str).expect("deserialize without provider_order");

    assert_eq!(config.general.refresh_interval_secs, 60);
    assert_eq!(config.general.selected_provider, "stepfun");
    assert_eq!(
        config.general.provider_order,
        vec!["deepseek", "stepfun", "opencodego"]
    );
}

#[test]
fn test_config_serializes_provider_order() {
    let mut config = Config::default();
    config.general.provider_order = vec!["opencodego".into(), "deepseek".into(), "stepfun".into()];

    let toml_str = toml::to_string_pretty(&config).expect("serialize to TOML");

    let config2: Config = toml::from_str(&toml_str).expect("deserialize serialized TOML");
    assert_eq!(
        config2.general.provider_order,
        vec!["opencodego", "deepseek", "stepfun"]
    );
    assert!(toml_str.contains("provider_order"));
}

#[test]
fn test_provider_item_skip_serializing() {
    let item = ProviderItem {
        enabled: true,
        api_key: None,
        username: None,
        password: None,
        cookie_header: None,
        workspace_id: None,
    };

    let toml_str = toml::to_string_pretty(&item).expect("serialize");

    // None fields should be absent from the TOML output
    assert!(!toml_str.contains("api_key"));
    assert!(!toml_str.contains("username"));
    assert!(!toml_str.contains("password"));
    assert!(!toml_str.contains("cookie_header"));
    assert!(!toml_str.contains("workspace_id"));

    // enabled should still be present
    assert!(toml_str.contains("enabled"));
}

#[test]
fn test_config_ignores_legacy_cached_fields() {
    let toml_str = r#"
[providers.stepfun]
enabled = true
username = "user@test.com"
password = "pass"
cached_token = "legacy-token"
cached_ingress_cookie = "legacy-cookie"

[general]
refresh_interval_secs = 60
selected_provider = "stepfun"
"#;

    let config: Config = toml::from_str(toml_str).expect("deserialize with legacy fields");

    assert!(config.providers["stepfun"].enabled);
    assert_eq!(
        config.providers["stepfun"].username,
        Some("user@test.com".into())
    );
    assert_eq!(config.providers["stepfun"].password, Some("pass".into()));
    assert_eq!(config.general.selected_provider, "stepfun");
}

#[test]
fn test_deepseek_config() {
    let mut config = Config::default();
    config.providers.get_mut("deepseek").unwrap().api_key = Some("sk-abc".into());

    let ds = config.deepseek_config();
    assert!(ds.enabled);
    assert_eq!(ds.api_key, Some("sk-abc".into()));
    assert!(ds.username.is_none());
    assert!(ds.password.is_none());
    assert!(ds.cookie_header.is_none());
    assert!(ds.workspace_id.is_none());
}

#[test]
fn test_stepfun_config() {
    let mut config = Config::default();
    config.providers.get_mut("stepfun").unwrap().username = Some("user@test.com".into());
    config.providers.get_mut("stepfun").unwrap().password = Some("pass".into());

    let sf = config.stepfun_config();
    assert!(sf.enabled);
    assert!(sf.api_key.is_none());
    assert_eq!(sf.username, Some("user@test.com".into()));
    assert_eq!(sf.password, Some("pass".into()));
    assert!(sf.cookie_header.is_none());
    assert!(sf.workspace_id.is_none());
}

#[test]
fn test_opencodego_config() {
    let mut config = Config::default();
    let item = config.providers.get_mut("opencodego").unwrap();
    item.enabled = true;
    item.cookie_header = Some("sid=abc".into());
    item.workspace_id = Some("wrk_abc".into());

    let og = config.opencodego_config();
    assert!(og.enabled);
    assert!(og.api_key.is_none());
    assert!(og.username.is_none());
    assert!(og.password.is_none());
    assert_eq!(og.cookie_header, Some("sid=abc".into()));
    assert_eq!(og.workspace_id, Some("wrk_abc".into()));
}

#[test]
fn test_config_missing_provider() {
    let config = Config {
        providers: HashMap::new(),
        general: GeneralConfig::default(),
    };

    // When provider is missing, enabled should default to true
    let ds = config.deepseek_config();
    assert!(ds.enabled);
    assert!(ds.api_key.is_none());

    let sf = config.stepfun_config();
    assert!(sf.enabled);
    assert!(sf.username.is_none());
    assert!(sf.password.is_none());

    let og = config.opencodego_config();
    assert!(!og.enabled);
    assert!(og.cookie_header.is_none());
    assert!(og.workspace_id.is_none());
}
