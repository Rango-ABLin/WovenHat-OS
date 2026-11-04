//! Bounded accessibility primitives shared by WovenHat applications.
//!
//! The module deliberately owns policy-free data and navigation only. A
//! compositor or desktop service can project the tree into its own surface,
//! while screen readers, captions, and input adapters consume the same stable
//! node identifiers. All storage is fixed-capacity so accessibility cannot
//! fail by allocating unbounded memory during a UI update.

pub const MAX_NODES: usize = 128;
pub const MAX_LABEL_BYTES: usize = 96;
pub const MAX_CAPTIONS: usize = 8;
pub const SNAPSHOT_HEADER_BYTES: usize = 10;
pub const SNAPSHOT_NODE_BYTES: usize = 2 + 2 + 1 + 1 + 4 + 4 + 4 + 4 + 1 + MAX_LABEL_BYTES;
pub const MAX_SNAPSHOT_BYTES: usize = SNAPSHOT_HEADER_BYTES + MAX_NODES * SNAPSHOT_NODE_BYTES;
pub const ANNOUNCEMENT_BYTES: usize = 4 + 2 + 1 + 1 + MAX_LABEL_BYTES;

const SNAPSHOT_MAGIC: &[u8; 4] = b"WAX1";
const ANNOUNCEMENT_MAGIC: &[u8; 4] = b"WAA1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Role {
    Window,
    Panel,
    Button,
    CheckBox,
    Radio,
    Slider,
    TextField,
    List,
    ListItem,
    Heading,
    StaticText,
    Image,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    id: NodeId,
    parent: Option<NodeId>,
    role: Role,
    bounds: Rect,
    label: [u8; MAX_LABEL_BYTES],
    label_len: u8,
    focusable: bool,
    enabled: bool,
}

impl Node {
    pub const fn id(self) -> NodeId {
        self.id
    }

    pub const fn parent(self) -> Option<NodeId> {
        self.parent
    }

    pub const fn role(self) -> Role {
        self.role
    }

    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    pub const fn is_focusable(self) -> bool {
        self.focusable
    }

    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    pub fn label(&self) -> &str {
        // Node labels are copied from validated UTF-8 input and live in the
        // tree's fixed storage. The returned view is intended for immediate
        // announcement/painting and does not outlive the tree in practice.
        // Use the bounded bytes accessor when a borrow tied to the tree is
        // required.
        core::str::from_utf8(&self.label[..self.label_len as usize]).unwrap_or("")
    }

    pub fn label_bytes(self) -> ([u8; MAX_LABEL_BYTES], usize) {
        (self.label, self.label_len as usize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeError {
    Capacity,
    InvalidParent,
    InvalidLabel,
    LabelTooLong,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    OutputTooSmall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tree {
    nodes: [Option<Node>; MAX_NODES],
    count: usize,
    focus: Option<NodeId>,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    pub const fn new() -> Self {
        Self {
            nodes: [None; MAX_NODES],
            count: 0,
            focus: None,
        }
    }

    pub const fn len(self) -> usize {
        self.count
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    pub const fn focused(self) -> Option<NodeId> {
        self.focus
    }

    pub fn node(&self, id: NodeId) -> Option<Node> {
        self.nodes.get(id.0 as usize).copied().flatten()
    }

    pub fn insert(
        &mut self,
        parent: Option<NodeId>,
        role: Role,
        bounds: Rect,
        label: &str,
        focusable: bool,
        enabled: bool,
    ) -> Result<NodeId, TreeError> {
        if label.is_empty() || label.len() > MAX_LABEL_BYTES {
            return Err(if label.len() > MAX_LABEL_BYTES {
                TreeError::LabelTooLong
            } else {
                TreeError::InvalidLabel
            });
        }
        if let Some(parent) = parent
            && self.node(parent).is_none()
        {
            return Err(TreeError::InvalidParent);
        }
        let Some(slot) = self.nodes.iter().position(Option::is_none) else {
            return Err(TreeError::Capacity);
        };
        let id = NodeId(slot as u16);
        let mut bytes = [0_u8; MAX_LABEL_BYTES];
        bytes[..label.len()].copy_from_slice(label.as_bytes());
        let node = Node {
            id,
            parent,
            role,
            bounds,
            label: bytes,
            label_len: label.len() as u8,
            focusable,
            enabled,
        };
        self.nodes[slot] = Some(node);
        self.count += 1;
        if self.focus.is_none() && focusable && enabled {
            self.focus = Some(id);
        }
        Ok(id)
    }

    pub fn set_focus(&mut self, id: NodeId) -> bool {
        if self
            .node(id)
            .is_some_and(|node| node.focusable && node.enabled)
        {
            self.focus = Some(id);
            true
        } else {
            false
        }
    }

    pub fn focus_next(&mut self) -> Option<NodeId> {
        self.focus_relative(1)
    }

    pub fn focus_previous(&mut self) -> Option<NodeId> {
        self.focus_relative(-1)
    }

    /// Encode the current semantic tree into a versioned, pointer-free wire
    /// record for a screen-reader or accessibility service. Every node uses a
    /// fixed-size record, so the required capacity is deterministic and no
    /// partial record is published when the caller's buffer is too small.
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, EncodeError> {
        if output.len() < SNAPSHOT_HEADER_BYTES {
            return Err(EncodeError::OutputTooSmall);
        }
        let required = SNAPSHOT_HEADER_BYTES
            .checked_add(self.count.saturating_mul(SNAPSHOT_NODE_BYTES))
            .ok_or(EncodeError::OutputTooSmall)?;
        if output.len() < required {
            return Err(EncodeError::OutputTooSmall);
        }
        output[..required].fill(0);
        output[..4].copy_from_slice(SNAPSHOT_MAGIC);
        output[4] = 1;
        output[6..8].copy_from_slice(&(self.count as u16).to_le_bytes());
        output[8..10].copy_from_slice(&self.focus.map_or(u16::MAX, |id| id.0).to_le_bytes());
        let mut cursor = SNAPSHOT_HEADER_BYTES;
        for node in self.nodes.iter().flatten() {
            output[cursor..cursor + 2].copy_from_slice(&node.id.0.to_le_bytes());
            output[cursor + 2..cursor + 4]
                .copy_from_slice(&node.parent.map_or(u16::MAX, |id| id.0).to_le_bytes());
            output[cursor + 4] = node.role as u8;
            output[cursor + 5] = u8::from(node.focusable) | (u8::from(node.enabled) << 1);
            output[cursor + 6..cursor + 10].copy_from_slice(&node.bounds.x.to_le_bytes());
            output[cursor + 10..cursor + 14].copy_from_slice(&node.bounds.y.to_le_bytes());
            output[cursor + 14..cursor + 18].copy_from_slice(&node.bounds.width.to_le_bytes());
            output[cursor + 18..cursor + 22].copy_from_slice(&node.bounds.height.to_le_bytes());
            output[cursor + 22] = node.label_len;
            output[cursor + 23..cursor + 23 + MAX_LABEL_BYTES].copy_from_slice(&node.label);
            cursor += SNAPSHOT_NODE_BYTES;
        }
        Ok(required)
    }

    fn focus_relative(&mut self, direction: isize) -> Option<NodeId> {
        if self.count == 0 {
            return None;
        }
        let start = self.focus.map_or(0, |id| id.0 as usize);
        for step in 1..=MAX_NODES {
            let candidate = if direction > 0 {
                (start + step) % MAX_NODES
            } else {
                (start + MAX_NODES - (step % MAX_NODES)) % MAX_NODES
            };
            let id = NodeId(candidate as u16);
            if self
                .node(id)
                .is_some_and(|node| node.focusable && node.enabled)
            {
                self.focus = Some(id);
                return Some(id);
            }
        }
        self.focus
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    ShiftTab,
    Up,
    Down,
    Left,
    Right,
}

pub fn navigate(tree: &mut Tree, key: Key) -> Option<NodeId> {
    match key {
        Key::Tab | Key::Down | Key::Right => tree.focus_next(),
        Key::ShiftTab | Key::Up | Key::Left => tree.focus_previous(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Announcement {
    pub node: NodeId,
    pub role: Role,
    pub label: [u8; MAX_LABEL_BYTES],
    pub label_len: u8,
}

impl Announcement {
    pub fn text(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len as usize]).unwrap_or("")
    }

    /// Encode one focus announcement for a screen-reader service without
    /// borrowing or exposing the tree's storage.
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, EncodeError> {
        if output.len() < ANNOUNCEMENT_BYTES {
            return Err(EncodeError::OutputTooSmall);
        }
        output[..ANNOUNCEMENT_BYTES].fill(0);
        output[..4].copy_from_slice(ANNOUNCEMENT_MAGIC);
        output[4..6].copy_from_slice(&self.node.0.to_le_bytes());
        output[6] = self.role as u8;
        output[7] = self.label_len;
        output[8..8 + MAX_LABEL_BYTES].copy_from_slice(&self.label);
        Ok(ANNOUNCEMENT_BYTES)
    }
}

pub fn announce_focus(tree: &Tree) -> Option<Announcement> {
    let node = tree.focused().and_then(|id| tree.node(id))?;
    let (label, length) = node.label_bytes();
    Some(Announcement {
        node: node.id,
        role: node.role,
        label,
        label_len: length as u8,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caption {
    pub speaker: [u8; MAX_LABEL_BYTES],
    pub speaker_len: u8,
    pub text: [u8; MAX_LABEL_BYTES],
    pub text_len: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionError {
    Full,
    Empty,
    TooLong,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptionBuffer {
    entries: [Option<Caption>; MAX_CAPTIONS],
    next: usize,
    count: usize,
}

impl Default for CaptionBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptionBuffer {
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_CAPTIONS],
            next: 0,
            count: 0,
        }
    }

    pub const fn len(self) -> usize {
        self.count
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    pub fn push(&mut self, speaker: &str, text: &str) -> Result<(), CaptionError> {
        if speaker.is_empty() || text.is_empty() {
            return Err(CaptionError::Empty);
        }
        if speaker.len() > MAX_LABEL_BYTES || text.len() > MAX_LABEL_BYTES {
            return Err(CaptionError::TooLong);
        }
        if self.count == MAX_CAPTIONS {
            return Err(CaptionError::Full);
        }
        let mut speaker_bytes = [0_u8; MAX_LABEL_BYTES];
        let mut text_bytes = [0_u8; MAX_LABEL_BYTES];
        speaker_bytes[..speaker.len()].copy_from_slice(speaker.as_bytes());
        text_bytes[..text.len()].copy_from_slice(text.as_bytes());
        self.entries[self.next] = Some(Caption {
            speaker: speaker_bytes,
            speaker_len: speaker.len() as u8,
            text: text_bytes,
            text_len: text.len() as u8,
        });
        self.next = (self.next + 1) % MAX_CAPTIONS;
        self.count += 1;
        Ok(())
    }

    pub fn get(&self, index: usize) -> Option<Caption> {
        if index >= self.count {
            return None;
        }
        let first = if self.count == MAX_CAPTIONS {
            self.next
        } else {
            0
        };
        self.entries[(first + index) % MAX_CAPTIONS]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub screen_reader: bool,
    pub captions: bool,
    pub high_contrast: bool,
    pub reduced_motion: bool,
    pub scale: u8,
    pub magnification: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    pub const fn new() -> Self {
        Self {
            screen_reader: false,
            captions: false,
            high_contrast: false,
            reduced_motion: false,
            scale: 1,
            magnification: 1,
        }
    }

    pub fn set_scale(&mut self, scale: u8) {
        self.scale = scale.clamp(1, 4);
    }

    pub fn set_magnification(&mut self, magnification: u8) {
        self.magnification = magnification.clamp(1, 4);
    }

    /// Apply the bounded boolean and small-integer fields understood by the
    /// persisted accessibility profile. Unknown, malformed, or oversized
    /// fields are ignored so a partially authored profile cannot broaden
    /// accessibility behavior accidentally.
    pub fn apply_json(&mut self, bytes: &[u8]) {
        if let Some(value) = json_bool(bytes, b"screen_reader") {
            self.screen_reader = value;
        }
        if let Some(value) = json_bool(bytes, b"captions") {
            self.captions = value;
        }
        if let Some(value) = json_bool(bytes, b"high_contrast") {
            self.high_contrast = value;
        }
        if let Some(value) = json_bool(bytes, b"reduced_motion") {
            self.reduced_motion = value;
        }
        if let Some(value) = json_u8(bytes, b"scale") {
            self.set_scale(value);
        }
        if let Some(value) = json_u8(bytes, b"magnification") {
            self.set_magnification(value);
        }
    }

    pub const fn transition_duration(self, base_milliseconds: u32) -> u32 {
        if self.reduced_motion {
            0
        } else {
            base_milliseconds
        }
    }
}

fn json_bool(bytes: &[u8], key: &[u8]) -> Option<bool> {
    let quoted = key.len().checked_add(2)?;
    for offset in 0..bytes.len().saturating_sub(quoted).saturating_add(1) {
        if bytes.get(offset) != Some(&b'"')
            || bytes.get(offset + 1..offset + 1 + key.len()) != Some(key)
            || bytes.get(offset + 1 + key.len()) != Some(&b'"')
        {
            continue;
        }
        let rest = bytes.get(offset + quoted..)?.trim_ascii_start();
        if rest.first() != Some(&b':') {
            continue;
        }
        let value = rest.get(1..)?.trim_ascii_start();
        let token = |prefix: &[u8]| {
            value.strip_prefix(prefix).is_some_and(|rest| {
                rest.first()
                    .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
            })
        };
        return if token(b"true") {
            Some(true)
        } else if token(b"false") {
            Some(false)
        } else {
            None
        };
    }
    None
}

fn json_u8(bytes: &[u8], key: &[u8]) -> Option<u8> {
    let quoted = key.len().checked_add(2)?;
    for offset in 0..bytes.len().saturating_sub(quoted).saturating_add(1) {
        if bytes.get(offset) != Some(&b'"')
            || bytes.get(offset + 1..offset + 1 + key.len()) != Some(key)
            || bytes.get(offset + 1 + key.len()) != Some(&b'"')
        {
            continue;
        }
        let rest = bytes.get(offset + quoted..)?.trim_ascii_start();
        if rest.first() != Some(&b':') {
            continue;
        }
        let digits = rest.get(1..)?.trim_ascii_start();
        let first = *digits.first()?;
        if !first.is_ascii_digit() {
            return None;
        }
        let mut value = first - b'0';
        for byte in digits.get(1..)?.iter().copied() {
            if !byte.is_ascii_digit() {
                break;
            }
            value = value.checked_mul(10)?.checked_add(byte - b'0')?;
        }
        return Some(value);
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub foreground: u32,
    pub background: u32,
    pub accent: u32,
}

impl Settings {
    pub const fn palette(self) -> Palette {
        if self.high_contrast {
            Palette {
                foreground: 0xffff_ffff,
                background: 0,
                accent: 0xffff_ff00,
            }
        } else {
            Palette {
                foreground: 0xffe8_e8f0,
                background: 0xff18_1a24,
                accent: 0xff66_99ff,
            }
        }
    }

    pub const fn magnify_point(self, x: i32, y: i32) -> (i32, i32) {
        let factor = self.magnification as i32;
        (x.saturating_mul(factor), y.saturating_mul(factor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_assigns_focus_and_navigates_deterministically() {
        let mut tree = Tree::new();
        let root = tree
            .insert(
                None,
                Role::Window,
                Rect::new(0, 0, 100, 100),
                "Main",
                false,
                true,
            )
            .unwrap();
        let first = tree
            .insert(
                Some(root),
                Role::Button,
                Rect::new(1, 1, 20, 10),
                "Open",
                true,
                true,
            )
            .unwrap();
        let second = tree
            .insert(
                Some(root),
                Role::Button,
                Rect::new(1, 20, 20, 10),
                "Close",
                true,
                true,
            )
            .unwrap();
        assert_eq!(tree.focused(), Some(first));
        assert_eq!(navigate(&mut tree, Key::Tab), Some(second));
        assert_eq!(navigate(&mut tree, Key::ShiftTab), Some(first));
        assert_eq!(announce_focus(&tree).unwrap().text(), "Open");
    }

    #[test]
    fn tree_rejects_bad_parent_and_bounds_label_storage() {
        let mut tree = Tree::new();
        assert_eq!(
            tree.insert(
                Some(NodeId(9)),
                Role::Button,
                Rect::default(),
                "x",
                true,
                true
            ),
            Err(TreeError::InvalidParent)
        );
        let long = "x".repeat(MAX_LABEL_BYTES + 1);
        assert_eq!(
            tree.insert(None, Role::StaticText, Rect::default(), &long, false, true),
            Err(TreeError::LabelTooLong)
        );
    }

    #[test]
    fn disabled_nodes_are_skipped() {
        let mut tree = Tree::new();
        let disabled = tree
            .insert(None, Role::Button, Rect::default(), "Disabled", true, false)
            .unwrap();
        let active = tree
            .insert(None, Role::Button, Rect::default(), "Active", true, true)
            .unwrap();
        assert_eq!(tree.focused(), Some(active));
        assert!(!tree.set_focus(disabled));
    }

    #[test]
    fn semantic_snapshot_is_versioned_bounded_and_pointer_free() {
        let mut tree = Tree::new();
        let root = tree
            .insert(
                None,
                Role::Window,
                Rect::new(-4, 8, 100, 40),
                "Main",
                false,
                true,
            )
            .unwrap();
        let button = tree
            .insert(
                Some(root),
                Role::Button,
                Rect::new(2, 3, 20, 10),
                "Open",
                true,
                true,
            )
            .unwrap();
        assert_eq!(tree.focused(), Some(button));
        let mut encoded = [0u8; MAX_SNAPSHOT_BYTES];
        let length = tree.encode(&mut encoded).unwrap();
        assert_eq!(length, SNAPSHOT_HEADER_BYTES + 2 * SNAPSHOT_NODE_BYTES);
        assert_eq!(&encoded[..4], b"WAX1");
        assert_eq!(encoded[4], 1);
        assert_eq!(u16::from_le_bytes([encoded[6], encoded[7]]), 2);
        assert_eq!(u16::from_le_bytes([encoded[8], encoded[9]]), button.0);
        let label_offset = SNAPSHOT_HEADER_BYTES + 23;
        assert_eq!(&encoded[label_offset..label_offset + 4], b"Main");
        assert_eq!(
            tree.encode(&mut encoded[..SNAPSHOT_HEADER_BYTES + SNAPSHOT_NODE_BYTES]),
            Err(EncodeError::OutputTooSmall)
        );
    }

    #[test]
    fn focus_announcement_has_stable_wire_shape() {
        let mut tree = Tree::new();
        tree.insert(None, Role::Button, Rect::default(), "Open", true, true)
            .unwrap();
        let announcement = announce_focus(&tree).unwrap();
        let mut encoded = [0u8; ANNOUNCEMENT_BYTES];
        assert_eq!(announcement.encode(&mut encoded).unwrap(), ANNOUNCEMENT_BYTES);
        assert_eq!(&encoded[..4], b"WAA1");
        assert_eq!(encoded[6], Role::Button as u8);
        assert_eq!(encoded[7], 4);
        assert_eq!(&encoded[8..12], b"Open");
    }

    #[test]
    fn captions_are_bounded_and_fifo() {
        let mut captions = CaptionBuffer::new();
        captions.push("A", "one").unwrap();
        captions.push("B", "two").unwrap();
        assert_eq!(captions.get(0).unwrap().text_len, 3);
        assert_eq!(&captions.get(1).unwrap().text[..3], b"two");
        assert_eq!(captions.len(), 2);
    }

    #[test]
    fn settings_are_bounded_and_accessible() {
        let mut settings = Settings::new();
        settings.set_scale(9);
        settings.set_magnification(0);
        assert_eq!(settings.scale, 4);
        assert_eq!(settings.magnification, 1);
        assert_eq!(settings.transition_duration(250), 250);
        settings.reduced_motion = true;
        settings.high_contrast = true;
        assert_eq!(settings.transition_duration(250), 0);
        assert_eq!(settings.palette().background, 0);
        assert_eq!(settings.magnify_point(2, -3), (2, -3));
        settings.set_magnification(3);
        assert_eq!(settings.magnify_point(2, -3), (6, -9));
    }

    #[test]
    fn settings_apply_bounded_json_profile() {
        let mut settings = Settings::new();
        settings.apply_json(
            br#"{"high_contrast": true, "reduced_motion":true, "scale": 9, "magnification": 3}"#,
        );
        assert!(settings.high_contrast);
        assert!(settings.reduced_motion);
        assert_eq!(settings.scale, 4);
        assert_eq!(settings.magnification, 3);
    }

    #[test]
    fn settings_ignore_malformed_or_unquoted_profile_fields() {
        let mut settings = Settings::new();
        settings.apply_json(br#"{high_contrast:true,"scale":9}"#);
        assert!(!settings.high_contrast);
        assert_eq!(settings.scale, 4);
        settings.apply_json(br#"{"high_contrast": false, "reduced_motion": nope}"#);
        assert!(!settings.high_contrast);
        assert!(!settings.reduced_motion);
    }
}
