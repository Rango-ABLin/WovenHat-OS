//! Exercise the exact parser linked into the UEFI entry implementation.

#[path = "../vendor/bootloader/uefi/src/config.rs"]
mod config;

use config::{Error, MAX_CONFIG_BYTES};

#[test]
fn legacy_defaults_and_explicit_v1_preserve_boot_settings() {
    let defaults = config::parse(b"{}").unwrap().boot;
    assert!(defaults.serial_logging && defaults.frame_buffer_logging);
    let configured = config::parse(
        br#"{"version":1,"serial_logging":false,"frame_buffer_logging":false,
        "log_level":"Warn","frame_buffer":{"minimum_framebuffer_width":640,
        "minimum_framebuffer_height":480},"_test_sentinel":42}"#,
    )
    .unwrap()
    .boot;
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

#[test]
fn selected_policy_is_bounded_and_copied_to_handoff() {
    let legacy = config::parse(b"{}").unwrap();
    assert_eq!(legacy.handoff.version, 1);
    assert_eq!(legacy.handoff.entry, 0);
    let selected = config::parse(
        br#"{"boot_entry":"recovery","root":"recovery","timeout_ms":17,"command_line":"recover"}"#,
    )
    .unwrap();
    assert_eq!(selected.handoff.entry, 1);
    assert_eq!(selected.handoff.root, 1);
    assert_eq!(selected.handoff.timeout_ms, 17);
    assert_eq!(selected.handoff.command_line_len, 7);
    assert_eq!(&selected.handoff.command_line[..7], b"recover");
    assert!(selected.handoff.command_line[7..].iter().all(|b| *b == 0));
    for bytes in [
        br#"{"timeout_ms":30001}"#.as_slice(),
        br#"{"boot_entry":"unknown"}"#,
    ] {
        assert!(config::parse(bytes).is_err());
    }
}

#[test]
fn command_line_capacity_is_bytes_not_characters() {
    for (text, accepted) in [
        ("a".repeat(256), true),
        ("a".repeat(257), false),
        ("é".repeat(128), true),
        ("é".repeat(129), false),
    ] {
        let wire = format!(r#"{{"command_line":"{text}"}}"#);
        assert_eq!(config::parse(wire.as_bytes()).is_ok(), accepted);
    }
}

#[test]
fn kernel_path_is_relative_bounded_data() {
    assert_eq!(
        config::parse(br#"{"kernel_path":"boot/alternate-kernel"}"#)
            .unwrap()
            .kernel_path
            .as_deref(),
        Some("boot/alternate-kernel")
    );
    for path in [
        "",
        "/kernel",
        "../kernel",
        "boot/../kernel",
        "boot//kernel",
        "boot/./kernel",
        "kernel;poweroff",
        "C:/kernel",
    ] {
        let wire = format!(r#"{{"kernel_path":"{path}"}}"#);
        assert!(config::parse(wire.as_bytes()).is_err(), "{path}");
    }
    assert!(config::parse(br#"{"command_line":"x\u0000y"}"#).is_err());
    assert!(config::parse(br#"{"command_line":"x\ny"}"#).is_err());
}

#[test]
fn json_escapes_are_decoded_once_and_invalid_scalars_are_rejected() {
    let decoded = config::parse(
        br#"{"kernel_path":"boot\u002fkernel","command_line":"\"a\" \\ \ud83d\ude00"}"#,
    )
    .unwrap();
    assert_eq!(decoded.kernel_path.as_deref(), Some("boot/kernel"));
    let length = usize::from(decoded.handoff.command_line_len);
    assert_eq!(
        core::str::from_utf8(&decoded.handoff.command_line[..length]).unwrap(),
        "\"a\" \\ 😀"
    );
    for wire in [
        br#"{"command_line":"\ud800"}"#.as_slice(),
        br#"{"command_line":"\udc00"}"#,
        br#"{"command_line":"\ud800\u0041"}"#,
        br#"{"command_line":"\q"}"#,
    ] {
        assert!(config::parse(wire).is_err());
    }
}

#[test]
fn handoff_rejects_unknown_versions_reserved_bytes_and_noncanonical_strings() {
    use bootloader_api::info::BootPolicy;
    assert!(BootPolicy::default().is_valid());
    let valid = config::parse(br#"{"boot_entry":"diagnostics","command_line":"test"}"#)
        .unwrap()
        .handoff;
    assert!(valid.is_valid());
    for mutate in [
        |p: &mut BootPolicy| p.version = 2,
        |p: &mut BootPolicy| p.entry = 3,
        |p: &mut BootPolicy| p.root = 2,
        |p: &mut BootPolicy| p.timeout_ms = 30_001,
        |p: &mut BootPolicy| p.reserved[0] = 1,
        |p: &mut BootPolicy| p.command_line_len = 257,
        |p: &mut BootPolicy| p.command_line[255] = 1,
        |p: &mut BootPolicy| p.command_line[0] = 0xff,
        |p: &mut BootPolicy| p.command_line[0] = b'\n',
        |p: &mut BootPolicy| p.version = 0,
    ] {
        let mut invalid = valid;
        mutate(&mut invalid);
        assert!(!invalid.is_valid());
    }
}

#[test]
fn incompatible_upstream_bootinfo_contract_is_rejected_before_handoff() {
    let mut contract = bootloader_api::BootloaderConfig::new_default().serialize();
    assert!(bootloader_api::BootloaderConfig::deserialize(&contract).is_ok());
    contract[15] = 0x3d; // upstream BootInfo without WovenHat's extension
    assert!(bootloader_api::BootloaderConfig::deserialize(&contract).is_err());
    assert!(
        core::mem::offset_of!(bootloader_api::BootInfo, boot_policy)
            > core::mem::offset_of!(bootloader_api::BootInfo, _test_sentinel)
    );
}
