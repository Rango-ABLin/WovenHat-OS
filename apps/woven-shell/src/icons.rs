//! App-tile icon drawing, extracted from `main.rs` verbatim (M1, behavior
//! preserving): the small glyph-per-app enum, its name/color/shared-icon-id
//! lookups, and the tile painter that draws it.

use libwoven::design_system::{IconMetrics, ICON_GRAMMAR};
use libwoven::graphics::Renderer;
use libwoven::icons::IconAppearance;
use libwoven::{rgb, Canvas};

use crate::geometry::Area;
use crate::tokens::theme_accent;
use crate::{color_of, fill_rounded, outline, rect_of};

/// The shell tile painter and the shared 8x8 pack now resolve icons through
/// one semantic catalog. Keeping this alias preserves the shell's local name
/// while removing the duplicate enum and name-mapping table.
pub use libwoven::icons::IconId as AppIcon;

pub fn app_icon(name: &[u8]) -> AppIcon {
    libwoven::icons::IconId::from_app_name(name)
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
    let IconAppearance { fill, ink } = IconAppearance::for_id(icon);
    let metrics: IconMetrics = ICON_GRAMMAR.metrics(size);
    // A rounded tile with a soft offset shadow, Fluent-style, instead of a
    // hard-edged square with a flat black square behind it. Uses the
    // shared `Renderer::shadow` primitive rather than a second hand-offset
    // `fill_rounded` call.
    let radius = metrics.radius;
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
        Area::new(
            x + metrics.inner_inset,
            y + metrics.inner_inset,
            size - 2 * metrics.inner_inset,
            metrics.inner_height,
        ),
        metrics.inner_radius,
        ink,
    );
    if selected {
        outline(
            canvas,
            Area::new(x - 1, y - 1, size + 2, size + 2),
            theme_accent(),
        );
    }
    let left = x + metrics.glyph_left;
    let top = y + metrics.glyph_top;
    let span = metrics.glyph_span;
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
