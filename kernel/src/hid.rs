//! HID report descriptors (Stage 13.16): which fields a device's reports
//! carry, and decoding reports into input events.
//!
//! Boot-protocol keyboards and mice have fixed reports and are handled in
//! `xhci.rs`. Every other HID device describes its reports in a report
//! descriptor; this module parses one into a bounded [`Layout`] and turns
//! reports into [`Input`] events for:
//! - mice and pointers, relative or absolute (tablets);
//! - pens and other digitizers;
//! - touch screens and touchpads;
//! - joysticks and game pads.
//!
//! The module is pure, with no allocation and no kernel state, so host tests
//! check it against real descriptors.

/// Fields kept per layout; a descriptor with more input fields is refused.
pub const MAX_FIELDS: usize = 48;
/// Events one report can produce.
pub const MAX_EVENTS: usize = 24;
/// Absolute positions are scaled to 0..=ABSOLUTE_MAX.
pub const ABSOLUTE_MAX: u16 = 32_767;
/// Pen pressure is scaled to 0..=PRESSURE_MAX.
pub const PRESSURE_MAX: u16 = 1_023;
/// Game-controller axes are reported as `AXIS_BASE + axis` (X, Y, Z, Rx,
/// Ry, Rz); buttons as their 1-based number; the hat switch as `HAT`.
pub const AXIS_BASE: u16 = 0x100;
pub const HAT: u16 = 0x200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A short or malformed item.
    Malformed,
    /// More input fields, nested collections or pushes than are kept.
    TooLarge,
    /// No top-level application collection this module decodes.
    Unsupported,
}

/// What a report field means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Usage {
    /// Button number, 1-based.
    Button(u8),
    X,
    Y,
    Z,
    Rx,
    Ry,
    Rz,
    Wheel,
    HatSwitch,
    TipSwitch,
    InRange,
    TipPressure,
    ContactId,
    Other,
}

/// The top-level application collection: what kind of device this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Application {
    Mouse,
    Pointer,
    Joystick,
    GamePad,
    Pen,
    TouchScreen,
    TouchPad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub report_id: u8,
    pub usage: Usage,
    /// Bit position within the report, after any report-ID byte.
    pub bit_offset: u16,
    pub bit_size: u8,
    pub logical_min: i32,
    pub logical_max: i32,
    pub relative: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub application: Application,
    /// Whether reports start with a report-ID byte.
    pub report_ids: bool,
    pub fields: [Option<Field>; MAX_FIELDS],
}

/// One decoded input event. Positions are scaled to 0..=ABSOLUTE_MAX.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Relative {
        dx: i16,
        dy: i16,
    },
    Absolute {
        x: u16,
        y: u16,
    },
    Wheel(i16),
    /// Button edges, 0-based (0 left, 1 right, 2 middle for pointers).
    Button {
        index: u8,
        pressed: bool,
    },
    Pen {
        x: u16,
        y: u16,
        pressure: u16,
        touching: bool,
    },
    Touch {
        contact: u8,
        x: u16,
        y: u16,
        active: bool,
    },
    /// A game-controller button (`control` = number), axis or hat change.
    Control {
        control: u16,
        value: i16,
    },
}

#[derive(Clone, Copy, Default)]
struct Contact {
    id: Option<u8>,
    tip: Option<bool>,
    x: Option<u16>,
    y: Option<u16>,
}

#[derive(Clone, Copy, Default)]
struct Globals {
    usage_page: u16,
    logical_min: i32,
    logical_max: i32,
    report_size: u32,
    report_count: u32,
    report_id: u8,
}

const MAX_USAGES: usize = 16;
const MAX_STACK: usize = 4;
const MAX_REPORT_IDS: usize = 16;

/// Parse a report descriptor's input fields and application collection.
pub fn parse(descriptor: &[u8]) -> Result<Layout, Error> {
    let mut globals = Globals::default();
    let mut stack = [Globals::default(); MAX_STACK];
    let mut depth_stack = 0usize;
    let mut usages = [0u32; MAX_USAGES];
    let mut usage_count = 0usize;
    let mut usage_range: Option<(u32, u32)> = None;
    let mut collection_depth = 0usize;
    let mut application = None;
    let mut fields = [None; MAX_FIELDS];
    let mut field_count = 0usize;
    // Next free bit for each report ID (0 when the device uses none).
    let mut offsets = [(0u8, 0u32); MAX_REPORT_IDS];
    let mut offset_count = 0usize;
    let mut report_ids = false;

    let mut index = 0usize;
    while index < descriptor.len() {
        let prefix = descriptor[index];
        if prefix == 0xfe {
            // Long item: skip it.
            let size = usize::from(*descriptor.get(index + 1).ok_or(Error::Malformed)?);
            index += 3 + size;
            continue;
        }
        let size = match prefix & 0x03 {
            3 => 4,
            other => usize::from(other),
        };
        let data = descriptor
            .get(index + 1..index + 1 + size)
            .ok_or(Error::Malformed)?;
        let unsigned = data
            .iter()
            .rev()
            .fold(0u32, |value, byte| (value << 8) | u32::from(*byte));
        let signed = match size {
            1 => i32::from(data[0] as i8),
            2 => i32::from(i16::from_le_bytes([data[0], data[1]])),
            4 => unsigned as i32,
            _ => 0,
        };
        let kind = (prefix >> 2) & 0x03;
        let tag = prefix >> 4;
        match (kind, tag) {
            // Main items.
            (0, 0x8) => {
                // Input.
                let constant = unsigned & 0x01 != 0;
                let variable = unsigned & 0x02 != 0;
                let relative = unsigned & 0x04 != 0;
                let bits = globals.report_size;
                if bits == 0 || bits > 32 {
                    return Err(Error::Malformed);
                }
                let slot = match offsets[..offset_count]
                    .iter()
                    .position(|(id, _)| *id == globals.report_id)
                {
                    Some(slot) => slot,
                    None => {
                        if offset_count == MAX_REPORT_IDS {
                            return Err(Error::TooLarge);
                        }
                        offsets[offset_count] = (globals.report_id, 0);
                        offset_count += 1;
                        offset_count - 1
                    }
                };
                for item in 0..globals.report_count {
                    let bit_offset = offsets[slot].1;
                    offsets[slot].1 += bits;
                    // Constant fields are padding; arrays (keyboard-style
                    // key lists) are not decoded here.
                    if constant || !variable {
                        continue;
                    }
                    let usage = match usage_range {
                        Some((first, last)) => first.saturating_add(item).min(last),
                        None if usage_count > 0 => usages[(item as usize).min(usage_count - 1)],
                        None => continue,
                    };
                    let usage = classify(usage, globals.usage_page);
                    if usage == Usage::Other {
                        continue;
                    }
                    if field_count == MAX_FIELDS {
                        return Err(Error::TooLarge);
                    }
                    if bit_offset + bits > 0xffff {
                        return Err(Error::TooLarge);
                    }
                    fields[field_count] = Some(Field {
                        report_id: globals.report_id,
                        usage,
                        bit_offset: bit_offset as u16,
                        bit_size: bits as u8,
                        logical_min: globals.logical_min,
                        logical_max: globals.logical_max,
                        relative,
                    });
                    field_count += 1;
                }
                usage_count = 0;
                usage_range = None;
            }
            (0, 0x9) | (0, 0xb) => {
                // Output and Feature: no input fields; locals are consumed.
                usage_count = 0;
                usage_range = None;
            }
            (0, 0xa) => {
                // Collection; type 1 is Application.
                if collection_depth == 0 && unsigned == 1 && application.is_none() {
                    let usage = usages.first().copied().filter(|_| usage_count > 0);
                    application = usage.and_then(|usage| application_of(usage, globals.usage_page));
                }
                collection_depth += 1;
                usage_count = 0;
                usage_range = None;
            }
            (0, 0xc) => {
                collection_depth = collection_depth.checked_sub(1).ok_or(Error::Malformed)?;
            }
            // Global items.
            (1, 0x0) => globals.usage_page = unsigned as u16,
            (1, 0x1) => globals.logical_min = signed,
            (1, 0x2) => {
                // A maximum is signed only when the minimum is negative.
                globals.logical_max = if globals.logical_min < 0 {
                    signed
                } else {
                    unsigned as i32
                };
            }
            (1, 0x7) => globals.report_size = unsigned,
            (1, 0x8) => {
                if unsigned == 0 || unsigned > 0xff {
                    return Err(Error::Malformed);
                }
                globals.report_id = unsigned as u8;
                report_ids = true;
            }
            (1, 0x9) => globals.report_count = unsigned,
            (1, 0xa) => {
                if depth_stack == MAX_STACK {
                    return Err(Error::TooLarge);
                }
                stack[depth_stack] = globals;
                depth_stack += 1;
            }
            (1, 0xb) => {
                depth_stack = depth_stack.checked_sub(1).ok_or(Error::Malformed)?;
                globals = stack[depth_stack];
            }
            // Local items. A 4-byte usage carries its own page.
            (2, 0x0) => {
                let usage = if size == 4 {
                    unsigned
                } else {
                    (u32::from(globals.usage_page) << 16) | unsigned
                };
                if usage_count < MAX_USAGES {
                    usages[usage_count] = usage;
                    usage_count += 1;
                }
            }
            (2, 0x1) => {
                let usage = if size == 4 {
                    unsigned
                } else {
                    (u32::from(globals.usage_page) << 16) | unsigned
                };
                usage_range = Some((usage, usage_range.map_or(usage, |(_, last)| last)));
            }
            (2, 0x2) => {
                let usage = if size == 4 {
                    unsigned
                } else {
                    (u32::from(globals.usage_page) << 16) | unsigned
                };
                usage_range = Some((usage_range.map_or(usage, |(first, _)| first), usage));
            }
            _ => {}
        }
        index += 1 + size;
    }
    if collection_depth != 0 {
        return Err(Error::Malformed);
    }
    Ok(Layout {
        application: application.ok_or(Error::Unsupported)?,
        report_ids,
        fields,
    })
}

/// A usage (page << 16 | id) as a field meaning.
fn classify(usage: u32, _page: u16) -> Usage {
    match (usage >> 16, usage & 0xffff) {
        (0x01, 0x30) => Usage::X,
        (0x01, 0x31) => Usage::Y,
        (0x01, 0x32) => Usage::Z,
        (0x01, 0x33) => Usage::Rx,
        (0x01, 0x34) => Usage::Ry,
        (0x01, 0x35) => Usage::Rz,
        (0x01, 0x38) => Usage::Wheel,
        (0x01, 0x39) => Usage::HatSwitch,
        (0x09, id @ 1..=0xff) => Usage::Button(id as u8),
        (0x0d, 0x42) => Usage::TipSwitch,
        (0x0d, 0x32) => Usage::InRange,
        (0x0d, 0x30) => Usage::TipPressure,
        (0x0d, 0x51) => Usage::ContactId,
        _ => Usage::Other,
    }
}

fn application_of(usage: u32, _page: u16) -> Option<Application> {
    Some(match (usage >> 16, usage & 0xffff) {
        (0x01, 0x01) => Application::Pointer,
        (0x01, 0x02) => Application::Mouse,
        (0x01, 0x04) => Application::Joystick,
        (0x01, 0x05) => Application::GamePad,
        (0x0d, 0x01) | (0x0d, 0x02) => Application::Pen,
        (0x0d, 0x04) => Application::TouchScreen,
        (0x0d, 0x05) => Application::TouchPad,
        _ => return None,
    })
}

/// A field's value in `report` (without its report-ID byte), sign-extended
/// when the field's logical range is signed.
pub fn field_value(field: &Field, report: &[u8]) -> Option<i32> {
    let start = usize::from(field.bit_offset);
    let bits = usize::from(field.bit_size);
    if start + bits > report.len() * 8 {
        return None;
    }
    let mut raw = 0u64;
    for bit in 0..bits {
        let position = start + bit;
        if report[position / 8] >> (position % 8) & 1 != 0 {
            raw |= 1 << bit;
        }
    }
    Some(
        if field.logical_min < 0 && bits < 64 && raw >> (bits - 1) & 1 != 0 {
            (raw as i64 - (1i64 << bits)) as i32
        } else {
            raw as i32
        },
    )
}

/// `value` within the field's logical range, scaled to 0..=max.
fn scale(field: &Field, value: i32, max: u16) -> u16 {
    let span = i64::from(field.logical_max) - i64::from(field.logical_min);
    if span <= 0 {
        return 0;
    }
    let clamped =
        i64::from(value.clamp(field.logical_min, field.logical_max)) - i64::from(field.logical_min);
    (clamped * i64::from(max) / span) as u16
}

/// Turns one device's reports into events, remembering what it needs to
/// report only changes (buttons, controller axes, touch contacts).
#[derive(Clone, Copy)]
pub struct Decoder {
    layout: Layout,
    buttons: u32,
    /// Last value of each controller axis and the hat; None until seen.
    controls: [Option<i16>; 8],
    contacts: u16,
}

impl Decoder {
    pub const fn new(layout: Layout) -> Self {
        Self {
            layout,
            buttons: 0,
            controls: [None; 8],
            contacts: 0,
        }
    }

    /// Decode one report (including its report-ID byte, if the device uses
    /// IDs) into `out`; returns how many events were written.
    pub fn decode(&mut self, report: &[u8], out: &mut [Option<Input>; MAX_EVENTS]) -> usize {
        let (id, body) = if self.layout.report_ids {
            match report.split_first() {
                Some((id, body)) => (*id, body),
                None => return 0,
            }
        } else {
            (0, report)
        };
        let fields = self.layout.fields;
        let fields = fields
            .iter()
            .flatten()
            .filter(|field| field.report_id == id);
        match self.layout.application {
            Application::Mouse | Application::Pointer => self.pointer(fields, body, out),
            Application::Pen => self.pen(fields, body, out),
            Application::TouchScreen | Application::TouchPad => self.touch(fields, body, out),
            Application::Joystick | Application::GamePad => self.controller(fields, body, out),
        }
    }

    fn push(out: &mut [Option<Input>; MAX_EVENTS], count: &mut usize, input: Input) {
        if *count < MAX_EVENTS {
            out[*count] = Some(input);
            *count += 1;
        }
    }

    fn pointer<'a>(
        &mut self,
        fields: impl Iterator<Item = &'a Field>,
        body: &[u8],
        out: &mut [Option<Input>; MAX_EVENTS],
    ) -> usize {
        let mut count = 0;
        let (mut dx, mut dy, mut x, mut y, mut wheel) = (0i32, 0i32, None, None, 0i32);
        let mut buttons = self.buttons;
        for field in fields {
            let Some(value) = field_value(field, body) else {
                continue;
            };
            match field.usage {
                Usage::X if field.relative => dx = value,
                Usage::Y if field.relative => dy = value,
                Usage::X => x = Some(scale(field, value, ABSOLUTE_MAX)),
                Usage::Y => y = Some(scale(field, value, ABSOLUTE_MAX)),
                Usage::Wheel => wheel = value,
                Usage::Button(number @ 1..=32) => {
                    let mask = 1u32 << (number - 1);
                    buttons = if value != 0 {
                        buttons | mask
                    } else {
                        buttons & !mask
                    };
                }
                _ => {}
            }
        }
        if dx != 0 || dy != 0 {
            let clamp = |value: i32| value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            Self::push(
                out,
                &mut count,
                Input::Relative {
                    dx: clamp(dx),
                    dy: clamp(dy),
                },
            );
        }
        if let (Some(x), Some(y)) = (x, y) {
            Self::push(out, &mut count, Input::Absolute { x, y });
        }
        if wheel != 0 {
            Self::push(out, &mut count, Input::Wheel(wheel.clamp(-127, 127) as i16));
        }
        self.button_edges(buttons, out, &mut count);
        count
    }

    fn button_edges(
        &mut self,
        buttons: u32,
        out: &mut [Option<Input>; MAX_EVENTS],
        count: &mut usize,
    ) {
        let changed = buttons ^ self.buttons;
        for index in 0..32u8 {
            if changed & (1 << index) != 0 {
                Self::push(
                    out,
                    count,
                    Input::Button {
                        index,
                        pressed: buttons & (1 << index) != 0,
                    },
                );
            }
        }
        self.buttons = buttons;
    }

    fn pen<'a>(
        &mut self,
        fields: impl Iterator<Item = &'a Field>,
        body: &[u8],
        out: &mut [Option<Input>; MAX_EVENTS],
    ) -> usize {
        let mut count = 0;
        let (mut x, mut y, mut pressure, mut touching, mut in_range) =
            (None, None, 0u16, false, true);
        for field in fields {
            let Some(value) = field_value(field, body) else {
                continue;
            };
            match field.usage {
                Usage::X => x = Some(scale(field, value, ABSOLUTE_MAX)),
                Usage::Y => y = Some(scale(field, value, ABSOLUTE_MAX)),
                Usage::TipPressure => pressure = scale(field, value, PRESSURE_MAX),
                Usage::TipSwitch => touching = value != 0,
                Usage::InRange => in_range = value != 0,
                _ => {}
            }
        }
        if let (Some(x), Some(y), true) = (x, y, in_range) {
            Self::push(
                out,
                &mut count,
                Input::Pen {
                    x,
                    y,
                    pressure,
                    touching,
                },
            );
        }
        count
    }

    fn touch<'a>(
        &mut self,
        fields: impl Iterator<Item = &'a Field>,
        body: &[u8],
        out: &mut [Option<Input>; MAX_EVENTS],
    ) -> usize {
        // A multi-touch report repeats each contact's fields. A field the
        // current contact already has starts the next contact; a Contact ID
        // names the current contact if it has none yet.
        let mut count = 0;
        let mut current = Contact::default();
        let mut seen = 0u16;
        let mut contacts = [Contact::default(); 16];
        let mut finished = 0usize;
        for field in fields {
            let Some(value) = field_value(field, body) else {
                continue;
            };
            let starts_next = match field.usage {
                Usage::ContactId => current.id.is_some(),
                Usage::TipSwitch => current.tip.is_some(),
                Usage::X => current.x.is_some(),
                Usage::Y => current.y.is_some(),
                _ => continue,
            };
            if starts_next {
                if finished < contacts.len() {
                    contacts[finished] = current;
                    finished += 1;
                }
                current = Contact::default();
            }
            match field.usage {
                Usage::ContactId => current.id = Some(value as u8),
                Usage::TipSwitch => current.tip = Some(value != 0),
                Usage::X => current.x = Some(scale(field, value, ABSOLUTE_MAX)),
                Usage::Y => current.y = Some(scale(field, value, ABSOLUTE_MAX)),
                _ => {}
            }
        }
        if finished < contacts.len() {
            contacts[finished] = current;
            finished += 1;
        }
        for contact in &contacts[..finished] {
            let (Some(x), Some(y)) = (contact.x, contact.y) else {
                continue;
            };
            let id = contact.id.unwrap_or(0);
            let active = contact.tip.unwrap_or(true);
            let bit = 1u16 << (id % 16);
            // Report active contacts, and each contact's lift once.
            if active || self.contacts & bit != 0 {
                Self::push(
                    out,
                    &mut count,
                    Input::Touch {
                        contact: id,
                        x,
                        y,
                        active,
                    },
                );
            }
            if active {
                seen |= bit;
            }
        }
        self.contacts = seen;
        count
    }

    fn controller<'a>(
        &mut self,
        fields: impl Iterator<Item = &'a Field>,
        body: &[u8],
        out: &mut [Option<Input>; MAX_EVENTS],
    ) -> usize {
        let mut count = 0;
        let mut buttons = self.buttons;
        for field in fields {
            let Some(value) = field_value(field, body) else {
                continue;
            };
            let axis = match field.usage {
                Usage::X => Some(0),
                Usage::Y => Some(1),
                Usage::Z => Some(2),
                Usage::Rx => Some(3),
                Usage::Ry => Some(4),
                Usage::Rz => Some(5),
                Usage::HatSwitch => Some(6),
                _ => None,
            };
            if let Some(axis) = axis {
                let scaled = if axis == 6 {
                    // Hat: its position (0-7), or -1 when centred (out of range).
                    if (field.logical_min..=field.logical_max).contains(&value) {
                        (value - field.logical_min) as i16
                    } else {
                        -1
                    }
                } else {
                    (i32::from(scale(field, value, u16::MAX)) - 32_768) as i16
                };
                if self.controls[axis] != Some(scaled) {
                    self.controls[axis] = Some(scaled);
                    let control = if axis == 6 {
                        HAT
                    } else {
                        AXIS_BASE + axis as u16
                    };
                    Self::push(
                        out,
                        &mut count,
                        Input::Control {
                            control,
                            value: scaled,
                        },
                    );
                }
            } else if let Usage::Button(number @ 1..=32) = field.usage {
                let mask = 1u32 << (number - 1);
                buttons = if value != 0 {
                    buttons | mask
                } else {
                    buttons & !mask
                };
            }
        }
        let changed = buttons ^ self.buttons;
        for index in 0..32u16 {
            if changed & (1 << index) != 0 {
                let value = i16::from(buttons & (1 << index) != 0);
                Self::push(
                    out,
                    &mut count,
                    Input::Control {
                        control: index + 1,
                        value,
                    },
                );
            }
        }
        self.buttons = buttons;
        count
    }
}
