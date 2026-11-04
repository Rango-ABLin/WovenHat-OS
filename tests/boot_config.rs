//! Exercise the exact parser linked into the UEFI entry implementation.

#[path = "../vendor/bootloader/uefi/src/config.rs"]
mod config;

use config::{Error, MAX_CONFIG_BYTES};

#[test]
fn legacy_defaults_and_explicit_v1_preserve_boot_settings() {
    let defaults = config::parse(b"{}").unwrap();
    assert!(defaults.serial_logging && defaults.frame_buffer_logging);
    let configured = config::parse(
        br#"{"version":1,"serial_logging":false,"frame_buffer_logging":false,
        "log_level":"Warn","frame_buffer":{"minimum_framebuffer_width":640,
        "minimum_framebuffer_height":480},"_test_sentinel":42}"#,
    )
    .unwrap();
    assert!(!configured.serial_logging && !configured.frame_buffer_logging);
    assert_eq!(
        configured.log_level,
        bootloader_boot_config::LevelFilter::Warn
    );
    assert_eq!(configured.frame_buffer.minimum_framebuffer_width, Some(640));
    assert_eq!(
        configured.frame_buffer.minimum_framebuffer_height,
        Some(480)
    );
    assert_eq!(configured._test_sentinel, 42);
}

#[test]
fn unsupported_versions_are_not_silently_downgraded() {
    for bytes in [br#"{"version":0}"#.as_slice(), br#"{"version":2}"#] {
        assert!(matches!(
            config::parse(bytes),
            Err(Error::UnsupportedVersion)
        ));
    }
    assert!(matches!(
        config::parse(br#"{"version":null}"#),
        Err(Error::Malformed)
    ));
}

#[test]
fn malformed_duplicate_and_unknown_policy_are_rejected() {
    for bytes in [
        b"".as_slice(),
        b"{",
        br#"{"serial_logging":true,"serial_logging":false}"#,
        br#"{"recovery_mode":true}"#,
        br#"{"frame_buffer":{"unknown":1}}"#,
        br#"{"serial_logging":"false"}"#,
    ] {
        assert!(matches!(config::parse(bytes), Err(Error::Malformed)));
    }
}

#[test]
fn only_json_whitespace_may_follow_the_configuration() {
    assert!(config::parse(b"{} \t\r\n").is_ok());
    for bytes in [b"{}garbage".as_slice(), b"{}{}", b"{}\x0c", b"{}\0"] {
        assert!(matches!(config::parse(bytes), Err(Error::TrailingData)));
    }
}

#[test]
fn configuration_size_is_bounded_including_whitespace() {
    let mut exact = vec![b' '; MAX_CONFIG_BYTES];
    exact[..2].copy_from_slice(b"{}");
    assert!(config::parse(&exact).is_ok());
    exact.push(b' ');
    assert!(matches!(config::parse(&exact), Err(Error::TooLarge)));
}
