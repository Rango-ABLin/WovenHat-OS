//! App-tile icon drawing, extracted from `main.rs` verbatim (M1, behavior
//! preserving): the small glyph-per-app enum, its name/color/shared-icon-id
//! lookups, and the tile painter that draws it.

use libwoven::graphics::Renderer;
use libwoven::{rgb, Canvas};

use crate::geometry::Area;
use crate::tokens::theme_accent;
use crate::{color_of, fill_rounded, outline, rect_of};

#[derive(Clone, Copy)]
pub enum AppIcon {
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

pub fn app_icon(name: &[u8]) -> AppIcon {
    match name {
        b"Files" | b"WovenFiles" => AppIcon::Files,
        b"Terminal" | b"WovenTerminal" => AppIcon::Terminal,
        b"Store" | b"WovenStore" => AppIcon::Store,
        b"Settings" | b"WovenSettings" => AppIcon::Settings,
        b"Security" | b"WovenSecurity" => AppIcon::Security,
        b"Images" | b"WovenImages" => AppIcon::Images,
        b"Calculator" | b"WovenCalculator" => AppIcon::Calculator,
        b"Editor" | b"WovenEditor" => AppIcon::Editor,
        b"Update" | b"WovenUpdate" => AppIcon::Update,
        b"AI" | b"WovenAI" => AppIcon::Ai,
        b"Monitor" | b"WovenMonitor" => AppIcon::Monitor,
        b"Installer" | b"WovenInstaller" => AppIcon::Installer,
        _ => AppIcon::Generic,
    }
}

fn icon_colors(icon: AppIcon) -> (u32, u32) {
    match icon {
        AppIcon::Files => (rgb(45, 150, 190), rgb(170, 230, 232)),
        AppIcon::Terminal => (rgb(35, 45, 62), rgb(100, 220, 172)),
        AppIcon::Store => (rgb(175, 82, 105), rgb(255, 210, 170)),
        AppIcon::Settings => (rgb(88, 105, 150), rgb(224, 232, 255)),
        AppIcon::Security => (rgb(54, 126, 112), rgb(210, 255, 218)),
        AppIcon::Images => (rgb(68, 126, 186), rgb(255, 220, 120)),
        AppIcon::Calculator => (rgb(124, 82, 160), rgb(242, 216, 255)),
        AppIcon::Editor => (rgb(170, 112, 55), rgb(255, 236, 172)),
        AppIcon::Update => (rgb(48, 132, 156), rgb(210, 255, 255)),
        AppIcon::Ai => (rgb(112, 74, 164), rgb(235, 210, 255)),
        AppIcon::Monitor => (rgb(60, 108, 150), rgb(198, 236, 255)),
        AppIcon::Installer => (rgb(92, 140, 92), rgb(214, 250, 214)),
        AppIcon::Generic => (rgb(74, 88, 112), rgb(220, 230, 242)),
    }
}

fn shared_icon_id(icon: AppIcon) -> libwoven::icons::IconId {
    match icon {
        AppIcon::Files => libwoven::icons::IconId::Files,
        AppIcon::Terminal => libwoven::icons::IconId::Terminal,
        AppIcon::Store => libwoven::icons::IconId::Store,
        AppIcon::Settings => libwoven::icons::IconId::Settings,
        AppIcon::Security => libwoven::icons::IconId::Security,
        AppIcon::Images => libwoven::icons::IconId::Images,
        AppIcon::Calculator => libwoven::icons::IconId::Calculator,
        AppIcon::Editor
        | AppIcon::Update
        | AppIcon::Ai
        | AppIcon::Monitor
        | AppIcon::Installer
        | AppIcon::Generic => libwoven::icons::IconId::Generic,
    }
}

/// Draw a compact, legible app tile. The glyphs are deliberately geometric so
/// they stay crisp in both the flat and 3D launcher textures.
pub fn draw_app_icon(
    canvas: &mut Canvas<'_>,
    x: i32,
    y: i32,
    icon: AppIcon,
    size: i32,
    selected: bool,
) {
    let (fill, ink) = icon_colors(icon);
    // A rounded tile with a soft offset shadow, Fluent-style, instead of a
    // hard-edged square with a flat black square behind it. Uses the
    // shared `Renderer::shadow` primitive rather than a second hand-offset
    // `fill_rounded` call.
    let radius = (size / 4).max(1) as usize;
    Renderer::new(canvas).shadow(rect_of(Area::new(x, y, size, size)), 1, 2, radius, color_of(rgb(0, 0, 0)), u8::MAX);
    fill_rounded(canvas, Area::new(x, y, size, size), radius, fill);
    canvas.fill_rect(x + 2, y + 2, (size - 4) as usize, 2, ink);
    if selected {
        outline(
            canvas,
            Area::new(x - 1, y - 1, size + 2, size + 2),
            theme_accent(),
        );
    }
    let left = x + size / 4;
    let top = y + size / 3;
    let span = (size / 2).max(4) as usize;
    match icon {
        AppIcon::Files => {
            canvas.fill_rect(left, top - 2, span / 2 + 2, 3, ink);
            canvas.fill_rect(left, top, span, span / 2 + 2, ink);
        }
        AppIcon::Terminal => {
            canvas.fill_rect(left, top, span, 2, ink);
            canvas.fill_rect(left, top + 4, 2, 2, ink);
            canvas.fill_rect(left + 4, top + 4, span.saturating_sub(4), 2, ink);
        }
        AppIcon::Store => {
            canvas.fill_rect(left, top, span, 2, ink);
            canvas.fill_rect(left + 2, top + 4, span.saturating_sub(4), 2, ink);
            canvas.fill_rect(left + 2, top + 8, span.saturating_sub(4), 2, ink);
        }
        AppIcon::Security => {
            canvas.fill_rect(left + 2, top - 1, span.saturating_sub(4), 2, ink);
            canvas.fill_rect(left, top + 1, span, 5, ink);
            canvas.fill_rect(left + 2, top + 6, span.saturating_sub(4), 2, ink);
        }
        AppIcon::Images => {
            canvas.fill_rect(left, top + 3, span, span.saturating_sub(2), ink);
            canvas.fill_rect(left + 3, top, 3, 3, ink);
        }
        AppIcon::Calculator => {
            canvas.fill_rect(left, top, span, 2, ink);
            canvas.fill_rect(left, top + 4, 2, 2, ink);
            canvas.fill_rect(left + 5, top + 4, 2, 2, ink);
            canvas.fill_rect(left, top + 8, 2, 2, ink);
            canvas.fill_rect(left + 5, top + 8, 2, 2, ink);
        }
        AppIcon::Editor => {
            canvas.fill_rect(left + 2, top - 1, 3, span + 2, ink);
            canvas.fill_rect(left, top + span as i32, span, 2, ink);
        }
        AppIcon::Settings => {
            canvas.fill_rect(left, top + 2, span, 3, ink);
            canvas.fill_rect(left + 3, top - 1, 3, span + 2, ink);
        }
        AppIcon::Monitor => {
            canvas.fill_rect(left, top, span, span.saturating_sub(3), ink);
            canvas.fill_rect(left + span as i32 / 2 - 1, top + span as i32 - 2, 2, 3, ink);
        }
        AppIcon::Installer => {
            canvas.fill_rect(left + 2, top - 1, 2, span.saturating_sub(2), ink);
            canvas.fill_rect(left, top + span as i32 - 4, span, 2, ink);
            canvas.fill_rect(left, top + span as i32, span, 2, ink);
        }
        AppIcon::Update | AppIcon::Ai | AppIcon::Generic => {
            canvas.fill_rect(left, top + 2, span, 2, ink);
            canvas.fill_rect(left + 2, top + 6, span.saturating_sub(4), 2, ink);
        }
    }
    let scale = (size.max(8) as usize / libwoven::icons::ICON_SIZE).clamp(1, 16);
    let _ = libwoven::icons::IconPack::draw(
        canvas,
        x + (size - (libwoven::icons::ICON_SIZE * scale) as i32) / 2,
        y + (size - (libwoven::icons::ICON_SIZE * scale) as i32) / 2,
        shared_icon_id(icon),
        if selected {
            libwoven::icons::IconState::Active
        } else {
            libwoven::icons::IconState::Normal
        },
        libwoven::icons::IconTheme::Dark,
        scale,
    );
}
