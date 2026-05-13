//! Auto-start management for codex-bar-cli daemon.
//!
//! Supports:
//! - **Linux (ZorinOS)**: systemd user service
//! - **macOS**: launchd plist

use std::fs;
use std::path::PathBuf;

// ── Platform detection ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOS,
    Unsupported,
}

pub fn detect_platform() -> Platform {
    if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "macos") {
        Platform::MacOS
    } else {
        Platform::Unsupported
    }
}

// ── Public API ──────────────────────────────────────────

/// Enable auto-start for the daemon.
pub fn enable() -> anyhow::Result<String> {
    match detect_platform() {
        Platform::Linux => enable_systemd(),
        Platform::MacOS => enable_launchd(),
        Platform::Unsupported => Err(anyhow::anyhow!(
            "Auto-start is not supported on this platform"
        )),
    }
}

/// Disable auto-start for the daemon.
pub fn disable() -> anyhow::Result<String> {
    match detect_platform() {
        Platform::Linux => disable_systemd(),
        Platform::MacOS => disable_launchd(),
        Platform::Unsupported => Err(anyhow::anyhow!(
            "Auto-start is not supported on this platform"
        )),
    }
}

/// Check auto-start status.
pub fn status() -> anyhow::Result<String> {
    match detect_platform() {
        Platform::Linux => status_systemd(),
        Platform::MacOS => status_launchd(),
        Platform::Unsupported => Ok("Auto-start is not supported on this platform".into()),
    }
}

// ── CLI binary path ─────────────────────────────────────

fn cli_bin_path() -> anyhow::Result<String> {
    let exe = std::env::current_exe()?;
    Ok(exe.to_string_lossy().to_string())
}

// ════════════════════════════════════════════════════════
// Linux: systemd user service
// ════════════════════════════════════════════════════════

const SERVICE_NAME: &str = "codex-bar-cli";

fn systemd_service_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("systemd")
        .join("user")
        .join(format!("{}.service", SERVICE_NAME))
}

fn systemd_unit_content() -> anyhow::Result<String> {
    let bin = cli_bin_path()?;
    let escaped_bin = systemd_escape_exec_arg(&bin);
    Ok(format!(
        "[Unit]\n\
         Description=CodexBar CLI Daemon - Monitor coding plan usage\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={bin} daemon\n\
         Restart=on-failure\n\
         RestartSec=10\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        bin = escaped_bin,
    ))
}

fn systemd_escape_exec_arg(arg: &str) -> String {
    let needs_quotes = arg.chars().any(|c| {
        c.is_whitespace()
            || matches!(
                c,
                '\\' | '"' | '\'' | ';' | '&' | '<' | '>' | '|' | '$' | '`' | '%'
            )
    });

    if !needs_quotes {
        return arg.to_string();
    }

    let mut escaped = String::with_capacity(arg.len() + 2);
    escaped.push('"');
    for ch in arg.chars() {
        match ch {
            '\\' | '"' | '$' | '`' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            '%' => escaped.push_str("%%"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn enable_systemd() -> anyhow::Result<String> {
    let path = systemd_service_path();
    let content = systemd_unit_content()?;

    // Write service file
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, &content)?;

    // Reload + enable + restart. Restart also refreshes an already-running
    // daemon, which triggers its startup fetch immediately after re-enable.
    run_systemctl(&["--user", "daemon-reload"])?;
    run_systemctl(&["--user", "enable", SERVICE_NAME])?;
    run_systemctl(&["--user", "restart", SERVICE_NAME])?;

    Ok(format!(
        "Auto-start enabled (systemd). Service file: {}",
        path.display()
    ))
}

fn disable_systemd() -> anyhow::Result<String> {
    let path = systemd_service_path();

    // Stop + disable
    let _ = run_systemctl(&["--user", "stop", SERVICE_NAME]);
    let _ = run_systemctl(&["--user", "disable", SERVICE_NAME]);

    // Remove service file
    if path.exists() {
        fs::remove_file(&path)?;
        let _ = run_systemctl(&["--user", "daemon-reload"]);
    }

    Ok("Auto-start disabled (systemd). Service file removed.".into())
}

fn status_systemd() -> anyhow::Result<String> {
    let path = systemd_service_path();
    let file_exists = path.exists();

    // Check if service is active
    let active = run_systemctl(&["--user", "is-active", SERVICE_NAME])
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    let enabled = run_systemctl(&["--user", "is-enabled", SERVICE_NAME])
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    Ok(format!(
        "Platform: Linux (systemd)\n\
         Service file: {} {}\n\
         Active: {}\n\
         Enabled: {}",
        path.display(),
        if file_exists {
            "(exists)"
        } else {
            "(not found)"
        },
        active,
        enabled,
    ))
}

fn run_systemctl(args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new("systemctl")
        .args(args)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("systemctl {:?}: {}", args, stderr.trim()))
    }
}

// ════════════════════════════════════════════════════════
// macOS: launchd plist
// ════════════════════════════════════════════════════════

const LAUNCHD_LABEL: &str = "com.codexbar.cli";

fn launchd_plist_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", LAUNCHD_LABEL))
}

fn launchd_plist_content() -> anyhow::Result<String> {
    let bin = cli_bin_path()?;
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("gnome-codex-bar")
        .join("logs");

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
             <key>Label</key>\n\
             <string>{label}</string>\n\
             <key>ProgramArguments</key>\n\
             <array>\n\
                 <string>{bin}</string>\n\
                 <string>daemon</string>\n\
             </array>\n\
             <key>RunAtLoad</key>\n\
             <true/>\n\
             <key>KeepAlive</key>\n\
             <true/>\n\
             <key>StandardOutPath</key>\n\
             <string>{log_dir}/daemon.log</string>\n\
             <key>StandardErrorPath</key>\n\
             <string>{log_dir}/daemon.err</string>\n\
         </dict>\n\
         </plist>\n",
        label = LAUNCHD_LABEL,
        bin = xml_escape(&bin),
        log_dir = xml_escape(&log_dir.display().to_string()),
    ))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn enable_launchd() -> anyhow::Result<String> {
    let path = launchd_plist_path();
    let content = launchd_plist_content()?;

    // Ensure log directory exists
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("gnome-codex-bar")
        .join("logs");
    fs::create_dir_all(&log_dir)?;

    // Write plist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, &content)?;

    // Unload first (in case already loaded), then load
    let _ = run_launchctl(&["unload", &path.to_string_lossy()]);
    run_launchctl(&["load", &path.to_string_lossy()])?;

    Ok(format!(
        "Auto-start enabled (launchd). Plist: {}",
        path.display()
    ))
}

fn disable_launchd() -> anyhow::Result<String> {
    let path = launchd_plist_path();

    // Unload
    if path.exists() {
        let _ = run_launchctl(&["unload", &path.to_string_lossy()]);
        fs::remove_file(&path)?;
    }

    Ok("Auto-start disabled (launchd). Plist removed.".into())
}

fn status_launchd() -> anyhow::Result<String> {
    let path = launchd_plist_path();
    let file_exists = path.exists();

    // Check if loaded via launchctl list
    let loaded = run_launchctl(&["list", LAUNCHD_LABEL])
        .map(|s| {
            // If the label exists, launchctl returns details (exit 0)
            // If not, it returns an error
            s.contains("PID") || s.contains(LAUNCHD_LABEL)
        })
        .unwrap_or(false);

    Ok(format!(
        "Platform: macOS (launchd)\n\
         Plist: {} {}\n\
         Loaded: {}",
        path.display(),
        if file_exists {
            "(exists)"
        } else {
            "(not found)"
        },
        if loaded { "yes" } else { "no" },
    ))
}

fn run_launchctl(args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new("launchctl")
        .args(args)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // launchctl list for non-existent label returns error, which is expected
        Err(anyhow::anyhow!("launchctl {:?}: {}", args, stderr.trim()))
    }
}

// ════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════

#[cfg(test)]
#[path = "../tests/unit/autostart.rs"]
mod tests;
