//! Stage 13.18: USB mass storage protocol — interface discovery, Bulk-Only
//! wrappers and SCSI command blocks — checked against real descriptors and
//! the specification's layouts.

#[path = "../kernel/src/usb_msc.rs"]
#[allow(dead_code)]
mod usb_msc;

use usb_msc::{Interface, Status, StatusError};

/// QEMU's usb-storage (hw/usb/dev-storage.c), high speed: configuration,
/// one Bulk-Only SCSI interface, bulk IN 0x81 and bulk OUT 0x02 of 512 bytes.
const QEMU_STORAGE: &[u8] = &[
    0x09, 0x02, 0x20, 0x00, 0x01, 0x01, 0x00, 0xc0, 0x00, // configuration
    0x09, 0x04, 0x00, 0x00, 0x02, 0x08, 0x06, 0x50, 0x00, // interface: 08/06/50
    0x07, 0x05, 0x81, 0x02, 0x00, 0x02, 0x00, // bulk IN 0x81, 512
    0x07, 0x05, 0x02, 0x02, 0x00, 0x02, 0x00, // bulk OUT 0x02, 512
];

#[test]
fn finds_the_bulk_only_interface_and_its_endpoints() {
    assert_eq!(
        usb_msc::parse_interface(QEMU_STORAGE, 1),
        Some(Interface {
            configuration: 1,
            interface: 0,
            bulk_in: 0x81,
            bulk_in_max_packet: 512,
            bulk_out: 0x02,
            bulk_out_max_packet: 512,
        })
    );
}

#[test]
fn skips_other_interfaces_and_needs_both_bulk_endpoints() {
    // A HID interface first, then the disk with its endpoints in the other
    // order and an interrupt endpoint that must be ignored.
    let config: &[u8] = &[
        0x09, 0x02, 0x3b, 0x00, 0x02, 0x01, 0x00, 0x80, 0x32, //
        0x09, 0x04, 0x00, 0x00, 0x01, 0x03, 0x01, 0x01, 0x00, // HID keyboard
        0x07, 0x05, 0x81, 0x03, 0x08, 0x00, 0x0a, //
        0x09, 0x04, 0x01, 0x00, 0x03, 0x08, 0x06, 0x50, 0x00, // disk
        0x07, 0x05, 0x83, 0x03, 0x08, 0x00, 0x0a, // interrupt: ignored
        0x07, 0x05, 0x04, 0x02, 0x40, 0x00, 0x00, // bulk OUT 0x04, 64
        0x07, 0x05, 0x85, 0x02, 0x40, 0x00, 0x00, // bulk IN 0x85, 64
    ];
    let found = usb_msc::parse_interface(config, 1).unwrap();
    assert_eq!((found.interface, found.bulk_in, found.bulk_out), (1, 0x85, 0x04));
    assert_eq!((found.bulk_in_max_packet, found.bulk_out_max_packet), (64, 64));

    // Only an IN endpoint: not usable.
    let mut half = QEMU_STORAGE.to_vec();
    half.truncate(QEMU_STORAGE.len() - 7);
    assert_eq!(usb_msc::parse_interface(&half, 1), None);
    // UAS or another protocol: not this driver's.
    let mut uas = QEMU_STORAGE.to_vec();
    uas[16] = 0x62;
    assert_eq!(usb_msc::parse_interface(&uas, 1), None);
}

#[test]
fn malformed_descriptors_end_the_walk() {
    let mut zero = QEMU_STORAGE.to_vec();
    zero[18] = 0; // an endpoint descriptor of length 0
    assert_eq!(usb_msc::parse_interface(&zero, 1), None);
    let mut overrun = QEMU_STORAGE.to_vec();
    overrun[25] = 0x40; // runs past the end
    assert_eq!(usb_msc::parse_interface(&overrun, 1), None);
}

#[test]
fn a_command_block_wrapper_has_the_specified_layout() {
    let block = usb_msc::command_block(0x1234_5678, 512, true, &usb_msc::read_write(false, 0x0102_0304, 1)).unwrap();
    assert_eq!(&block[0..4], b"USBC");
    assert_eq!(&block[4..8], &0x1234_5678u32.to_le_bytes());
    assert_eq!(&block[8..12], &512u32.to_le_bytes());
    assert_eq!(block[12], 0x80, "data in");
    assert_eq!(block[13], 0, "LUN 0");
    assert_eq!(block[14], 10, "command length");
    assert_eq!(&block[15..25], &[0x28, 0, 1, 2, 3, 4, 0, 0, 1, 0]);
    assert!(block[25..].iter().all(|&byte| byte == 0));

    let out = usb_msc::command_block(7, 512, false, &usb_msc::read_write(true, 9, 1)).unwrap();
    assert_eq!((out[12], out[15]), (0, 0x2a));
    assert_eq!(usb_msc::command_block(1, 0, false, &[]), None);
    assert_eq!(usb_msc::command_block(1, 0, false, &[0; 17]), None);
}

fn status(tag: u32, residue: u32, code: u8) -> [u8; 13] {
    let mut bytes = [0u8; 13];
    bytes[0..4].copy_from_slice(b"USBS");
    bytes[4..8].copy_from_slice(&tag.to_le_bytes());
    bytes[8..12].copy_from_slice(&residue.to_le_bytes());
    bytes[12] = code;
    bytes
}

#[test]
fn a_command_status_wrapper_is_validated() {
    assert_eq!(usb_msc::parse_status(&status(5, 0, 0), 5, 512), Ok(Status::Passed { residue: 0 }));
    assert_eq!(usb_msc::parse_status(&status(5, 12, 0), 5, 36), Ok(Status::Passed { residue: 12 }));
    assert_eq!(usb_msc::parse_status(&status(5, 0, 1), 5, 512), Ok(Status::Failed));
    assert_eq!(usb_msc::parse_status(&status(5, 0, 2), 5, 512), Ok(Status::PhaseError));
    assert_eq!(usb_msc::parse_status(&status(6, 0, 0), 5, 512), Err(StatusError::Tag));
    assert_eq!(usb_msc::parse_status(&status(5, 0, 3), 5, 512), Err(StatusError::Unknown));
    assert_eq!(usb_msc::parse_status(&status(5, 513, 0), 5, 512), Err(StatusError::Residue));
    let mut wrong = status(5, 0, 0);
    wrong[0] = b'X';
    assert_eq!(usb_msc::parse_status(&wrong, 5, 512), Err(StatusError::Signature));
    assert_eq!(usb_msc::parse_status(&status(5, 0, 0)[..12], 5, 512), Err(StatusError::Length));
}

#[test]
fn capacity_and_sense_data_are_decoded() {
    // An 8 GB stick: last LBA 15728639, 512-byte blocks.
    let data = [0x00, 0xef, 0xff, 0xff, 0x00, 0x00, 0x02, 0x00];
    assert_eq!(usb_msc::parse_capacity(&data), Some((15_728_640, 512)));
    assert_eq!(usb_msc::parse_capacity(&[0xff, 0xff, 0xff, 0xff, 0, 0, 2, 0]), None, "needs READ CAPACITY (16)");
    assert_eq!(usb_msc::parse_capacity(&[0, 0, 0, 1, 0, 0, 0, 0]), None, "no block length");
    assert_eq!(usb_msc::parse_capacity(&data[..7]), None);

    let mut sense = [0u8; 18];
    sense[0] = 0x70;
    sense[2] = 0x06; // UNIT ATTENTION
    sense[12] = 0x28; // medium may have changed
    assert_eq!(usb_msc::parse_sense(&sense), Some((6, 0x28, 0)));
    sense[0] = 0x72; // descriptor format: not decoded
    assert_eq!(usb_msc::parse_sense(&sense), None);
}

#[test]
fn scsi_command_blocks() {
    assert_eq!(usb_msc::test_unit_ready(), [0; 6]);
    assert_eq!(usb_msc::inquiry(36), [0x12, 0, 0, 0, 36, 0]);
    assert_eq!(usb_msc::request_sense(18), [0x03, 0, 0, 0, 18, 0]);
    assert_eq!(usb_msc::read_capacity()[0], 0x25);
    assert_eq!(usb_msc::synchronize_cache()[0], 0x35);
    assert_eq!(usb_msc::read_write(true, 0xa0b0c0d0, 0x0102), [0x2a, 0, 0xa0, 0xb0, 0xc0, 0xd0, 0, 1, 2, 0]);
}
