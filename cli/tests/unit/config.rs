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
    assert!(config.general.budget_monthly.is_none());
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

    let toml_str = toml::to_string_pretty(&config).expect("serialize to TOML");
    let config2: Config = toml::from_str(&toml_str).expect("deserialize from TOML");

    assert_eq!(
        config2.providers["deepseek"].api_key,
        Some("sk-test123".into())
    );
    assert_eq!(config2.providers["deepseek"].enabled, true);
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
        cached_token: None,
        cached_ingress_cookie: None,
    };

    let toml_str = toml::to_string_pretty(&item).expect("serialize");

    // None fields should be absent from the TOML output
    assert!(!toml_str.contains("api_key"));
    assert!(!toml_str.contains("username"));
    assert!(!toml_str.contains("password"));
    assert!(!toml_str.contains("cookie_header"));
    assert!(!toml_str.contains("workspace_id"));
    assert!(!toml_str.contains("cached_token"));
    assert!(!toml_str.contains("cached_ingress_cookie"));

    // enabled should still be present
    assert!(toml_str.contains("enabled"));
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
