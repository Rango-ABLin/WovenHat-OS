//! Stage 13.16: HID report-descriptor parsing and report decoding, checked
//! against real descriptors.

#[path = "../kernel/src/hid.rs"]
#[allow(dead_code)]
mod hid;

use hid::{Application, Input, MAX_EVENTS};

fn decode(decoder: &mut hid::Decoder, report: &[u8]) -> Vec<Input> {
    let mut out = [None; MAX_EVENTS];
    let count = decoder.decode(report, &mut out);
    out[..count].iter().map(|event| event.unwrap()).collect()
}

/// QEMU's usb-tablet (hw/usb/dev-hid.c): three buttons, absolute 15-bit X
/// and Y, relative wheel.
const QEMU_TABLET: &[u8] = &[
    0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29, 0x03,
    0x15, 0x00, 0x25, 0x01, 0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01, 0x75, 0x05, 0x81, 0x01,
    0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x15, 0x00, 0x26, 0xff, 0x7f, 0x35, 0x00, 0x46, 0xff, 0x7f,
    0x75, 0x10, 0x95, 0x02, 0x81, 0x02, 0x05, 0x01, 0x09, 0x38, 0x15, 0x81, 0x25, 0x7f, 0x35, 0x00,
    0x45, 0x00, 0x75, 0x08, 0x95, 0x01, 0x81, 0x06, 0xc0, 0xc0,
];

#[test]
fn tablet_reports_absolute_position_wheel_and_button_edges() {
    let layout = hid::parse(QEMU_TABLET).expect("tablet descriptor");
    assert_eq!(layout.application, Application::Mouse);
    assert!(!layout.report_ids);
    let mut decoder = hid::Decoder::new(layout);
    // Left button, x = 0x4000, y = 0x7fff, wheel +1.
    assert_eq!(
        decode(&mut decoder, &[0x01, 0x00, 0x40, 0xff, 0x7f, 0x01]),
        vec![
            Input::Absolute {
                x: 0x4000,
                y: 0x7fff
            },
            Input::Wheel(1),
            Input::Button {
                index: 0,
                pressed: true
            },
        ]
    );
    // Same buttons, new position, wheel back: no button events.
    assert_eq!(
        decode(&mut decoder, &[0x01, 0x10, 0x00, 0x20, 0x00, 0x00]),
        vec![Input::Absolute { x: 0x10, y: 0x20 }]
    );
    // Release, wheel -1 (0xff sign-extends).
    assert_eq!(
        decode(&mut decoder, &[0x00, 0x10, 0x00, 0x20, 0x00, 0xff]),
        vec![
            Input::Absolute { x: 0x10, y: 0x20 },
            Input::Wheel(-1),
            Input::Button {
                index: 0,
                pressed: false
            }
        ]
    );
}

/// A common 14-button game pad: hat switch (null state), X/Y/Z/Rz bytes.
const GAME_PAD: &[u8] = &[
    0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x15, 0x00, 0x25, 0x01, 0x35, 0x00, 0x45, 0x01, 0x75, 0x01,
    0x95, 0x0e, 0x05, 0x09, 0x19, 0x01, 0x29, 0x0e, 0x81, 0x02, 0x95, 0x02, 0x81, 0x01, 0x05, 0x01,
    0x25, 0x07, 0x46, 0x3b, 0x01, 0x75, 0x04, 0x95, 0x01, 0x65, 0x14, 0x09, 0x39, 0x81, 0x42, 0x65,
    0x00, 0x95, 0x01, 0x81, 0x01, 0x26, 0xff, 0x00, 0x46, 0xff, 0x00, 0x09, 0x30, 0x09, 0x31, 0x09,
    0x32, 0x09, 0x35, 0x75, 0x08, 0x95, 0x04, 0x81, 0x02, 0xc0,
];

#[test]
fn game_pad_reports_axis_hat_and_button_changes_only() {
    let layout = hid::parse(GAME_PAD).expect("game pad descriptor");
    assert_eq!(layout.application, Application::GamePad);
    let mut decoder = hid::Decoder::new(layout);
    // Buttons 1 and 3, hat centred (15, outside 0..=7), X=128 Y=255 Z=0 Rz=128.
    assert_eq!(
        decode(&mut decoder, &[0x05, 0x00, 0x0f, 0x80, 0xff, 0x00, 0x80]),
        vec![
            Input::Control {
                control: hid::HAT,
                value: -1
            },
            Input::Control {
                control: hid::AXIS_BASE,
                value: 128
            },
            Input::Control {
                control: hid::AXIS_BASE + 1,
                value: 32_767
            },
            Input::Control {
                control: hid::AXIS_BASE + 2,
                value: -32_768
            },
            Input::Control {
                control: hid::AXIS_BASE + 5,
                value: 128
            },
            Input::Control {
                control: 1,
                value: 1
            },
            Input::Control {
                control: 3,
                value: 1
            },
        ]
    );
    // Button 1 released, hat up (0), X fully left; the rest unchanged.
    assert_eq!(
        decode(&mut decoder, &[0x04, 0x00, 0x00, 0x00, 0xff, 0x00, 0x80]),
        vec![
            Input::Control {
                control: hid::HAT,
                value: 0
            },
            Input::Control {
                control: hid::AXIS_BASE,
                value: -32_768
            },
            Input::Control {
                control: 1,
                value: 0
            },
        ]
    );
    // An identical report changes nothing.
    assert!(decode(&mut decoder, &[0x04, 0x00, 0x00, 0x00, 0xff, 0x00, 0x80]).is_empty());
}

/// A digitizer pen (report ID 2): tip, barrel, invert, eraser, in range,
/// 15-bit X/Y, 10-bit pressure.
const PEN: &[u8] = &[
    0x05, 0x0d, 0x09, 0x02, 0xa1, 0x01, 0x85, 0x02, 0x09, 0x20, 0xa1, 0x00, 0x09, 0x42, 0x09, 0x44,
    0x09, 0x3c, 0x09, 0x45, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x04, 0x81, 0x02, 0x95, 0x01,
    0x81, 0x03, 0x09, 0x32, 0x81, 0x02, 0x95, 0x02, 0x81, 0x03, 0x05, 0x01, 0x09, 0x30, 0x26, 0xff,
    0x7f, 0x75, 0x10, 0x95, 0x01, 0x81, 0x02, 0x09, 0x31, 0x81, 0x02, 0x05, 0x0d, 0x09, 0x30, 0x26,
    0xff, 0x03, 0x81, 0x02, 0xc0, 0xc0,
];

#[test]
fn pen_reports_position_pressure_and_contact() {
    let layout = hid::parse(PEN).expect("pen descriptor");
    assert_eq!(layout.application, Application::Pen);
    assert!(layout.report_ids);
    let mut decoder = hid::Decoder::new(layout);
    // Tip down, in range; x=0x4000 y=0x2000 pressure=0x1ff.
    assert_eq!(
        decode(
            &mut decoder,
            &[0x02, 0x21, 0x00, 0x40, 0x00, 0x20, 0xff, 0x01]
        ),
        vec![Input::Pen {
            x: 0x4000,
            y: 0x2000,
            pressure: 0x1ff,
            touching: true
        }]
    );
    // Hovering: in range, tip up.
    assert_eq!(
        decode(
            &mut decoder,
            &[0x02, 0x20, 0x00, 0x40, 0x00, 0x20, 0x00, 0x00]
        ),
        vec![Input::Pen {
            x: 0x4000,
            y: 0x2000,
            pressure: 0,
            touching: false
        }]
    );
    // Out of range: nothing.
    assert!(
        decode(
            &mut decoder,
            &[0x02, 0x00, 0x00, 0x40, 0x00, 0x20, 0x00, 0x00]
        )
        .is_empty()
    );
    // Another report ID is not this device's pen report.
    assert!(
        decode(
            &mut decoder,
            &[0x03, 0x21, 0x00, 0x40, 0x00, 0x20, 0xff, 0x01]
        )
        .is_empty()
    );
}

/// A two-contact touch screen (report ID 1): per contact, tip switch,
/// contact ID, 12-bit X/Y.
const TOUCH_SCREEN: &[u8] = &[
    0x05, 0x0d, 0x09, 0x04, 0xa1, 0x01, 0x85, 0x01, // touch screen, report 1
    0x09, 0x22, 0xa1, 0x02, // finger
    0x09, 0x42, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, // tip
    0x75, 0x07, 0x81, 0x03, // pad
    0x09, 0x51, 0x25, 0x0f, 0x75, 0x08, 0x81, 0x02, // contact id
    0x05, 0x01, 0x26, 0xff, 0x0f, 0x75, 0x10, 0x09, 0x30, 0x81, 0x02, 0x09, 0x31, 0x81,
    0x02, // x, y
    0xc0, //
    0x05, 0x0d, 0x09, 0x22, 0xa1, 0x02, // second finger
    0x09, 0x42, 0x25, 0x01, 0x75, 0x01, 0x81, 0x02, 0x75, 0x07, 0x81, 0x03, //
    0x09, 0x51, 0x25, 0x0f, 0x75, 0x08, 0x81, 0x02, //
    0x05, 0x01, 0x26, 0xff, 0x0f, 0x75, 0x10, 0x09, 0x30, 0x81, 0x02, 0x09, 0x31, 0x81,
    0x02, //
    0xc0, 0xc0,
];

#[test]
fn touch_screen_reports_each_contact_and_its_lift() {
    let layout = hid::parse(TOUCH_SCREEN).expect("touch descriptor");
    assert_eq!(layout.application, Application::TouchScreen);
    let mut decoder = hid::Decoder::new(layout);
    let scaled = |value: u16| (u32::from(value) * 32_767 / 4_095) as u16;
    // Contact 3 down at (4095, 0); contact 5 down at (2048, 1024).
    let report = [
        0x01, 0x01, 0x03, 0xff, 0x0f, 0x00, 0x00, 0x01, 0x05, 0x00, 0x08, 0x00, 0x04,
    ];
    assert_eq!(
        decode(&mut decoder, &report),
        vec![
            Input::Touch {
                contact: 3,
                x: 32_767,
                y: 0,
                active: true
            },
            Input::Touch {
                contact: 5,
                x: scaled(2048),
                y: scaled(1024),
                active: true
            },
        ]
    );
    // Contact 3 lifts; contact 5 stays.
    let report = [
        0x01, 0x00, 0x03, 0xff, 0x0f, 0x00, 0x00, 0x01, 0x05, 0x00, 0x08, 0x00, 0x04,
    ];
    assert_eq!(
        decode(&mut decoder, &report),
        vec![
            Input::Touch {
                contact: 3,
                x: 32_767,
                y: 0,
                active: false
            },
            Input::Touch {
                contact: 5,
                x: scaled(2048),
                y: scaled(1024),
                active: true
            },
        ]
    );
    // A lifted contact is reported once, not again.
    let report = [
        0x01, 0x00, 0x03, 0xff, 0x0f, 0x00, 0x00, 0x00, 0x05, 0x00, 0x08, 0x00, 0x04,
    ];
    assert_eq!(
        decode(&mut decoder, &report),
        vec![Input::Touch {
            contact: 5,
            x: scaled(2048),
            y: scaled(1024),
            active: false
        }]
    );
}

#[test]
fn malformed_and_unsupported_descriptors_are_refused() {
    // Truncated item.
    assert_eq!(hid::parse(&[0x05]).unwrap_err(), hid::Error::Malformed);
    // Unbalanced collections.
    assert_eq!(
        hid::parse(&[0x05, 0x01, 0x09, 0x02, 0xa1, 0x01]).unwrap_err(),
        hid::Error::Malformed
    );
    assert_eq!(hid::parse(&[0xc0]).unwrap_err(), hid::Error::Malformed);
    // A keyboard-only application is not decoded here.
    assert_eq!(
        hid::parse(&[0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0xc0]).unwrap_err(),
        hid::Error::Unsupported
    );
    // Report size 0 is malformed.
    assert_eq!(
        hid::parse(&[
            0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x75, 0x00, 0x95, 0x01, 0x81, 0x02, 0xc0
        ])
        .unwrap_err(),
        hid::Error::Malformed
    );
    // A report shorter than its fields decodes what it can and never panics.
    let mut decoder = hid::Decoder::new(hid::parse(QEMU_TABLET).unwrap());
    let _ = decode(&mut decoder, &[0x01]);
    let _ = decode(&mut decoder, &[]);
}
