//! WovenHat Design System (WHDS) v1 — the brand foundation.
//!
//! This module is deliberately additive. It documents and exposes the visual
//! identity that the existing shell and apps already converge on (the navy to
//! teal canvas and its diagonal *weave*), and adds the roles the current
//! `ui::Theme` / shell `tokens` layer does not express: a background role, a
//! warning role, a documented type scale with real leading, a single icon
//! grammar, and measured WCAG contrast checks.
//!
//! It is allocation-free, `no_std` safe and has no syscall, kernel-ABI, VFS or
//! framebuffer dependency. Nothing here changes accepted rendering behavior;
//! adoption is an explicit, separately qualified migration for call sites.
//!
//! # Composition, not replacement
//!
//! Shared roles (accent, text, focus, danger, success, ...) are read directly
//! from [`crate::ui::Theme`] constants so the two systems cannot drift. The
//! type scale delegates its em sizes to [`crate::ui::DesignTokens`]. This
//! module owns only the genuinely brand-specific additions.
//!
//! # A real finding recorded here
//!
//! `ui::Theme::DARK.surface` is `rgb(45, 65, 90)`. Measured against it, the
//! shared `muted` ink is only 4.12:1 and `danger` only 3.01:1 — both below the
//! WCAG AA 4.5:1 threshold for body text. The design-system `DARK.surface` is
//! therefore the deeper canvas-adjacent `rgb(20, 32, 50)`, which raises those
//! pairings to 6.47:1 and 4.74:1. The accepted theme is not modified here;
//! this is evidence for a future, separately gated theme migration.

/// The version of this design-system contract. Bump only on an intentional,
/// documented visual break.
pub const SYSTEM_VERSION: u16 = 1;

// ---------------------------------------------------------------------------
// 1. Brand identity — the weave
// ---------------------------------------------------------------------------

/// The one visual signature inherited by every WovenHat surface: a vertical
/// navy-to-teal gradient overlaid with a faint diagonal fabric weave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weave {
    /// Gradient colour at the top of the canvas. Matches shell
    /// `WALLPAPER_TOP`.
    pub canvas_top: u32,
    /// Gradient colour at the bottom of the canvas. Matches shell
    /// `WALLPAPER_BOTTOM`.
    pub canvas_bottom: u32,
    /// Tint used for the lit half of a weave band.
    pub tint: u32,
    /// Shade used for the unlit half of a weave band.
    pub shade: u32,
    /// Paint opacity of the weave over the gradient (0..=255).
    pub tint_opacity: u8,
    /// Diagonal band pitch in pixels. A wide pitch keeps the motif reading as
    /// fabric rather than a checkerboard.
    pub pitch: i32,
}

/// The canonical WovenHat weave. These values mirror
/// `apps/woven-shell/src/tokens.rs` so generated assets and the live desktop
/// share one documented identity.
pub const WEAVE: Weave = Weave {
    canvas_top: crate::rgb(11, 29, 58),
    canvas_bottom: crate::rgb(16, 78, 82),
    tint: crate::rgb(255, 255, 255),
    shade: crate::rgb(0, 0, 0),
    tint_opacity: 6,
    pitch: 220,
};

impl Weave {
    /// Position inside the diagonal band, in `0..pitch`. Uses wrapping
    /// addition so pathological coordinates can never overflow.
    pub const fn phase(self, x: i32, y: i32) -> i32 {
        let sum = x.wrapping_add(y);
        let phase = sum % self.pitch;
        if phase < 0 { phase + self.pitch } else { phase }
    }

    /// Whether a pixel falls on the lit half of the weave.
    pub const fn is_highlight(self, x: i32, y: i32) -> bool {
        self.phase(x, y) < self.pitch / 2
    }

    /// The weave colour at a point. Pure and deterministic: asset generators
    /// and the renderer can agree on the same motif without sharing code.
    pub const fn sample(self, x: i32, y: i32) -> u32 {
        if self.is_highlight(x, y) {
            self.tint
        } else {
            self.shade
        }
    }
}

/// Hand-tuned steps of the two brand ramps. Kept as constants so `no_std`
/// callers pay nothing for colour selection.
pub mod brand {
    pub const NAVY_950: u32 = crate::rgb(8, 20, 40);
    pub const NAVY_900: u32 = crate::rgb(11, 29, 58);
    pub const NAVY_800: u32 = crate::rgb(16, 78, 82);
    pub const TEAL_700: u32 = crate::rgb(24, 98, 128);
    pub const TEAL_600: u32 = crate::rgb(28, 115, 145);
    pub const TEAL_500: u32 = crate::rgb(76, 190, 188);
    pub const TEAL_300: u32 = crate::rgb(110, 235, 235);
    pub const GOLD_400: u32 = crate::rgb(255, 205, 96);
}

// ---------------------------------------------------------------------------
// 2. Semantic palette
// ---------------------------------------------------------------------------

/// A complete, role-named palette for one theme. Every field is an opaque
/// `0x00RRGGBB` colour suitable for `Canvas`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticPalette {
    pub background: u32,
    pub surface: u32,
    pub surface_raised: u32,
    pub border: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub text: u32,
    pub text_muted: u32,
    pub disabled: u32,
    pub focus: u32,
    pub success: u32,
    pub warning: u32,
    pub danger: u32,
}

impl SemanticPalette {
    /// Brand-dark. Shared roles come from the accepted theme; `surface` is
    /// deliberately deeper for AA legibility (see module docs).
    pub const DARK: Self = Self {
        background: brand::NAVY_950,
        surface: crate::rgb(20, 32, 50),
        surface_raised: crate::rgb(30, 44, 64),
        border: crate::ui::Theme::DARK.border,
        accent: crate::ui::Theme::DARK.accent,
        on_accent: crate::ui::Theme::DARK.on_accent,
        text: crate::ui::Theme::DARK.text,
        text_muted: crate::ui::Theme::DARK.muted,
        disabled: crate::ui::Theme::DARK.disabled,
        focus: crate::ui::Theme::DARK.focus,
        success: crate::ui::Theme::DARK.success,
        warning: crate::rgb(240, 180, 90),
        danger: crate::ui::Theme::DARK.danger,
    };

    /// Brand-light. The accent is darkened relative to the dark theme so white
    /// ink on teal still clears AA (4.77:1, measured).
    pub const LIGHT: Self = Self {
        background: crate::rgb(239, 243, 248),
        surface: crate::rgb(255, 255, 255),
        surface_raised: crate::rgb(239, 243, 248),
        border: crate::ui::Theme::LIGHT.border,
        accent: crate::ui::Theme::LIGHT.accent,
        on_accent: crate::ui::Theme::LIGHT.on_accent,
        text: crate::ui::Theme::LIGHT.text,
        text_muted: crate::ui::Theme::LIGHT.muted,
        disabled: crate::ui::Theme::LIGHT.disabled,
        focus: crate::ui::Theme::LIGHT.focus,
        success: crate::ui::Theme::LIGHT.success,
        warning: crate::rgb(150, 88, 0),
        danger: crate::ui::Theme::LIGHT.danger,
    };

    /// Maximum-contrast theme. Every role is either black, white or a fully
    /// saturated signal colour; decorative subtlety is intentionally absent.
    pub const HIGH_CONTRAST: Self = Self {
        background: crate::rgb(0, 0, 0),
        surface: crate::rgb(0, 0, 0),
        surface_raised: crate::rgb(0, 0, 0),
        border: crate::ui::Theme::HIGH_CONTRAST.border,
        accent: crate::ui::Theme::HIGH_CONTRAST.accent,
        on_accent: crate::ui::Theme::HIGH_CONTRAST.on_accent,
        text: crate::ui::Theme::HIGH_CONTRAST.text,
        text_muted: crate::ui::Theme::HIGH_CONTRAST.muted,
        disabled: crate::ui::Theme::HIGH_CONTRAST.disabled,
        focus: crate::ui::Theme::HIGH_CONTRAST.focus,
        success: crate::ui::Theme::HIGH_CONTRAST.success,
        warning: crate::rgb(255, 200, 80),
        danger: crate::ui::Theme::HIGH_CONTRAST.danger,
    };

    /// Resolve a palette from the two existing user-facing switches. High
    /// contrast wins over light, matching `ui::Theme::for_accessibility`.
    pub const fn resolve(high_contrast: bool, light: bool) -> Self {
        if high_contrast {
            Self::HIGH_CONTRAST
        } else if light {
            Self::LIGHT
        } else {
            Self::DARK
        }
    }

    /// Ink for the shared code-native icon contract. This is the single
    /// grammar both the `icons` pack and shell tiles should eventually use.
    pub const fn icon_ink(self, state: crate::icons::IconState) -> u32 {
        match state {
            crate::icons::IconState::Normal => self.text,
            crate::icons::IconState::Active => self.accent,
            crate::icons::IconState::Disabled => self.disabled,
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Accessibility — measured contrast, not guessed contrast
// ---------------------------------------------------------------------------

/// Linear sRGB value for each 8-bit channel, scaled so 1.0 == 10000. Generated
/// from the IEC 61966-2-1 transfer function and rounded to nearest integer.
/// Keeping it as a table makes WCAG checks exact, allocation-free and
/// floating-point free in `no_std`.
const SRGB_LINEAR: [u16; 256] = [
    0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 37, 40, 44, 48, 52, 56, 60, 65, 70, 75, 80, 86, 91,
    97, 103, 110, 116, 123, 130, 137, 144, 152, 160, 168, 176, 185, 194, 203, 212, 222, 232, 242,
    252, 262, 273, 284, 296, 307, 319, 331, 343, 356, 369, 382, 395, 409, 423, 437, 452, 467, 482,
    497, 513, 529, 545, 561, 578, 595, 612, 630, 648, 666, 685, 704, 723, 742, 762, 782, 802, 823,
    844, 865, 887, 908, 931, 953, 976, 999, 1022, 1046, 1070, 1095, 1119, 1144, 1170, 1195, 1221,
    1248, 1274, 1301, 1329, 1356, 1384, 1413, 1441, 1470, 1500, 1529, 1559, 1590, 1620, 1651, 1683,
    1714, 1746, 1779, 1812, 1845, 1878, 1912, 1946, 1981, 2016, 2051, 2086, 2122, 2159, 2195, 2232,
    2270, 2307, 2346, 2384, 2423, 2462, 2502, 2542, 2582, 2623, 2664, 2705, 2747, 2789, 2831, 2874,
    2918, 2961, 3005, 3050, 3095, 3140, 3185, 3231, 3278, 3325, 3372, 3419, 3467, 3515, 3564, 3613,
    3663, 3712, 3763, 3813, 3864, 3916, 3968, 4020, 4072, 4125, 4179, 4233, 4287, 4342, 4397, 4452,
    4508, 4564, 4621, 4678, 4735, 4793, 4851, 4910, 4969, 5029, 5089, 5149, 5210, 5271, 5333, 5395,
    5457, 5520, 5583, 5647, 5711, 5776, 5841, 5906, 5972, 6038, 6105, 6172, 6240, 6308, 6376, 6445,
    6514, 6584, 6654, 6724, 6795, 6867, 6939, 7011, 7084, 7157, 7231, 7305, 7379, 7454, 7529, 7605,
    7682, 7758, 7835, 7913, 7991, 8070, 8148, 8228, 8308, 8388, 8469, 8550, 8632, 8714, 8796, 8879,
    8963, 9047, 9131, 9216, 9301, 9387, 9473, 9560, 9647, 9734, 9823, 9911, 10000,
];

/// WCAG relative luminance in `0..=10000` for an opaque `0x00RRGGBB` colour.
pub fn relative_luminance(color: u32) -> u32 {
    let red = SRGB_LINEAR[((color >> 16) & 0xff) as usize] as u32;
    let green = SRGB_LINEAR[((color >> 8) & 0xff) as usize] as u32;
    let blue = SRGB_LINEAR[(color & 0xff) as usize] as u32;
    red.saturating_mul(2126)
        .saturating_add(green.saturating_mul(7152))
        .saturating_add(blue.saturating_mul(722))
        / 10_000
}

/// WCAG contrast ratio in hundredths (e.g. `450` means 4.5:1).
pub fn contrast_ratio(a: u32, b: u32) -> u32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    // 0.05 * 10000 == 500 is the WCAG offset.
    (lighter + 500).saturating_mul(100) / (darker + 500)
}

/// AA contrast for normal body text (4.5:1).
pub fn meets_aa(foreground: u32, background: u32) -> bool {
    contrast_ratio(foreground, background) >= 450
}

/// AA contrast for large text (3:1).
pub fn meets_aa_large(foreground: u32, background: u32) -> bool {
    contrast_ratio(foreground, background) >= 300
}

/// WCAG 1.4.11 non-text contrast for focus rings and control boundaries (3:1).
pub fn meets_non_text(foreground: u32, background: u32) -> bool {
    contrast_ratio(foreground, background) >= 300
}

// ---------------------------------------------------------------------------
// 4. Type scale
// ---------------------------------------------------------------------------

/// The six roles of the WovenHat type scale. Em sizes come from the shared
/// `ui::DesignTokens` so this scale cannot disagree with WovenUI widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeRole {
    Caption,
    Body,
    Subtitle,
    Title,
    Headline,
    Display,
}

impl TypeRole {
    /// Em size in pixels for this role.
    pub const fn em_px(self) -> u16 {
        let tokens = crate::ui::DesignTokens::DEFAULT;
        match self {
            Self::Caption => tokens.type_caption,
            Self::Body => tokens.type_body,
            Self::Subtitle => tokens.type_subtitle,
            Self::Title => tokens.type_title,
            Self::Headline => tokens.type_headline,
            Self::Display => tokens.type_display,
        }
    }

    /// Leading as a percentage of the em size. Tighter for display sizes,
    /// looser for body copy. This is the metric the old fixed 9px bitmap line
    /// height could not express.
    pub const fn leading_percent(self) -> u16 {
        match self {
            Self::Caption => 130,
            Self::Body => 140,
            Self::Subtitle => 135,
            Self::Title => 125,
            Self::Headline => 120,
            Self::Display => 112,
        }
    }

    /// Resulting line height in whole pixels, always at least the em size + 1.
    pub const fn line_height_px(self) -> u16 {
        let em = self.em_px();
        let leading = ((em as u32 * self.leading_percent() as u32) / 100) as u16;
        if leading > em { leading } else { em + 1 }
    }
}

// ---------------------------------------------------------------------------
// 5. Icon grammar
// ---------------------------------------------------------------------------

/// Geometry shared by every icon in the system. The 8x8 pack and the shell's
/// tile painter are the two current producers; both are expected to converge
/// on this grid during migration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconGrammar {
    pub grid: i32,
    pub keyline: i32,
    pub stroke: i32,
    /// Corner radius in grid units. 6 of 24 reproduces the shell tile's
    /// established `size / 4` rounding exactly.
    pub corner_radius: i32,
    pub touch_target: i32,
    /// Fixed-pixel inset of the tile's top ink bar.
    pub inner_inset: i32,
    /// Divisors that express the tile's internal proportions.
    pub inner_height_divisor: i32,
    pub inner_radius_divisor: i32,
    pub glyph_left_divisor: i32,
    pub glyph_top_divisor: i32,
    pub glyph_span_divisor: i32,
}

pub const ICON_GRAMMAR: IconGrammar = IconGrammar {
    grid: 24,
    keyline: 20,
    stroke: 2,
    corner_radius: 6,
    touch_target: 32,
    inner_inset: 3,
    inner_height_divisor: 5,
    inner_radius_divisor: 10,
    glyph_left_divisor: 4,
    glyph_top_divisor: 3,
    glyph_span_divisor: 2,
};

// Geometry invariants are checked at compile time, so no malformed grammar
// can ever ship in a guest image.
const _: () = {
    assert!(ICON_GRAMMAR.keyline <= ICON_GRAMMAR.grid);
    assert!(ICON_GRAMMAR.stroke > 0 && ICON_GRAMMAR.stroke * 2 <= ICON_GRAMMAR.keyline);
    assert!(ICON_GRAMMAR.corner_radius * 2 <= ICON_GRAMMAR.grid);
    assert!(ICON_GRAMMAR.touch_target >= ICON_GRAMMAR.grid);
    assert!(ICON_GRAMMAR.inner_inset > 0);
    assert!(ICON_GRAMMAR.inner_height_divisor > 0);
    assert!(ICON_GRAMMAR.inner_radius_divisor > 0);
    assert!(ICON_GRAMMAR.glyph_left_divisor > 0);
    assert!(ICON_GRAMMAR.glyph_top_divisor > 0);
    assert!(ICON_GRAMMAR.glyph_span_divisor > 0);
};

/// Concrete tile geometry for one pixel size, derived solely from
/// [`ICON_GRAMMAR`]. Both icon producers use this instead of private ad-hoc
/// division, so they cannot drift apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconMetrics {
    pub size: i32,
    pub radius: usize,
    pub inner_inset: i32,
    pub inner_height: i32,
    pub inner_radius: usize,
    pub glyph_left: i32,
    pub glyph_top: i32,
    pub glyph_span: i32,
}

impl IconGrammar {
    /// Derive tile geometry for `size`, preserving the shell's accepted pixel
    /// layout (saturating math keeps degenerate sizes fail-closed).
    pub const fn metrics(self, size: i32) -> IconMetrics {
        // Explicit branches instead of Ord::min/max: this toolchain does not
        // allow conditionally-const comparison methods in const functions.
        let raw_radius = size.saturating_mul(self.corner_radius) / self.grid;
        let radius = if raw_radius < 1 { 1 } else { raw_radius } as usize;
        let half = size / 2;
        let inset = if self.inner_inset < half {
            self.inner_inset
        } else {
            half
        };
        let raw_height = size / self.inner_height_divisor;
        let floored_height = if raw_height < 3 { 3 } else { raw_height };
        let inner_height = if floored_height > size {
            size
        } else {
            floored_height
        };
        let raw_radius_inner = size / self.inner_radius_divisor;
        let inner_radius = if raw_radius_inner < 2 {
            2
        } else {
            raw_radius_inner
        } as usize;
        let raw_span = size / self.glyph_span_divisor;
        let floored_span = if raw_span < 4 { 4 } else { raw_span };
        let glyph_span = if floored_span > size {
            size
        } else {
            floored_span
        };
        IconMetrics {
            size,
            radius,
            inner_inset: inset,
            inner_height,
            inner_radius,
            glyph_left: size / self.glyph_left_divisor,
            glyph_top: size / self.glyph_top_divisor,
            glyph_span,
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Motion
// ---------------------------------------------------------------------------

/// Named motion steps. Durations come from `ui::DesignTokens` and collapse to
/// zero when the user requests reduced motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Instant,
    Fast,
    Normal,
    Slow,
}

impl Motion {
    pub const fn duration_ms(self, reduced_motion: bool) -> u32 {
        if reduced_motion {
            return 0;
        }
        let tokens = crate::ui::DesignTokens::DEFAULT;
        match self {
            Self::Instant => 0,
            Self::Fast => tokens.motion_fast_ms,
            Self::Normal => tokens.motion_normal_ms,
            Self::Slow => tokens.motion_slow_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Weave wallpaper assets
// ---------------------------------------------------------------------------

/// Procedural wallpapers built from the same weave identity as the live
/// desktop. This is a pure, bounded renderer: it writes a complete 24-bit BMP
/// into caller-owned storage, with no allocation, float math or file I/O.
pub mod wallpaper {
    /// The three accessibility variants of the weave family.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Variant {
        Dark,
        Light,
        HighContrast,
    }

    impl Variant {
        pub const ALL: [Self; 3] = [Self::Dark, Self::Light, Self::HighContrast];

        pub const fn name(self) -> &'static str {
            match self {
                Self::Dark => "dark",
                Self::Light => "light",
                Self::HighContrast => "high-contrast",
            }
        }

        /// Build a spec at sample-asset resolution. The weave pitch scales
        /// with the width (14 px at the established 160 px sample width), the
        /// same motif as the live 220 px pitch on a 1280 px desktop.
        pub const fn spec(self, width: u16, height: u16) -> Spec {
            let scaled = (width as i32) * 14 / 160;
            let pitch = if scaled < 4 { 4 } else { scaled };
            match self {
                Self::Dark => Spec {
                    width,
                    height,
                    top: super::WEAVE.canvas_top,
                    bottom: super::WEAVE.canvas_bottom,
                    // 6/255 is the documented weave tint opacity: enough to
                    // read as fabric, faint enough to keep the gradient clear.
                    weave_delta: 6,
                    pitch,
                },
                Self::Light => Spec {
                    width,
                    height,
                    top: crate::rgb(214, 230, 242),
                    bottom: crate::rgb(235, 244, 246),
                    weave_delta: 6,
                    pitch,
                },
                Self::HighContrast => Spec {
                    width,
                    height,
                    top: crate::rgb(0, 0, 0),
                    bottom: crate::rgb(10, 10, 10),
                    weave_delta: 28,
                    pitch,
                },
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Spec {
        pub width: u16,
        pub height: u16,
        pub top: u32,
        pub bottom: u32,
        /// Brightness added/subtracted on alternating diagonal bands.
        pub weave_delta: i16,
        pub pitch: i32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum WallpaperError {
        TooLarge,
        BufferTooSmall,
    }

    const HEADER_BYTES: usize = 54;
    const MAX_DIMENSION: usize = 8192;
    const MAX_PIXELS: usize = 16 * 1024 * 1024;

    /// Exact byte length of the BMP for `width` x `height`, or `None` when the
    /// geometry is invalid or exceeds the bounded asset contract.
    pub fn bmp_len(width: u16, height: u16) -> Option<usize> {
        let w = width as usize;
        let h = height as usize;
        if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION {
            return None;
        }
        if w.checked_mul(h)? > MAX_PIXELS {
            return None;
        }
        let row = w.checked_mul(3)?.checked_add(3)? & !3;
        row.checked_mul(h)?.checked_add(HEADER_BYTES)
    }

    fn put_u16(out: &mut [u8], offset: usize, value: u16) {
        out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(out: &mut [u8], offset: usize, value: u32) {
        out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_i32(out: &mut [u8], offset: usize, value: i32) {
        out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn channels(color: u32) -> (i32, i32, i32) {
        (
            ((color >> 16) & 0xff) as i32,
            ((color >> 8) & 0xff) as i32,
            (color & 0xff) as i32,
        )
    }

    fn channel(top: i32, bottom: i32, t: i32) -> u8 {
        let value = top + (bottom - top) * t / 255;
        value.clamp(0, 255) as u8
    }

    /// Render `spec` as a complete bottom-up 24-bit BMP. Returns the number of
    /// bytes written. Existing bytes beyond that length are left untouched.
    pub fn render_bmp(spec: &Spec, out: &mut [u8]) -> Result<usize, WallpaperError> {
        let len = bmp_len(spec.width, spec.height).ok_or(WallpaperError::TooLarge)?;
        if out.len() < len {
            return Err(WallpaperError::BufferTooSmall);
        }
        if spec.pitch <= 0 {
            return Err(WallpaperError::TooLarge);
        }
        let out = &mut out[..len];
        out.fill(0);
        out[..2].copy_from_slice(b"BM");
        put_u32(out, 2, len as u32);
        put_u32(out, 10, HEADER_BYTES as u32);
        put_u32(out, 14, 40);
        put_i32(out, 18, spec.width as i32);
        put_i32(out, 22, spec.height as i32);
        put_u16(out, 26, 1);
        put_u16(out, 28, 24);
        put_u32(out, 34, (len - HEADER_BYTES) as u32);

        let width = spec.width as usize;
        let height = spec.height as usize;
        let row_bytes = (width * 3 + 3) & !3;
        let (top_r, top_g, top_b) = channels(spec.top);
        let (bottom_r, bottom_g, bottom_b) = channels(spec.bottom);
        for y in 0..height {
            let t = (y * 255 / height) as i32;
            let r = channel(top_r, bottom_r, t) as i32;
            let g = channel(top_g, bottom_g, t) as i32;
            let b = channel(top_b, bottom_b, t) as i32;
            let row = HEADER_BYTES + (height - 1 - y) * row_bytes;
            for x in 0..width {
                let band = ((x as i32 + y as i32) / spec.pitch) % 2 == 0;
                let delta = (if band {
                    spec.weave_delta
                } else {
                    -spec.weave_delta
                }) as i32;
                let offset = row + x * 3;
                out[offset] = (b + delta).clamp(0, 255) as u8;
                out[offset + 1] = (g + delta).clamp(0, 255) as u8;
                out[offset + 2] = (r + delta).clamp(0, 255) as u8;
            }
        }
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_and_weave_are_stable_and_deterministic() {
        assert_eq!(SYSTEM_VERSION, 1);
        assert_eq!(WEAVE.pitch, 220);
        assert_eq!(WEAVE.canvas_top, crate::rgb(11, 29, 58));
        assert_eq!(WEAVE.canvas_bottom, crate::rgb(16, 78, 82));
        assert_eq!(WEAVE.phase(0, 0), 0);
        // The motif runs along x+y, and repeats exactly once per pitch.
        assert_eq!(WEAVE.phase(10, 20), WEAVE.phase(20, 10));
        assert_eq!(WEAVE.phase(3, 4), WEAVE.phase(3 + WEAVE.pitch, 4));
        assert!(WEAVE.phase(-3, 4) >= 0 && WEAVE.phase(-3, 4) < WEAVE.pitch);
        assert_eq!(WEAVE.sample(3, 4), WEAVE.sample(4, 3));
        assert_ne!(WEAVE.sample(0, 0), WEAVE.sample(WEAVE.pitch / 2, 0));
    }

    #[test]
    fn palettes_are_opaque_distinct_and_resolve_in_priority_order() {
        for palette in [
            SemanticPalette::DARK,
            SemanticPalette::LIGHT,
            SemanticPalette::HIGH_CONTRAST,
        ] {
            for role in [
                palette.background,
                palette.surface,
                palette.surface_raised,
                palette.border,
                palette.accent,
                palette.on_accent,
                palette.text,
                palette.text_muted,
                palette.disabled,
                palette.focus,
                palette.success,
                palette.warning,
                palette.danger,
            ] {
                assert_eq!(role >> 24, 0, "palette colours must be 0x00RRGGBB");
            }
            assert_ne!(palette.text, palette.background);
            assert_ne!(palette.accent, palette.on_accent);
            assert_ne!(palette.focus, palette.surface);
        }
        assert_eq!(
            SemanticPalette::resolve(true, true),
            SemanticPalette::HIGH_CONTRAST
        );
        assert_eq!(
            SemanticPalette::resolve(false, true),
            SemanticPalette::LIGHT
        );
        assert_eq!(
            SemanticPalette::resolve(false, false),
            SemanticPalette::DARK
        );
        // Shared roles must track the accepted theme exactly.
        assert_eq!(SemanticPalette::DARK.accent, crate::ui::Theme::DARK.accent);
        assert_eq!(
            SemanticPalette::LIGHT.accent,
            crate::ui::Theme::LIGHT.accent
        );
        assert_eq!(
            SemanticPalette::HIGH_CONTRAST.accent,
            crate::ui::Theme::HIGH_CONTRAST.accent
        );
    }

    #[test]
    fn every_palette_meets_wcag_aa_for_text_and_non_text_roles() {
        for (name, palette) in [
            ("dark", SemanticPalette::DARK),
            ("light", SemanticPalette::LIGHT),
            ("high contrast", SemanticPalette::HIGH_CONTRAST),
        ] {
            assert!(
                meets_aa(palette.text, palette.surface),
                "{name}: primary text on surface"
            );
            assert!(
                meets_aa(palette.text_muted, palette.surface),
                "{name}: secondary text on surface"
            );
            assert!(
                meets_non_text(palette.focus, palette.surface),
                "{name}: focus ring on surface"
            );
            for status in [palette.success, palette.warning, palette.danger] {
                assert!(
                    meets_aa(status, palette.surface),
                    "{name}: status text on surface"
                );
            }
            assert!(
                meets_aa(palette.on_accent, palette.accent),
                "{name}: ink on accent"
            );
        }
    }

    #[test]
    fn contrast_math_is_exact_at_the_endpoints_and_ordered() {
        assert_eq!(relative_luminance(0x000000), 0);
        assert_eq!(relative_luminance(0xffffff), 10_000);
        assert_eq!(contrast_ratio(0x000000, 0xffffff), 2100);
        assert_eq!(contrast_ratio(0xffffff, 0xffffff), 100);
        assert!(contrast_ratio(0x000000, 0x808080) > contrast_ratio(0x000000, 0x707070));
        assert!(!meets_aa(0x96a4b8, crate::ui::Theme::DARK.surface));
        assert!(meets_aa(0x96a4b8, SemanticPalette::DARK.surface));
    }

    #[test]
    fn icon_ink_uses_one_grammar_and_icon_geometry_is_bounded() {
        let p = SemanticPalette::DARK;
        assert_eq!(p.icon_ink(crate::icons::IconState::Normal), p.text);
        assert_eq!(p.icon_ink(crate::icons::IconState::Active), p.accent);
        assert_eq!(p.icon_ink(crate::icons::IconState::Disabled), p.disabled);

        // The four sizes actually used by WovenShell. These numbers are the
        // accepted tile layout; a future refactor must not silently shift them.
        let expected = [
            (14, 3, 3, 2, 3, 4, 7),
            (32, 8, 6, 3, 8, 10, 16),
            (40, 10, 8, 4, 10, 13, 20),
            (48, 12, 9, 4, 12, 16, 24),
        ];
        for (size, radius, inner_h, inner_r, left, top, span) in expected {
            let m = ICON_GRAMMAR.metrics(size);
            assert_eq!(m.size, size);
            assert_eq!(m.radius, radius);
            assert_eq!(m.inner_height, inner_h);
            assert_eq!(m.inner_radius, inner_r);
            assert_eq!(m.glyph_left, left);
            assert_eq!(m.glyph_top, top);
            assert_eq!(m.glyph_span, span);
            assert!(m.inner_height <= m.size && m.glyph_span <= m.size);
        }
        // Degenerate sizes stay bounded rather than underflowing.
        let tiny = ICON_GRAMMAR.metrics(2);
        assert!(tiny.inner_inset <= 1 && tiny.inner_height <= 2 && tiny.glyph_span <= 2);
    }

    #[test]
    fn type_scale_is_monotonic_and_leading_never_overlaps() {
        let roles = [
            TypeRole::Caption,
            TypeRole::Body,
            TypeRole::Subtitle,
            TypeRole::Title,
            TypeRole::Headline,
            TypeRole::Display,
        ];
        for pair in roles.windows(2) {
            assert!(pair[0].em_px() < pair[1].em_px());
        }
        for role in roles {
            assert!(role.line_height_px() > role.em_px());
        }
        // The old bitmap path used a 9px line for a 12px em, which overlapped.
        assert_eq!(TypeRole::Body.em_px(), 12);
        assert!(TypeRole::Body.line_height_px() >= 16);
    }

    #[test]
    fn motion_respects_reduced_motion_and_stays_ordered() {
        for motion in [Motion::Instant, Motion::Fast, Motion::Normal, Motion::Slow] {
            assert_eq!(motion.duration_ms(true), 0);
        }
        assert!(Motion::Instant.duration_ms(false) < Motion::Fast.duration_ms(false));
        assert!(Motion::Fast.duration_ms(false) < Motion::Normal.duration_ms(false));
        assert!(Motion::Normal.duration_ms(false) < Motion::Slow.duration_ms(false));
    }

    #[test]
    fn weave_wallpaper_bmp_is_well_formed_and_deterministic() {
        use wallpaper::{self, Variant};

        let spec = Variant::Dark.spec(160, 90);
        let len = wallpaper::bmp_len(160, 90).unwrap();
        assert_eq!(len, 43_254);
        let mut first = vec![0u8; len];
        let mut second = vec![0u8; len];
        assert_eq!(wallpaper::render_bmp(&spec, &mut first), Ok(len));
        assert_eq!(wallpaper::render_bmp(&spec, &mut second), Ok(len));
        assert_eq!(first, second);

        assert_eq!(&first[..2], b"BM");
        assert_eq!(
            u32::from_le_bytes(first[2..6].try_into().unwrap()),
            len as u32
        );
        assert_eq!(u32::from_le_bytes(first[10..14].try_into().unwrap()), 54);
        assert_eq!(u32::from_le_bytes(first[14..18].try_into().unwrap()), 40);
        assert_eq!(i32::from_le_bytes(first[18..22].try_into().unwrap()), 160);
        assert_eq!(i32::from_le_bytes(first[22..26].try_into().unwrap()), 90);
        assert_eq!(u16::from_le_bytes(first[26..28].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(first[28..30].try_into().unwrap()), 24);
        assert_eq!(
            u32::from_le_bytes(first[34..38].try_into().unwrap()),
            (len - 54) as u32
        );
        // Bottom-up row 0 is the last logical row; the top-left pixel lands
        // in the final row. Top colour + the +6 weave tint, stored as BGR.
        let top_left = len - 480;
        assert_eq!(&first[top_left..top_left + 3], &[64, 35, 17]);
    }

    #[test]
    fn weave_wallpaper_variants_are_distinct_and_bounded() {
        use wallpaper::{self, Variant, WallpaperError};

        let len = wallpaper::bmp_len(320, 200).unwrap();
        assert_eq!(len, 192_054);
        let mut images: Vec<Vec<u8>> = Vec::new();
        for variant in Variant::ALL {
            let mut image = vec![0u8; len];
            assert_eq!(
                wallpaper::render_bmp(&variant.spec(320, 200), &mut image),
                Ok(len)
            );
            images.push(image);
        }
        assert_ne!(images[0], images[1]);
        assert_ne!(images[1], images[2]);
        assert_ne!(images[0], images[2]);

        assert_eq!(wallpaper::bmp_len(0, 10), None);
        assert_eq!(wallpaper::bmp_len(10, 0), None);
        assert_eq!(wallpaper::bmp_len(8193, 10), None);
        assert_eq!(wallpaper::bmp_len(8192, 8192), None);
        let spec = Variant::Dark.spec(320, 200);
        assert_eq!(
            wallpaper::render_bmp(&spec, &mut [0u8; 16]),
            Err(WallpaperError::BufferTooSmall)
        );
        let mut bad_pitch = spec;
        bad_pitch.pitch = 0;
        assert_eq!(
            wallpaper::render_bmp(&bad_pitch, &mut vec![0u8; len]),
            Err(WallpaperError::TooLarge)
        );
    }

    #[test]
    fn committed_wallpaper_assets_match_the_renderer_and_manifests() {
        use sha2::{Digest, Sha256};
        use wallpaper::{self, Variant};

        let names = ["dark", "light", "high-contrast"];
        for (variant, name) in Variant::ALL.iter().zip(names) {
            let bytes: &[u8] = match variant {
                Variant::Dark => {
                    include_bytes!("../../assets/wallpapers/wovenhat-weave-dark-320x200.bmp")
                }
                Variant::Light => {
                    include_bytes!("../../assets/wallpapers/wovenhat-weave-light-320x200.bmp")
                }
                Variant::HighContrast => include_bytes!(
                    "../../assets/wallpapers/wovenhat-weave-high-contrast-320x200.bmp"
                ),
            };
            let len = wallpaper::bmp_len(320, 200).unwrap();
            let mut expected = vec![0u8; len];
            assert_eq!(
                wallpaper::render_bmp(&variant.spec(320, 200), &mut expected),
                Ok(len)
            );
            assert_eq!(bytes, expected, "asset drift for {name}");

            let digest = Sha256::digest(bytes);
            let mut hex = [0u8; 64];
            for (index, byte) in digest.iter().enumerate() {
                hex[index * 2] = b"0123456789abcdef"[(byte >> 4) as usize];
                hex[index * 2 + 1] = b"0123456789abcdef"[(byte & 0xf) as usize];
            }
            let manifest: &[u8] = match variant {
                Variant::Dark => {
                    include_bytes!("../../assets/wallpapers/weave-dark-320x200.manifest.toml")
                }
                Variant::Light => {
                    include_bytes!("../../assets/wallpapers/weave-light-320x200.manifest.toml")
                }
                Variant::HighContrast => include_bytes!(
                    "../../assets/wallpapers/weave-high-contrast-320x200.manifest.toml"
                ),
            };
            assert!(
                manifest.windows(64).any(|window| window == hex),
                "digest mismatch for {name}"
            );
            assert!(
                manifest
                    .windows(b"CC0-1.0".len())
                    .any(|window| window == b"CC0-1.0")
            );
        }
    }
}
