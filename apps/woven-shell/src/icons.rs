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

/// Draw a compact Fluent-inspired app tile. The icon is composed from
/// antialiased rounded primitives; the former low-resolution bitmap overlay
/// made every launcher icon read as jagged pixel art at desktop scale.
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
    Renderer::new(canvas).shadow(
        rect_of(Area::new(x, y, size, size)),
        1,
        2,
        radius,
        color_of(rgb(0, 0, 0)),
        72,
    );
    fill_rounded(canvas, Area::new(x, y, size, size), radius, fill);
    fill_rounded(
        canvas,
        Area::new(x + 3, y + 3, size - 6, (size / 5).max(3)),
        (size / 10).max(2) as usize,
        ink,
    );
    if selected {
        outline(
            canvas,
            Area::new(x - 1, y - 1, size + 2, size + 2),
            theme_accent(),
        );
    }
    let left = x + size / 4;
    let top = y + size / 3;
    let span = (size / 2).max(4);
    match icon {
        AppIcon::Files => {
            fill_rounded(canvas, Area::new(left, top - 2, span / 2 + 2, 5), 2, ink);
            fill_rounded(canvas, Area::new(left, top + 3, span, span / 2), 3, ink);
        }
        AppIcon::Terminal => {
            fill_rounded(canvas, Area::new(left, top, span, 3), 1, ink);
            fill_rounded(canvas, Area::new(left, top + 5, 4, 4), 1, ink);
            fill_rounded(canvas, Area::new(left + 7, top + 5, span.saturating_sub(7), 3), 1, ink);
        }
        AppIcon::Store => {
            fill_rounded(canvas, Area::new(left, top, span, 3), 1, ink);
            fill_rounded(canvas, Area::new(left + 2, top + 5, span.saturating_sub(4), 3), 1, ink);
            fill_rounded(canvas, Area::new(left + 2, top + 10, span.saturating_sub(4), 3), 1, ink);
        }
        AppIcon::Security => {
            fill_rounded(canvas, Area::new(left + 2, top - 1, span.saturating_sub(4), 3), 1, ink);
            fill_rounded(canvas, Area::new(left, top + 2, span, 6), 2, ink);
            fill_rounded(canvas, Area::new(left + 2, top + 8, span.saturating_sub(4), 3), 1, ink);
        }
        AppIcon::Images => {
            fill_rounded(canvas, Area::new(left, top + 3, span, span.saturating_sub(2)), 3, ink);
            fill_rounded(canvas, Area::new(left + 4, top, 5, 5), 2, ink);
        }
        AppIcon::Calculator => {
            fill_rounded(canvas, Area::new(left, top, span, 3), 1, ink);
            for (column, row) in [(0, 5), (7, 5), (0, 12), (7, 12)] {
                fill_rounded(canvas, Area::new(left + column, top + row, 4, 4), 1, ink);
            }
        }
        AppIcon::Editor => {
            fill_rounded(canvas, Area::new(left + 2, top - 1, 5, span + 2), 2, ink);
            fill_rounded(canvas, Area::new(left, top + span, span, 3), 1, ink);
        }
        AppIcon::Settings => {
            fill_rounded(canvas, Area::new(left, top + 2, span, 5), 2, ink);
            fill_rounded(canvas, Area::new(left + 4, top - 1, 5, span + 2), 2, ink);
        }
        AppIcon::Monitor => {
            fill_rounded(canvas, Area::new(left, top, span, span.saturating_sub(3)), 2, ink);
            fill_rounded(
                canvas,
                Area::new(left + span / 2 - 2, top + span - 2, 5, 5),
                1,
                ink,
            );
        }
        AppIcon::Installer => {
            fill_rounded(canvas, Area::new(left + 3, top - 1, 5, span.saturating_sub(2)), 2, ink);
            fill_rounded(canvas, Area::new(left, top + span - 4, span, 4), 1, ink);
            fill_rounded(canvas, Area::new(left, top + span, span, 4), 1, ink);
        }
        AppIcon::Update | AppIcon::Ai | AppIcon::Generic => {
            fill_rounded(canvas, Area::new(left, top + 2, span, 4), 1, ink);
            fill_rounded(canvas, Area::new(left + 2, top + 8, span.saturating_sub(4), 4), 1, ink);
        }
    }
}
