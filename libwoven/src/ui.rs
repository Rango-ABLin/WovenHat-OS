//! Small, allocation-free WovenUI primitives for native applications.
//!
//! Widgets own only bounded state and draw through the caller-owned [`Canvas`].
//! They do not acquire input or framebuffer ownership themselves; applications
//! route events to the focused widget and decide when a damaged region is
//! presented.

use crate::{Canvas, InputEvent, Key, layout::Frame};

/// Shared colors for the stock WovenUI theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub surface: u32,
    pub accent: u32,
    pub text: u32,
    pub disabled: u32,
    pub focus: u32,
}

impl Theme {
    pub const DARK: Self = Self {
        surface: crate::rgb(45, 65, 90),
        accent: crate::rgb(100, 220, 180),
        text: crate::rgb(235, 240, 250),
        disabled: crate::rgb(76, 84, 100),
        focus: crate::rgb(255, 205, 96),
    };
}

/// A bounded push button. The caller supplies its label and routes pointer
/// events; a completed click is reported on release.
pub struct Button<'a> {
    pub frame: Frame,
    pub label: &'a str,
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub enabled: bool,
}

impl<'a> Button<'a> {
    pub const fn new(frame: Frame, label: &'a str) -> Self {
        Self {
            frame,
            label,
            hovered: false,
            pressed: false,
            focused: false,
            enabled: true,
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.frame.x
            && y >= self.frame.y
            && x < self.frame.x.saturating_add(self.frame.width.max(0))
            && y < self.frame.y.saturating_add(self.frame.height.max(0))
    }

    /// Route a pointer event. Returns `true` exactly once for an accepted
    /// release inside the button.
    pub fn pointer(&mut self, x: i32, y: i32, pressed: bool) -> bool {
        if !self.enabled {
            self.hovered = false;
            self.pressed = false;
            return false;
        }
        self.hovered = self.contains(x, y);
        if pressed {
            self.pressed = self.hovered;
            false
        } else {
            let clicked = self.pressed && self.hovered;
            self.pressed = false;
            clicked
        }
    }

    /// Activate the button from a keyboard or accessibility action.
    pub fn activate(&mut self) -> bool {
        self.enabled
    }

    pub fn draw(&self, canvas: &mut Canvas<'_>, theme: Theme) {
        let fill = if !self.enabled {
            theme.disabled
        } else if self.pressed || self.hovered {
            theme.accent
        } else {
            theme.surface
        };
        canvas.fill_rect(
            self.frame.x,
            self.frame.y,
            self.frame.width.max(0) as usize,
            self.frame.height.max(0) as usize,
            fill,
        );
        if self.focused {
            canvas.fill_rect(
                self.frame.x,
                self.frame.y,
                self.frame.width.max(0) as usize,
                1,
                theme.focus,
            );
            canvas.fill_rect(
                self.frame.x,
                self.frame
                    .y
                    .saturating_add(self.frame.height.saturating_sub(1)),
                self.frame.width.max(0) as usize,
                1,
                theme.focus,
            );
        }
        let text_width = crate::font::text_width(self.label) as i32;
        let text_x = self.frame.x + ((self.frame.width - text_width) / 2).max(0);
        let text_y = self.frame.y + ((self.frame.height - crate::font::HEIGHT as i32) / 2).max(0);
        canvas.draw_text(text_x, text_y, self.label, theme.text);
    }
}

/// A non-interactive text label with an optional focus indicator.
pub struct Label<'a> {
    pub frame: Frame,
    pub text: &'a str,
    pub focused: bool,
}

impl<'a> Label<'a> {
    pub const fn new(frame: Frame, text: &'a str) -> Self {
        Self {
            frame,
            text,
            focused: false,
        }
    }

    pub fn draw(&self, canvas: &mut Canvas<'_>, theme: Theme) {
        if self.focused {
            canvas.fill_rect(
                self.frame.x,
                self.frame.y + self.frame.height.saturating_sub(1),
                self.frame.width.max(0) as usize,
                1,
                theme.focus,
            );
        }
        canvas.draw_text(self.frame.x, self.frame.y, self.text, theme.text);
    }
}

/// A fixed-size checkbox with bounded pointer interaction.
pub struct Checkbox<'a> {
    pub frame: Frame,
    pub label: &'a str,
    pub checked: bool,
    pub focused: bool,
    pub enabled: bool,
}

impl<'a> Checkbox<'a> {
    pub const BOX_SIZE: i32 = 10;

    pub const fn new(frame: Frame, label: &'a str) -> Self {
        Self {
            frame,
            label,
            checked: false,
            focused: false,
            enabled: true,
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.frame.x
            && y >= self.frame.y
            && x < self.frame.x.saturating_add(self.frame.width.max(0))
            && y < self.frame.y.saturating_add(self.frame.height.max(0))
    }

    pub fn activate(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        self.checked = !self.checked;
        true
    }

    pub fn pointer(&mut self, x: i32, y: i32, pressed: bool) -> bool {
        if pressed || !self.contains(x, y) {
            return false;
        }
        self.activate()
    }

    pub fn draw(&self, canvas: &mut Canvas<'_>, theme: Theme) {
        let border = if self.enabled {
            theme.text
        } else {
            theme.disabled
        };
        canvas.fill_rect(
            self.frame.x,
            self.frame.y,
            Self::BOX_SIZE.min(self.frame.width.max(0)) as usize,
            Self::BOX_SIZE.min(self.frame.height.max(0)) as usize,
            border,
        );
        if self.checked {
            canvas.fill_rect(
                self.frame.x + 2,
                self.frame.y + 2,
                Self::BOX_SIZE.saturating_sub(4) as usize,
                Self::BOX_SIZE.saturating_sub(4) as usize,
                theme.accent,
            );
        }
        if self.focused {
            canvas.fill_rect(
                self.frame.x,
                self.frame.y + Self::BOX_SIZE + 1,
                Self::BOX_SIZE as usize,
                1,
                theme.focus,
            );
        }
        canvas.draw_text(
            self.frame.x + Self::BOX_SIZE + 5,
            self.frame.y,
            self.label,
            theme.text,
        );
    }
}

/// A fixed-capacity UTF-8 text field. Input is deliberately event-based so an
/// application can apply its own permission, clipboard, and submission policy.
pub struct TextField<const N: usize> {
    bytes: [u8; N],
    len: usize,
    pub frame: Frame,
    pub focused: bool,
}

impl<const N: usize> TextField<N> {
    pub const fn new(frame: Frame) -> Self {
        Self {
            bytes: [0; N],
            len: 0,
            frame,
            focused: false,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }

    /// Give focus on a press inside the bounded field frame.
    pub fn pointer(&mut self, x: i32, y: i32, pressed: bool) -> bool {
        if !pressed {
            return false;
        }
        let inside = x >= self.frame.x
            && y >= self.frame.y
            && x < self.frame.x.saturating_add(self.frame.width.max(0))
            && y < self.frame.y.saturating_add(self.frame.height.max(0));
        let changed = self.focused != inside;
        self.focused = inside;
        changed
    }

    pub fn handle(&mut self, event: InputEvent) -> bool {
        if !self.focused {
            return false;
        }
        match event {
            InputEvent::Key(Key::Backspace) => {
                if self.len > 0 {
                    self.len -= 1;
                    while self.len > 0 && (self.bytes[self.len] & 0b1100_0000) == 0b1000_0000 {
                        self.len -= 1;
                    }
                    true
                } else {
                    false
                }
            }
            InputEvent::Key(Key::Char(ch)) => {
                let mut encoded = [0; 4];
                let text = ch.encode_utf8(&mut encoded);
                if self.len.saturating_add(text.len()) > N {
                    return false;
                }
                self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
                self.len += text.len();
                true
            }
            _ => false,
        }
    }

    pub fn draw(&self, canvas: &mut Canvas<'_>, theme: Theme) {
        canvas.fill_rect(
            self.frame.x,
            self.frame.y,
            self.frame.width.max(0) as usize,
            self.frame.height.max(0) as usize,
            theme.surface,
        );
        if self.focused {
            canvas.fill_rect(
                self.frame.x,
                self.frame.y,
                self.frame.width.max(0) as usize,
                1,
                theme.focus,
            );
        }
        canvas.draw_text(
            self.frame.x + 5,
            self.frame.y + 5,
            self.as_str(),
            theme.text,
        );
    }
}

/// A fixed-capacity selectable list. Items are borrowed by the caller and no
/// allocation occurs when the list is populated or navigated.
pub struct ListBox<'a, const N: usize> {
    items: [Option<&'a str>; N],
    len: usize,
    pub frame: Frame,
    pub selected: usize,
    pub focused: bool,
    pub enabled: bool,
}

impl<'a, const N: usize> ListBox<'a, N> {
    pub const fn new(frame: Frame) -> Self {
        Self {
            items: [None; N],
            len: 0,
            frame,
            selected: 0,
            focused: false,
            enabled: true,
        }
    }

    pub fn push(&mut self, item: &'a str) -> bool {
        if self.len == N {
            return false;
        }
        self.items[self.len] = Some(item);
        self.len += 1;
        true
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn selected_item(&self) -> Option<&'a str> {
        self.items.get(self.selected).copied().flatten()
    }

    pub fn move_selection(&mut self, delta: i32) -> bool {
        if !self.enabled || self.len == 0 {
            return false;
        }
        let next = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.selected.saturating_add(delta as usize)
        }
        .min(self.len - 1);
        let changed = next != self.selected;
        self.selected = next;
        changed
    }

    pub fn pointer(&mut self, x: i32, y: i32, pressed: bool) -> bool {
        if !self.enabled || pressed || self.len == 0 {
            return false;
        }
        if x < self.frame.x
            || y < self.frame.y
            || x >= self.frame.x.saturating_add(self.frame.width.max(0))
            || y >= self.frame.y.saturating_add(self.frame.height.max(0))
        {
            return false;
        }
        let row_height = crate::font::HEIGHT as i32 + 4;
        let row = ((y - self.frame.y) / row_height) as usize;
        if row >= self.len {
            return false;
        }
        let changed = row != self.selected;
        self.selected = row;
        changed
    }

    pub fn draw(&self, canvas: &mut Canvas<'_>, theme: Theme) {
        canvas.fill_rect(
            self.frame.x,
            self.frame.y,
            self.frame.width.max(0) as usize,
            self.frame.height.max(0) as usize,
            theme.surface,
        );
        let row_height = crate::font::HEIGHT as i32 + 4;
        for index in 0..self.len {
            let y = self.frame.y + index as i32 * row_height;
            if y >= self.frame.y.saturating_add(self.frame.height.max(0)) {
                break;
            }
            if index == self.selected {
                canvas.fill_rect(
                    self.frame.x,
                    y,
                    self.frame.width.max(0) as usize,
                    row_height as usize,
                    theme.accent,
                );
            }
            if let Some(item) = self.items[index] {
                canvas.draw_text(self.frame.x + 4, y + 2, item, theme.text);
            }
        }
        if self.focused {
            canvas.fill_rect(
                self.frame.x,
                self.frame.y,
                self.frame.width.max(0) as usize,
                1,
                theme.focus,
            );
        }
    }
}

/// Place `count` equal-height controls vertically inside `area`.
pub fn stack_vertical(area: Frame, count: usize, gap: i32) -> ([Frame; 16], usize) {
    let count = count.min(16);
    let mut frames = [Frame::new(0, 0, 0, 0); 16];
    if count == 0 {
        return (frames, 0);
    }
    let total_gap = gap.max(0).saturating_mul(count.saturating_sub(1) as i32);
    let height = (area.height.saturating_sub(total_gap) / count as i32).max(0);
    for index in 0..count {
        let y = area.y + index as i32 * (height + gap.max(0));
        let actual_height = if index + 1 == count {
            area.y + area.height - y
        } else {
            height
        };
        frames[index] = Frame::new(area.x, y, area.width, actual_height.max(0));
    }
    (frames, count)
}

#[cfg(test)]
mod tests {
    use super::{Button, Checkbox, Label, ListBox, TextField, Theme, stack_vertical};
    use crate::{Canvas, InputEvent, Key, layout::Frame};

    #[test]
    fn button_click_requires_press_and_release_inside() {
        let mut button = Button::new(Frame::new(10, 20, 40, 16), "OK");
        assert!(!button.pointer(12, 22, true));
        assert!(button.pointer(12, 22, false));
        assert!(!button.pointer(12, 22, false));
        assert!(!button.pointer(12, 22, true));
        assert!(!button.pointer(80, 80, false));
        assert!(button.activate());
        button.enabled = false;
        assert!(!button.activate());
    }

    #[test]
    fn text_field_is_utf8_bounded_and_backspaces_codepoints() {
        let mut field = TextField::<5>::new(Frame::new(0, 0, 40, 16));
        assert!(field.pointer(1, 1, true));
        assert!(!field.pointer(1, 1, true));
        assert!(field.handle(InputEvent::Key(Key::Char('é'))));
        assert_eq!(field.as_str(), "é");
        assert!(field.handle(InputEvent::Key(Key::Backspace)));
        assert_eq!(field.as_str(), "");
        assert!(field.handle(InputEvent::Key(Key::Char('a'))));
        assert!(field.handle(InputEvent::Key(Key::Char('b'))));
        assert!(field.handle(InputEvent::Key(Key::Char('界'))));
        assert!(!field.handle(InputEvent::Key(Key::Char('!'))));
        assert_eq!(field.as_str(), "ab界");
        assert!(field.pointer(100, 100, true));
        assert!(!field.focused);
    }

    #[test]
    fn vertical_stack_covers_area_without_overlap() {
        let (frames, count) = stack_vertical(Frame::new(2, 4, 20, 31), 3, 2);
        assert_eq!(count, 3);
        assert_eq!(frames[0], Frame::new(2, 4, 20, 9));
        assert_eq!(frames[1], Frame::new(2, 15, 20, 9));
        assert_eq!(frames[2], Frame::new(2, 26, 20, 9));
    }

    #[test]
    fn checkbox_activation_is_bounded_and_disabled_is_inert() {
        let mut checkbox = Checkbox::new(Frame::new(2, 2, 40, 12), "Ready");
        assert!(checkbox.pointer(4, 4, false));
        assert!(checkbox.checked);
        checkbox.enabled = false;
        assert!(!checkbox.pointer(4, 4, false));
        assert!(checkbox.checked);
    }

    #[test]
    fn label_and_checkbox_draw_inside_their_frames() {
        let mut pixels = [0; 32 * 20];
        let mut canvas = Canvas::new(&mut pixels, 0, 0, 32, 20).unwrap();
        let theme = Theme::DARK;
        let mut label = Label::new(Frame::new(1, 1, 20, 8), "A");
        label.focused = true;
        label.draw(&mut canvas, theme);
        let mut checkbox = Checkbox::new(Frame::new(1, 10, 28, 10), "B");
        checkbox.checked = true;
        checkbox.draw(&mut canvas, theme);
        assert!(pixels.iter().any(|pixel| *pixel == theme.focus));
        assert!(pixels.iter().any(|pixel| *pixel == theme.accent));
    }

    #[test]
    fn list_box_is_capacity_bounded_and_navigates() {
        let mut list = ListBox::<2>::new(Frame::new(0, 0, 80, 32));
        assert!(list.push("One"));
        assert!(list.push("Two"));
        assert!(!list.push("Three"));
        assert_eq!(list.len(), 2);
        assert_eq!(list.selected_item(), Some("One"));
        assert!(list.move_selection(1));
        assert_eq!(list.selected_item(), Some("Two"));
        assert!(!list.move_selection(1));
        assert!(list.pointer(4, 2, false));
        assert_eq!(list.selected_item(), Some("One"));
        assert!(list.pointer(4, 14, false));
        assert_eq!(list.selected_item(), Some("Two"));
    }

    #[test]
    fn list_box_draws_selection_and_focus_markers() {
        let mut list = ListBox::<2>::new(Frame::new(1, 1, 28, 24));
        list.push("One");
        list.push("Two");
        list.focused = true;
        let mut pixels = [0; 32 * 32];
        let mut canvas = Canvas::new(&mut pixels, 0, 0, 32, 32).unwrap();
        list.draw(&mut canvas, Theme::DARK);
        assert!(pixels.iter().any(|pixel| *pixel == Theme::DARK.accent));
        assert!(pixels.iter().any(|pixel| *pixel == Theme::DARK.focus));
    }
}
