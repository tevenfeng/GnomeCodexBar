use super::*;

use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use cbc::Encryptor;

type Aes128CbcEnc = Encryptor<Aes128>;

/// Encrypt a cookie value the way Chromium does.
fn encrypt_value(value: &[u8], password: &[u8], rounds: u32, random_prefix: bool) -> Vec<u8> {
    let key = derive_chromium_key(password, rounds);
    let mut plaintext = Vec::new();
    if random_prefix {
        plaintext.resize(RANDOM_PREFIX_LEN, 0xAB);
    }
    plaintext.extend_from_slice(value);

    let cipher = Aes128CbcEnc::new_from_slices(&key, &CBC_IV).unwrap();
    let mut buffer = vec![0u8; plaintext.len() + 16];
    buffer[..plaintext.len()].copy_from_slice(&plaintext);
    let encrypted = cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
        .unwrap()
        .to_vec();

    let mut encoded = b"v11".to_vec();
    encoded.extend_from_slice(&encrypted);
    encoded
}

#[test]
fn derive_key_matches_known_vectors() {
    assert_eq!(
        derive_chromium_key(b"peanuts", LINUX_KEY_ROUNDS),
        [
            0xfd, 0x62, 0x1f, 0xe5, 0xa2, 0xb4, 0x02, 0x53, 0x9d, 0xfa, 0x14, 0x7c, 0xa9, 0x27,
            0x27, 0x78
        ]
    );
    assert_eq!(
        derive_chromium_key(b"peanuts", MACOS_KEY_ROUNDS),
        [
            0xd9, 0xa0, 0x9d, 0x49, 0x9b, 0x4e, 0x1b, 0x74, 0x61, 0xf2, 0x8e, 0x67, 0x97, 0x2c,
            0x6d, 0xbd
        ]
    );
    assert_eq!(
        derive_chromium_key(b"Brave Safe Storage", LINUX_KEY_ROUNDS),
        [
            0x67, 0xd6, 0xb7, 0x91, 0x1c, 0x85, 0xed, 0xc1, 0x33, 0xd4, 0xd5, 0xa3, 0xfa, 0xe7,
            0xa6, 0x0d
        ]
    );
}

#[test]
fn decrypts_modern_value_with_random_prefix() {
    let encoded = encrypt_value(b"Fe26.2**session", b"peanuts", LINUX_KEY_ROUNDS, true);
    assert_eq!(
        decrypt_chromium_value(&encoded, b"peanuts", LINUX_KEY_ROUNDS).as_deref(),
        Some("Fe26.2**session")
    );
}

#[test]
fn decrypts_legacy_value_without_prefix() {
    let encoded = encrypt_value(b"zh", b"peanuts", LINUX_KEY_ROUNDS, false);
    assert_eq!(
        decrypt_chromium_value(&encoded, b"peanuts", LINUX_KEY_ROUNDS).as_deref(),
        Some("zh")
    );
}

#[test]
fn decrypts_with_macos_round_count() {
    let encoded = encrypt_value(b"secret", b"Chrome Safe Storage", MACOS_KEY_ROUNDS, true);
    assert_eq!(
        decrypt_chromium_value(&encoded, b"Chrome Safe Storage", MACOS_KEY_ROUNDS).as_deref(),
        Some("secret")
    );
}

#[test]
fn rejects_wrong_password_and_malformed_payloads() {
    let encoded = encrypt_value(b"secret", b"right", LINUX_KEY_ROUNDS, true);
    assert_eq!(decrypt_chromium_value(&encoded, b"wrong", LINUX_KEY_ROUNDS), None);
    // Unknown version prefix.
    assert_eq!(decrypt_chromium_value(b"v99abcdefghijklmnop", b"x", 1), None);
    // Ciphertext that is not a whole number of blocks.
    assert_eq!(decrypt_chromium_value(b"v11short", b"x", 1), None);
    assert_eq!(decrypt_chromium_value(b"v11", b"x", 1), None);
}

#[test]
fn strip_version_prefix_only_accepts_v10_and_v11() {
    assert_eq!(strip_version_prefix(b"v10payload"), Some(&b"payload"[..]));
    assert_eq!(strip_version_prefix(b"v11payload"), Some(&b"payload"[..]));
    assert_eq!(strip_version_prefix(b"payload"), None);
    assert_eq!(strip_version_prefix(b"v12payload"), None);
}

#[test]
fn decode_cookie_plaintext_handles_prefix_and_plaintext() {
    let mut prefixed = vec![0u8; RANDOM_PREFIX_LEN];
    prefixed.extend_from_slice(b"value");
    assert_eq!(decode_cookie_plaintext(&prefixed).as_deref(), Some("value"));

    assert_eq!(decode_cookie_plaintext(b"plain").as_deref(), Some("plain"));
    assert_eq!(decode_cookie_plaintext(b""), None);
    // Random-looking bytes without a printable tail are not a cookie value.
    assert_eq!(decode_cookie_plaintext(&[0x00, 0x01, 0x02, 0x03]), None);
}

#[test]
fn known_browsers_cover_chromium_and_firefox_families() {
    let browsers = known_browsers();
    for expected in ["edge", "chrome", "chromium", "brave", "firefox"] {
        assert!(browsers.contains(&expected), "missing {expected}");
    }
}

#[test]
fn resolve_order_honours_preference_and_rejects_unknown() {
    let edge = resolve_order(Some("EDGE")).unwrap();
    assert_eq!(edge.chromium.len(), 1);
    assert_eq!(edge.chromium[0].id, "edge");
    assert!(edge.firefox.is_empty());

    let firefox = resolve_order(Some("firefox")).unwrap();
    assert!(firefox.chromium.is_empty());
    assert_eq!(firefox.firefox.len(), 1);

    let all = resolve_order(None).unwrap();
    assert_eq!(all.chromium.len(), CHROMIUM_BROWSERS.len());
    assert_eq!(all.firefox.len(), FIREFOX_BROWSERS.len());

    assert!(resolve_order(Some("netscape")).is_err());
}

#[test]
fn platform_rounds_matches_target_platform() {
    if cfg!(target_os = "macos") {
        assert_eq!(platform_rounds(), MACOS_KEY_ROUNDS);
    } else {
        assert_eq!(platform_rounds(), LINUX_KEY_ROUNDS);
    }
}
