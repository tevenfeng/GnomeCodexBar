use super::*;

#[test]
fn test_detect_platform() {
    // Just verify it doesn't panic and returns a valid variant
    let platform = detect_platform();
    match platform {
        Platform::Linux | Platform::MacOS | Platform::Unsupported => {}
    }
}

#[test]
fn test_systemd_service_path() {
    let path = systemd_service_path();
    assert!(path.to_string_lossy().contains("systemd"));
    assert!(path.to_string_lossy().contains("user"));
    assert!(path.to_string_lossy().contains("codex-bar-cli.service"));
}

#[test]
fn test_launchd_plist_path() {
    let path = launchd_plist_path();
    assert!(path.to_string_lossy().contains("LaunchAgents"));
    assert!(path.to_string_lossy().contains("com.codexbar.cli.plist"));
}

#[test]
fn test_systemd_unit_content() {
    let content = systemd_unit_content().unwrap();

    assert!(content.contains("[Unit]"));
    assert!(content.contains("[Service]"));
    assert!(content.contains("[Install]"));
    assert!(content.contains("Type=simple"));
    assert!(content.contains("daemon"));
    assert!(content.contains("Restart=on-failure"));
    assert!(content.contains("WantedBy=default.target"));
}

#[test]
fn test_launchd_plist_content() {
    let content = launchd_plist_content().unwrap();

    assert!(content.contains("<?xml version="));
    assert!(content.contains("<!DOCTYPE plist"));
    assert!(content.contains("<key>Label</key>"));
    assert!(content.contains("com.codexbar.cli"));
    assert!(content.contains("<key>RunAtLoad</key>"));
    assert!(content.contains("<true/>"));
    assert!(content.contains("<key>KeepAlive</key>"));
    assert!(content.contains("daemon"));
    assert!(content.contains("StandardOutPath"));
    assert!(content.contains("StandardErrorPath"));
}

#[test]
fn test_systemd_unit_has_network_dependency() {
    let content = systemd_unit_content().unwrap();
    assert!(content.contains("After=network-online.target"));
    assert!(content.contains("Wants=network-online.target"));
}

#[test]
fn test_launchd_plist_has_log_paths() {
    let content = launchd_plist_content().unwrap();
    assert!(content.contains("daemon.log"));
    assert!(content.contains("daemon.err"));
}

#[test]
fn test_systemd_unit_uses_absolute_path() {
    let content = systemd_unit_content().unwrap();
    // ExecStart should contain an absolute path (starts with /)
    for line in content.lines() {
        if line.contains("ExecStart=") {
            let path = line.split('=').nth(1).unwrap();
            assert!(path.starts_with('/'), "ExecStart path should be absolute: {}", path);
        }
    }
}
