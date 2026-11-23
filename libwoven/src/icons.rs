//! Versioned, code-native WovenHat icon pack.
//!
//! Icons are deliberately vector-like 8x8 masks rendered with integer
//! scaling. There are no external SVG parser capabilities at this boundary;
//! untrusted SVG remains a separate asset-import concern.

use crate::Canvas;

pub const PACK_VERSION: u16 = 2;
pub const ICON_SIZE: usize = 8;

/// The single semantic icon catalog shared by every producer.
///
/// Version 2 adds the five shell application identities (Editor, Update, Ai,
/// Monitor, Installer) to the original eight so the shared pack and the shell
/// tile painter can no longer disagree about which applications exist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconId {
    Files,
    Terminal,
    Store,
    Settings,
    Security,
    Images,
    Calculator,
    Editor,
    Update,
    Ai,
    Monitor,
    Installer,
    Generic,
}

impl IconId {
    pub const ALL: [Self; 13] = [
        Self::Files,
        Self::Terminal,
        Self::Store,
        Self::Settings,
        Self::Security,
        Self::Images,
        Self::Calculator,
        Self::Editor,
        Self::Update,
        Self::Ai,
        Self::Monitor,
        Self::Installer,
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
            Self::Editor => "editor",
            Self::Update => "update",
            Self::Ai => "ai",
            Self::Monitor => "monitor",
            Self::Installer => "installer",
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
            Self::Editor => "text/plain",
            Self::Update => "application/x-woven-update",
            Self::Ai => "application/x-woven-ai",
            Self::Monitor => "application/x-woven-monitor",
            Self::Installer => "application/x-woven-installer",
            Self::Generic => "application/octet-stream",
        }
    }

    /// Resolve an application or window title to one shared identity. Both
    /// the bare and `Woven`-prefixed catalog names are accepted, matching the
    /// shell's established behavior. Unknown names fail closed to `Generic`.
    pub fn from_app_name(name: &[u8]) -> Self {
        match name {
            b"Files" | b"WovenFiles" => Self::Files,
            b"Terminal" | b"WovenTerminal" => Self::Terminal,
            b"Store" | b"WovenStore" => Self::Store,
            b"Settings" | b"WovenSettings" => Self::Settings,
            b"Security" | b"WovenSecurity" => Self::Security,
            b"Images" | b"WovenImages" => Self::Images,
            b"Calculator" | b"WovenCalculator" => Self::Calculator,
            b"Editor" | b"WovenEditor" => Self::Editor,
            b"Update" | b"WovenUpdate" => Self::Update,
            b"AI" | b"WovenAI" => Self::Ai,
            b"Monitor" | b"WovenMonitor" => Self::Monitor,
            b"Installer" | b"WovenInstaller" => Self::Installer,
            _ => Self::Generic,
        }
    }
}

/// The one tile-appearance table. The shell tile painter previously kept a
/// private copy of these pairs; both producers now read the same values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconAppearance {
    pub fill: u32,
    pub ink: u32,
}

impl IconAppearance {
    pub const fn for_id(id: IconId) -> Self {
        let (fill, ink) = match id {
            IconId::Files => (crate::rgb(45, 150, 190), crate::rgb(170, 230, 232)),
            IconId::Terminal => (crate::rgb(35, 45, 62), crate::rgb(100, 220, 172)),
            IconId::Store => (crate::rgb(175, 82, 105), crate::rgb(255, 210, 170)),
            IconId::Settings => (crate::rgb(88, 105, 150), crate::rgb(224, 232, 255)),
            IconId::Security => (crate::rgb(54, 126, 112), crate::rgb(210, 255, 218)),
            IconId::Images => (crate::rgb(68, 126, 186), crate::rgb(255, 220, 120)),
            IconId::Calculator => (crate::rgb(124, 82, 160), crate::rgb(242, 216, 255)),
            IconId::Editor => (crate::rgb(170, 112, 55), crate::rgb(255, 236, 172)),
            IconId::Update => (crate::rgb(48, 132, 156), crate::rgb(210, 255, 255)),
            IconId::Ai => (crate::rgb(112, 74, 164), crate::rgb(235, 210, 255)),
            IconId::Monitor => (crate::rgb(60, 108, 150), crate::rgb(198, 236, 255)),
            IconId::Installer => (crate::rgb(92, 140, 92), crate::rgb(214, 250, 214)),
            IconId::Generic => (crate::rgb(74, 88, 112), crate::rgb(220, 230, 242)),
        };
        Self { fill, ink }
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
    /// State ink comes from one shared contract, so the 8x8 pack and the tile
    /// painter cannot disagree about what "active" or "disabled" means.
    pub const fn ink(self, state: IconState) -> u32 {
        match self {
            Self::Light => crate::design_system::SemanticPalette::LIGHT.icon_ink(state),
            Self::Dark => crate::design_system::SemanticPalette::DARK.icon_ink(state),
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
            IconId::Editor => [
                b".....##.",
                b"....###.",
                b"...###..",
                b"..###...",
                b".###....",
                b"###.....",
                b"##......",
                b"........",
            ],
            IconId::Update => [
                b"..####..",
                b".##..##.",
                b"##....##",
                b"##.....#",
                b"##......",
                b".##...#.",
                b"..#####.",
                b"........",
            ],
            IconId::Ai => [
                b"...##...",
                b"...##...",
                b".######.",
                b"########",
                b".######.",
                b"...##...",
                b"...##...",
                b"........",
            ],
            IconId::Monitor => [
                b"########",
                b"#......#",
                b"#......#",
                b"#......#",
                b"########",
                b"...##...",
                b".######.",
                b"........",
            ],
            IconId::Installer => [
                b"...##...",
                b"...##...",
                b"...##...",
                b".######.",
                b"########",
                b"...##...",
                b".######.",
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
        assert_eq!(IconPack::VERSION, 2);
        assert_eq!(IconId::ALL.len(), 13);
        for id in IconId::ALL {
            assert!(!id.name().is_empty());
            assert!(!id.mime().is_empty());
            for row in IconPack::mask(id) {
                assert_eq!(row.len(), ICON_SIZE);
            }
            assert!(IconPack::mask(id).iter().any(|row| row.contains(&b'#')));
        }
    }

    #[test]
    fn app_name_mapping_covers_the_whole_shared_catalog() {
        for id in IconId::ALL {
            // `name()` is the semantic id, not necessarily the catalog title,
            // so only the titles that have a mapping are round-tripped here.
            if let Some(title) = catalog_title(id) {
                assert_eq!(IconId::from_app_name(title), id);
                let mut prefixed = [0u8; 24];
                prefixed[..5].copy_from_slice(b"Woven");
                prefixed[5..5 + title.len()].copy_from_slice(title);
                assert_eq!(IconId::from_app_name(&prefixed[..5 + title.len()]), id);
            }
        }
        assert_eq!(IconId::from_app_name(b"NotAnApp"), IconId::Generic);
    }

    /// Catalog display titles, kept here for the round-trip test rather than
    /// in the production enum (which stores canonical lowercase names).
    fn catalog_title(id: IconId) -> Option<&'static [u8]> {
        match id {
            IconId::Files => Some(b"Files"),
            IconId::Terminal => Some(b"Terminal"),
            IconId::Store => Some(b"Store"),
            IconId::Settings => Some(b"Settings"),
            IconId::Security => Some(b"Security"),
            IconId::Images => Some(b"Images"),
            IconId::Calculator => Some(b"Calculator"),
            IconId::Editor => Some(b"Editor"),
            IconId::Update => Some(b"Update"),
            IconId::Ai => Some(b"AI"),
            IconId::Monitor => Some(b"Monitor"),
            IconId::Installer => Some(b"Installer"),
            IconId::Generic => None,
        }
    }

    #[test]
    fn appearance_table_is_opaque_and_distinct() {
        for id in IconId::ALL {
            let style = IconAppearance::for_id(id);
            assert_eq!(style.fill >> 24, 0);
            assert_eq!(style.ink >> 24, 0);
            assert_ne!(style.fill, style.ink);
        }
        assert_ne!(
            IconAppearance::for_id(IconId::Files),
            IconAppearance::for_id(IconId::Terminal)
        );
    }

    #[test]
    fn mask_pack_and_tile_painter_share_the_state_ink_contract() {
        use crate::design_system::SemanticPalette;
        for state in [IconState::Normal, IconState::Active, IconState::Disabled] {
            assert_eq!(
                IconTheme::Light.ink(state),
                SemanticPalette::LIGHT.icon_ink(state)
            );
            assert_eq!(
                IconTheme::Dark.ink(state),
                SemanticPalette::DARK.icon_ink(state)
            );
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
