//! Read cookies straight out of locally installed browsers.
//!
//! Used to pick up the OpenCode console session cookie without asking the user
//! to copy it out of the developer tools.
//!
//! Chromium-based browsers store cookie values encrypted in a SQLite database:
//!
//! * `v10` — encrypted with the hardcoded `peanuts` password.
//! * `v11` — encrypted with a password kept in the OS credential store
//!   (Secret Service on Linux, Keychain on macOS).
//!
//! Either way the scheme is `AES-128-CBC` with a key of 16 spaces as IV and a key
//! derived as `PBKDF2-HMAC-SHA1(password, "saltysalt", rounds, 16)`. Recent
//! Chromium builds prepend 32 random bytes to the value before encrypting.
//!
//! Firefox keeps cookie values in plain text, so its database is read directly.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use aes::Aes128;
use anyhow::{Context, Result};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use cbc::Decryptor;
use pbkdf2::pbkdf2_hmac;
use sha1::Sha1;

type Aes128CbcDec = Decryptor<Aes128>;

pub const KEY_SALT: &[u8] = b"saltysalt";
pub const CBC_IV: [u8; 16] = [0x20; 16];
/// PBKDF2 rounds used by Chromium on Linux.
pub const LINUX_KEY_ROUNDS: u32 = 1;
/// PBKDF2 rounds used by Chromium on macOS.
pub const MACOS_KEY_ROUNDS: u32 = 1003;
/// Chromium's hardcoded fallback password (`--password-store=basic`).
pub const FALLBACK_PASSWORD: &[u8] = b"peanuts";
/// Chromium >= M127 prefixes the cookie value with this many random bytes.
pub const RANDOM_PREFIX_LEN: usize = 32;

/// Attributes used by Chromium for its "Safe Storage" entry in the Secret Service.
const CHROMIUM_KEYRING_SCHEMA: &str = "chrome_libsecret_os_crypt_password_v2";

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

// ── Browser definitions ─────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct ChromiumBrowser {
    id: &'static str,
    /// Directory name under the config root (Linux) / Application Support (macOS).
    dir: &'static str,
    /// Keychain service names to try on macOS.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    keychain_services: &'static [&'static str],
    /// Flatpak application ids shipping this browser.
    flatpak_ids: &'static [&'static str],
    /// Snap package names shipping this browser.
    snap_names: &'static [&'static str],
}

const CHROMIUM_BROWSERS: &[ChromiumBrowser] = &[
    ChromiumBrowser {
        id: "edge",
        dir: "microsoft-edge",
        keychain_services: &["Microsoft Edge Safe Storage", "Chromium Safe Storage"],
        flatpak_ids: &["com.microsoft.Edge"],
        snap_names: &["microsoft-edge"],
    },
    ChromiumBrowser {
        id: "chrome",
        dir: "google-chrome",
        keychain_services: &["Chrome Safe Storage", "Chromium Safe Storage"],
        flatpak_ids: &["com.google.Chrome"],
        snap_names: &[],
    },
    ChromiumBrowser {
        id: "chromium",
        dir: "chromium",
        keychain_services: &["Chromium Safe Storage"],
        flatpak_ids: &["org.chromium.Chromium"],
        snap_names: &["chromium"],
    },
    ChromiumBrowser {
        id: "brave",
        dir: "BraveSoftware/Brave-Browser",
        keychain_services: &["Brave Safe Storage", "Chromium Safe Storage"],
        flatpak_ids: &["com.brave.Browser"],
        snap_names: &["brave"],
    },
    ChromiumBrowser {
        id: "vivaldi",
        dir: "vivaldi",
        keychain_services: &["Vivaldi Safe Storage", "Chrome Safe Storage"],
        flatpak_ids: &["com.vivaldi.Vivaldi"],
        snap_names: &[],
    },
    ChromiumBrowser {
        id: "opera",
        dir: "opera",
        keychain_services: &["Opera Safe Storage", "Chrome Safe Storage"],
        flatpak_ids: &["com.opera.Opera"],
        snap_names: &[],
    },
];

#[derive(Debug, Clone, Copy)]
struct FirefoxBrowser {
    id: &'static str,
    /// Directory (may contain `~/.mozilla/firefox` style nesting) under the home dir.
    linux_dir: &'static str,
    /// Directory under Application Support on macOS.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    macos_dir: &'static str,
}

const FIREFOX_BROWSERS: &[FirefoxBrowser] = &[
    FirefoxBrowser {
        id: "firefox",
        linux_dir: ".mozilla/firefox",
        macos_dir: "Firefox/Profiles",
    },
    FirefoxBrowser {
        id: "librewolf",
        linux_dir: ".librewolf",
        macos_dir: "librewolf",
    },
    FirefoxBrowser {
        id: "zen",
        linux_dir: ".zen",
        macos_dir: "zen",
    },
    FirefoxBrowser {
        id: "waterfox",
        linux_dir: ".waterfox",
        macos_dir: "Waterfox/Profiles",
    },
];

/// Every browser id this module knows how to read, in resolution order.
pub fn known_browsers() -> Vec<&'static str> {
    CHROMIUM_BROWSERS
        .iter()
        .map(|b| b.id)
        .chain(FIREFOX_BROWSERS.iter().map(|b| b.id))
        .collect()
}

// ── Public result types ─────────────────────────────────

/// A cookie recovered from a browser profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserCookie {
    pub browser: &'static str,
    pub cookie_name: String,
    pub value: String,
    /// Profile directory the cookie came from (`None` for Firefox-free reads).
    pub profile: PathBuf,
}

impl BrowserCookie {
    /// Render a `Cookie:` request header value.
    pub fn header_value(&self) -> String {
        format!("{}={}", self.cookie_name, self.value)
    }
}

// ── Crypto helpers ──────────────────────────────────────

/// Derive Chromium's AES key from a "Safe Storage" password.
pub fn derive_chromium_key(password: &[u8], rounds: u32) -> [u8; 16] {
    let mut key = [0u8; 16];
    pbkdf2_hmac::<Sha1>(password, KEY_SALT, rounds, &mut key);
    key
}

fn is_printable(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.iter().all(|b| (0x20..0x7f).contains(b))
}

/// Turn a decrypted Chromium plaintext into the cookie value.
///
/// Newer builds prefix the value with 32 random bytes, older ones store it as-is.
pub fn decode_cookie_plaintext(plaintext: &[u8]) -> Option<String> {
    if plaintext.is_empty() {
        return None;
    }
    if plaintext.len() > RANDOM_PREFIX_LEN && is_printable(&plaintext[RANDOM_PREFIX_LEN..]) {
        return Some(String::from_utf8_lossy(&plaintext[RANDOM_PREFIX_LEN..]).into_owned());
    }
    if is_printable(plaintext) {
        return Some(String::from_utf8_lossy(plaintext).into_owned());
    }
    None
}

/// Decrypt a `v10`/`v11` Chromium cookie value with one candidate password.
pub fn decrypt_chromium_value(encoded: &[u8], password: &[u8], rounds: u32) -> Option<String> {
    let payload = strip_version_prefix(encoded)?;
    if payload.is_empty() || payload.len() % 16 != 0 {
        return None;
    }
    let key = derive_chromium_key(password, rounds);
    let mut buffer = payload.to_vec();
    let decryptor = Aes128CbcDec::new_from_slices(&key, &CBC_IV).ok()?;
    let plaintext = decryptor.decrypt_padded_mut::<Pkcs7>(&mut buffer).ok()?;
    decode_cookie_plaintext(plaintext)
}

/// Return the ciphertext portion of a versioned Chromium value.
pub fn strip_version_prefix(encoded: &[u8]) -> Option<&[u8]> {
    if encoded.starts_with(b"v10") || encoded.starts_with(b"v11") {
        Some(&encoded[3..])
    } else {
        None
    }
}

// ── Platform key stores ─────────────────────────────────

/// PBKDF2 round count for the platform currently running.
pub fn platform_rounds() -> u32 {
    if cfg!(target_os = "macos") {
        MACOS_KEY_ROUNDS
    } else {
        LINUX_KEY_ROUNDS
    }
}

/// A password candidate together with a human readable origin (for diagnostics).
#[derive(Debug, Clone)]
pub struct KeyCandidate {
    pub source: String,
    pub password: Vec<u8>,
}

/// Collect password candidates from the OS credential store.
///
/// On Linux every `chrome_libsecret_os_crypt_password_v2` entry is tried, because
/// Chromium builds do not agree on which `application` name they store their key
/// under (Microsoft Edge on Linux, for example, reuses the `chromium` entry).
/// On macOS the Keychain is queried per browser service name.
pub async fn key_candidates() -> Vec<KeyCandidate> {
    let mut candidates = keyring_candidates().await;
    candidates.push(KeyCandidate {
        source: "built-in fallback".to_string(),
        password: FALLBACK_PASSWORD.to_vec(),
    });
    candidates
}

#[cfg(target_os = "linux")]
async fn keyring_candidates() -> Vec<KeyCandidate> {
    use secret_service::{EncryptionType, SecretService};

    let mut out = Vec::new();
    let service = match SecretService::connect(EncryptionType::Plain).await {
        Ok(service) => service,
        Err(error) => {
            log::debug!("Secret Service unavailable, cannot read Chromium cookie key: {error}");
            return out;
        }
    };

    let attributes = HashMap::from([("xdg:schema", CHROMIUM_KEYRING_SCHEMA)]);
    let items = match service.search_items(attributes).await {
        Ok(items) => items,
        Err(error) => {
            log::debug!("Secret Service search failed: {error}");
            return out;
        }
    };

    for item in items.unlocked {
        let label = item
            .get_label()
            .await
            .unwrap_or_else(|_| "unknown".to_string());
        match item.get_secret().await {
            Ok(secret) if !secret.is_empty() => out.push(KeyCandidate {
                source: format!("keyring item {label:?}"),
                password: secret,
            }),
            Ok(_) => {}
            Err(error) => log::debug!("Failed to read keyring item {label:?}: {error}"),
        }
    }

    if out.is_empty() {
        log::debug!("No Chromium \"Safe Storage\" entries found in the Secret Service");
    }
    out
}

#[cfg(not(target_os = "linux"))]
async fn keyring_candidates() -> Vec<KeyCandidate> {
    let mut out = Vec::new();
    for browser in CHROMIUM_BROWSERS {
        for service in browser.keychain_services {
            if let Some(password) = macos_keychain_password(service) {
                out.push(KeyCandidate {
                    source: format!("keychain {service:?}"),
                    password,
                });
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn macos_keychain_password(service: &str) -> Option<Vec<u8>> {
    let output = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-w", "-s", service])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let password = output.stdout;
    let trimmed = password.strip_suffix(b"\n").unwrap_or(&password);
    (!trimmed.is_empty()).then(|| trimmed.to_vec())
}

// ── Profile discovery ───────────────────────────────────

fn chromium_data_dirs(browser: &ChromiumBrowser) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(target_os = "linux")]
    {
        if let Some(config) = dirs::config_dir() {
            dirs.push(config.join(browser.dir));
        }
        if let Some(home) = dirs::home_dir() {
            for app_id in browser.flatpak_ids {
                dirs.push(
                    home.join(".var/app")
                        .join(app_id)
                        .join("config")
                        .join(browser.dir),
                );
            }
            for snap in browser.snap_names {
                dirs.push(
                    home.join("snap")
                        .join(snap)
                        .join("current/.config")
                        .join(browser.dir),
                );
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(support) = dirs::data_dir() {
            dirs.push(support.join(browser.dir));
        }
    }
    dirs
}

fn firefox_data_dirs(browser: &FirefoxBrowser) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join(browser.linux_dir));
        }
        if let Some(home) = dirs::home_dir() {
            if browser.id == "firefox" {
                dirs.push(home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"));
                dirs.push(home.join("snap/firefox/common/.mozilla/firefox"));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(support) = dirs::data_dir() {
            dirs.push(support.join(browser.macos_dir));
        }
    }
    dirs
}

/// Chromium keeps one directory per profile; prefer `Default`, then the rest.
fn chromium_profiles(data_dir: &Path) -> Vec<PathBuf> {
    let mut names: Vec<String> = Vec::new();

    if let Ok(text) = fs::read_to_string(data_dir.join("Local State")) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(cache) = value
                .get("profile")
                .and_then(|profile| profile.get("info_cache"))
                .and_then(|cache| cache.as_object())
            {
                names.extend(cache.keys().cloned());
            }
        }
    }

    if let Ok(entries) = fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "Default" || name.starts_with("Profile ") {
                names.push(name);
            }
        }
    }

    if !names.iter().any(|name| name == "Default") {
        names.push("Default".to_string());
    }

    let mut profiles: Vec<PathBuf> = names
        .into_iter()
        .map(|name| data_dir.join(name))
        .filter(|path| path.join("Cookies").is_file())
        .collect();
    profiles.sort_by_key(|path| {
        let is_default = path.file_name().map(|n| n == "Default").unwrap_or(false);
        (!is_default, path.clone())
    });
    profiles.dedup();
    profiles
}

/// Firefox stores one directory per profile, sometimes nested one level deeper.
fn firefox_profiles(data_dir: &Path) -> Vec<PathBuf> {
    let mut profiles = Vec::new();
    let Ok(entries) = fs::read_dir(data_dir) else {
        return profiles;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.join("cookies.sqlite").is_file() {
            profiles.push(path);
            continue;
        }
        if let Ok(nested) = fs::read_dir(&path) {
            for inner in nested.flatten() {
                let inner_path = inner.path();
                if inner_path.join("cookies.sqlite").is_file() {
                    profiles.push(inner_path);
                }
            }
        }
    }
    profiles.sort();
    profiles
}

// ── Database access ─────────────────────────────────────

#[derive(Debug, Clone)]
struct RawCookie {
    name: String,
    /// Encrypted value (Chromium), empty for Firefox.
    encrypted: Vec<u8>,
    /// Plain value (Firefox, or Chromium cookies stored unencrypted).
    plain: String,
}

/// Copy a live database (plus its WAL) somewhere private before reading, so a
/// running browser cannot block us with a lock.
fn with_database_copy<T>(database: &Path, action: impl FnOnce(&Path) -> Result<T>) -> Result<T> {
    let dir = std::env::temp_dir().join(format!(
        "codex-bar-cookies-{}-{}",
        std::process::id(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).with_context(|| format!("create temp dir {}", dir.display()))?;

    let result = (|| {
        let file_name = database
            .file_name()
            .map(|name| name.to_owned())
            .unwrap_or_else(|| "Cookies".into());
        let destination = dir.join(file_name);
        fs::copy(database, &destination)
            .with_context(|| format!("copy {}", database.display()))?;
        for suffix in ["-wal", "-shm"] {
            let source = PathBuf::from(format!("{}{}", database.display(), suffix));
            if source.is_file() {
                let _ = fs::copy(&source, PathBuf::from(format!("{}{}", destination.display(), suffix)));
            }
        }
        action(&destination)
    })();

    let _ = fs::remove_dir_all(&dir);
    result
}

fn read_chromium_cookies(database: &Path, host: &str, names: &[&str]) -> Result<Vec<RawCookie>> {
    with_database_copy(database, |path| {
        let connection = rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let mut statement = connection.prepare(
            "SELECT name, encrypted_value, value FROM cookies \
             WHERE host_key = ?1 OR host_key = ?2",
        )?;
        let rows = statement.query_map(rusqlite::params![host, format!(".{host}")], |row| {
            Ok(RawCookie {
                name: row.get(0)?,
                encrypted: row.get::<_, Vec<u8>>(1).unwrap_or_default(),
                plain: row.get::<_, String>(2).unwrap_or_default(),
            })
        })?;
        let mut found = Vec::new();
        for row in rows {
            let row = row?;
            if names.contains(&row.name.as_str()) {
                found.push(row);
            }
        }
        Ok(found)
    })
}

fn read_firefox_cookies(database: &Path, host: &str, names: &[&str]) -> Result<Vec<RawCookie>> {
    with_database_copy(database, |path| {
        let connection = rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let mut statement = connection.prepare(
            "SELECT name, value FROM moz_cookies WHERE host = ?1 OR host = ?2",
        )?;
        let rows = statement.query_map(rusqlite::params![host, format!(".{host}")], |row| {
            Ok(RawCookie {
                name: row.get(0)?,
                encrypted: Vec::new(),
                plain: row.get::<_, String>(1).unwrap_or_default(),
            })
        })?;
        let mut found = Vec::new();
        for row in rows {
            let row = row?;
            if names.contains(&row.name.as_str()) {
                found.push(row);
            }
        }
        Ok(found)
    })
}

// ── Lookup ──────────────────────────────────────────────

/// Search local browsers for one of `cookie_names` belonging to `host`.
///
/// `preferred` restricts the search to a single browser id (see
/// [`known_browsers`]). Returns `Ok(None)` when the cookie simply is not there.
pub async fn find_cookie(
    preferred: Option<&str>,
    host: &str,
    cookie_names: &[&str],
) -> Result<Option<BrowserCookie>> {
    let order = resolve_order(preferred)?;

    // Firefox values are not encrypted, so look there first and skip the keyring
    // entirely when it already has what we need.
    for browser in order.firefox.iter().copied() {
        for data_dir in firefox_data_dirs(browser) {
            for profile in firefox_profiles(&data_dir) {
                let database = profile.join("cookies.sqlite");
                match read_firefox_cookies(&database, host, cookie_names) {
                    Ok(rows) => {
                        if let Some(row) = pick_row(&rows, cookie_names) {
                            if !row.plain.is_empty() {
                                return Ok(Some(BrowserCookie {
                                    browser: browser.id,
                                    cookie_name: row.name.clone(),
                                    value: row.plain.clone(),
                                    profile,
                                }));
                            }
                        }
                    }
                    Err(error) => log::debug!(
                        "Could not read Firefox cookies at {}: {error}",
                        database.display()
                    ),
                }
            }
        }
    }

    let profiles = chromium_profiles_with_cookies(&order);
    if profiles.is_empty() {
        log::debug!("No Chromium profile with a cookie database was found");
        return Ok(None);
    }

    let candidates = key_candidates().await;
    let rounds = platform_rounds();

    for (browser_id, profile, database) in profiles {
        let rows = match read_chromium_cookies(&database, host, cookie_names) {
            Ok(rows) => rows,
            Err(error) => {
                log::debug!(
                    "Could not read Chromium cookies at {}: {error}",
                    database.display()
                );
                continue;
            }
        };
        let Some(row) = pick_row(&rows, cookie_names) else {
            continue;
        };
        if !row.plain.is_empty() && row.encrypted.is_empty() {
            return Ok(Some(BrowserCookie {
                browser: browser_id,
                cookie_name: row.name.clone(),
                value: row.plain.clone(),
                profile,
            }));
        }
        for candidate in &candidates {
            if let Some(value) = decrypt_chromium_value(&row.encrypted, &candidate.password, rounds) {
                log::debug!(
                    "Decrypted {} from {} using {}",
                    row.name,
                    profile.display(),
                    candidate.source
                );
                return Ok(Some(BrowserCookie {
                    browser: browser_id,
                    cookie_name: row.name.clone(),
                    value,
                    profile,
                }));
            }
        }
        return Err(anyhow::anyhow!(
            "found {} in the {} profile at {} but could not decrypt it; \
             the browser's cookie key is not available to this process",
            row.name,
            browser_id,
            profile.display()
        ));
    }

    Ok(None)
}

/// Pick the highest priority cookie name that is present.
fn pick_row<'a>(rows: &'a [RawCookie], cookie_names: &[&str]) -> Option<&'a RawCookie> {
    cookie_names
        .iter()
        .find_map(|name| rows.iter().find(|row| row.name == *name))
}

struct BrowserOrder {
    chromium: Vec<&'static ChromiumBrowser>,
    firefox: Vec<&'static FirefoxBrowser>,
}

fn resolve_order(preferred: Option<&str>) -> Result<BrowserOrder> {
    let preferred = preferred.map(|value| value.trim().to_ascii_lowercase());
    let matches = |id: &str| match &preferred {
        Some(wanted) => id == wanted,
        None => true,
    };
    let order = BrowserOrder {
        chromium: CHROMIUM_BROWSERS
            .iter()
            .filter(|browser| matches(browser.id))
            .collect(),
        firefox: FIREFOX_BROWSERS
            .iter()
            .filter(|browser| matches(browser.id))
            .collect(),
    };
    if order.chromium.is_empty() && order.firefox.is_empty() {
        return Err(anyhow::anyhow!(
            "unknown browser {:?}; supported: {}",
            preferred.unwrap_or_default(),
            known_browsers().join(", ")
        ));
    }
    Ok(order)
}

fn chromium_profiles_with_cookies(order: &BrowserOrder) -> Vec<(&'static str, PathBuf, PathBuf)> {
    let mut found = Vec::new();
    for browser in &order.chromium {
        for data_dir in chromium_data_dirs(browser) {
            for profile in chromium_profiles(&data_dir) {
                found.push((browser.id, profile.clone(), profile.join("Cookies")));
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "../tests/unit/browser_cookies.rs"]
mod tests;
