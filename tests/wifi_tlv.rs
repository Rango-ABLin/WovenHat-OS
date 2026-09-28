#[path = "../kernel/src/wifi_firmware_tlv.rs"]
pub mod wifi_tlv;
pub use wifi_tlv as tlv;
#[path = "../kernel/src/wifi_ax200_image.rs"]
pub mod ax200_image;

use ax200_image::*;
use wifi_tlv::*;

fn image() -> Vec<u8> {
    let mut bytes = vec![0; INTEL_TLV_UCODE_HEADER_SIZE];
    bytes[4..8].copy_from_slice(&INTEL_TLV_UCODE_MAGIC.to_le_bytes());
    bytes
}

fn record(bytes: &mut Vec<u8>, kind: u32, payload: &[u8]) {
    bytes.extend(kind.to_le_bytes());
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
    while !bytes.len().is_multiple_of(4) { bytes.push(0); }
}

fn section(bytes: &mut Vec<u8>, kind: u32, offset: u32, data: &[u8]) {
    let mut payload = offset.to_le_bytes().to_vec();
    payload.extend(data);
    record(bytes, kind, &payload);
}

fn runtime_image(paging: bool) -> Vec<u8> {
    let mut bytes = image();
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x1000, &[1]);
    section(&mut bytes, INTEL_TLV_SEC_RT, INTEL_CPU_SEPARATOR, &[]);
    section(&mut bytes, INTEL_TLV_SECURE_SEC_RT, 0x2000, &[2]);
    if paging {
        section(&mut bytes, INTEL_TLV_SEC_RT, INTEL_PAGING_SEPARATOR, &[]);
        section(&mut bytes, INTEL_TLV_SEC_RT, 0x3000, &[3]);
        record(&mut bytes, INTEL_TLV_PAGING, &4096u32.to_le_bytes());
    }
    bytes
}

#[test]
fn canonical_parser_rejects_empty_images_and_bad_separator_state() {
    assert_eq!(IntelTlvFirmware::parse(&image()).err(), Some(IntelTlvError::NoSections));

    let mut bytes = image();
    section(&mut bytes, INTEL_TLV_SEC_RT, INTEL_CPU_SEPARATOR, &[]);
    assert_eq!(IntelTlvFirmware::parse(&bytes).err(), Some(IntelTlvError::InvalidSeparator));

    let mut bytes = image();
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x1000, &[1]);
    section(&mut bytes, INTEL_TLV_SEC_RT, INTEL_PAGING_SEPARATOR, &[]);
    assert_eq!(IntelTlvFirmware::parse(&bytes).err(), Some(IntelTlvError::InvalidSeparator));
}

#[test]
fn canonical_parser_groups_runtime_payloads_without_exposing_separators() {
    let bytes = runtime_image(true);
    let fw = IntelTlvFirmware::parse(&bytes).unwrap();
    assert_eq!(fw.section_count(), 3);
    assert_eq!(fw.sections().count(), 3);
    assert_eq!(fw.section_group(0), Some(IntelFirmwareSectionGroup::Lmac));
    assert_eq!(fw.section_group(1), Some(IntelFirmwareSectionGroup::Umac));
    assert_eq!(fw.section_group(2), Some(IntelFirmwareSectionGroup::Paging));
    assert!(fw.sections().all(|section| !section.is_separator()));
    assert_eq!(fw.paging_size(), Some(4096));
}

#[test]
fn ax200_layout_routes_canonical_groups() {
    let bytes = runtime_image(true);
    let fw = IntelTlvFirmware::parse(&bytes).unwrap();
    let plan = Ax200RuntimeImage::validate(&fw).unwrap();
    assert_eq!(plan.counts(), [1, 1, 1]);
    assert_eq!(
        plan.payloads().collect::<Vec<_>>(),
        vec![
            (Ax200ImageRegion::Lmac, &[1][..]),
            (Ax200ImageRegion::Umac, &[2][..]),
            (Ax200ImageRegion::Paging, &[3][..]),
        ]
    );
}

#[test]
fn ax200_layout_requires_complete_runtime_group_progression() {
    let mut bytes = image();
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x1000, &[1]);
    let fw = IntelTlvFirmware::parse(&bytes).unwrap();
    assert!(matches!(Ax200RuntimeImage::validate(&fw), Err(Ax200ImageError::MissingSeparator)));

    let bytes = runtime_image(false);
    let fw = IntelTlvFirmware::parse(&bytes).unwrap();
    assert!(matches!(Ax200RuntimeImage::validate(&fw), Err(Ax200ImageError::MissingSeparator)));
}

#[test]
fn ax200_layout_enforces_chunk_bound_before_dma() {
    let mut bytes = image();
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x1000, &[1]);
    section(&mut bytes, INTEL_TLV_SEC_RT, INTEL_CPU_SEPARATOR, &[]);
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x2000, &[2]);
    section(&mut bytes, INTEL_TLV_SEC_RT, INTEL_PAGING_SEPARATOR, &[]);
    section(
        &mut bytes,
        INTEL_TLV_SEC_RT,
        0x3000,
        &vec![0; AX200_MAX_IMAGE_CHUNK + 1],
    );
    record(&mut bytes, INTEL_TLV_PAGING, &4096u32.to_le_bytes());
    let fw = IntelTlvFirmware::parse(&bytes).unwrap();
    assert!(matches!(
        Ax200RuntimeImage::validate(&fw),
        Err(Ax200ImageError::SectionTooLarge)
    ));
}

#[test]
fn canonical_parser_enforces_bounded_section_table() {
    let mut bytes = image();
    for index in 0..=MAX_INTEL_TLV_SECTIONS {
        section(&mut bytes, INTEL_TLV_SEC_RT, 0x1000 + index as u32 * 0x100, &[1]);
    }
    assert_eq!(
        IntelTlvFirmware::parse(&bytes).err(),
        Some(IntelTlvError::TooManySections)
    );
}

#[test]
fn paging_metadata_requires_paging_payload_for_ax200() {
    let mut bytes = image();
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x1000, &[1]);
    section(&mut bytes, INTEL_TLV_SEC_RT, INTEL_CPU_SEPARATOR, &[]);
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x2000, &[2]);
    section(&mut bytes, INTEL_TLV_SEC_RT, INTEL_PAGING_SEPARATOR, &[]);
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x3000, &[3]);
    let fw = IntelTlvFirmware::parse(&bytes).unwrap();
    assert!(matches!(
        Ax200RuntimeImage::validate(&fw),
        Err(Ax200ImageError::PagingMetadata)
    ));
}

#[test]
fn header_unknown_records_and_bounds_remain_fail_closed() {
    let mut bytes = image();
    bytes[72..76].copy_from_slice(&77u32.to_le_bytes());
    bytes[76..80].copy_from_slice(&3u32.to_le_bytes());
    record(&mut bytes, 0xff00, &[1, 2, 3]);
    section(&mut bytes, INTEL_TLV_SEC_RT, 0x1000, &[1]);
    let fw = IntelTlvFirmware::parse(&bytes).unwrap();
    assert_eq!((fw.version(), fw.build()), (77, 3));
    assert_eq!(fw.bytes(), bytes);

    let mut oversized = image();
    oversized.resize(MAX_FIRMWARE_IMAGE_SIZE + 1, 0);
    assert_eq!(IntelTlvFirmware::parse(&oversized).err(), Some(IntelTlvError::ImageTooLarge));
}
