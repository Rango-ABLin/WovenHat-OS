//! Versioned, code-native WovenHat icon pack.
//!
//! Icons are deliberately vector-like 8x8 masks rendered with integer
//! scaling. There are no external SVG parser capabilities at this boundary;
//! untrusted SVG remains a separate asset-import concern.

use crate::Canvas;

pub const PACK_VERSION: u16 = 1;
pub const ICON_SIZE: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconId {
    Files,
    Terminal,
    Store,
    Settings,
    Security,
    Images,
    Calculator,
    Generic,
}

impl IconId {
    pub const ALL: [Self; 8] = [
        Self::Files,
        Self::Terminal,
        Self::Store,
        Self::Settings,
        Self::Security,
        Self::Images,
        Self::Calculator,
        Self::Generic,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Terminal => "terminal",
            Self::Store => "store",
            Self::Settings => "settings",
            Self::Security => "security",
            Self::Images => "images",
            Self::Calculator => "calculator",
            Self::Generic => "generic",
        }
    }

    pub const fn mime(self) -> &'static str {
        match self {
            Self::Files => "inode/directory",
            Self::Terminal => "application/x-terminal",
            Self::Store => "application/x-woven-store",
            Self::Settings => "application/x-woven-settings",
            Self::Security => "application/x-woven-security",
            Self::Images => "image/*",
            Self::Calculator => "application/x-woven-calculator",
            Self::Generic => "application/octet-stream",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconState {
    Normal,
    Active,
    Disabled,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconTheme {
    Light,
    Dark,
}

impl IconTheme {
    pub const fn ink(self, state: IconState) -> u32 {
        match (self, state) {
            (_, IconState::Disabled) => crate::graphics::Color::rgb(125, 135, 150).packed(),
            (Self::Light, IconState::Active) => crate::graphics::Color::rgb(20, 120, 150).packed(),
            (Self::Dark, IconState::Active) => crate::graphics::Color::rgb(110, 235, 235).packed(),
            (Self::Light, IconState::Normal) => crate::graphics::Color::rgb(30, 45, 65).packed(),
            (Self::Dark, IconState::Normal) => crate::graphics::Color::rgb(225, 235, 250).packed(),
        }
    }
}

pub struct IconPack;

impl IconPack {
    pub const VERSION: u16 = PACK_VERSION;

    pub const fn mask(id: IconId) -> [&'static [u8; ICON_SIZE]; ICON_SIZE] {
        match id {
            IconId::Files => [
                b".######.",
                b"##....##",
                b"########",
                b"##....##",
                b"##....##",
                b"##....##",
                b"########",
                b"........",
            ],
            IconId::Terminal => [
                b"########",
                b"#.......",
                b"#.#.....",
                b"#..#....",
                b"#...#...",
                b"#....#..",
                b"########",
                b"........",
            ],
            IconId::Store => [
                b"..####..",
                b".######.",
                b"########",
                b"##....##",
                b"##....##",
                b"##....##",
                b"########",
                b"........",
            ],
            IconId::Settings => [
                b"..##....",
                b".####...",
                b"########",
                b".####...",
                b"..##....",
                b".####...",
                b"########",
                b"........",
            ],
            IconId::Security => [
                b"...##...",
                b"..####..",
                b".######.",
                b"########",
                b".######.",
                b"..####..",
                b"...##...",
                b"........",
            ],
            IconId::Images => [
                b"..##....",
                b".####...",
                b"########",
                b"##..#..#",
                b"#..###.#",
                b"#.....##",
                b"########",
                b"........",
            ],
            IconId::Calculator => [
                b"########",
                b"#..##..#",
                b"#..##..#",
                b"########",
                b"#..##..#",
                b"#..##..#",
                b"########",
                b"........",
            ],
            IconId::Generic => [
                b"..####..",
                b".######.",
                b"########",
                b"###..###",
                b"##....##",
                b"#......#",
                b"########",
                b"........",
            ],
        }
    }

    pub fn draw(
        canvas: &mut Canvas<'_>,
        x: i32,
        y: i32,
        id: IconId,
        state: IconState,
        theme: IconTheme,
        scale: usize,
    ) -> bool {
        let scale = scale.clamp(1, 16);
        let mask = Self::mask(id);
        let color = theme.ink(state);
        let mut painted = false;
        for (row, bits) in mask.iter().enumerate() {
            for (column, bit) in bits.iter().enumerate() {
                if *bit == b'#' {
                    canvas.fill_rect(
                        x.saturating_add((column * scale) as i32),
                        y.saturating_add((row * scale) as i32),
                        scale,
                        scale,
                        color,
                    );
                    painted = true;
                }
            }
        }
        painted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_pack_has_semantic_coverage_and_metadata() {
        assert_eq!(IconPack::VERSION, 1);
        for id in IconId::ALL {
            assert!(!id.name().is_empty());
            assert!(!id.mime().is_empty());
            assert!(IconPack::mask(id).iter().any(|row| row.contains(&b'#')));
        }
    }

    #[test]
    fn icon_rendering_is_sharp_and_theme_aware_at_integer_dpi() {
        let mut light = [0u32; 64 * 64];
        let mut dark = [0u32; 64 * 64];
        let light_painted;
        let dark_painted;
        {
            let mut light_canvas = Canvas::new(&mut light, 0, 0, 64, 64).unwrap();
            light_painted = IconPack::draw(
                &mut light_canvas,
                4,
                4,
                IconId::Files,
                IconState::Normal,
                IconTheme::Light,
                4,
            );
        }
        {
            let mut dark_canvas = Canvas::new(&mut dark, 0, 0, 64, 64).unwrap();
            dark_painted = IconPack::draw(
                &mut dark_canvas,
                4,
                4,
                IconId::Files,
                IconState::Active,
                IconTheme::Dark,
                4,
            );
        }
        assert!(light_painted && dark_painted);
        assert_ne!(light, dark);
        assert_eq!(
            light.iter().filter(|pixel| **pixel != 0).count(),
            4 * 4 * 38
        );
        assert_eq!(dark.iter().filter(|pixel| **pixel != 0).count(), 4 * 4 * 38);
    }
}
