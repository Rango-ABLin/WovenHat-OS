use alloc::vec::Vec;

use crate::display::{Damage, Surface};
use crate::graphics::Color;

/// Geometry comes from the display stack, which sits below the widget layer.
///
/// `gui` previously defined its own `Rect` with the same fields. Two copies of
/// the same concept drift: the display version gained clipping, union and
/// saturating edges that the widget version never had.
pub use crate::display::Rect;

/// Color scheme for high-contrast mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighContrastScheme {
    /// Default: cyan on white
    Default,
    /// Inverted: white on cyan
    Inverted,
    /// Solarized: dark yellow on blue-brown
    Solarized,
    /// Custom user scheme
    Custom(u32, u32), // (text, background)
}

impl HighContrastScheme {
    pub fn text_color(self, base: Color) -> u32 {
        match self {
            HighContrastScheme::Default => Color::CYAN.packed(),
            HighContrastScheme::Inverted => Color::WHITE.packed(),
            HighContrastScheme::Solarized => Color::rgb(220, 170, 90).packed(),
            HighContrastScheme::Custom(text, _) => text,
        }
    }

    pub fn background_color(self, base: Color) -> u32 {
        match self {
            HighContrastScheme::Default => Color::WHITE.packed(),
            HighContrastScheme::Inverted => Color::CYAN.packed(),
            HighContrastScheme::Solarized => Color::rgb(30, 10, 50).packed(),
            HighContrastScheme::Custom(_, background) => background,
        }
    }
}

#[derive(Clone, Copy)]
pub enum InputEvent {
    PointerDown { x: i32, y: i32 },
    Key(char),
}

pub struct Button {
    pub bounds: Rect,
    pub label: &'static str,
    pub color: Color,
    pub radius: i32,
    pub pressed: bool,
    pub focused: bool,
    pub high_contrast: HighContrastScheme,
}

impl Button {
    pub const fn new(bounds: Rect, label: &'static str, color: Color) -> Self {
        Self {
            bounds,
            label,
            color,
            radius: 0,
            pressed: false,
            focused: false,
            high_contrast: HighContrastScheme::Default,
        }
    }

    pub const fn with_radius(mut self, radius: i32) -> Self {
        self.radius = radius;
        self
    }

    /// Draw into `surface`, returning the region touched.
    fn render(&self, surface: &mut Surface) -> Rect {
        let bg_color = self.high_contrast.background_color(self.color);
        let text_color = self.high_contrast.text_color(self.color);

        let display_color = if self.pressed {
            //pressed color is slightly darker
            Color::rgb(
                self.color.r().saturating_sub(50),
                self.color.g().saturating_sub(50),
                self.color.b().saturating_sub(50),
            )
        } else if self.focused {
            //focused color with brighter border
            surface.fill_rect(self.bounds, bg_color);
            // Draw focus ring - a larger rectangle behind the button
            let focus_ring = Rect::new(
                self.bounds.x - 3,
                self.bounds.y - 3,
                self.bounds.width + 6,
                self.bounds.height + 6,
            );
            surface.fill_rect(focus_ring, Color::rgb(100, 150, 255).packed());
            // Draw the button on top with slightly different color
            let focused_color = Color::rgb(
                min(self.color.r() + 30, 255),
                min(self.color.g() + 30, 255),
                min(self.color.b() + 30, 255),
            );
            surface.fill_rect(self.bounds, focused_color.packed());
            // Draw focus outline
            surface.stroke_rect(
                Rect::new(
                    self.bounds.x - 2,
                    self.bounds.y - 2,
                    self.bounds.width + 4,
                    self.bounds.height + 4,
                ),
                Color::WHITE.packed(),
            );
            // Draw label with focus colors
            let label_width = crate::font::text_width(self.label) as i32;
            let label_height = (crate::font::line_count(self.label) as i32 * crate::font::LINE_HEIGHT)
                - (crate::font::LINE_HEIGHT - crate::font::HEIGHT as i32);
            return surface.draw_text(
                self.bounds.x + (self.bounds.width as i32 - label_width).max(0) / 2,
                self.bounds.y + ((self.bounds.height as i32 - (crate::font::LINE_HEIGHT as i32 - crate::font::HEIGHT as i32)) / 2).max(0),
                self.label,
                text_color,
            );
        } else {
            bg_color
        };

        // Draw the button background with optional rounded corners
        if self.radius > 0 {
            // Draw rounded rectangle by filling then drawing edges
            // Simple approach: fill the bounds, then draw corner arcs manually
            surface.fill_rect(self.bounds, display_color);
            // Draw corner highlights for gradient effect
            if self.radius > 0 {
                let r = self.radius as usize;
                // Top-left corner highlight
                surface.set_pixel(self.bounds.x + r as i32 - 1, self.bounds.y, Color::WHITE.packed());
                surface.set_pixel(self.bounds.x, self.bounds.y + r as i32 - 1, Color::WHITE.packed());
                // Bottom-right corner shadow
                surface.set_pixel(self.bounds.x + self.bounds.width - r as i32, self.bounds.y + self.bounds.height, Color::BLACK.packed());
                surface.set_pixel(self.bounds.x + self.bounds.width - 1, self.bounds.y + self.bounds.height - r as i32, Color::BLACK.packed());
            }
        } else {
            surface.fill_rect(self.bounds, display_color);
        }

        // Centre the label with better vertical alignment
        let label_width = crate::font::text_width(self.label) as i32;
        let label_lines = crate::font::line_count(self.label) as i32;
        // Calculate the total text height including line spacing
        let text_height = label_lines * (crate::font::LINE_HEIGHT as i32 - crate::font::HEIGHT as i32) + crate::font::HEIGHT as i32;
        // Center the text block within the button bounds
        let text_x = self.bounds.x + (self.bounds.width as i32 - label_width).max(0) / 2;
        let text_y = self.bounds.y + (self.bounds.height as i32 - text_height) / 2 + crate::font::HEIGHT as i32 / 2;
        let touched = surface.draw_text(
            text_x,
            text_y,
            self.label,
            text_color,
        );

        if self.focused && self.radius == 0 && self.high_contrast == HighContrastScheme::Default {
            touched = touched.union(surface.stroke_rect(
                Rect::new(
                    self.bounds.x - 2,
                    self.bounds.y - 2,
                    self.bounds.width + 4,
                    self.bounds.height + 4,
                ),
                Color::WHITE.packed(),
            ));
        }

        touched
    }

    fn handle(&mut self, event: &InputEvent) -> bool {
        match event {
            InputEvent::PointerDown { x, y } if self.bounds.contains(*x, *y) => {
                self.pressed = !self.pressed;
                true
            }
            _ => false,
        }
    }
}

pub struct Window {
    pub bounds: Rect,
    pub title: &'static str,
    pub title_bar_height: usize,
    pub title_color: Color,
    pub body_color: Color,
    pub buttons: Vec<Button>,
}

impl Window {
    pub fn new(bounds: Rect, title: &'static str) -> Self {
        Self {
            bounds,
            title,
            title_bar_height: 24,
            title_color: Color::CYAN,
            body_color: Color::WHITE,
            buttons: Vec::new(),
        }
    }

    pub fn add_button(&mut self, button: Button) {
        self.buttons.push(button);
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        self.bounds.contains(x, y)
    }

    fn clear_focus(&mut self) {
        for button in &mut self.buttons {
            button.focused = false;
        }
    }

    /// Draw into `surface`, returning the region touched.
    fn render(&self, surface: &mut Surface) -> Rect {
        let mut touched = surface.fill_rect(
            Rect::new(
                self.bounds.x,
                self.bounds.y,
                self.bounds.width,
                self.title_bar_height,
            ),
            self.title_color.packed(),
        );
        // Centre the title in its bar, vertically by glyph height so it does
        // not drift when the bar height changes.
        let title_width = crate::font::text_width(self.title) as i32;
        touched = touched.union(surface.draw_text(
            self.bounds.x + (self.bounds.width as i32 - title_width).max(0) / 2,
            self.bounds.y + (self.title_bar_height as i32 - crate::font::HEIGHT as i32).max(0) / 2,
            self.title,
            Color::DARK_BLUE.packed(),
        ));
        touched = touched.union(surface.fill_rect(
            Rect::new(
                self.bounds.x,
                self.bounds.y + self.title_bar_height as i32,
                self.bounds.width,
                self.bounds.height.saturating_sub(self.title_bar_height),
            ),
            self.body_color.packed(),
        ));
        touched = touched.union(surface.stroke_rect(self.bounds, Color::BLACK.packed()));
        for button in &self.buttons {
            touched = touched.union(button.render(surface));
        }
        touched
    }

    fn handle(&mut self, event: &InputEvent) -> bool {
        if let InputEvent::Key('\t') = event {
            if self.buttons.is_empty() {
                return false;
            }
            let current = self.buttons.iter().position(|button| button.focused);
            let next = current.map_or(0, |index| (index + 1) % self.buttons.len());
            for (index, button) in self.buttons.iter_mut().enumerate() {
                button.focused = index == next;
            }
            return true;
        }
        if let InputEvent::Key('\n') = event {
            if let Some(button) = self.buttons.iter_mut().find(|button| button.focused) {
                button.pressed = !button.pressed;
                return true;
            }
        }
        if let InputEvent::PointerDown { x, y } = event {
            let mut handled = false;
            for button in &mut self.buttons {
                let hit = button.bounds.contains(*x, *y);
                button.focused = hit;
                if hit {
                    handled = button.handle(event);
                }
            }
            return handled;
        }
        false
    }
}

pub struct Desktop {
    pub background: Color,
    pub windows: Vec<Window>,
}

impl Desktop {
    pub fn new(background: Color) -> Self {
        Self {
            background,
            windows: Vec::new(),
        }
    }

    pub fn add_window(&mut self, window: Window) {
        self.windows.push(window);
    }

    /// Draw the whole desktop into `surface`, reporting every region touched.
    ///
    /// The caller feeds these regions to a `Damage` list so presentation copies
    /// only what changed instead of the whole screen.
    pub fn render(&self, surface: &mut Surface, damage: &mut Damage) {
        damage.add(surface.fill_rect(surface.bounds(), self.background.packed()));
        for window in &self.windows {
            damage.add(window.render(surface));
        }
    }

    pub fn handle(&mut self, event: &InputEvent) -> bool {
        if let InputEvent::PointerDown { x, y } = event {
            let Some(active_index) = self
                .windows
                .iter()
                .rposition(|window| window.contains(*x, *y))
            else {
                for window in &mut self.windows {
                    window.clear_focus();
                }
                return false;
            };

            for (index, window) in self.windows.iter_mut().enumerate() {
                if index != active_index {
                    window.clear_focus();
                }
            }

            let mut active = self.windows.remove(active_index);
            active.handle(event);
            self.windows.push(active);
            return true;
        }

        self.windows
            .last_mut()
            .is_some_and(|window| window.handle(event))
    }
}

pub fn self_test() -> bool {
    let bounds = Rect::new(10, 10, 80, 30);
    let mut desktop = Desktop::new(Color::DARK_BLUE);
    let mut window = Window::new(Rect::new(0, 0, 160, 120), "TEST");
    window.add_button(Button::new(bounds, "ACTIVATE", Color::CYAN));
    window.add_button(Button::new(
        Rect::new(10, 50, 80, 30),
        "SECOND",
        Color::CYAN,
    ));
    desktop.add_window(window);

    let first_focused =
        desktop.handle(&InputEvent::Key('\t')) && desktop.windows[0].buttons[0].focused;
    let second_focused =
        desktop.handle(&InputEvent::Key('\t')) && desktop.windows[0].buttons[1].focused;
    let activated = desktop.handle(&InputEvent::Key('\n'));
    let pointer_activation = desktop.handle(&InputEvent::PointerDown { x: 20, y: 20 });
    first_focused
        && second_focused
        && activated
        && pointer_activation
        && desktop.windows[0].buttons[0].pressed
        && desktop.windows[0].buttons[1].pressed
}
