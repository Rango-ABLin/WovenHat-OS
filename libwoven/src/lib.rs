#![cfg_attr(not(test), no_std)]
//! The WovenHat userspace API.
//!
//! This is the boundary an application links against instead of issuing `int
//! 0x80` by hand. The [`syscall`] module holds one thin `unsafe` wrapper per
//! kernel entry point, and the typed modules above it provide the safe surface
//! where the unsafety can actually be discharged.
//!
//! # Calling convention
//!
//! `rax` carries the syscall number, `rdi`, `rsi` and `rdx` the three
//! arguments, and the result returns in `rax`. [`ERROR`] (`u64::MAX`) signals
//! failure; a few calls additionally use `u64::MAX - 1` to mean "would block".
//! Raw wrappers return the kernel's value unchanged so a caller can tell those
//! apart.
//!
//! Successful process waits and thread joins carry a zero-extended 32-bit
//! exit-status payload. After checking the full-width error sentinels, casting
//! that payload to `i32` recovers negative statuses, including -1 and -2.

pub mod process {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Id(pub u64);
}
pub mod ai_assistant;
pub mod ai_ipc;
pub mod ai_model;
pub mod ai_monitor;
pub mod ai_provider;
pub mod identity;
pub mod package;
pub mod package_ipc;
pub mod semantic_search;
pub mod store;

#[cfg(test)]
#[path = "../../kernel/src/exit_status.rs"]
mod kernel_exit_status;

/// The kernel's 5x7 bitmap font, shared source rather than a copy, so text
/// a client renders matches text the kernel renders pixel for pixel.
#[allow(unexpected_cfgs)]
#[path = "../../kernel/src/font.rs"]
pub mod font;

/// Process arguments as the kernel lays them out on the initial stack: `argc`,
/// then `argc` pointers to NUL-terminated strings, then a NULL.
///
/// Obtained through [`entry!`], which captures the initial stack pointer
/// before any Rust code can move it.
#[derive(Clone, Copy)]
pub struct Args {
    argc: usize,
    argv: *const *const u8,
}

impl Args {
    /// The kernel rejects argument strings of 256 bytes or more, so a scan
    /// that runs this far has found a malformed stack.
    const MAX_ARG_BYTES: usize = 256;

    /// # Safety
    /// `stack` must be the process's initial stack pointer, as captured by
    /// [`entry!`], with the kernel's argument layout still intact.
    pub unsafe fn from_initial_stack(stack: *const u64) -> Self {
        // SAFETY: the caller guarantees the kernel's layout at `stack`.
        let argc = unsafe { stack.read() } as usize;
        Self {
            argc,
            argv: unsafe { stack.add(1) } as *const *const u8,
        }
    }

    pub const fn len(&self) -> usize {
        self.argc
    }

    pub const fn is_empty(&self) -> bool {
        self.argc == 0
    }

    /// Argument `index` without its terminator, or `None` past the end.
    pub fn get(&self, index: usize) -> Option<&'static [u8]> {
        if index >= self.argc {
            return None;
        }
        // SAFETY: `index < argc`, and the kernel wrote `argc` valid pointers
        // to strings that live on the stack for the life of the process.
        let start = unsafe { self.argv.add(index).read() };
        let mut length = 0;
        while length < Self::MAX_ARG_BYTES {
            // SAFETY: bounded by the kernel's own argument size limit.
            if unsafe { start.add(length).read() } == 0 {
                // SAFETY: `length` bytes from `start` were just read.
                return Some(unsafe { core::slice::from_raw_parts(start, length) });
            }
            length += 1;
        }
        None
    }
}

/// Define the process entry point and hand `main` the process arguments.
///
/// `main` must have the signature `fn(libwoven::Args) -> !`. The stub is
/// assembly because the arguments are found relative to the entry stack
/// pointer, which Rust code cannot observe reliably: it passes `rsp` as the
/// first argument, keeps the stack 16-byte aligned for the call, and traps if
/// `main` ever returns.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        core::arch::global_asm!(
            ".globl _start",
            "_start:",
            "mov rdi, rsp",
            "and rsp, -16",
            "call {start}",
            "ud2",
            start = sym __libwoven_start,
        );

        #[doc(hidden)]
        unsafe extern "C" fn __libwoven_start(stack: *const u64) -> ! {
            // SAFETY: `stack` is the untouched entry stack pointer.
            let args = unsafe { $crate::Args::from_initial_stack(stack) };
            $main(args)
        }
    };
}
pub mod thread {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Id(pub u64);

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Join {
        Exited(i32),
        Running,
        Invalid,
    }

    pub(crate) const fn decode_join(result: u64) -> Join {
        if result == crate::syscall::WOULD_BLOCK {
            Join::Running
        } else if result == crate::syscall::ERROR {
            Join::Invalid
        } else {
            Join::Exited(result as i32)
        }
    }
}
pub mod fs {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Handle(pub u64);
}
pub mod net {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Socket(pub u64);
}
pub mod ipc {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Handle(pub u64);
}
pub mod async_io {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Operation(pub u64);
}
pub mod security {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Capability(pub u64);

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(u8)]
    pub enum AuditAction {
        CapabilityGrant = 0,
        CapabilityRevoke = 1,
        IpcSend = 2,
        FileWrite = 3,
        ProcessFault = 4,
        ProcessExec = 5,
        ProcessFork = 6,
        WovenGuardAllow = 7,
        WovenGuardDeny = 8,
        CapabilityLineageIssue = 9,
        CapabilityDerived = 10,
        CapabilityLineageRelease = 11,
        CapabilityLineageRevoke = 12,
        PermissionRequest = 13,
        PermissionDecision = 14,
        PermissionRevoke = 15,
        SandboxProfileBind = 16,
        SandboxProfileDeny = 17,
        SandboxServicePublish = 18,
        SandboxServiceDiscover = 19,
        SandboxServiceUse = 20,
        SandboxFileRead = 21,
        SandboxFileWrite = 22,
        SandboxDeviceAccess = 23,
        SecretStore = 24,
        SecretRevoke = 25,
        IdentityOpen = 26,
        IdentityLock = 27,
        IdentityUnlock = 28,
        IdentityElevate = 29,
        IdentityClose = 30,
        IdentityAdminAddUser = 31,
        IdentityAdminDeleteUser = 32,
        IdentityAdminAddGroup = 33,
        IdentityAdminAddMember = 34,
        IdentityAdminRemoveMember = 35,
        IdentityAdminQuery = 36,
        IdentityAdminSetLockout = 37,
        IdentityHardware = 38,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct AuditEvent {
        pub sequence: u64,
        pub tick: u64,
        pub cpu: u16,
        pub action: AuditAction,
        pub actor: u64,
        pub target: u64,
        pub detail: u64,
        pub allowed: bool,
    }
}
pub mod time {
    pub type Ticks = u64;
    /// Timer interrupts per second, matching the kernel's PIT programming.
    pub const TICKS_PER_SECOND: u64 = 100;
}
pub mod graphics {
    /// A format-independent sRGB colour used by WovenGraphics.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Color {
        pub red: u8,
        pub green: u8,
        pub blue: u8,
    }

    impl Color {
        pub const BLACK: Self = Self::rgb(0, 0, 0);
        pub const WHITE: Self = Self::rgb(255, 255, 255);
        pub const TITLE: Self = Self::rgb(40, 220, 230);
        pub const SURFACE: Self = Self::rgb(230, 230, 240);

        pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
            Self { red, green, blue }
        }

        pub const fn packed(self) -> u32 {
            crate::rgb(self.red, self.green, self.blue)
        }
    }

    /// Logical geometry shared by the renderer and future display drivers.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Rect {
        pub x: i32,
        pub y: i32,
        pub width: usize,
        pub height: usize,
    }

    /// A logical point used by bounded paths and future vector primitives.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    impl Point {
        pub const fn new(x: i32, y: i32) -> Self {
            Self { x, y }
        }
    }

    const MAX_PATH_POINTS: usize = 64;

    /// A bounded open or closed path. The fixed storage keeps scene geometry
    /// allocation-free and makes the renderer's worst-case work explicit.
    pub struct Path {
        points: [Point; MAX_PATH_POINTS],
        length: usize,
        closed: bool,
    }

    impl Default for Path {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Path {
        pub const fn new() -> Self {
            Self {
                points: [Point::new(0, 0); MAX_PATH_POINTS],
                length: 0,
                closed: false,
            }
        }

        pub fn move_to(&mut self, point: Point) -> bool {
            self.length = 0;
            self.closed = false;
            self.line_to(point)
        }

        pub fn line_to(&mut self, point: Point) -> bool {
            if self.length >= MAX_PATH_POINTS || self.closed {
                return false;
            }
            self.points[self.length] = point;
            self.length += 1;
            true
        }

        /// Append a quadratic Bézier segment as four deterministic line
        /// segments. The fixed subdivision keeps curve work bounded and
        /// avoids floating-point state in freestanding userspace.
        pub fn quadratic_to(&mut self, control: Point, end: Point) -> bool {
            if self.length == 0 || self.closed || self.length + 4 > MAX_PATH_POINTS {
                return false;
            }
            let start = self.points[self.length - 1];
            let points = [
                Self::quadratic_sample(start, control, end, 9, 6, 1),
                Self::quadratic_sample(start, control, end, 4, 8, 4),
                Self::quadratic_sample(start, control, end, 1, 6, 9),
                end,
            ];
            self.points[self.length..self.length + points.len()].copy_from_slice(&points);
            self.length += points.len();
            true
        }

        fn quadratic_sample(
            start: Point,
            control: Point,
            end: Point,
            start_weight: i32,
            control_weight: i32,
            end_weight: i32,
        ) -> Point {
            let x = i64::from(start.x) * i64::from(start_weight)
                + i64::from(control.x) * i64::from(control_weight)
                + i64::from(end.x) * i64::from(end_weight);
            let y = i64::from(start.y) * i64::from(start_weight)
                + i64::from(control.y) * i64::from(control_weight)
                + i64::from(end.y) * i64::from(end_weight);
            Point::new(
                (x / 16).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                (y / 16).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            )
        }

        pub fn close(&mut self) -> bool {
            if self.length < 2 || self.closed {
                return false;
            }
            self.closed = true;
            true
        }

        pub const fn len(&self) -> usize {
            self.length
        }

        pub const fn is_empty(&self) -> bool {
            self.length == 0
        }

        fn points(&self) -> &[Point] {
            &self.points[..self.length]
        }
    }

    const MAX_CONTOURS: usize = 8;

    /// A bounded collection of closed contours. Even-odd filling treats
    /// nested contours as holes without allocating or changing `Path`'s
    /// single-subpath contract.
    pub struct CompoundPath {
        paths: [Path; MAX_CONTOURS],
        length: usize,
    }

    impl Default for CompoundPath {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CompoundPath {
        pub fn new() -> Self {
            Self {
                paths: core::array::from_fn(|_| Path::new()),
                length: 0,
            }
        }

        pub fn push(&mut self, path: Path) -> bool {
            if self.length >= MAX_CONTOURS {
                return false;
            }
            self.paths[self.length] = path;
            self.length += 1;
            true
        }

        pub const fn len(&self) -> usize {
            self.length
        }

        pub const fn is_empty(&self) -> bool {
            self.length == 0
        }

        fn paths(&self) -> &[Path] {
            &self.paths[..self.length]
        }
    }

    /// A fixed-point 2D affine transform. Values use signed 16.16 format so
    /// transformed geometry remains deterministic in freestanding userspace
    /// without pulling floating-point state into the renderer.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Transform {
        pub xx: i32,
        pub xy: i32,
        pub yx: i32,
        pub yy: i32,
        pub tx: i32,
        pub ty: i32,
    }

    impl Transform {
        pub const IDENTITY: Self = Self {
            xx: 1 << 16,
            xy: 0,
            yx: 0,
            yy: 1 << 16,
            tx: 0,
            ty: 0,
        };

        pub const fn new(xx: i32, xy: i32, yx: i32, yy: i32, tx: i32, ty: i32) -> Self {
            Self {
                xx,
                xy,
                yx,
                yy,
                tx,
                ty,
            }
        }

        pub const fn scale(x: i32, y: i32) -> Self {
            Self::new(
                x.saturating_mul(1 << 16),
                0,
                0,
                y.saturating_mul(1 << 16),
                0,
                0,
            )
        }

        pub const fn translate(x: i32, y: i32) -> Self {
            Self::new(
                1 << 16,
                0,
                0,
                1 << 16,
                x.saturating_mul(1 << 16),
                y.saturating_mul(1 << 16),
            )
        }

        fn map(self, x: i32, y: i32) -> (i32, i32) {
            let mapped_x = (i128::from(self.xx) * i128::from(x)
                + i128::from(self.xy) * i128::from(y)
                + (1 << 15))
                / (1 << 16)
                + i128::from(self.tx) / (1 << 16);
            let mapped_y = (i128::from(self.yx) * i128::from(x)
                + i128::from(self.yy) * i128::from(y)
                + (1 << 15))
                / (1 << 16)
                + i128::from(self.ty) / (1 << 16);
            (
                mapped_x.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32,
                mapped_y.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32,
            )
        }
    }

    /// Metrics for a UTF-8 string after the bounded shared-font fallback has
    /// selected a glyph for each scalar value.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct TextMetrics {
        pub width: usize,
        pub height: usize,
        pub lines: usize,
    }

    /// The first WovenGraphics font provider. It intentionally shares the
    /// kernel's glyph table so userspace and compositor text remain pixel
    /// identical while a richer TrueType/OpenType provider can replace this
    /// implementation behind the same scene-facing API.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Font;

    impl Font {
        pub const DEFAULT: Self = Self;

        pub fn measure(self, text: &str) -> TextMetrics {
            self.measure_scaled(text, 1)
        }

        /// Measure text at a bounded integer scale. A zero scale is treated
        /// as one so callers never create a degenerate layout.
        pub fn measure_scaled(self, text: &str, scale: usize) -> TextMetrics {
            let scale = scale.clamp(1, 16);
            TextMetrics {
                width: crate::font::text_width(text).saturating_mul(scale),
                height: crate::font::line_count(text)
                    .saturating_mul(crate::font::LINE_HEIGHT as usize)
                    .saturating_mul(scale),
                lines: crate::font::line_count(text),
            }
        }
    }

    /// Errors returned while validating a bounded TrueType sfnt face.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum TrueTypeError {
        TooSmall,
        BadSignature,
        BadTableDirectory,
        MissingTable,
        UnsupportedCmap,
        UnsupportedGlyph,
        UnsupportedVariation,
        UnsupportedHinting,
        TooLarge,
    }

    /// One decoded point from a simple TrueType glyph outline.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct OutlinePoint {
        pub x: i16,
        pub y: i16,
        pub on_curve: bool,
        pub contour_end: bool,
    }

    /// One allocation-free placement produced by the bounded TrueType text
    /// layout pass.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct GlyphPlacement {
        pub character: char,
        pub origin: Point,
        pub advance: i32,
    }

    /// One bounded OpenType-shaped glyph. `source` identifies the first
    /// character contributing to the glyph; ligatures report their consumed
    /// source count while retaining the selected glyph ID.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct ShapedGlyph {
        pub glyph_id: u16,
        pub source: char,
        pub source_count: u8,
        pub origin: Point,
        pub advance: i32,
    }

    /// Explicit bounded OpenType shaping inputs. `script_tag` and
    /// `language_tag` scope ScriptList/LangSys feature selection; `ppem` is
    /// required when selected GPOS records reference device tables.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct TextShapingOptions {
        pub script_tag: Option<[u8; 4]>,
        pub language_tag: Option<[u8; 4]>,
        pub ppem: Option<u16>,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum TextDirection {
        Ltr,
        Rtl,
    }

    impl TextDirection {
        /// Classify the strong directional characters handled by the bounded
        /// bidi pass. Punctuation, whitespace and combining marks are
        /// neutral and inherit surrounding strong direction.
        pub fn classify(character: char) -> Option<Self> {
            let code = u32::from(character);
            if (0x0590..=0x08ff).contains(&code)
                || (0xfb1d..=0xfdff).contains(&code)
                || (0xfe70..=0xfeff).contains(&code)
            {
                Some(Self::Rtl)
            } else if character.is_alphanumeric() {
                Some(Self::Ltr)
            } else {
                None
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct VariationAxis {
        pub tag: [u8; 4],
        pub minimum: i32,
        pub default: i32,
        pub maximum: i32,
    }

    /// Bounded state exposed by the supported, coordinate-independent hint
    /// instruction subset.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct HintState {
        pub projection_axis: u8,
        pub freedom_axis: u8,
        pub round_to_grid: bool,
        pub stack_depth: u8,
    }

    fn hint_next_pc(program: &[u8], length: usize, pc: usize) -> Result<usize, TrueTypeError> {
        let opcode = *program.get(pc).ok_or(TrueTypeError::BadTableDirectory)?;
        let payload = match opcode {
            0x40 => {
                let count = usize::from(
                    *program
                        .get(pc + 1)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                2usize.checked_add(count).ok_or(TrueTypeError::TooLarge)?
            }
            0x41 => {
                let count = usize::from(
                    *program
                        .get(pc + 1)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                2usize
                    .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                    .ok_or(TrueTypeError::TooLarge)?
            }
            0xb0..=0xb7 => usize::from(opcode - 0xb0) + 2,
            0xb8..=0xbf => (usize::from(opcode - 0xb8) + 1)
                .checked_mul(2)
                .and_then(|count| count.checked_add(1))
                .ok_or(TrueTypeError::TooLarge)?,
            0x1c | 0x78 | 0x79 => 2,
            _ => 1,
        };
        let next = pc.checked_add(payload).ok_or(TrueTypeError::TooLarge)?;
        if next > length {
            return Err(TrueTypeError::BadTableDirectory);
        }
        Ok(next)
    }

    fn hint_skip_to_else_or_eif(
        program: &[u8],
        length: usize,
        mut pc: usize,
    ) -> Result<usize, TrueTypeError> {
        let mut depth = 0usize;
        while pc < length {
            let opcode = program[pc];
            let next = hint_next_pc(program, length, pc)?;
            match opcode {
                0x58 => depth = depth.checked_add(1).ok_or(TrueTypeError::TooLarge)?,
                0x59 => {
                    if depth == 0 {
                        return Ok(next);
                    }
                    depth -= 1;
                }
                0x1b if depth == 0 => return Ok(next),
                _ => {}
            }
            pc = next;
        }
        Err(TrueTypeError::UnsupportedHinting)
    }

    fn hint_skip_to_eif(
        program: &[u8],
        length: usize,
        mut pc: usize,
    ) -> Result<usize, TrueTypeError> {
        let mut depth = 0usize;
        while pc < length {
            let opcode = program[pc];
            let next = hint_next_pc(program, length, pc)?;
            match opcode {
                0x58 => depth = depth.checked_add(1).ok_or(TrueTypeError::TooLarge)?,
                0x59 => {
                    if depth == 0 {
                        return Ok(next);
                    }
                    depth -= 1;
                }
                _ => {}
            }
            pc = next;
        }
        Err(TrueTypeError::UnsupportedHinting)
    }

    fn hint_relative_target(
        program: &[u8],
        length: usize,
        pc: usize,
    ) -> Result<usize, TrueTypeError> {
        let offset = i32::from(i8::from_be_bytes([*program
            .get(pc + 1)
            .ok_or(TrueTypeError::BadTableDirectory)?]));
        let base = i32::try_from(pc.checked_add(2).ok_or(TrueTypeError::TooLarge)?)
            .map_err(|_| TrueTypeError::TooLarge)?;
        let target = base.checked_add(offset).ok_or(TrueTypeError::TooLarge)?;
        if target < 0 || target > i32::try_from(length).map_err(|_| TrueTypeError::TooLarge)? {
            return Err(TrueTypeError::UnsupportedHinting);
        }
        usize::try_from(target).map_err(|_| TrueTypeError::UnsupportedHinting)
    }

    /// A borrowed, validated TrueType face. The face owns no memory and only
    /// exposes checked metadata/cmap lookup; outline rasterization remains a
    /// separate backend so malformed font bytes cannot reach the Canvas.
    pub struct TrueTypeFace<'a> {
        bytes: &'a [u8],
        units_per_em: u16,
        glyph_count: u16,
        cmap_offset: usize,
        cmap_length: usize,
        cmap4: Option<(usize, usize)>,
        cmap12: Option<(usize, usize)>,
        hmtx_offset: usize,
        hmtx_length: usize,
        horizontal_metrics: u16,
        glyf: Option<(usize, usize)>,
        loca: Option<(usize, usize)>,
        index_to_loc_format: i16,
        kern: Option<(usize, usize)>,
        gsub: Option<(usize, usize)>,
        gpos: Option<(usize, usize)>,
        fvar: Option<(usize, usize)>,
        gvar: Option<(usize, usize)>,
    }

    enum GsubSubstitution {
        Single { glyph: u16, consumed: usize },
        Multiple { glyphs: [u16; 16], count: usize },
    }

    impl<'a> TrueTypeFace<'a> {
        pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, TrueTypeError> {
            if bytes.len() < 12 {
                return Err(TrueTypeError::TooSmall);
            }
            if read_u32(bytes, 0) != Some(0x0001_0000) {
                return Err(TrueTypeError::BadSignature);
            }
            let table_count = usize::from(read_u16(bytes, 4).ok_or(TrueTypeError::TooSmall)?);
            let directory_end = 12usize
                .checked_add(
                    table_count
                        .checked_mul(16)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                )
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if directory_end > bytes.len() {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let (head_offset, head_length) =
                find_table(bytes, table_count, *b"head").ok_or(TrueTypeError::MissingTable)?;
            let (maxp_offset, maxp_length) =
                find_table(bytes, table_count, *b"maxp").ok_or(TrueTypeError::MissingTable)?;
            let (cmap_offset, cmap_length) =
                find_table(bytes, table_count, *b"cmap").ok_or(TrueTypeError::MissingTable)?;
            let (hhea_offset, hhea_length) =
                find_table(bytes, table_count, *b"hhea").ok_or(TrueTypeError::MissingTable)?;
            let (hmtx_offset, hmtx_length) =
                find_table(bytes, table_count, *b"hmtx").ok_or(TrueTypeError::MissingTable)?;
            let glyf = find_table(bytes, table_count, *b"glyf");
            let loca = find_table(bytes, table_count, *b"loca");
            let kern = find_table(bytes, table_count, *b"kern");
            let gsub = find_table(bytes, table_count, *b"GSUB");
            let gpos = find_table(bytes, table_count, *b"GPOS");
            let fvar = find_table(bytes, table_count, *b"fvar");
            let gvar = find_table(bytes, table_count, *b"gvar");
            if head_length < 20
                || maxp_length < 6
                || cmap_length < 4
                || hhea_length < 36
                || hmtx_length < 4
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let units_per_em =
                read_u16(bytes, head_offset + 18).ok_or(TrueTypeError::BadTableDirectory)?;
            let glyph_count =
                read_u16(bytes, maxp_offset + 4).ok_or(TrueTypeError::BadTableDirectory)?;
            let horizontal_metrics =
                read_u16(bytes, hhea_offset + 34).ok_or(TrueTypeError::BadTableDirectory)?;
            let index_to_loc_format = if head_length >= 52 {
                read_i16(bytes, head_offset + 50).ok_or(TrueTypeError::BadTableDirectory)?
            } else {
                0
            };
            if units_per_em == 0
                || glyph_count == 0
                || horizontal_metrics == 0
                || (index_to_loc_format != 0 && index_to_loc_format != 1)
                || usize::from(horizontal_metrics).saturating_mul(4) > hmtx_length
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if let Some((_, loca_length)) = loca {
                let loca_entries = usize::from(glyph_count).saturating_add(1);
                let loca_bytes = if index_to_loc_format == 0 {
                    loca_entries.saturating_mul(2)
                } else {
                    loca_entries.saturating_mul(4)
                };
                if loca_bytes > loca_length || glyf.is_none() {
                    return Err(TrueTypeError::BadTableDirectory);
                }
            }
            let cmap_records = usize::from(
                read_u16(bytes, cmap_offset + 2).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let records_end = cmap_offset
                .checked_add(
                    4usize
                        .checked_add(
                            cmap_records
                                .checked_mul(8)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                )
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if records_end > cmap_offset + cmap_length {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let mut cmap4 = None;
            for index in 0..cmap_records {
                let record = cmap_offset + 4 + index * 8;
                let platform = read_u16(bytes, record).ok_or(TrueTypeError::BadTableDirectory)?;
                let encoding =
                    read_u16(bytes, record + 2).ok_or(TrueTypeError::BadTableDirectory)?;
                let relative = usize::try_from(
                    read_u32(bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                )
                .map_err(|_| TrueTypeError::BadTableDirectory)?;
                let subtable = cmap_offset
                    .checked_add(relative)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if subtable + 4 > cmap_offset + cmap_length || read_u16(bytes, subtable) != Some(4)
                {
                    continue;
                }
                if platform == 3 || (platform == 0 && encoding == 3) {
                    let length = usize::from(
                        read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if length >= 16
                        && subtable
                            .checked_add(length)
                            .is_some_and(|end| end <= cmap_offset + cmap_length)
                    {
                        cmap4 = Some((subtable, length));
                        if platform == 3 {
                            break;
                        }
                    }
                }
            }
            let mut cmap12 = None;
            for index in 0..cmap_records {
                let record = cmap_offset + 4 + index * 8;
                let platform = read_u16(bytes, record).ok_or(TrueTypeError::BadTableDirectory)?;
                let encoding =
                    read_u16(bytes, record + 2).ok_or(TrueTypeError::BadTableDirectory)?;
                let relative = usize::try_from(
                    read_u32(bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                )
                .map_err(|_| TrueTypeError::BadTableDirectory)?;
                let subtable = cmap_offset
                    .checked_add(relative)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if subtable
                    .checked_add(16)
                    .is_none_or(|end| end > cmap_offset + cmap_length)
                    || read_u16(bytes, subtable) != Some(12)
                    || !((platform == 3 && encoding == 10) || platform == 0)
                {
                    continue;
                }
                let length = usize::try_from(
                    read_u32(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                )
                .map_err(|_| TrueTypeError::BadTableDirectory)?;
                if length >= 16
                    && subtable
                        .checked_add(length)
                        .is_some_and(|end| end <= cmap_offset + cmap_length)
                {
                    cmap12 = Some((subtable, length));
                    if platform == 3 {
                        break;
                    }
                }
            }
            if cmap4.is_none() && cmap12.is_none() {
                return Err(TrueTypeError::UnsupportedCmap);
            }
            Ok(Self {
                bytes,
                units_per_em,
                glyph_count,
                cmap_offset,
                cmap_length,
                cmap4,
                cmap12,
                hmtx_offset,
                hmtx_length,
                horizontal_metrics,
                glyf,
                loca,
                index_to_loc_format,
                kern,
                gsub,
                gpos,
                fvar,
                gvar,
            })
        }

        pub const fn units_per_em(&self) -> u16 {
            self.units_per_em
        }

        pub const fn glyph_count(&self) -> u16 {
            self.glyph_count
        }

        /// Return the mapped glyph's advance width in font units.
        pub fn advance_width(&self, character: char) -> Option<u16> {
            let glyph = usize::from(self.glyph_index(character)?);
            self.horizontal_metric(glyph).map(|metric| metric.0)
        }

        fn horizontal_metric(&self, glyph: usize) -> Option<(u16, Option<i16>)> {
            let metrics = usize::from(self.horizontal_metrics);
            let metric = glyph.min(metrics.checked_sub(1)?);
            let offset = self.hmtx_offset.checked_add(metric.checked_mul(4)?)?;
            if offset.checked_add(4)? > self.hmtx_offset + self.hmtx_length {
                return None;
            }
            let advance = read_u16(self.bytes, offset)?;
            let lsb = if glyph < metrics {
                read_i16(self.bytes, offset + 2)
            } else {
                let lsb_offset = self
                    .hmtx_offset
                    .checked_add(metrics.checked_mul(4)?)?
                    .checked_add((glyph - metrics).checked_mul(2)?)?;
                if lsb_offset.checked_add(2)? > self.hmtx_offset + self.hmtx_length {
                    None
                } else {
                    read_i16(self.bytes, lsb_offset)
                }
            };
            Some((advance, lsb))
        }

        /// Return a simple-glyph advance after applying its bounded `gvar`
        /// phantom-point deltas at normalized F2DOT14 coordinates.
        pub fn advance_width_with_variation(
            &self,
            character: char,
            normalized_coordinates: &[i16],
        ) -> Result<u16, TrueTypeError> {
            let glyph = self
                .glyph_index(character)
                .ok_or(TrueTypeError::UnsupportedGlyph)?;
            self.advance_width_glyph_with_variation(glyph, normalized_coordinates)
        }

        /// Return a direct glyph's advance after applying bounded gvar
        /// phantom-point deltas.
        pub fn advance_width_glyph_with_variation(
            &self,
            glyph: u16,
            normalized_coordinates: &[i16],
        ) -> Result<u16, TrueTypeError> {
            let (advance, lsb) = self
                .horizontal_metric(usize::from(glyph))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let lsb = lsb.ok_or(TrueTypeError::UnsupportedVariation)?;
            let Some(data) = self.glyph_variation_data(glyph)? else {
                return Ok(advance);
            };
            if data.is_empty() {
                return Ok(advance);
            }
            let axis_count = self.gvar_axis_count()?;
            if axis_count > 32 || normalized_coordinates.len() != axis_count {
                return Err(TrueTypeError::UnsupportedVariation);
            }
            let mut points = [OutlinePoint {
                x: 0,
                y: 0,
                on_curve: false,
                contour_end: false,
            }; 132];
            let outline_count =
                self.outline_points_for_glyph(usize::from(glyph), &mut points[..128], 0)?;
            let variation_point_count = outline_count
                .checked_add(4)
                .ok_or(TrueTypeError::TooLarge)?;
            if variation_point_count > points.len() || outline_count == 0 {
                return Err(TrueTypeError::UnsupportedVariation);
            }
            let min_x = points[..outline_count]
                .iter()
                .map(|point| i32::from(point.x))
                .min()
                .ok_or(TrueTypeError::UnsupportedVariation)?;
            let left = min_x
                .checked_sub(i32::from(lsb))
                .ok_or(TrueTypeError::TooLarge)?;
            let right = left
                .checked_add(i32::from(advance))
                .ok_or(TrueTypeError::TooLarge)?;
            points[outline_count].x = i16::try_from(left).map_err(|_| TrueTypeError::TooLarge)?;
            points[outline_count + 1].x =
                i16::try_from(right).map_err(|_| TrueTypeError::TooLarge)?;
            self.apply_gvar_tuples(
                data,
                normalized_coordinates,
                &mut points[..variation_point_count],
            )?;
            let width = i32::from(points[outline_count + 1].x)
                .checked_sub(i32::from(points[outline_count].x))
                .ok_or(TrueTypeError::TooLarge)?;
            u16::try_from(width).map_err(|_| TrueTypeError::TooLarge)
        }

        /// Copy a validated simple-glyph TrueType instruction program into a
        /// caller-owned buffer. Instruction execution is intentionally not
        /// performed by the deterministic renderer yet.
        pub fn glyph_hint_program(
            &self,
            character: char,
            output: &mut [u8],
        ) -> Result<usize, TrueTypeError> {
            let glyph = usize::from(
                self.glyph_index(character)
                    .ok_or(TrueTypeError::UnsupportedGlyph)?,
            );
            let Some((start, length)) = self.simple_glyph_instruction_span(glyph)? else {
                return Err(TrueTypeError::UnsupportedHinting);
            };
            if length > output.len() {
                return Err(TrueTypeError::TooLarge);
            }
            output[..length].copy_from_slice(
                self.bytes
                    .get(start..start + length)
                    .ok_or(TrueTypeError::BadTableDirectory)?,
            );
            Ok(length)
        }

        /// Execute the bounded coordinate-independent hint subset. Vector
        /// state, rounding state and stack primitives are supported; point
        /// movement, storage, CVT and control-flow instructions fail closed
        /// until their resource model is implemented.
        pub fn execute_glyph_hints(
            &self,
            character: char,
            points: &mut [OutlinePoint],
            state: &mut HintState,
        ) -> Result<usize, TrueTypeError> {
            self.execute_glyph_hints_with_cvt(character, points, &[], state)
        }

        /// Execute the bounded hint subset with caller-owned CVT values.
        /// `MIAP` can place a point on a CVT value; all indices and storage
        /// remain bounded, and unsupported instructions fail closed.
        pub fn execute_glyph_hints_with_cvt(
            &self,
            character: char,
            points: &mut [OutlinePoint],
            cvt: &[i16],
            state: &mut HintState,
        ) -> Result<usize, TrueTypeError> {
            let mut storage = [0i32; 0];
            self.execute_glyph_hints_with_resources(character, points, cvt, &mut storage, state)
        }

        /// Execute the bounded hint subset with caller-owned CVT and storage
        /// resources. `WS` and `RS` are supported; unbounded resource access
        /// and unsupported instructions fail closed.
        pub fn execute_glyph_hints_with_resources(
            &self,
            character: char,
            points: &mut [OutlinePoint],
            cvt: &[i16],
            storage: &mut [i32],
            state: &mut HintState,
        ) -> Result<usize, TrueTypeError> {
            if points.len() > 128 {
                return Err(TrueTypeError::TooLarge);
            }
            let mut program = [0u8; 256];
            let length = self.glyph_hint_program(character, &mut program)?;
            let mut stack = [0i32; 256];
            let mut stack_depth = 0usize;
            let mut pc = 0usize;
            let mut steps = 0usize;
            while pc < length {
                steps = steps.checked_add(1).ok_or(TrueTypeError::TooLarge)?;
                if steps > 1024 {
                    return Err(TrueTypeError::TooLarge);
                }
                let opcode = program[pc];
                pc += 1;
                match opcode {
                    0x1b => pc = hint_skip_to_eif(&program, length, pc)?,
                    0x1c => pc = hint_relative_target(&program, length, pc - 1)?,
                    0x58 => {
                        if stack_depth == 0 {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        let condition = stack[stack_depth - 1];
                        stack_depth -= 1;
                        if condition == 0 {
                            pc = hint_skip_to_else_or_eif(&program, length, pc)?;
                        }
                    }
                    0x59 => {}
                    0x78 | 0x79 => {
                        if stack_depth == 0 {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        let condition = stack[stack_depth - 1];
                        stack_depth -= 1;
                        let jump = (opcode == 0x78 && condition != 0)
                            || (opcode == 0x79 && condition == 0);
                        if jump {
                            pc = hint_relative_target(&program, length, pc - 1)?;
                        } else {
                            pc = pc.checked_add(1).ok_or(TrueTypeError::TooLarge)?;
                            if pc > length {
                                return Err(TrueTypeError::BadTableDirectory);
                            }
                        }
                    }
                    0x00 | 0x02 => state.projection_axis = 1,
                    0x01 | 0x03 => state.projection_axis = 0,
                    0x04 => state.freedom_axis = 1,
                    0x05 => state.freedom_axis = 0,
                    0x18 | 0x19 | 0x7c | 0x7d => state.round_to_grid = true,
                    0x2e | 0x2f => {
                        if stack_depth == 0 {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        let point = usize::try_from(stack[stack_depth - 1])
                            .map_err(|_| TrueTypeError::UnsupportedHinting)?;
                        stack_depth -= 1;
                        if point >= points.len() {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        if opcode & 1 != 0 {
                            state.round_to_grid = true;
                        }
                    }
                    0x3e | 0x3f => {
                        if stack_depth < 2 {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        let point = usize::try_from(stack[stack_depth - 2])
                            .map_err(|_| TrueTypeError::UnsupportedHinting)?;
                        let cvt_index = usize::try_from(stack[stack_depth - 1])
                            .map_err(|_| TrueTypeError::UnsupportedHinting)?;
                        stack_depth -= 2;
                        if point >= points.len() || cvt_index >= cvt.len() {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        let value = cvt[cvt_index];
                        if state.freedom_axis == 0 {
                            points[point].x = value;
                        } else {
                            points[point].y = value;
                        }
                        if opcode & 1 != 0 {
                            state.round_to_grid = true;
                        }
                    }
                    0x42 => {
                        if stack_depth < 2 {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        let index = usize::try_from(stack[stack_depth - 2])
                            .map_err(|_| TrueTypeError::UnsupportedHinting)?;
                        if index >= storage.len() {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        storage[index] = stack[stack_depth - 1];
                        stack_depth -= 2;
                    }
                    0x43 => {
                        if stack_depth == 0 {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        let index = usize::try_from(stack[stack_depth - 1])
                            .map_err(|_| TrueTypeError::UnsupportedHinting)?;
                        if index >= storage.len() || stack_depth == stack.len() {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        stack[stack_depth - 1] = storage[index];
                    }
                    0x20 => {
                        if stack_depth == 0 || stack_depth == stack.len() {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        stack[stack_depth] = stack[stack_depth - 1];
                        stack_depth += 1;
                    }
                    0x21 => {
                        if stack_depth == 0 {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        stack_depth -= 1;
                    }
                    0x22 => stack_depth = 0,
                    0x23 => {
                        if stack_depth < 2 {
                            return Err(TrueTypeError::UnsupportedHinting);
                        }
                        stack.swap(stack_depth - 1, stack_depth - 2);
                    }
                    0x40 => {
                        let count =
                            usize::from(*program.get(pc).ok_or(TrueTypeError::BadTableDirectory)?);
                        pc += 1;
                        if stack_depth
                            .checked_add(count)
                            .is_none_or(|end| end > stack.len())
                            || pc.checked_add(count).is_none_or(|end| end > length)
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        for value in program[pc..pc + count].iter().copied() {
                            stack[stack_depth] = i32::from(value);
                            stack_depth += 1;
                        }
                        pc += count;
                    }
                    0x41 => {
                        let count =
                            usize::from(*program.get(pc).ok_or(TrueTypeError::BadTableDirectory)?);
                        pc += 1;
                        if stack_depth
                            .checked_add(count)
                            .is_none_or(|end| end > stack.len())
                            || pc
                                .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                                .is_none_or(|end| end > length)
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        for _ in 0..count {
                            stack[stack_depth] =
                                i32::from(i16::from_be_bytes([program[pc], program[pc + 1]]));
                            stack_depth += 1;
                            pc += 2;
                        }
                    }
                    0xb0..=0xb7 => {
                        let count = usize::from(opcode - 0xb0) + 1;
                        if stack_depth
                            .checked_add(count)
                            .is_none_or(|end| end > stack.len())
                            || pc.checked_add(count).is_none_or(|end| end > length)
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        for value in program[pc..pc + count].iter().copied() {
                            stack[stack_depth] = i32::from(value);
                            stack_depth += 1;
                        }
                        pc += count;
                    }
                    0xb8..=0xbf => {
                        let count = usize::from(opcode - 0xb8) + 1;
                        if stack_depth
                            .checked_add(count)
                            .is_none_or(|end| end > stack.len())
                            || pc
                                .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                                .is_none_or(|end| end > length)
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        for _ in 0..count {
                            stack[stack_depth] =
                                i32::from(i16::from_be_bytes([program[pc], program[pc + 1]]));
                            stack_depth += 1;
                            pc += 2;
                        }
                    }
                    _ => return Err(TrueTypeError::UnsupportedHinting),
                }
            }
            state.stack_depth = u8::try_from(stack_depth).map_err(|_| TrueTypeError::TooLarge)?;
            Ok(length)
        }

        fn simple_glyph_instruction_span(
            &self,
            glyph: usize,
        ) -> Result<Option<(usize, usize)>, TrueTypeError> {
            let (glyf_offset, glyf_length) = self.glyf.ok_or(TrueTypeError::UnsupportedGlyph)?;
            let (loca_offset, loca_length) = self.loca.ok_or(TrueTypeError::UnsupportedGlyph)?;
            if glyph >= usize::from(self.glyph_count) {
                return Err(TrueTypeError::UnsupportedGlyph);
            }
            let loca_entry = |index: usize| -> Option<usize> {
                if self.index_to_loc_format == 0 {
                    let entry = loca_offset.checked_add(index.checked_mul(2)?)?;
                    if entry.checked_add(2)? > loca_offset + loca_length {
                        return None;
                    }
                    usize::from(read_u16(self.bytes, entry)?).checked_mul(2)
                } else {
                    let entry = loca_offset.checked_add(index.checked_mul(4)?)?;
                    if entry.checked_add(4)? > loca_offset + loca_length {
                        return None;
                    }
                    usize::try_from(read_u32(self.bytes, entry)?).ok()
                }
            };
            let start = loca_entry(glyph).ok_or(TrueTypeError::BadTableDirectory)?;
            let end = loca_entry(glyph + 1).ok_or(TrueTypeError::BadTableDirectory)?;
            if start > end || end > glyf_length {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if start == end {
                return Ok(Some((glyf_offset, 0)));
            }
            let glyph_start = glyf_offset
                .checked_add(start)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let glyph_end = glyf_offset
                .checked_add(end)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if glyph_end > self.bytes.len() || glyph_end - glyph_start < 10 {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let contours =
                read_i16(self.bytes, glyph_start).ok_or(TrueTypeError::BadTableDirectory)?;
            if contours < 0 {
                return Ok(None);
            }
            let contours =
                usize::try_from(contours).map_err(|_| TrueTypeError::BadTableDirectory)?;
            let endpoints_end = glyph_start
                .checked_add(
                    10usize
                        .checked_add(contours.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::TooLarge)?;
            if endpoints_end
                .checked_add(2)
                .is_none_or(|end| end > glyph_end)
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let length = usize::from(
                read_u16(self.bytes, endpoints_end).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            const MAX_HINT_INSTRUCTIONS: usize = 256;
            if length > MAX_HINT_INSTRUCTIONS {
                return Err(TrueTypeError::TooLarge);
            }
            let instruction_start = endpoints_end + 2;
            let instruction_end = instruction_start
                .checked_add(length)
                .ok_or(TrueTypeError::TooLarge)?;
            if instruction_end > glyph_end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            Ok(Some((instruction_start, length)))
        }

        /// Return a legacy TrueType format-0 horizontal kerning adjustment in
        /// font units. Unsupported or malformed optional tables return None.
        pub fn kerning(&self, left: char, right: char) -> Option<i16> {
            let table = self.kern?;
            let left = self.glyph_index(left)?;
            let right = self.glyph_index(right)?;
            let table_end = table.0.checked_add(table.1)?;
            if table.1 < 4 || table_end > self.bytes.len() {
                return None;
            }
            let subtables = usize::from(read_u16(self.bytes, table.0 + 2)?);
            let mut cursor = table.0 + 4;
            for _ in 0..subtables {
                if cursor.checked_add(6)? > table_end {
                    return None;
                }
                let length = usize::from(read_u16(self.bytes, cursor + 2)?);
                let subtable_end = cursor.checked_add(length)?;
                if length < 14 || subtable_end > table_end {
                    return None;
                }
                let coverage = read_u16(self.bytes, cursor + 4)?;
                if coverage >> 8 == 0 && coverage & 1 != 0 {
                    let pairs = usize::from(read_u16(self.bytes, cursor + 6)?);
                    let pairs_end = cursor.checked_add(14)?.checked_add(pairs.checked_mul(6)?)?;
                    if pairs_end > subtable_end {
                        return None;
                    }
                    for pair in 0..pairs {
                        let entry = cursor + 14 + pair * 6;
                        if read_u16(self.bytes, entry)? == left
                            && read_u16(self.bytes, entry + 2)? == right
                        {
                            return read_i16(self.bytes, entry + 4);
                        }
                    }
                }
                cursor = subtable_end;
            }
            None
        }

        /// Read bounded `fvar` axis metadata in 16.16 font units. Variation
        /// deltas are not applied by this parser; callers select and render
        /// only the base outline until a gvar backend is available.
        pub fn variation_axes(&self, output: &mut [VariationAxis]) -> Result<usize, TrueTypeError> {
            let Some((offset, length)) = self.fvar else {
                return Ok(0);
            };
            if length < 16 {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let axis_offset = usize::from(
                read_u16(self.bytes, offset + 8).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let axis_count = usize::from(
                read_u16(self.bytes, offset + 12).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let axis_size = usize::from(
                read_u16(self.bytes, offset + 14).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if axis_size < 20 || axis_count > output.len() {
                return Err(TrueTypeError::TooLarge);
            }
            let table_end = offset
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let axes_end = offset
                .checked_add(axis_offset)
                .and_then(|start| start.checked_add(axis_count.checked_mul(axis_size)?))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if axes_end > table_end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            for (index, slot) in output.iter_mut().enumerate().take(axis_count) {
                let axis = offset + axis_offset + index * axis_size;
                let mut tag = [0u8; 4];
                tag.copy_from_slice(
                    self.bytes
                        .get(axis..axis + 4)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                *slot = VariationAxis {
                    tag,
                    minimum: read_i32(self.bytes, axis + 4)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                    default: read_i32(self.bytes, axis + 8)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                    maximum: read_i32(self.bytes, axis + 12)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                };
            }
            Ok(axis_count)
        }

        /// Return the checked raw `gvar` payload for one glyph. The payload
        /// is borrowed from the face and remains encoded; applying tuple
        /// deltas is a separate bounded rasterization step.
        pub fn glyph_variation_data(&self, glyph: u16) -> Result<Option<&'a [u8]>, TrueTypeError> {
            let Some((offset, length)) = self.gvar else {
                return Ok(None);
            };
            if length < 20 {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if read_u32(self.bytes, offset) != Some(0x0001_0000) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let axis_count = usize::from(
                read_u16(self.bytes, offset + 4).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let shared_tuple_count = usize::from(
                read_u16(self.bytes, offset + 6).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let shared_tuples_offset = usize::try_from(
                read_u32(self.bytes, offset + 8).ok_or(TrueTypeError::BadTableDirectory)?,
            )
            .map_err(|_| TrueTypeError::BadTableDirectory)?;
            let glyph_count = usize::from(
                read_u16(self.bytes, offset + 12).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if usize::from(glyph) >= glyph_count || glyph_count != usize::from(self.glyph_count) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let flags =
                read_u16(self.bytes, offset + 14).ok_or(TrueTypeError::BadTableDirectory)?;
            let data_offset = usize::try_from(
                read_u32(self.bytes, offset + 16).ok_or(TrueTypeError::BadTableDirectory)?,
            )
            .map_err(|_| TrueTypeError::BadTableDirectory)?;
            let table_end = offset
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let shared_end = offset
                .checked_add(shared_tuples_offset)
                .and_then(|start| {
                    start.checked_add(axis_count.checked_mul(shared_tuple_count)?.checked_mul(2)?)
                })
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let offsets_start = offset
                .checked_add(20)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let offset_width = if flags & 1 != 0 { 4 } else { 2 };
            let offsets_end = offsets_start
                .checked_add(
                    (glyph_count + 1)
                        .checked_mul(offset_width)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let data_start = offset
                .checked_add(data_offset)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if shared_end > table_end
                || offsets_end > table_end
                || data_start < offsets_end
                || data_start > table_end
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let read_offset = |index: usize| -> Option<usize> {
                let entry = offsets_start.checked_add(index.checked_mul(offset_width)?)?;
                if flags & 1 != 0 {
                    usize::try_from(read_u32(self.bytes, entry)?).ok()
                } else {
                    usize::from(read_u16(self.bytes, entry)?).checked_mul(2)
                }
            };
            let start = data_start
                .checked_add(
                    read_offset(usize::from(glyph)).ok_or(TrueTypeError::BadTableDirectory)?,
                )
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let end = data_start
                .checked_add(
                    read_offset(usize::from(glyph) + 1).ok_or(TrueTypeError::BadTableDirectory)?,
                )
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if start > end || end > table_end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            self.bytes
                .get(start..end)
                .map(Some)
                .ok_or(TrueTypeError::BadTableDirectory)
        }

        /// Decode `gvar` tuples at normalized F2DOT14 axis coordinates.
        /// Simple glyphs receive point deltas; composite glyphs apply bounded
        /// component-position deltas and recursively vary their components.
        pub fn outline_points_with_variation(
            &self,
            character: char,
            normalized_coordinates: &[i16],
            output: &mut [OutlinePoint],
        ) -> Result<usize, TrueTypeError> {
            let glyph = self
                .glyph_index(character)
                .ok_or(TrueTypeError::UnsupportedGlyph)?;
            let axis_count = self.gvar_axis_count()?;
            if axis_count > 32 || normalized_coordinates.len() != axis_count {
                return Err(TrueTypeError::UnsupportedVariation);
            }
            self.outline_points_for_glyph_variation(
                usize::from(glyph),
                output,
                0,
                Some(normalized_coordinates),
            )
        }

        fn gvar_axis_count(&self) -> Result<usize, TrueTypeError> {
            let Some((offset, length)) = self.gvar else {
                return Ok(0);
            };
            if length < 20 {
                return Err(TrueTypeError::BadTableDirectory);
            }
            Ok(usize::from(
                read_u16(self.bytes, offset + 4).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
        }

        fn apply_gvar_tuples(
            &self,
            data: &[u8],
            coordinates: &[i16],
            points: &mut [OutlinePoint],
        ) -> Result<(), TrueTypeError> {
            const PRIVATE_POINTS: u16 = 0x2000;
            const INTERMEDIATE_REGION: u16 = 0x4000;
            const EMBEDDED_PEAK: u16 = 0x8000;
            const SHARED_POINTS: u16 = 0x8000;
            const MAX_TUPLES: usize = 64;
            const MAX_POINTS: usize = 128;
            if points.len() > MAX_POINTS || data.len() < 2 {
                return Err(TrueTypeError::TooLarge);
            }
            let tuple_count_word = read_u16(data, 0).ok_or(TrueTypeError::BadTableDirectory)?;
            if tuple_count_word & 0x7000 != 0 {
                return Err(TrueTypeError::UnsupportedVariation);
            }
            let tuple_count = usize::from(tuple_count_word & 0x0fff);
            if tuple_count > MAX_TUPLES {
                return Err(TrueTypeError::TooLarge);
            }
            #[derive(Clone, Copy)]
            struct Tuple {
                data_start: usize,
                data_end: usize,
                flags: u16,
                peak: [i16; 32],
                start: [i16; 32],
                end: [i16; 32],
            }
            let mut tuples = [Tuple {
                data_start: 0,
                data_end: 0,
                flags: 0,
                peak: [0; 32],
                start: [0; 32],
                end: [0; 32],
            }; MAX_TUPLES];
            let mut shared_points = [0usize; MAX_POINTS];
            let (shared_count, mut cursor) = if tuple_count_word & SHARED_POINTS != 0 {
                Self::decode_gvar_points(data, 2, data.len(), &mut shared_points, points.len())?
            } else {
                (0, 2)
            };
            for tuple in tuples.iter_mut().take(tuple_count) {
                let data_size =
                    usize::from(read_u16(data, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
                let flags = read_u16(data, cursor + 2).ok_or(TrueTypeError::BadTableDirectory)?;
                cursor = cursor
                    .checked_add(4)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if flags & EMBEDDED_PEAK == 0 {
                    return Err(TrueTypeError::UnsupportedVariation);
                }
                for peak in tuple.peak.iter_mut().take(coordinates.len()) {
                    *peak = read_i16(data, cursor).ok_or(TrueTypeError::BadTableDirectory)?;
                    cursor += 2;
                }
                if flags & INTERMEDIATE_REGION != 0 {
                    for start in tuple.start.iter_mut().take(coordinates.len()) {
                        *start = read_i16(data, cursor).ok_or(TrueTypeError::BadTableDirectory)?;
                        cursor += 2;
                    }
                    for end in tuple.end.iter_mut().take(coordinates.len()) {
                        *end = read_i16(data, cursor).ok_or(TrueTypeError::BadTableDirectory)?;
                        cursor += 2;
                    }
                }
                let data_start = cursor;
                let data_end = data_start
                    .checked_add(data_size)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if data_end > data.len() {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                tuple.data_start = data_start;
                tuple.data_end = data_end;
                tuple.flags = flags;
                cursor = data_end;
            }
            for tuple in tuples.iter().take(tuple_count) {
                let mut point_indices = [0usize; MAX_POINTS];
                let (point_count, mut delta_cursor) = if tuple.flags & PRIVATE_POINTS != 0 {
                    let (count, _) = Self::decode_gvar_points(
                        data,
                        tuple.data_start,
                        tuple.data_end,
                        &mut point_indices,
                        points.len(),
                    )?;
                    let end = Self::gvar_points_end(
                        data,
                        tuple.data_start,
                        tuple.data_end,
                        points.len(),
                    )?;
                    (count, end)
                } else if tuple_count_word & SHARED_POINTS != 0 {
                    point_indices[..shared_count].copy_from_slice(&shared_points[..shared_count]);
                    (shared_count, tuple.data_start)
                } else {
                    for (index, point) in point_indices.iter_mut().enumerate().take(points.len()) {
                        *point = index;
                    }
                    (points.len(), tuple.data_start)
                };
                let mut x_deltas = [0i16; MAX_POINTS];
                let mut y_deltas = [0i16; MAX_POINTS];
                delta_cursor = Self::decode_gvar_deltas(
                    data,
                    delta_cursor,
                    tuple.data_end,
                    point_count,
                    &mut x_deltas,
                )?;
                let _ = Self::decode_gvar_deltas(
                    data,
                    delta_cursor,
                    tuple.data_end,
                    point_count,
                    &mut y_deltas,
                )?;
                let scalar = Self::gvar_scalar(
                    coordinates,
                    &tuple.peak,
                    &tuple.start,
                    &tuple.end,
                    tuple.flags & INTERMEDIATE_REGION != 0,
                );
                for index in 0..point_count {
                    let point = point_indices[index];
                    let x = i64::from(points[point].x)
                        + (i64::from(x_deltas[index]) * i64::from(scalar)).div_euclid(16384);
                    let y = i64::from(points[point].y)
                        + (i64::from(y_deltas[index]) * i64::from(scalar)).div_euclid(16384);
                    points[point].x = i16::try_from(x).map_err(|_| TrueTypeError::TooLarge)?;
                    points[point].y = i16::try_from(y).map_err(|_| TrueTypeError::TooLarge)?;
                }
            }
            Ok(())
        }

        fn gvar_scalar(
            coordinates: &[i16],
            peak: &[i16; 32],
            start: &[i16; 32],
            end: &[i16; 32],
            intermediate: bool,
        ) -> i16 {
            let mut scalar = 16384i64;
            for (index, (coordinate, peak)) in coordinates.iter().zip(peak.iter()).enumerate() {
                let coordinate = i64::from(*coordinate);
                let peak = i64::from(*peak);
                if peak == 0 {
                    continue;
                }
                if intermediate {
                    let start = i64::from(start[index]);
                    let end = i64::from(end[index]);
                    if start > peak || peak > end || coordinate < start || coordinate > end {
                        return 0;
                    }
                    let axis_scalar = if coordinate < peak {
                        if peak == start {
                            16384
                        } else {
                            (coordinate - start) * 16384 / (peak - start)
                        }
                    } else if coordinate > peak {
                        if end == peak {
                            16384
                        } else {
                            (end - coordinate) * 16384 / (end - peak)
                        }
                    } else {
                        16384
                    };
                    scalar = scalar * axis_scalar.clamp(0, 16384) / 16384;
                    continue;
                }
                if coordinate == 0 || (coordinate < 0) != (peak < 0) {
                    return 0;
                }
                let magnitude = coordinate.abs();
                let peak_magnitude = peak.abs();
                if magnitude < peak_magnitude {
                    scalar = scalar * magnitude / peak_magnitude;
                }
            }
            scalar as i16
        }

        fn decode_gvar_points(
            data: &[u8],
            start: usize,
            end: usize,
            output: &mut [usize],
            point_limit: usize,
        ) -> Result<(usize, usize), TrueTypeError> {
            let count = usize::from(read_u16(data, start).ok_or(TrueTypeError::BadTableDirectory)?);
            if count == 0 {
                if point_limit > output.len() {
                    return Err(TrueTypeError::TooLarge);
                }
                for (index, point) in output.iter_mut().enumerate().take(point_limit) {
                    *point = index;
                }
                return Ok((point_limit, start + 2));
            }
            if count > output.len() || count > point_limit {
                return Err(TrueTypeError::TooLarge);
            }
            let mut cursor = start + 2;
            let mut previous = 0usize;
            let mut written = 0;
            while written < count {
                let control = *data.get(cursor).ok_or(TrueTypeError::BadTableDirectory)?;
                cursor += 1;
                let run = usize::from(control & 0x7f) + 1;
                if written + run > count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                for _ in 0..run {
                    let value = if control & 0x80 != 0 {
                        usize::from(read_u16(data, cursor).ok_or(TrueTypeError::BadTableDirectory)?)
                    } else {
                        usize::from(*data.get(cursor).ok_or(TrueTypeError::BadTableDirectory)?)
                    };
                    cursor += if control & 0x80 != 0 { 2 } else { 1 };
                    previous = previous.checked_add(value).ok_or(TrueTypeError::TooLarge)?;
                    if previous >= point_limit {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    output[written] = previous;
                    written += 1;
                }
            }
            if cursor > end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            Ok((written, cursor))
        }

        fn gvar_points_end(
            data: &[u8],
            start: usize,
            end: usize,
            point_limit: usize,
        ) -> Result<usize, TrueTypeError> {
            let count = usize::from(read_u16(data, start).ok_or(TrueTypeError::BadTableDirectory)?);
            if count == 0 {
                return Ok(start + 2);
            }
            let mut cursor = start + 2;
            let mut written = 0;
            while written < count {
                let control = *data.get(cursor).ok_or(TrueTypeError::BadTableDirectory)?;
                cursor += 1;
                let run = usize::from(control & 0x7f) + 1;
                written = written.checked_add(run).ok_or(TrueTypeError::TooLarge)?;
                cursor = cursor
                    .checked_add(if control & 0x80 != 0 { run * 2 } else { run })
                    .ok_or(TrueTypeError::TooLarge)?;
                if written > point_limit || cursor > end {
                    return Err(TrueTypeError::BadTableDirectory);
                }
            }
            Ok(cursor)
        }

        fn decode_gvar_deltas(
            data: &[u8],
            start: usize,
            end: usize,
            count: usize,
            output: &mut [i16],
        ) -> Result<usize, TrueTypeError> {
            let mut cursor = start;
            let mut written = 0;
            while written < count {
                let control = *data.get(cursor).ok_or(TrueTypeError::BadTableDirectory)?;
                cursor += 1;
                let run = usize::from(control & 0x3f) + 1;
                if written + run > count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                for slot in output.iter_mut().skip(written).take(run) {
                    *slot = if control & 0x40 != 0 {
                        0
                    } else if control & 0x80 != 0 {
                        let value =
                            read_i16(data, cursor).ok_or(TrueTypeError::BadTableDirectory)?;
                        cursor += 2;
                        value
                    } else {
                        let value = i8::from_ne_bytes([*data
                            .get(cursor)
                            .ok_or(TrueTypeError::BadTableDirectory)?]);
                        cursor += 1;
                        i16::from(value)
                    };
                }
                written += run;
            }
            if cursor > end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            Ok(cursor)
        }

        /// Decode one simple or bounded composite TrueType glyph into
        /// caller-owned outline points. Malformed or oversized data fails
        /// closed; no font bytes are retained by the output.
        pub fn outline_points(
            &self,
            character: char,
            output: &mut [OutlinePoint],
        ) -> Result<usize, TrueTypeError> {
            let glyph = self
                .glyph_index(character)
                .ok_or(TrueTypeError::UnsupportedGlyph)?;
            self.outline_points_glyph(glyph, output)
        }

        /// Decode a validated glyph ID directly, including glyphs introduced
        /// by OpenType substitutions and not addressable by a Unicode scalar.
        pub fn outline_points_glyph(
            &self,
            glyph: u16,
            output: &mut [OutlinePoint],
        ) -> Result<usize, TrueTypeError> {
            if glyph >= self.glyph_count {
                return Err(TrueTypeError::UnsupportedGlyph);
            }
            self.outline_points_for_glyph(usize::from(glyph), output, 0)
        }

        fn outline_points_for_glyph(
            &self,
            glyph: usize,
            output: &mut [OutlinePoint],
            depth: usize,
        ) -> Result<usize, TrueTypeError> {
            self.outline_points_for_glyph_variation(glyph, output, depth, None)
        }

        fn outline_points_for_glyph_variation(
            &self,
            glyph: usize,
            output: &mut [OutlinePoint],
            depth: usize,
            variation: Option<&[i16]>,
        ) -> Result<usize, TrueTypeError> {
            let (glyf_offset, glyf_length) = self.glyf.ok_or(TrueTypeError::UnsupportedGlyph)?;
            let (loca_offset, loca_length) = self.loca.ok_or(TrueTypeError::UnsupportedGlyph)?;
            let loca = |index: usize| -> Option<usize> {
                if index > usize::from(self.glyph_count) {
                    return None;
                }
                if self.index_to_loc_format == 0 {
                    let offset = loca_offset.checked_add(index.checked_mul(2)?)?;
                    if offset.checked_add(2)? > loca_offset + loca_length {
                        return None;
                    }
                    Some(usize::from(read_u16(self.bytes, offset)?) * 2)
                } else {
                    let offset = loca_offset.checked_add(index.checked_mul(4)?)?;
                    if offset.checked_add(4)? > loca_offset + loca_length {
                        return None;
                    }
                    usize::try_from(read_u32(self.bytes, offset)?).ok()
                }
            };
            let start = loca(glyph).ok_or(TrueTypeError::BadTableDirectory)?;
            let end = loca(glyph + 1).ok_or(TrueTypeError::BadTableDirectory)?;
            if start > end || end > glyf_length {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if start == end {
                return Ok(0);
            }
            let glyph_start = glyf_offset
                .checked_add(start)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let glyph_end = glyf_offset
                .checked_add(end)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if glyph_end > self.bytes.len() || glyph_end - glyph_start < 10 {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let contours =
                read_i16(self.bytes, glyph_start).ok_or(TrueTypeError::BadTableDirectory)?;
            if contours < 0 {
                if depth >= 8 {
                    return Err(TrueTypeError::TooLarge);
                }
                return self.composite_points(
                    glyph,
                    glyph_start,
                    glyph_end,
                    output,
                    depth + 1,
                    variation,
                );
            }
            let contours =
                usize::try_from(contours).map_err(|_| TrueTypeError::BadTableDirectory)?;
            if contours == 0 {
                return Ok(0);
            }
            let endpoints_end = glyph_start
                .checked_add(
                    10usize
                        .checked_add(contours.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::TooLarge)?;
            if endpoints_end + 2 > glyph_end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let last_point = usize::from(
                read_u16(self.bytes, endpoints_end - 2).ok_or(TrueTypeError::BadTableDirectory)?,
            ) + 1;
            const MAX_POINTS: usize = 128;
            if last_point > MAX_POINTS || last_point > output.len() {
                return Err(TrueTypeError::TooLarge);
            }
            let instructions = usize::from(
                read_u16(self.bytes, endpoints_end).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let flags_start = endpoints_end
                .checked_add(
                    2usize
                        .checked_add(instructions)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::TooLarge)?;
            if flags_start > glyph_end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let mut flags = [0u8; MAX_POINTS];
            let mut cursor = flags_start;
            let mut point = 0;
            while point < last_point {
                let flag = *self
                    .bytes
                    .get(cursor)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                cursor += 1;
                let repeat = if flag & 0x08 != 0 {
                    let count = usize::from(
                        *self
                            .bytes
                            .get(cursor)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    cursor += 1;
                    count
                } else {
                    0
                };
                for _ in 0..=repeat {
                    if point >= last_point {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    flags[point] = flag;
                    point += 1;
                }
            }
            let mut x = 0i32;
            let mut x_coordinates = [0i16; MAX_POINTS];
            for index in 0..last_point {
                let flag = flags[index];
                let dx = if flag & 0x02 != 0 {
                    let value = i32::from(
                        *self
                            .bytes
                            .get(cursor)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    cursor += 1;
                    if flag & 0x10 != 0 { value } else { -value }
                } else if flag & 0x10 != 0 {
                    0
                } else {
                    let value = i32::from(
                        read_i16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    cursor += 2;
                    value
                };
                x = x.checked_add(dx).ok_or(TrueTypeError::TooLarge)?;
                x_coordinates[index] = i16::try_from(x).map_err(|_| TrueTypeError::TooLarge)?;
            }
            let mut y = 0i32;
            let mut y_coordinates = [0i16; MAX_POINTS];
            for index in 0..last_point {
                let flag = flags[index];
                let dy = if flag & 0x04 != 0 {
                    let value = i32::from(
                        *self
                            .bytes
                            .get(cursor)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    cursor += 1;
                    if flag & 0x20 != 0 { value } else { -value }
                } else if flag & 0x20 != 0 {
                    0
                } else {
                    let value = i32::from(
                        read_i16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    cursor += 2;
                    value
                };
                y = y.checked_add(dy).ok_or(TrueTypeError::TooLarge)?;
                y_coordinates[index] = i16::try_from(y).map_err(|_| TrueTypeError::TooLarge)?;
            }
            for index in 0..last_point {
                let flag = flags[index];
                output[index] = OutlinePoint {
                    x: x_coordinates[index],
                    y: y_coordinates[index],
                    on_curve: flag & 0x01 != 0,
                    contour_end: (0..contours).any(|contour| {
                        read_u16(self.bytes, glyph_start + 10 + contour * 2)
                            .is_some_and(|end_point| usize::from(end_point) == index)
                    }),
                };
            }
            if cursor > glyph_end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if let Some(coordinates) = variation
                && let Some(data) = self.glyph_variation_data(glyph as u16)?
                && !data.is_empty()
            {
                let variation_point_count =
                    last_point.checked_add(4).ok_or(TrueTypeError::TooLarge)?;
                let mut varied_points = [OutlinePoint {
                    x: 0,
                    y: 0,
                    on_curve: false,
                    contour_end: false,
                }; 132];
                varied_points[..last_point].copy_from_slice(&output[..last_point]);
                self.apply_gvar_tuples(
                    data,
                    coordinates,
                    &mut varied_points[..variation_point_count],
                )?;
                output[..last_point].copy_from_slice(&varied_points[..last_point]);
            }
            Ok(last_point)
        }

        fn composite_points(
            &self,
            glyph: usize,
            glyph_start: usize,
            glyph_end: usize,
            output: &mut [OutlinePoint],
            depth: usize,
            variation: Option<&[i16]>,
        ) -> Result<usize, TrueTypeError> {
            const ARG_WORDS: u16 = 0x0001;
            const ARG_XY: u16 = 0x0002;
            const MORE: u16 = 0x0020;
            const SCALE: u16 = 0x0008;
            const XY_SCALE: u16 = 0x0040;
            const TWO_BY_TWO: u16 = 0x0080;
            const INSTRUCTIONS: u16 = 0x0100;
            let mut cursor = glyph_start
                .checked_add(10)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let mut written = 0;
            const MAX_COMPONENTS: usize = 32;
            let mut component_starts = [0usize; MAX_COMPONENTS];
            let mut component_ends = [0usize; MAX_COMPONENTS];
            let mut component_total = 0usize;
            let final_flags = loop {
                if cursor
                    .checked_add(4)
                    .ok_or(TrueTypeError::BadTableDirectory)?
                    > glyph_end
                {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let flags = read_u16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?;
                let component = usize::from(
                    read_u16(self.bytes, cursor + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                cursor += 4;
                let xy_arguments = flags & ARG_XY != 0;
                let (arg1, arg2) = if flags & ARG_WORDS != 0 {
                    if cursor
                        .checked_add(4)
                        .ok_or(TrueTypeError::BadTableDirectory)?
                        > glyph_end
                    {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let arg1 = if xy_arguments {
                        i32::from(
                            read_i16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    } else {
                        i32::from(
                            read_u16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    };
                    let arg2 = if xy_arguments {
                        i32::from(
                            read_i16(self.bytes, cursor + 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    } else {
                        i32::from(
                            read_u16(self.bytes, cursor + 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    };
                    cursor += 4;
                    (arg1, arg2)
                } else {
                    if cursor
                        .checked_add(2)
                        .ok_or(TrueTypeError::BadTableDirectory)?
                        > glyph_end
                    {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let arg1 = if xy_arguments {
                        i8::from_ne_bytes([self.bytes[cursor]]) as i32
                    } else {
                        i32::from(self.bytes[cursor])
                    };
                    let arg2 = if xy_arguments {
                        i8::from_ne_bytes([self.bytes[cursor + 1]]) as i32
                    } else {
                        i32::from(self.bytes[cursor + 1])
                    };
                    cursor += 2;
                    (arg1, arg2)
                };
                let mut dx = if xy_arguments { arg1 } else { 0 };
                let mut dy = if xy_arguments { arg2 } else { 0 };
                let mut xx = 16384i32;
                let mut xy = 0i32;
                let mut yx = 0i32;
                let mut yy = 16384i32;
                if flags & TWO_BY_TWO != 0 {
                    if cursor
                        .checked_add(8)
                        .ok_or(TrueTypeError::BadTableDirectory)?
                        > glyph_end
                    {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    xx = i32::from(
                        read_i16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    yx = i32::from(
                        read_i16(self.bytes, cursor + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    xy = i32::from(
                        read_i16(self.bytes, cursor + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    yy = i32::from(
                        read_i16(self.bytes, cursor + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    cursor += 8;
                } else if flags & XY_SCALE != 0 {
                    if cursor
                        .checked_add(4)
                        .ok_or(TrueTypeError::BadTableDirectory)?
                        > glyph_end
                    {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    xx = i32::from(
                        read_i16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    yy = i32::from(
                        read_i16(self.bytes, cursor + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    cursor += 4;
                } else if flags & SCALE != 0 {
                    if cursor
                        .checked_add(2)
                        .ok_or(TrueTypeError::BadTableDirectory)?
                        > glyph_end
                    {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    xx = i32::from(
                        read_i16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    yy = xx;
                    cursor += 2;
                }
                let mut component_points = [OutlinePoint {
                    x: 0,
                    y: 0,
                    on_curve: false,
                    contour_end: false,
                }; 128];
                let component_count = self.outline_points_for_glyph_variation(
                    component,
                    &mut component_points,
                    depth,
                    variation,
                )?;
                if !xy_arguments {
                    let parent_index =
                        usize::try_from(arg1).map_err(|_| TrueTypeError::BadTableDirectory)?;
                    let component_index =
                        usize::try_from(arg2).map_err(|_| TrueTypeError::BadTableDirectory)?;
                    if parent_index >= written || component_index >= component_count {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let anchor = component_points[component_index];
                    let transformed_x = (i64::from(xx) * i64::from(anchor.x)
                        + i64::from(xy) * i64::from(anchor.y))
                    .div_euclid(16384);
                    let transformed_y = (i64::from(yx) * i64::from(anchor.x)
                        + i64::from(yy) * i64::from(anchor.y))
                    .div_euclid(16384);
                    dx = i64::from(output[parent_index].x)
                        .checked_sub(transformed_x)
                        .ok_or(TrueTypeError::TooLarge)?
                        .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                        as i32;
                    dy = i64::from(output[parent_index].y)
                        .checked_sub(transformed_y)
                        .ok_or(TrueTypeError::TooLarge)?
                        .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                        as i32;
                }
                if component_total >= MAX_COMPONENTS {
                    return Err(TrueTypeError::TooLarge);
                }
                component_starts[component_total] = written;
                for point in component_points.iter().take(component_count) {
                    if written >= output.len() {
                        return Err(TrueTypeError::TooLarge);
                    }
                    let x = (i64::from(xx) * i64::from(point.x)
                        + i64::from(xy) * i64::from(point.y))
                    .div_euclid(16384)
                    .checked_add(i64::from(dx))
                    .ok_or(TrueTypeError::TooLarge)?;
                    let y = (i64::from(yx) * i64::from(point.x)
                        + i64::from(yy) * i64::from(point.y))
                    .div_euclid(16384)
                    .checked_add(i64::from(dy))
                    .ok_or(TrueTypeError::TooLarge)?;
                    output[written] = OutlinePoint {
                        x: i16::try_from(x).map_err(|_| TrueTypeError::TooLarge)?,
                        y: i16::try_from(y).map_err(|_| TrueTypeError::TooLarge)?,
                        on_curve: point.on_curve,
                        contour_end: point.contour_end,
                    };
                    written += 1;
                }
                component_ends[component_total] = written;
                component_total += 1;
                if flags & MORE == 0 {
                    break flags;
                }
            };
            if final_flags & INSTRUCTIONS != 0 {
                let instruction_length = usize::from(
                    read_u16(self.bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                cursor = cursor
                    .checked_add(2)
                    .and_then(|value| value.checked_add(instruction_length))
                    .ok_or(TrueTypeError::TooLarge)?;
            }
            if cursor > glyph_end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if let Some(coordinates) = variation
                && let Some(data) = self.glyph_variation_data(glyph as u16)?
                && !data.is_empty()
            {
                let variation_point_count = component_total
                    .checked_add(4)
                    .ok_or(TrueTypeError::TooLarge)?;
                let mut component_offsets = [OutlinePoint {
                    x: 0,
                    y: 0,
                    on_curve: false,
                    contour_end: false,
                }; 128];
                self.apply_gvar_tuples(
                    data,
                    coordinates,
                    &mut component_offsets[..variation_point_count],
                )?;
                for component in 0..component_total {
                    for point in output
                        .iter_mut()
                        .take(component_ends[component])
                        .skip(component_starts[component])
                    {
                        point.x = point
                            .x
                            .checked_add(component_offsets[component].x)
                            .ok_or(TrueTypeError::TooLarge)?;
                        point.y = point
                            .y
                            .checked_add(component_offsets[component].y)
                            .ok_or(TrueTypeError::TooLarge)?;
                    }
                }
            }
            Ok(written)
        }

        /// Assemble one simple-glyph contour into a bounded path.
        ///
        /// `origin` is the baseline origin in canvas coordinates and `scale`
        /// is the requested pixel height per em. TrueType's positive Y axis
        /// is inverted into canvas coordinates. The caller selects a contour
        /// because `Path` deliberately remains a single closed subpath; a
        /// later scene/path slice can add multi-contour fill semantics.
        pub fn outline_contour(
            &self,
            character: char,
            contour: usize,
            origin: Point,
            scale: u16,
            output: &mut Path,
        ) -> Result<usize, TrueTypeError> {
            let glyph = self
                .glyph_index(character)
                .ok_or(TrueTypeError::UnsupportedGlyph)?;
            self.outline_contour_glyph(glyph, contour, origin, scale, output)
        }

        /// Assemble one contour from a direct glyph ID.
        pub fn outline_contour_glyph(
            &self,
            glyph: u16,
            contour: usize,
            origin: Point,
            scale: u16,
            output: &mut Path,
        ) -> Result<usize, TrueTypeError> {
            self.outline_contour_glyph_variation(glyph, contour, origin, scale, output, None)
        }

        /// Assemble one contour from a glyph ID with normalized variation
        /// coordinates applied to its outline.
        pub fn outline_contour_glyph_with_variation(
            &self,
            glyph: u16,
            contour: usize,
            origin: Point,
            scale: u16,
            normalized_coordinates: &[i16],
            output: &mut Path,
        ) -> Result<usize, TrueTypeError> {
            self.outline_contour_glyph_variation(
                glyph,
                contour,
                origin,
                scale,
                output,
                Some(normalized_coordinates),
            )
        }

        fn outline_contour_glyph_variation(
            &self,
            glyph: u16,
            contour: usize,
            origin: Point,
            scale: u16,
            output: &mut Path,
            variation: Option<&[i16]>,
        ) -> Result<usize, TrueTypeError> {
            let mut points = [OutlinePoint {
                x: 0,
                y: 0,
                on_curve: false,
                contour_end: false,
            }; 128];
            let point_count = self.outline_points_for_glyph_variation(
                usize::from(glyph),
                &mut points,
                0,
                variation,
            )?;
            if point_count == 0 {
                *output = Path::new();
                return Ok(0);
            }
            let mut start = 0;
            let mut contour_index = 0;
            let mut selected = None;
            for (index, point) in points.iter().enumerate().take(point_count) {
                if point.contour_end {
                    if contour == contour_index {
                        selected = Some((start, index));
                        break;
                    }
                    start = index + 1;
                    contour_index += 1;
                }
            }
            let (start, end) = selected.ok_or(TrueTypeError::UnsupportedGlyph)?;
            if start > end {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let scale = i64::from(scale.max(1));
            let units = i64::from(self.units_per_em.max(1));
            let map = |point: OutlinePoint| -> Point {
                let x = i64::from(origin.x) + (i64::from(point.x) * scale).div_euclid(units);
                let y = i64::from(origin.y) - (i64::from(point.y) * scale).div_euclid(units);
                Point::new(
                    x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                    y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                )
            };
            let midpoint = |a: OutlinePoint, b: OutlinePoint| -> OutlinePoint {
                OutlinePoint {
                    x: ((i32::from(a.x) + i32::from(b.x)) / 2) as i16,
                    y: ((i32::from(a.y) + i32::from(b.y)) / 2) as i16,
                    on_curve: true,
                    contour_end: false,
                }
            };
            let first = points[start];
            let last = points[end];
            let (initial, mut index) = if first.on_curve {
                (first, start + 1)
            } else if last.on_curve {
                (last, start)
            } else {
                (midpoint(last, first), start)
            };
            let mut path = Path::new();
            if !path.move_to(map(initial)) {
                return Err(TrueTypeError::TooLarge);
            }
            while index <= end {
                let point = points[index];
                if point.on_curve {
                    if !path.line_to(map(point)) {
                        return Err(TrueTypeError::TooLarge);
                    }
                    index += 1;
                    continue;
                }
                let next_index = if index == end { start } else { index + 1 };
                let next = points[next_index];
                if next.on_curve {
                    if !path.quadratic_to(map(point), map(next)) {
                        return Err(TrueTypeError::TooLarge);
                    }
                    index += 2;
                } else {
                    let implied = midpoint(point, next);
                    if !path.quadratic_to(map(point), map(implied)) {
                        return Err(TrueTypeError::TooLarge);
                    }
                    index += 1;
                }
            }
            if !path.close() {
                return Err(TrueTypeError::TooLarge);
            }
            *output = path;
            Ok(end - start + 1)
        }

        /// Assemble all simple contours of a glyph into a bounded compound
        /// path. Compound TrueType glyph components remain unsupported.
        pub fn outline_compound(
            &self,
            character: char,
            origin: Point,
            scale: u16,
            output: &mut CompoundPath,
        ) -> Result<usize, TrueTypeError> {
            let glyph = self
                .glyph_index(character)
                .ok_or(TrueTypeError::UnsupportedGlyph)?;
            self.outline_compound_glyph(glyph, origin, scale, output)
        }

        /// Assemble all contours from a direct glyph ID.
        pub fn outline_compound_glyph(
            &self,
            glyph: u16,
            origin: Point,
            scale: u16,
            output: &mut CompoundPath,
        ) -> Result<usize, TrueTypeError> {
            self.outline_compound_glyph_variation(glyph, origin, scale, output, None)
        }

        /// Assemble all contours from a glyph ID with normalized variation
        /// coordinates applied to each component and outline.
        pub fn outline_compound_glyph_with_variation(
            &self,
            glyph: u16,
            origin: Point,
            scale: u16,
            normalized_coordinates: &[i16],
            output: &mut CompoundPath,
        ) -> Result<usize, TrueTypeError> {
            self.outline_compound_glyph_variation(
                glyph,
                origin,
                scale,
                output,
                Some(normalized_coordinates),
            )
        }

        fn outline_compound_glyph_variation(
            &self,
            glyph: u16,
            origin: Point,
            scale: u16,
            output: &mut CompoundPath,
            variation: Option<&[i16]>,
        ) -> Result<usize, TrueTypeError> {
            let mut compound = CompoundPath::new();
            for contour in 0..=MAX_CONTOURS {
                let mut path = Path::new();
                match self.outline_contour_glyph_variation(
                    glyph, contour, origin, scale, &mut path, variation,
                ) {
                    Ok(0) if contour == 0 => {
                        *output = compound;
                        return Ok(0);
                    }
                    Ok(_) if contour == MAX_CONTOURS => return Err(TrueTypeError::TooLarge),
                    Ok(_) => {
                        if !compound.push(path) {
                            return Err(TrueTypeError::TooLarge);
                        }
                    }
                    Err(TrueTypeError::UnsupportedGlyph) if contour > 0 => break,
                    Err(error) => return Err(error),
                }
            }
            if compound.is_empty() {
                return Err(TrueTypeError::UnsupportedGlyph);
            }
            let contours = compound.len();
            *output = compound;
            Ok(contours)
        }

        /// Rasterize one simple TrueType glyph through the bounded
        /// antialiased compound-path renderer.
        pub fn render_glyph(
            &self,
            character: char,
            origin: Point,
            scale: u16,
            color: Color,
            renderer: &mut Renderer<'_, '_>,
        ) -> Result<usize, TrueTypeError> {
            let glyph = self
                .glyph_index(character)
                .ok_or(TrueTypeError::UnsupportedGlyph)?;
            self.render_glyph_id(glyph, origin, scale, color, renderer)
        }

        /// Rasterize a direct glyph ID, including a glyph selected by GSUB.
        pub fn render_glyph_id(
            &self,
            glyph: u16,
            origin: Point,
            scale: u16,
            color: Color,
            renderer: &mut Renderer<'_, '_>,
        ) -> Result<usize, TrueTypeError> {
            let mut compound = CompoundPath::new();
            let contours = self.outline_compound_glyph(glyph, origin, scale, &mut compound)?;
            if contours != 0 && !renderer.fill_compound_path_antialiased(&compound, color) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            Ok(contours)
        }

        /// Rasterize a direct glyph ID with normalized variation coordinates.
        pub fn render_glyph_id_with_variation(
            &self,
            glyph: u16,
            origin: Point,
            scale: u16,
            normalized_coordinates: &[i16],
            color: Color,
            renderer: &mut Renderer<'_, '_>,
        ) -> Result<usize, TrueTypeError> {
            let mut compound = CompoundPath::new();
            let contours = self.outline_compound_glyph_with_variation(
                glyph,
                origin,
                scale,
                normalized_coordinates,
                &mut compound,
            )?;
            if contours != 0 && !renderer.fill_compound_path_antialiased(&compound, color) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            Ok(contours)
        }

        /// Lay out one single-line string using checked horizontal metrics.
        /// Kerning, shaping and line breaking are intentionally separate
        /// stages; unsupported characters fail without partial output.
        pub fn layout_text(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            output: &mut [GlyphPlacement],
        ) -> Result<usize, TrueTypeError> {
            let scale = i64::from(scale.max(1));
            let units = i64::from(self.units_per_em.max(1));
            let mut cursor = origin.x;
            let mut count = 0;
            let mut previous = None;
            for character in text.chars() {
                if character == '\n' || count >= output.len() {
                    return Err(TrueTypeError::TooLarge);
                }
                let advance = self
                    .advance_width(character)
                    .ok_or(TrueTypeError::UnsupportedGlyph)?;
                if let Some(previous) = previous
                    && let Some(kern) = self.kerning(previous, character)
                {
                    let adjustment = (i64::from(kern) * scale).div_euclid(units);
                    cursor = cursor.saturating_add(
                        adjustment.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                    );
                }
                let pixels = (i64::from(advance) * scale)
                    .div_euclid(units)
                    .clamp(1, i64::from(i32::MAX)) as i32;
                output[count] = GlyphPlacement {
                    character,
                    origin: Point::new(cursor, origin.y),
                    advance: pixels,
                };
                cursor = cursor.saturating_add(pixels);
                previous = Some(character);
                count += 1;
            }
            Ok(count)
        }

        /// Lay out a single line in an explicit direction. RTL mirrors the
        /// bounded placements; it does not claim Unicode bidi or shaping.
        pub fn layout_text_direction(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            direction: TextDirection,
            output: &mut [GlyphPlacement],
        ) -> Result<usize, TrueTypeError> {
            let count = self.layout_text(text, origin, scale, output)?;
            if direction == TextDirection::Ltr || count == 0 {
                return Ok(count);
            }
            let end = output[count - 1]
                .origin
                .x
                .saturating_add(output[count - 1].advance);
            let width = end.saturating_sub(origin.x);
            let mut consumed = 0;
            for index in 0..count {
                let source = count - 1 - index;
                let advance = output[source].advance;
                let mut placement = output[source];
                placement.origin = Point::new(
                    origin
                        .x
                        .saturating_add(width)
                        .saturating_sub(consumed)
                        .saturating_sub(advance),
                    origin.y,
                );
                output[index] = placement;
                consumed = consumed.saturating_add(advance);
            }
            Ok(count)
        }

        /// Resolve strong directional runs and neutrals, then emit visual
        /// run order into caller-owned placements. This bounded pass handles
        /// common Hebrew/Arabic and left-to-right text, but does not claim
        /// the full Unicode Bidirectional Algorithm or script shaping.
        pub fn layout_text_bidi(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            base_direction: TextDirection,
            output: &mut [GlyphPlacement],
        ) -> Result<usize, TrueTypeError> {
            let mut logical = [GlyphPlacement {
                character: '\0',
                origin: Point::new(0, 0),
                advance: 0,
            }; 128];
            let count = self.layout_text(text, origin, scale, &mut logical)?;
            if count == 0 {
                return Ok(0);
            }
            if output.len() < count {
                return Err(TrueTypeError::TooLarge);
            }

            let mut directions = [base_direction; 128];
            let mut strong = [None; 128];
            for index in 0..count {
                strong[index] = TextDirection::classify(logical[index].character);
            }
            for index in 0..count {
                if let Some(direction) = strong[index] {
                    directions[index] = direction;
                    continue;
                }
                let previous = (0..index).rev().find_map(|candidate| strong[candidate]);
                let next = ((index + 1)..count).find_map(|candidate| strong[candidate]);
                directions[index] = if previous.is_some() && previous == next {
                    previous.unwrap_or(base_direction)
                } else {
                    base_direction
                };
            }

            let mut order = [0usize; 128];
            let mut order_count = 0;
            let mut emit_run = |start: usize, end: usize, direction: TextDirection| {
                if direction == TextDirection::Rtl {
                    for index in (start..end).rev() {
                        order[order_count] = index;
                        order_count += 1;
                    }
                } else {
                    for index in start..end {
                        order[order_count] = index;
                        order_count += 1;
                    }
                }
            };
            if base_direction == TextDirection::Ltr {
                let mut start = 0;
                while start < count {
                    let direction = directions[start];
                    let mut end = start + 1;
                    while end < count && directions[end] == direction {
                        end += 1;
                    }
                    emit_run(start, end, direction);
                    start = end;
                }
            } else {
                let mut end = count;
                while end != 0 {
                    let direction = directions[end - 1];
                    let mut start = end - 1;
                    while start != 0 && directions[start - 1] == direction {
                        start -= 1;
                    }
                    emit_run(start, end, direction);
                    end = start;
                }
            }

            let mut cursor = origin.x;
            for (index, source) in order[..order_count].iter().copied().enumerate() {
                let mut placement = logical[source];
                placement.origin = Point::new(cursor, origin.y);
                cursor = cursor.saturating_add(placement.advance);
                output[index] = placement;
            }
            Ok(order_count)
        }

        fn ligature_substitution(
            &self,
            glyphs: &[u16],
            position: usize,
            script_tag: Option<[u8; 4]>,
            language_tag: Option<[u8; 4]>,
        ) -> Result<Option<GsubSubstitution>, TrueTypeError> {
            let Some((table, length)) = self.gsub else {
                return Ok(None);
            };
            if length < 10
                || table
                    .checked_add(length)
                    .is_none_or(|end| end > self.bytes.len())
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if read_u32(self.bytes, table) != Some(0x0001_0000) {
                return Err(TrueTypeError::UnsupportedCmap);
            }
            let script = usize::from(read_u16(self.bytes, table + 4).unwrap_or(0));
            let feature = usize::from(
                read_u16(self.bytes, table + 6).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let lookup = usize::from(
                read_u16(self.bytes, table + 8).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let feature_base = table
                .checked_add(feature)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let feature_end = table
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if script != 0
                && table
                    .checked_add(script)
                    .is_none_or(|end| end > feature_end)
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if feature_base
                .checked_add(2)
                .is_none_or(|end| end > feature_end)
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let feature_count = usize::from(
                read_u16(self.bytes, feature_base).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if feature_count > 64
                || feature_base
                    .checked_add(
                        2usize
                            .checked_add(
                                feature_count
                                    .checked_mul(6)
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > feature_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            let mut feature_indices = [0u16; 64];
            let mut feature_index_count = 0usize;
            if let Some(script_tag) = script_tag {
                if script == 0 {
                    return Ok(None);
                }
                let script_list = table
                    .checked_add(script)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let script_count = usize::from(
                    read_u16(self.bytes, script_list).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if script_count > 32
                    || script_list
                        .checked_add(
                            2usize
                                .checked_add(
                                    script_count.checked_mul(6).ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > feature_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                let mut selected_script = None;
                for index in 0..script_count {
                    let record = script_list + 2 + index * 6;
                    if self
                        .bytes
                        .get(record..record + 4)
                        .ok_or(TrueTypeError::BadTableDirectory)?
                        == script_tag
                    {
                        selected_script = Some(
                            script_list
                                .checked_add(usize::from(
                                    read_u16(self.bytes, record + 4)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        break;
                    }
                }
                let Some(script_table) = selected_script else {
                    return Ok(None);
                };
                let Some(lang_sys) =
                    layout_lang_sys(self.bytes, script_table, language_tag, feature_end)?
                else {
                    return Ok(None);
                };
                let required =
                    read_u16(self.bytes, lang_sys + 2).ok_or(TrueTypeError::BadTableDirectory)?;
                let count = usize::from(
                    read_u16(self.bytes, lang_sys + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if count > 63
                    || lang_sys
                        .checked_add(
                            6usize
                                .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > feature_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                if required != u16::MAX {
                    feature_indices[feature_index_count] = required;
                    feature_index_count += 1;
                }
                for index in 0..count {
                    feature_indices[feature_index_count] =
                        read_u16(self.bytes, lang_sys + 6 + index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                    feature_index_count += 1;
                }
            } else {
                for index in 0..feature_count {
                    feature_indices[feature_index_count] =
                        u16::try_from(index).map_err(|_| TrueTypeError::TooLarge)?;
                    feature_index_count += 1;
                }
            }
            let mut lookup_indices = [0u16; 32];
            let mut lookup_count = 0usize;
            for feature_index in feature_indices.iter().copied().take(feature_index_count) {
                let feature_index = usize::from(feature_index);
                if feature_index >= feature_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let record = feature_base + 2 + feature_index * 6;
                let tag = self
                    .bytes
                    .get(record..record + 4)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if tag != b"liga"
                    && tag != b"rlig"
                    && tag != b"ccmp"
                    && tag != b"locl"
                    && tag != b"smcp"
                    && tag != b"subs"
                    && tag != b"sups"
                    && tag != b"case"
                    && tag != b"cpsp"
                    && tag != b"calt"
                    && tag != b"clig"
                {
                    continue;
                }
                let feature_offset = usize::from(
                    read_u16(self.bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let feature_table = feature_base
                    .checked_add(feature_offset)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if feature_table
                    .checked_add(4)
                    .is_none_or(|end| end > feature_end)
                {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let count = usize::from(
                    read_u16(self.bytes, feature_table + 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if count > 32
                    || feature_table
                        .checked_add(
                            4usize
                                .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > feature_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                if lookup_count.checked_add(count).is_none_or(|end| end > 32) {
                    return Err(TrueTypeError::TooLarge);
                }
                for item in 0..count {
                    lookup_indices[lookup_count] =
                        read_u16(self.bytes, feature_table + 4 + item * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                    lookup_count += 1;
                }
            }
            if lookup_count == 0 {
                return Ok(None);
            }
            let lookup_base = table
                .checked_add(lookup)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if lookup_base
                .checked_add(2)
                .is_none_or(|end| end > feature_end)
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let lookup_total = usize::from(
                read_u16(self.bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if lookup_total > 128
                || lookup_base
                    .checked_add(
                        2usize
                            .checked_add(
                                lookup_total.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > feature_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for selected in lookup_indices.iter().copied().take(lookup_count) {
                let selected = usize::from(selected);
                if selected >= lookup_total {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let lookup_table = lookup_base
                    .checked_add(usize::from(
                        read_u16(self.bytes, lookup_base + 2 + selected * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if lookup_table
                    .checked_add(6)
                    .is_none_or(|end| end > feature_end)
                {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let lookup_type =
                    read_u16(self.bytes, lookup_table).ok_or(TrueTypeError::BadTableDirectory)?;
                if lookup_type == 7 {
                    let subtable_count = usize::from(
                        read_u16(self.bytes, lookup_table + 4)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if subtable_count > 32
                        || lookup_table
                            .checked_add(
                                6usize
                                    .checked_add(
                                        subtable_count
                                            .checked_mul(2)
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .is_none_or(|end| end > feature_end)
                    {
                        return Err(TrueTypeError::TooLarge);
                    }
                    for subtable_index in 0..subtable_count {
                        let extension = lookup_table
                            .checked_add(usize::from(
                                read_u16(self.bytes, lookup_table + 6 + subtable_index * 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if extension.checked_add(8).is_none_or(|end| end > feature_end)
                            || read_u16(self.bytes, extension) != Some(1)
                        {
                            continue;
                        }
                        let extension_type = read_u16(self.bytes, extension + 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if extension_type != 1
                            && extension_type != 2
                            && extension_type != 3
                            && extension_type != 4
                            && extension_type != 5
                            && extension_type != 6
                            && extension_type != 8
                        {
                            continue;
                        }
                        let offset = usize::try_from(
                            read_u32(self.bytes, extension + 4)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                        .map_err(|_| TrueTypeError::TooLarge)?;
                        let subtable = extension
                            .checked_add(offset)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if extension_type == 1 {
                            if let Some(substitute) = gsub_single_subtable(
                                self.bytes,
                                subtable,
                                glyphs[position],
                                feature_end,
                            )? {
                                return Ok(Some(GsubSubstitution::Single {
                                    glyph: substitute,
                                    consumed: 1,
                                }));
                            }
                        } else if extension_type == 2 {
                            if let Some((glyphs, count)) = gsub_multiple_subtable(
                                self.bytes,
                                subtable,
                                glyphs[position],
                                feature_end,
                            )? {
                                return Ok(Some(GsubSubstitution::Multiple { glyphs, count }));
                            }
                        } else if extension_type == 3 {
                            if let Some(substitute) = gsub_alternate_subtable(
                                self.bytes,
                                subtable,
                                glyphs[position],
                                feature_end,
                            )? {
                                return Ok(Some(GsubSubstitution::Single {
                                    glyph: substitute,
                                    consumed: 1,
                                }));
                            }
                        } else if extension_type == 4 {
                            if let Some((substitute, consumed)) = gsub_ligature_subtable(
                                self.bytes,
                                subtable,
                                glyphs,
                                position,
                                feature_end,
                            )? {
                                return Ok(Some(GsubSubstitution::Single {
                                    glyph: substitute,
                                    consumed,
                                }));
                            }
                        } else {
                            if extension_type == 5
                                && let Some(substitute) = gsub_contextual_class_substitution(
                                    self.bytes,
                                    lookup_base,
                                    lookup_base,
                                    glyphs,
                                    position,
                                    feature_end,
                                    Some((subtable, extension_type)),
                                )?
                            {
                                return Ok(Some(GsubSubstitution::Single {
                                    glyph: substitute,
                                    consumed: 1,
                                }));
                            }
                            if extension_type == 6
                                && let Some(substitute) = gsub_chaining_class_substitution(
                                    self.bytes,
                                    lookup_base,
                                    lookup_base,
                                    glyphs,
                                    position,
                                    feature_end,
                                    Some((subtable, extension_type)),
                                )?
                            {
                                return Ok(Some(GsubSubstitution::Single {
                                    glyph: substitute,
                                    consumed: 1,
                                }));
                            }
                            if extension_type == 8
                                && let Some(substitute) = gsub_reverse_chaining_substitution(
                                    self.bytes,
                                    subtable,
                                    glyphs,
                                    position,
                                    feature_end,
                                )?
                            {
                                return Ok(Some(GsubSubstitution::Single {
                                    glyph: substitute,
                                    consumed: 1,
                                }));
                            }
                            if let Some(substitute) = gsub_contextual_substitution(
                                self.bytes,
                                lookup_base,
                                lookup_base,
                                glyphs,
                                position,
                                feature_end,
                                Some((subtable, extension_type)),
                            )? {
                                return Ok(Some(GsubSubstitution::Single {
                                    glyph: substitute,
                                    consumed: 1,
                                }));
                            }
                        }
                    }
                    continue;
                }
                if lookup_type == 5 {
                    if let Some(substitute) = gsub_contextual_class_substitution(
                        self.bytes,
                        lookup_table,
                        lookup_base,
                        glyphs,
                        position,
                        feature_end,
                        None,
                    )? {
                        return Ok(Some(GsubSubstitution::Single {
                            glyph: substitute,
                            consumed: 1,
                        }));
                    }
                    if let Some(substitute) = gsub_contextual_substitution(
                        self.bytes,
                        lookup_table,
                        lookup_base,
                        glyphs,
                        position,
                        feature_end,
                        None,
                    )? {
                        return Ok(Some(GsubSubstitution::Single {
                            glyph: substitute,
                            consumed: 1,
                        }));
                    }
                    continue;
                }
                if lookup_type == 6 {
                    if let Some(substitute) = gsub_chaining_class_substitution(
                        self.bytes,
                        lookup_table,
                        lookup_base,
                        glyphs,
                        position,
                        feature_end,
                        None,
                    )? {
                        return Ok(Some(GsubSubstitution::Single {
                            glyph: substitute,
                            consumed: 1,
                        }));
                    }
                    if let Some(substitute) = gsub_contextual_substitution(
                        self.bytes,
                        lookup_table,
                        lookup_base,
                        glyphs,
                        position,
                        feature_end,
                        None,
                    )? {
                        return Ok(Some(GsubSubstitution::Single {
                            glyph: substitute,
                            consumed: 1,
                        }));
                    }
                    continue;
                }
                if lookup_type != 1
                    && lookup_type != 2
                    && lookup_type != 3
                    && lookup_type != 4
                    && lookup_type != 8
                {
                    continue;
                }
                let subtable_count = usize::from(
                    read_u16(self.bytes, lookup_table + 4)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if subtable_count > 32
                    || lookup_table
                        .checked_add(
                            6usize
                                .checked_add(
                                    subtable_count
                                        .checked_mul(2)
                                        .ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > feature_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                for subtable_index in 0..subtable_count {
                    let subtable = lookup_table
                        .checked_add(usize::from(
                            read_u16(self.bytes, lookup_table + 6 + subtable_index * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if lookup_type == 3 {
                        if let Some(substitute) = gsub_alternate_subtable(
                            self.bytes,
                            subtable,
                            glyphs[position],
                            feature_end,
                        )? {
                            return Ok(Some(GsubSubstitution::Single {
                                glyph: substitute,
                                consumed: 1,
                            }));
                        }
                        continue;
                    }
                    if lookup_type == 8 {
                        if let Some(substitute) = gsub_reverse_chaining_substitution(
                            self.bytes,
                            subtable,
                            glyphs,
                            position,
                            feature_end,
                        )? {
                            return Ok(Some(GsubSubstitution::Single {
                                glyph: substitute,
                                consumed: 1,
                            }));
                        }
                        continue;
                    }
                    if lookup_type == 2 {
                        if let Some((glyphs, count)) = gsub_multiple_subtable(
                            self.bytes,
                            subtable,
                            glyphs[position],
                            feature_end,
                        )? {
                            return Ok(Some(GsubSubstitution::Multiple { glyphs, count }));
                        }
                        continue;
                    }
                    if lookup_type == 1 {
                        if subtable.checked_add(6).is_none_or(|end| end > feature_end) {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let format = read_u16(self.bytes, subtable)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some(coverage_index) = gsub_coverage_index(
                            self.bytes,
                            coverage,
                            feature_end,
                            glyphs[position],
                        )?
                        else {
                            continue;
                        };
                        let substitute = match format {
                            1 => {
                                let delta = i32::from(
                                    read_i16(self.bytes, subtable + 4)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                );
                                let glyph = i32::from(glyphs[position]) + delta;
                                u16::try_from(glyph)
                                    .map_err(|_| TrueTypeError::BadTableDirectory)?
                            }
                            2 => {
                                let count = usize::from(
                                    read_u16(self.bytes, subtable + 4)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                );
                                if count > 128 || coverage_index >= count {
                                    return Err(TrueTypeError::TooLarge);
                                }
                                let end = subtable
                                    .checked_add(
                                        6usize
                                            .checked_add(
                                                count
                                                    .checked_mul(2)
                                                    .ok_or(TrueTypeError::TooLarge)?,
                                            )
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?;
                                if end > feature_end {
                                    return Err(TrueTypeError::BadTableDirectory);
                                }
                                read_u16(self.bytes, subtable + 6 + coverage_index * 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?
                            }
                            _ => return Err(TrueTypeError::UnsupportedCmap),
                        };
                        return Ok(Some(GsubSubstitution::Single {
                            glyph: substitute,
                            consumed: 1,
                        }));
                    }
                    if subtable.checked_add(6).is_none_or(|end| end > feature_end)
                        || read_u16(self.bytes, subtable) != Some(1)
                    {
                        continue;
                    }
                    let coverage = subtable
                        .checked_add(usize::from(
                            read_u16(self.bytes, subtable + 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let Some(coverage_index) =
                        gsub_coverage_index(self.bytes, coverage, feature_end, glyphs[position])?
                    else {
                        continue;
                    };
                    let set_count = usize::from(
                        read_u16(self.bytes, subtable + 4)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if coverage_index >= set_count
                        || set_count > 128
                        || subtable
                            .checked_add(
                                6usize
                                    .checked_add(
                                        set_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .is_none_or(|end| end > feature_end)
                    {
                        return Err(TrueTypeError::TooLarge);
                    }
                    let set = subtable
                        .checked_add(usize::from(
                            read_u16(self.bytes, subtable + 6 + coverage_index * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let ligature_count = usize::from(
                        read_u16(self.bytes, set).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if ligature_count > 64
                        || set
                            .checked_add(
                                2usize
                                    .checked_add(
                                        ligature_count
                                            .checked_mul(2)
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .is_none_or(|end| end > feature_end)
                    {
                        return Err(TrueTypeError::TooLarge);
                    }
                    for ligature_index in 0..ligature_count {
                        let ligature = set
                            .checked_add(usize::from(
                                read_u16(self.bytes, set + 2 + ligature_index * 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if ligature.checked_add(4).is_none_or(|end| end > feature_end) {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let ligature_glyph = read_u16(self.bytes, ligature)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let component_count = usize::from(
                            read_u16(self.bytes, ligature + 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if !(2..=32).contains(&component_count)
                            || ligature
                                .checked_add(
                                    4usize
                                        .checked_add(
                                            (component_count - 1)
                                                .checked_mul(2)
                                                .ok_or(TrueTypeError::TooLarge)?,
                                        )
                                        .ok_or(TrueTypeError::TooLarge)?,
                                )
                                .is_none_or(|end| end > feature_end)
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        if position + component_count > glyphs.len() {
                            continue;
                        }
                        let mut matches = true;
                        for component in 1..component_count {
                            if read_u16(self.bytes, ligature + 4 + (component - 1) * 2)
                                != Some(glyphs[position + component])
                            {
                                matches = false;
                                break;
                            }
                        }
                        if matches {
                            return Ok(Some(GsubSubstitution::Single {
                                glyph: ligature_glyph,
                                consumed: component_count,
                            }));
                        }
                    }
                }
            }
            Ok(None)
        }

        fn gsub_substitution_pipeline(
            &self,
            glyphs: &[u16; 128],
            position: usize,
            script_tag: Option<[u8; 4]>,
            language_tag: Option<[u8; 4]>,
        ) -> Result<GsubSubstitution, TrueTypeError> {
            let mut working = *glyphs;
            let mut glyph = working[position];
            let mut consumed = 1usize;
            for _ in 0..8 {
                working[position] = glyph;
                let Some(next) =
                    self.ligature_substitution(&working, position, script_tag, language_tag)?
                else {
                    break;
                };
                match next {
                    GsubSubstitution::Single {
                        glyph: next,
                        consumed: next_consumed,
                    } => {
                        if next == glyph && next_consumed == 1 {
                            break;
                        }
                        glyph = next;
                        consumed = next_consumed;
                        if consumed != 1 {
                            return Ok(GsubSubstitution::Single { glyph, consumed });
                        }
                    }
                    GsubSubstitution::Multiple { glyphs, count } => {
                        return Ok(GsubSubstitution::Multiple { glyphs, count });
                    }
                }
            }
            Ok(GsubSubstitution::Single { glyph, consumed })
        }

        fn gpos_pair_adjustment(
            &self,
            left: u16,
            right: u16,
            context: GposContextNeighbors,
            script_tag: Option<[u8; 4]>,
            language_tag: Option<[u8; 4]>,
            ppem: Option<u16>,
        ) -> Result<Option<(i32, i32, i32, i32)>, TrueTypeError> {
            let Some((table, length)) = self.gpos else {
                return Ok(None);
            };
            let table_end = table
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if length < 10 || table_end > self.bytes.len() {
                return Err(TrueTypeError::BadTableDirectory);
            }
            if read_u32(self.bytes, table) != Some(0x0001_0000) {
                return Err(TrueTypeError::UnsupportedCmap);
            }
            let feature_offset = usize::from(
                read_u16(self.bytes, table + 6).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let lookup_offset = usize::from(
                read_u16(self.bytes, table + 8).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let feature_base = table
                .checked_add(feature_offset)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if feature_base
                .checked_add(2)
                .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let feature_count = usize::from(
                read_u16(self.bytes, feature_base).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if feature_count > 64
                || feature_base
                    .checked_add(
                        2usize
                            .checked_add(
                                feature_count
                                    .checked_mul(6)
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            let script = usize::from(
                read_u16(self.bytes, table + 4).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            let mut feature_indices = [0u16; 64];
            let mut feature_index_count = 0usize;
            if let Some(script_tag) = script_tag {
                if script == 0 {
                    return Ok(None);
                }
                let script_list = table
                    .checked_add(script)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let script_count = usize::from(
                    read_u16(self.bytes, script_list).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if script_count > 32
                    || script_list
                        .checked_add(
                            2usize
                                .checked_add(
                                    script_count.checked_mul(6).ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                let mut selected_script = None;
                for index in 0..script_count {
                    let record = script_list + 2 + index * 6;
                    if self
                        .bytes
                        .get(record..record + 4)
                        .ok_or(TrueTypeError::BadTableDirectory)?
                        == script_tag
                    {
                        selected_script = Some(
                            script_list
                                .checked_add(usize::from(
                                    read_u16(self.bytes, record + 4)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        break;
                    }
                }
                let Some(script_table) = selected_script else {
                    return Ok(None);
                };
                let Some(lang_sys) =
                    layout_lang_sys(self.bytes, script_table, language_tag, table_end)?
                else {
                    return Ok(None);
                };
                let required =
                    read_u16(self.bytes, lang_sys + 2).ok_or(TrueTypeError::BadTableDirectory)?;
                let count = usize::from(
                    read_u16(self.bytes, lang_sys + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if count > 63
                    || lang_sys
                        .checked_add(
                            6usize
                                .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                if required != u16::MAX {
                    feature_indices[feature_index_count] = required;
                    feature_index_count += 1;
                }
                for index in 0..count {
                    feature_indices[feature_index_count] =
                        read_u16(self.bytes, lang_sys + 6 + index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                    feature_index_count += 1;
                }
            } else {
                for index in 0..feature_count {
                    feature_indices[feature_index_count] =
                        u16::try_from(index).map_err(|_| TrueTypeError::TooLarge)?;
                    feature_index_count += 1;
                }
            }
            let mut lookup_indices = [0u16; 32];
            let mut selected_count = 0usize;
            for feature_index in feature_indices.iter().copied().take(feature_index_count) {
                let feature_index = usize::from(feature_index);
                if feature_index >= feature_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let record = feature_base + 2 + feature_index * 6;
                if self
                    .bytes
                    .get(record..record + 4)
                    .ok_or(TrueTypeError::BadTableDirectory)?
                    != b"kern"
                {
                    continue;
                }
                let feature_table = feature_base
                    .checked_add(usize::from(
                        read_u16(self.bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if feature_table
                    .checked_add(4)
                    .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let count = usize::from(
                    read_u16(self.bytes, feature_table + 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if count > 32 || selected_count.checked_add(count).is_none_or(|end| end > 32) {
                    return Err(TrueTypeError::TooLarge);
                }
                let end = feature_table
                    .checked_add(
                        4usize
                            .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if end > table_end {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                for item in 0..count {
                    lookup_indices[selected_count] =
                        read_u16(self.bytes, feature_table + 4 + item * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                    selected_count += 1;
                }
            }
            if selected_count == 0 {
                return if right != u16::MAX {
                    gpos_contextual_single_adjustment(
                        self.bytes,
                        table,
                        table_end,
                        left,
                        right,
                        GposContextOptions {
                            script_tag,
                            language_tag,
                            ppem,
                            previous: context.previous,
                            previous2: context.previous2,
                            next: context.next,
                            next2: context.next2,
                            third: None,
                            fourth: None,
                            fifth: None,
                            input_count: 2,
                            target_sequence_index: 0,
                        },
                    )
                } else {
                    Ok(None)
                };
            }
            let mut accumulated = (0i32, 0i32, 0i32, 0i32);
            let mut found = false;
            let lookup_base = table
                .checked_add(lookup_offset)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if lookup_base.checked_add(2).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let lookup_count = usize::from(
                read_u16(self.bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if lookup_count > 128
                || lookup_base
                    .checked_add(
                        2usize
                            .checked_add(
                                lookup_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for selected in lookup_indices.iter().copied().take(selected_count) {
                let selected = usize::from(selected);
                if selected >= lookup_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let lookup_table = lookup_base
                    .checked_add(usize::from(
                        read_u16(self.bytes, lookup_base + 2 + selected * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if lookup_table
                    .checked_add(6)
                    .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let lookup_type =
                    read_u16(self.bytes, lookup_table).ok_or(TrueTypeError::BadTableDirectory)?;
                let extension_subtable = if lookup_type == 9 {
                    let extension = lookup_table
                        .checked_add(usize::from(
                            read_u16(self.bytes, lookup_table + 6)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if extension.checked_add(8).is_none_or(|end| end > table_end) {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    if read_u16(self.bytes, extension) != Some(1)
                        || read_u16(self.bytes, extension + 2) != Some(2)
                    {
                        None
                    } else {
                        Some(
                            extension
                                .checked_add(
                                    usize::try_from(
                                        read_u32(self.bytes, extension + 4)
                                            .ok_or(TrueTypeError::BadTableDirectory)?,
                                    )
                                    .map_err(|_| TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    }
                } else {
                    None
                };
                if lookup_type != 1 && lookup_type != 2 && extension_subtable.is_none() {
                    continue;
                }
                let subtable_count = if extension_subtable.is_some() {
                    1
                } else {
                    usize::from(
                        read_u16(self.bytes, lookup_table + 4)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    )
                };
                if subtable_count > 32 {
                    return Err(TrueTypeError::TooLarge);
                }
                let lookup_end = if let Some(subtable) = extension_subtable {
                    subtable.checked_add(2).ok_or(TrueTypeError::TooLarge)?
                } else {
                    lookup_table
                        .checked_add(
                            6usize
                                .checked_add(
                                    subtable_count
                                        .checked_mul(2)
                                        .ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?
                };
                if lookup_end > table_end {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                for subtable_index in 0..subtable_count {
                    let subtable = if let Some(subtable) = extension_subtable {
                        subtable
                    } else {
                        lookup_table
                            .checked_add(usize::from(
                                read_u16(self.bytes, lookup_table + 6 + subtable_index * 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?
                    };
                    if lookup_type == 1 {
                        if right != u16::MAX {
                            continue;
                        }
                        if subtable.checked_add(6).is_none_or(|end| end > table_end) {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let format = read_u16(self.bytes, subtable)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some(coverage_index) =
                            gsub_coverage_index(self.bytes, coverage, table_end, left)?
                        else {
                            continue;
                        };
                        let value_format = read_u16(self.bytes, subtable + 4)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let record = match format {
                            1 => subtable + 6,
                            2 => {
                                let count = usize::from(
                                    read_u16(self.bytes, subtable + 6)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                );
                                let record_size = gpos_value_record_size(value_format)?;
                                if count > 128 || coverage_index >= count {
                                    return Err(TrueTypeError::TooLarge);
                                }
                                subtable
                                    .checked_add(
                                        6usize
                                            .checked_add(
                                                2usize
                                                    .checked_add(
                                                        count
                                                            .checked_mul(record_size)
                                                            .ok_or(TrueTypeError::TooLarge)?,
                                                    )
                                                    .ok_or(TrueTypeError::TooLarge)?,
                                            )
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .is_none_or(|end| end > table_end)
                                    .then_some(())
                                    .ok_or(TrueTypeError::BadTableDirectory)?;
                                subtable
                                    .checked_add(
                                        8usize
                                            .checked_add(
                                                count
                                                    .checked_mul(record_size)
                                                    .ok_or(TrueTypeError::TooLarge)?,
                                            )
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .is_none_or(|end| end > table_end)
                                    .then_some(())
                                    .ok_or(TrueTypeError::BadTableDirectory)?;
                                subtable
                                    .checked_add(
                                        8usize
                                            .checked_add(
                                                coverage_index
                                                    .checked_mul(record_size)
                                                    .ok_or(TrueTypeError::TooLarge)?,
                                            )
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?
                            }
                            _ => continue,
                        };
                        let adjustment = gpos_value_adjustment(
                            self.bytes,
                            record,
                            value_format,
                            table_end,
                            table,
                            ppem,
                        )?;
                        accumulated.0 = accumulated.0.saturating_add(adjustment.0);
                        accumulated.1 = accumulated.1.saturating_add(adjustment.1);
                        accumulated.2 = accumulated.2.saturating_add(adjustment.2);
                        accumulated.3 = accumulated.3.saturating_add(adjustment.3);
                        found = true;
                        continue;
                    }
                    if right == u16::MAX {
                        continue;
                    }
                    if subtable.checked_add(10).is_none_or(|end| end > table_end) {
                        continue;
                    }
                    let format =
                        read_u16(self.bytes, subtable).ok_or(TrueTypeError::BadTableDirectory)?;
                    if format == 2 {
                        let coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if gsub_coverage_index(self.bytes, coverage, table_end, left)?.is_none() {
                            continue;
                        }
                        let value_format1 = read_u16(self.bytes, subtable + 4)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let value_format2 = read_u16(self.bytes, subtable + 6)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let class_def1 = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 8)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let class_def2 = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 10)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let class1_count = usize::from(
                            read_u16(self.bytes, subtable + 12)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        let class2_count = usize::from(
                            read_u16(self.bytes, subtable + 14)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if class1_count > 128 || class2_count > 128 {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let class1 =
                            usize::from(gpos_class_index(self.bytes, class_def1, left, table_end)?);
                        let class2 = usize::from(gpos_class_index(
                            self.bytes, class_def2, right, table_end,
                        )?);
                        if class1 >= class1_count || class2 >= class2_count {
                            continue;
                        }
                        let first_value_size = gpos_value_record_size(value_format1)?;
                        let second_value_size = gpos_value_record_size(value_format2)?;
                        let record_size = first_value_size
                            .checked_add(second_value_size)
                            .ok_or(TrueTypeError::TooLarge)?;
                        let records_start =
                            subtable.checked_add(16).ok_or(TrueTypeError::TooLarge)?;
                        let record = records_start
                            .checked_add(
                                class1
                                    .checked_mul(class2_count)
                                    .and_then(|value| value.checked_add(class2))
                                    .and_then(|value| value.checked_mul(record_size))
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        let records_end = records_start
                            .checked_add(
                                class1_count
                                    .checked_mul(class2_count)
                                    .and_then(|value| value.checked_mul(record_size))
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        if records_end > table_end {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let adjustment = gpos_value_adjustment(
                            self.bytes,
                            record,
                            value_format1,
                            table_end,
                            table,
                            ppem,
                        )?;
                        accumulated.0 = accumulated.0.saturating_add(adjustment.0);
                        accumulated.1 = accumulated.1.saturating_add(adjustment.1);
                        accumulated.2 = accumulated.2.saturating_add(adjustment.2);
                        accumulated.3 = accumulated.3.saturating_add(adjustment.3);
                        found = true;
                        continue;
                    }
                    if format != 1 {
                        continue;
                    }
                    let coverage = subtable
                        .checked_add(usize::from(
                            read_u16(self.bytes, subtable + 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let Some(coverage_index) =
                        gsub_coverage_index(self.bytes, coverage, table_end, left)?
                    else {
                        continue;
                    };
                    let value_format1 = read_u16(self.bytes, subtable + 4)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let value_format2 = read_u16(self.bytes, subtable + 6)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let pair_set_count = usize::from(
                        read_u16(self.bytes, subtable + 8)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if pair_set_count > 128 || coverage_index >= pair_set_count {
                        return Err(TrueTypeError::TooLarge);
                    }
                    let offset_end = subtable
                        .checked_add(
                            10usize
                                .checked_add(
                                    pair_set_count
                                        .checked_mul(2)
                                        .ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?;
                    if offset_end > table_end {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let first_value_size = gpos_value_record_size(value_format1)?;
                    let second_value_size = gpos_value_record_size(value_format2)?;
                    let pair_set = subtable
                        .checked_add(usize::from(
                            read_u16(self.bytes, subtable + 10 + coverage_index * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if pair_set.checked_add(2).is_none_or(|end| end > table_end) {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let pair_count = usize::from(
                        read_u16(self.bytes, pair_set).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if pair_count > 128 {
                        return Err(TrueTypeError::TooLarge);
                    }
                    let record_size = 2usize
                        .checked_add(first_value_size)
                        .and_then(|size| size.checked_add(second_value_size))
                        .ok_or(TrueTypeError::TooLarge)?;
                    let records_end = pair_set
                        .checked_add(
                            2usize
                                .checked_add(
                                    pair_count
                                        .checked_mul(record_size)
                                        .ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?;
                    if records_end > table_end {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    for pair in 0..pair_count {
                        let record = pair_set + 2 + pair * record_size;
                        if read_u16(self.bytes, record) != Some(right) {
                            continue;
                        }
                        let first_value = gpos_value_adjustment(
                            self.bytes,
                            record + 2,
                            value_format1,
                            table_end,
                            table,
                            ppem,
                        )?;
                        accumulated.0 = accumulated.0.saturating_add(first_value.0);
                        accumulated.1 = accumulated.1.saturating_add(first_value.1);
                        accumulated.2 = accumulated.2.saturating_add(first_value.2);
                        accumulated.3 = accumulated.3.saturating_add(first_value.3);
                        found = true;
                        break;
                    }
                }
            }
            if found {
                Ok(Some(accumulated))
            } else if right != u16::MAX {
                gpos_contextual_single_adjustment(
                    self.bytes,
                    table,
                    table_end,
                    left,
                    right,
                    GposContextOptions {
                        script_tag,
                        language_tag,
                        ppem,
                        previous: context.previous,
                        previous2: context.previous2,
                        next: context.next,
                        next2: context.next2,
                        third: None,
                        fourth: None,
                        fifth: None,
                        input_count: 2,
                        target_sequence_index: 0,
                    },
                )
            } else {
                Ok(None)
            }
        }

        fn gpos_contextual_adjustment(
            &self,
            left: u16,
            right: u16,
            options: GposContextOptions,
        ) -> Result<Option<(i32, i32, i32, i32)>, TrueTypeError> {
            let Some((table, length)) = self.gpos else {
                return Ok(None);
            };
            let table_end = table
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if length < 10 || table_end > self.bytes.len() {
                return Err(TrueTypeError::BadTableDirectory);
            }
            gpos_contextual_single_adjustment(self.bytes, table, table_end, left, right, options)
        }

        fn gpos_mark_to_ligature_adjustment(
            &self,
            ligature: u16,
            mark: u16,
            script_tag: Option<[u8; 4]>,
            language_tag: Option<[u8; 4]>,
            ppem: Option<u16>,
        ) -> Result<Option<(i32, i32)>, TrueTypeError> {
            let Some((table, length)) = self.gpos else {
                return Ok(None);
            };
            let table_end = table
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if length < 10 || table_end > self.bytes.len() {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let feature_base = table
                .checked_add(usize::from(
                    read_u16(self.bytes, table + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let (feature_indices, feature_index_count, feature_count) =
                gpos_selected_feature_indices(
                    self.bytes,
                    table,
                    table_end,
                    script_tag,
                    language_tag,
                )?;
            let lookup_base = table
                .checked_add(usize::from(
                    read_u16(self.bytes, table + 8).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let lookup_count = usize::from(
                read_u16(self.bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if lookup_count > 128
                || lookup_base
                    .checked_add(
                        2usize
                            .checked_add(
                                lookup_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for feature_index in feature_indices.iter().copied().take(feature_index_count) {
                let feature_index = usize::from(feature_index);
                if feature_index >= feature_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let record = feature_base + 2 + feature_index * 6;
                if self
                    .bytes
                    .get(record..record + 4)
                    .ok_or(TrueTypeError::BadTableDirectory)?
                    != b"mark"
                {
                    continue;
                }
                let feature = feature_base
                    .checked_add(usize::from(
                        read_u16(self.bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let lookup_total = usize::from(
                    read_u16(self.bytes, feature + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if lookup_total > 32
                    || feature
                        .checked_add(
                            4usize
                                .checked_add(
                                    lookup_total.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                for item in 0..lookup_total {
                    let selected = usize::from(
                        read_u16(self.bytes, feature + 4 + item * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if selected >= lookup_count {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let lookup = lookup_base
                        .checked_add(usize::from(
                            read_u16(self.bytes, lookup_base + 2 + selected * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let Some((is_extension, extension_subtable)) =
                        gpos_attachment_lookup(self.bytes, lookup, 5, table_end)?
                    else {
                        continue;
                    };
                    let subtable_count = if is_extension {
                        1
                    } else {
                        usize::from(
                            read_u16(self.bytes, lookup + 4)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    };
                    if subtable_count > 32 {
                        return Err(TrueTypeError::TooLarge);
                    }
                    for subtable_index in 0..subtable_count {
                        let subtable = if is_extension {
                            extension_subtable
                        } else {
                            lookup
                                .checked_add(usize::from(
                                    read_u16(self.bytes, lookup + 6 + subtable_index * 2)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?
                        };
                        if subtable.checked_add(12).is_none_or(|end| end > table_end)
                            || read_u16(self.bytes, subtable) != Some(1)
                        {
                            continue;
                        }
                        let mark_coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let ligature_coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 4)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some(mark_index) =
                            gsub_coverage_index(self.bytes, mark_coverage, table_end, mark)?
                        else {
                            continue;
                        };
                        let Some(ligature_index) = gsub_coverage_index(
                            self.bytes,
                            ligature_coverage,
                            table_end,
                            ligature,
                        )?
                        else {
                            continue;
                        };
                        let class_count = usize::from(
                            read_u16(self.bytes, subtable + 6)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        let mark_array = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 8)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let ligature_array = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 10)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if class_count == 0 || class_count > 32 {
                            return Err(TrueTypeError::TooLarge);
                        }
                        if mark_array.checked_add(2).is_none_or(|end| end > table_end)
                            || ligature_array
                                .checked_add(2)
                                .is_none_or(|end| end > table_end)
                        {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let mark_count = usize::from(
                            read_u16(self.bytes, mark_array)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        let ligature_count = usize::from(
                            read_u16(self.bytes, ligature_array)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if mark_count > 128
                            || ligature_count > 128
                            || mark_index >= mark_count
                            || ligature_index >= ligature_count
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let mark_record = mark_array
                            .checked_add(
                                2usize
                                    .checked_add(
                                        mark_index.checked_mul(4).ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        if mark_record.checked_add(4).is_none_or(|end| end > table_end) {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let mark_class = usize::from(
                            read_u16(self.bytes, mark_record)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if mark_class >= class_count {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let mark_anchor_offset = read_u16(self.bytes, mark_record + 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if mark_anchor_offset == 0 {
                            continue;
                        }
                        let ligature_record = ligature_array
                            .checked_add(
                                2usize
                                    .checked_add(
                                        ligature_index
                                            .checked_mul(2)
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        let ligature_offset = read_u16(self.bytes, ligature_record)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if ligature_offset == 0 {
                            continue;
                        }
                        let ligature_attach = ligature_array
                            .checked_add(usize::from(ligature_offset))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if ligature_attach
                            .checked_add(2)
                            .is_none_or(|end| end > table_end)
                        {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let component_count = usize::from(
                            read_u16(self.bytes, ligature_attach)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if component_count == 0 || component_count > 128 {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let component = component_count - 1;
                        let component_record = ligature_attach
                            .checked_add(
                                2usize
                                    .checked_add(
                                        component
                                            .checked_mul(class_count * 2)
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        let ligature_anchor_offset = read_u16(
                            self.bytes,
                            component_record
                                .checked_add(mark_class * 2)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                        if ligature_anchor_offset == 0 {
                            continue;
                        }
                        let mark_anchor = mark_array
                            .checked_add(usize::from(mark_anchor_offset))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let ligature_anchor = ligature_attach
                            .checked_add(usize::from(ligature_anchor_offset))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some((mark_x, mark_y)) =
                            gpos_anchor_coordinates(self, mark_anchor, ppem, table_end, mark)?
                        else {
                            continue;
                        };
                        let Some((ligature_x, ligature_y)) = gpos_anchor_coordinates(
                            self,
                            ligature_anchor,
                            ppem,
                            table_end,
                            ligature,
                        )?
                        else {
                            continue;
                        };
                        return Ok(Some((ligature_x - mark_x, ligature_y - mark_y)));
                    }
                }
            }
            Ok(None)
        }

        fn gpos_mark_to_base_adjustment(
            &self,
            base: u16,
            mark: u16,
            script_tag: Option<[u8; 4]>,
            language_tag: Option<[u8; 4]>,
            ppem: Option<u16>,
        ) -> Result<Option<(i32, i32)>, TrueTypeError> {
            let Some((table, length)) = self.gpos else {
                return Ok(None);
            };
            let table_end = table
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if length < 10 || table_end > self.bytes.len() {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let feature_base = table
                .checked_add(usize::from(
                    read_u16(self.bytes, table + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let (feature_indices, feature_index_count, feature_count) =
                gpos_selected_feature_indices(
                    self.bytes,
                    table,
                    table_end,
                    script_tag,
                    language_tag,
                )?;
            let lookup_base = table
                .checked_add(usize::from(
                    read_u16(self.bytes, table + 8).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let lookup_count = usize::from(
                read_u16(self.bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if lookup_count > 128
                || lookup_base
                    .checked_add(
                        2usize
                            .checked_add(
                                lookup_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for feature_index in feature_indices.iter().copied().take(feature_index_count) {
                let feature_index = usize::from(feature_index);
                if feature_index >= feature_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let record = feature_base + 2 + feature_index * 6;
                let tag = self
                    .bytes
                    .get(record..record + 4)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if tag != b"mark" && tag != b"mkmk" {
                    continue;
                }
                let feature = feature_base
                    .checked_add(usize::from(
                        read_u16(self.bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let lookup_total = usize::from(
                    read_u16(self.bytes, feature + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if lookup_total > 32
                    || feature
                        .checked_add(
                            4usize
                                .checked_add(
                                    lookup_total.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                for item in 0..lookup_total {
                    let selected = usize::from(
                        read_u16(self.bytes, feature + 4 + item * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if selected >= lookup_count {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let lookup = lookup_base
                        .checked_add(usize::from(
                            read_u16(self.bytes, lookup_base + 2 + selected * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let Some((is_extension, extension_subtable)) =
                        gpos_attachment_lookup(self.bytes, lookup, 4, table_end)?
                    else {
                        continue;
                    };
                    let subtable_count = if is_extension {
                        1
                    } else {
                        usize::from(
                            read_u16(self.bytes, lookup + 4)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    };
                    if subtable_count > 32 {
                        return Err(TrueTypeError::TooLarge);
                    }
                    for subtable_index in 0..subtable_count {
                        let subtable = if is_extension {
                            extension_subtable
                        } else {
                            lookup
                                .checked_add(usize::from(
                                    read_u16(self.bytes, lookup + 6 + subtable_index * 2)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?
                        };
                        if subtable.checked_add(12).is_none_or(|end| end > table_end)
                            || read_u16(self.bytes, subtable) != Some(1)
                        {
                            continue;
                        }
                        let mark_coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let base_coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 4)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some(mark_index) =
                            gsub_coverage_index(self.bytes, mark_coverage, table_end, mark)?
                        else {
                            continue;
                        };
                        let Some(base_index) =
                            gsub_coverage_index(self.bytes, base_coverage, table_end, base)?
                        else {
                            continue;
                        };
                        let class_count = usize::from(
                            read_u16(self.bytes, subtable + 6)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        let mark_array = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 8)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let base_array = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 10)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if class_count == 0 || class_count > 32 {
                            return Err(TrueTypeError::TooLarge);
                        }
                        if mark_array.checked_add(2).is_none_or(|end| end > table_end)
                            || base_array.checked_add(2).is_none_or(|end| end > table_end)
                        {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let mark_count = usize::from(
                            read_u16(self.bytes, mark_array)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        let base_count = usize::from(
                            read_u16(self.bytes, base_array)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if mark_count > 128 || base_count > 128 || mark_index >= mark_count {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let mark_record = mark_array
                            .checked_add(
                                2usize
                                    .checked_add(
                                        mark_index.checked_mul(4).ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        if mark_record.checked_add(4).is_none_or(|end| end > table_end)
                            || base_index >= base_count
                        {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let mark_class = usize::from(
                            read_u16(self.bytes, mark_record)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if mark_class >= class_count {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let base_anchor_record = base_array
                            .checked_add(
                                2usize
                                    .checked_add(
                                        base_index
                                            .checked_mul(class_count)
                                            .and_then(|value| value.checked_mul(2))
                                            .and_then(|value| value.checked_add(mark_class * 2))
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        let mark_anchor = mark_array
                            .checked_add(usize::from(
                                read_u16(self.bytes, mark_record + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let base_anchor_offset = read_u16(self.bytes, base_anchor_record)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if base_anchor_offset == 0 {
                            continue;
                        }
                        let base_anchor = base_array
                            .checked_add(usize::from(base_anchor_offset))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some((mark_x, mark_y)) =
                            gpos_anchor_coordinates(self, mark_anchor, ppem, table_end, mark)?
                        else {
                            continue;
                        };
                        let Some((base_x, base_y)) =
                            gpos_anchor_coordinates(self, base_anchor, ppem, table_end, base)?
                        else {
                            continue;
                        };
                        return Ok(Some((base_x - mark_x, base_y - mark_y)));
                    }
                }
            }
            Ok(None)
        }

        fn gpos_mark_to_mark_adjustment(
            &self,
            first: u16,
            second: u16,
            script_tag: Option<[u8; 4]>,
            language_tag: Option<[u8; 4]>,
            ppem: Option<u16>,
        ) -> Result<Option<(i32, i32)>, TrueTypeError> {
            let Some((table, length)) = self.gpos else {
                return Ok(None);
            };
            let table_end = table
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if length < 10 || table_end > self.bytes.len() {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let feature_base = table
                .checked_add(usize::from(
                    read_u16(self.bytes, table + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let (feature_indices, feature_index_count, feature_count) =
                gpos_selected_feature_indices(
                    self.bytes,
                    table,
                    table_end,
                    script_tag,
                    language_tag,
                )?;
            let lookup_base = table
                .checked_add(usize::from(
                    read_u16(self.bytes, table + 8).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let lookup_count = usize::from(
                read_u16(self.bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if lookup_count > 128 {
                return Err(TrueTypeError::TooLarge);
            }
            for feature_index in feature_indices.iter().copied().take(feature_index_count) {
                let feature_index = usize::from(feature_index);
                if feature_index >= feature_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let record = feature_base + 2 + feature_index * 6;
                if self
                    .bytes
                    .get(record..record + 4)
                    .ok_or(TrueTypeError::BadTableDirectory)?
                    != b"mkmk"
                {
                    continue;
                }
                let feature = feature_base
                    .checked_add(usize::from(
                        read_u16(self.bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let feature_lookup_count = usize::from(
                    read_u16(self.bytes, feature + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if feature_lookup_count > 32
                    || feature
                        .checked_add(
                            4usize
                                .checked_add(
                                    feature_lookup_count
                                        .checked_mul(2)
                                        .ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                for item in 0..feature_lookup_count {
                    let selected = usize::from(
                        read_u16(self.bytes, feature + 4 + item * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if selected >= lookup_count {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let lookup = lookup_base
                        .checked_add(usize::from(
                            read_u16(self.bytes, lookup_base + 2 + selected * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let Some((is_extension, extension_subtable)) =
                        gpos_attachment_lookup(self.bytes, lookup, 6, table_end)?
                    else {
                        continue;
                    };
                    let subtable_count = if is_extension {
                        1
                    } else {
                        usize::from(
                            read_u16(self.bytes, lookup + 4)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    };
                    if subtable_count > 32 {
                        return Err(TrueTypeError::TooLarge);
                    }
                    for subtable_index in 0..subtable_count {
                        let subtable = if is_extension {
                            extension_subtable
                        } else {
                            lookup
                                .checked_add(usize::from(
                                    read_u16(self.bytes, lookup + 6 + subtable_index * 2)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?
                        };
                        if subtable.checked_add(12).is_none_or(|end| end > table_end)
                            || read_u16(self.bytes, subtable) != Some(1)
                        {
                            continue;
                        }
                        let first_coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let second_coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 4)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some(first_index) =
                            gsub_coverage_index(self.bytes, first_coverage, table_end, first)?
                        else {
                            continue;
                        };
                        let Some(second_index) =
                            gsub_coverage_index(self.bytes, second_coverage, table_end, second)?
                        else {
                            continue;
                        };
                        let class_count = usize::from(
                            read_u16(self.bytes, subtable + 6)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if class_count == 0 || class_count > 32 {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let first_array = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 8)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let second_array = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 10)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if first_array.checked_add(2).is_none_or(|end| end > table_end)
                            || second_array
                                .checked_add(2)
                                .is_none_or(|end| end > table_end)
                        {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let first_count = usize::from(
                            read_u16(self.bytes, first_array)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        let second_count = usize::from(
                            read_u16(self.bytes, second_array)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if first_count > 128 || second_count > 128 || first_index >= first_count {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let first_record = first_array
                            .checked_add(
                                2usize
                                    .checked_add(
                                        first_index
                                            .checked_mul(4)
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        if first_record
                            .checked_add(4)
                            .is_none_or(|end| end > table_end)
                            || second_index >= second_count
                        {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let class = usize::from(
                            read_u16(self.bytes, first_record)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if class >= class_count {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let second_record = second_array
                            .checked_add(
                                2usize
                                    .checked_add(
                                        second_index
                                            .checked_mul(class_count)
                                            .and_then(|value| value.checked_add(class))
                                            .and_then(|value| value.checked_mul(2))
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        let first_anchor = first_array
                            .checked_add(usize::from(
                                read_u16(self.bytes, first_record + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let second_offset = read_u16(self.bytes, second_record)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if second_offset == 0 {
                            continue;
                        }
                        let second_anchor = second_array
                            .checked_add(usize::from(second_offset))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some((first_x, first_y)) =
                            gpos_anchor_coordinates(self, first_anchor, ppem, table_end, first)?
                        else {
                            continue;
                        };
                        let Some((second_x, second_y)) =
                            gpos_anchor_coordinates(self, second_anchor, ppem, table_end, second)?
                        else {
                            continue;
                        };
                        return Ok(Some((first_x - second_x, first_y - second_y)));
                    }
                }
            }
            Ok(None)
        }

        fn gpos_cursive_adjustment(
            &self,
            left: u16,
            right: u16,
            script_tag: Option<[u8; 4]>,
            language_tag: Option<[u8; 4]>,
            ppem: Option<u16>,
        ) -> Result<Option<(i32, i32)>, TrueTypeError> {
            let Some((table, length)) = self.gpos else {
                return Ok(None);
            };
            let table_end = table
                .checked_add(length)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if length < 10 || table_end > self.bytes.len() {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let feature_base = table
                .checked_add(usize::from(
                    read_u16(self.bytes, table + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let (feature_indices, feature_index_count, feature_count) =
                gpos_selected_feature_indices(
                    self.bytes,
                    table,
                    table_end,
                    script_tag,
                    language_tag,
                )?;
            let lookup_base = table
                .checked_add(usize::from(
                    read_u16(self.bytes, table + 8).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let lookup_count = usize::from(
                read_u16(self.bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if lookup_count > 128
                || feature_base
                    .checked_add(
                        2usize
                            .checked_add(
                                feature_count
                                    .checked_mul(6)
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
                || lookup_base
                    .checked_add(
                        2usize
                            .checked_add(
                                lookup_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for feature_index in feature_indices.iter().copied().take(feature_index_count) {
                let feature_index = usize::from(feature_index);
                if feature_index >= feature_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let record = feature_base + 2 + feature_index * 6;
                if self
                    .bytes
                    .get(record..record + 4)
                    .ok_or(TrueTypeError::BadTableDirectory)?
                    != b"curs"
                {
                    continue;
                }
                let feature = feature_base
                    .checked_add(usize::from(
                        read_u16(self.bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let count = usize::from(
                    read_u16(self.bytes, feature + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if count > 32
                    || feature
                        .checked_add(
                            4usize
                                .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                for item in 0..count {
                    let selected = usize::from(
                        read_u16(self.bytes, feature + 4 + item * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if selected >= lookup_count {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let lookup = lookup_base
                        .checked_add(usize::from(
                            read_u16(self.bytes, lookup_base + 2 + selected * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let Some((is_extension, extension_subtable)) =
                        gpos_attachment_lookup(self.bytes, lookup, 3, table_end)?
                    else {
                        continue;
                    };
                    let subtable_count = if is_extension {
                        1
                    } else {
                        usize::from(
                            read_u16(self.bytes, lookup + 4)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        )
                    };
                    if subtable_count > 32 {
                        return Err(TrueTypeError::TooLarge);
                    }
                    for subtable_index in 0..subtable_count {
                        let subtable = if is_extension {
                            extension_subtable
                        } else {
                            lookup
                                .checked_add(usize::from(
                                    read_u16(self.bytes, lookup + 6 + subtable_index * 2)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?
                        };
                        if subtable.checked_add(6).is_none_or(|end| end > table_end)
                            || read_u16(self.bytes, subtable) != Some(1)
                        {
                            continue;
                        }
                        let coverage = subtable
                            .checked_add(usize::from(
                                read_u16(self.bytes, subtable + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some(left_index) =
                            gsub_coverage_index(self.bytes, coverage, table_end, left)?
                        else {
                            continue;
                        };
                        let Some(right_index) =
                            gsub_coverage_index(self.bytes, coverage, table_end, right)?
                        else {
                            continue;
                        };
                        let count = usize::from(
                            read_u16(self.bytes, subtable + 4)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if count > 128 || left_index >= count || right_index >= count {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let records_end = subtable
                            .checked_add(
                                6usize
                                    .checked_add(
                                        count.checked_mul(4).ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        if records_end > table_end {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let left_record = subtable + 6 + left_index * 4;
                        let right_record = subtable + 6 + right_index * 4;
                        let exit_offset = read_u16(self.bytes, left_record + 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let entry_offset = read_u16(self.bytes, right_record)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if exit_offset == 0 || entry_offset == 0 {
                            continue;
                        }
                        let exit = subtable
                            .checked_add(usize::from(exit_offset))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let entry = subtable
                            .checked_add(usize::from(entry_offset))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let Some((exit_x, exit_y)) =
                            gpos_anchor_coordinates(self, exit, ppem, table_end, left)?
                        else {
                            continue;
                        };
                        let Some((entry_x, entry_y)) =
                            gpos_anchor_coordinates(self, entry, ppem, table_end, right)?
                        else {
                            continue;
                        };
                        return Ok(Some((exit_x - entry_x, exit_y - entry_y)));
                    }
                }
            }
            Ok(None)
        }

        fn apply_gpos_adjustments(
            &self,
            output: &mut [ShapedGlyph],
            count: usize,
            scale: i64,
            script_tag: Option<[u8; 4]>,
            language_tag: Option<[u8; 4]>,
            ppem: Option<u16>,
        ) -> Result<(), TrueTypeError> {
            let units = i64::from(self.units_per_em.max(1));
            for index in 0..count.saturating_sub(1) {
                let first = output[index].glyph_id;
                let second = output[index + 1].glyph_id;
                let Some((x_adjustment, y_adjustment)) = self.gpos_mark_to_mark_adjustment(
                    first,
                    second,
                    script_tag,
                    language_tag,
                    ppem,
                )?
                else {
                    continue;
                };
                let x_adjustment = (i64::from(x_adjustment) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_adjustment = (i64::from(y_adjustment) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index + 1].origin.x =
                    output[index + 1].origin.x.saturating_add(x_adjustment);
                output[index + 1].origin.y =
                    output[index + 1].origin.y.saturating_add(y_adjustment);
            }
            for index in 0..count.saturating_sub(1) {
                let left = output[index].glyph_id;
                let right = output[index + 1].glyph_id;
                let Some((x_placement, y_placement, x_advance, y_advance)) = self
                    .gpos_pair_adjustment(
                        left,
                        right,
                        GposContextNeighbors {
                            previous: (index > 0).then(|| output[index - 1].glyph_id),
                            previous2: (index > 1).then(|| output[index - 2].glyph_id),
                            next: (index + 2 < count).then(|| output[index + 2].glyph_id),
                            next2: (index + 3 < count).then(|| output[index + 3].glyph_id),
                        },
                        script_tag,
                        language_tag,
                        ppem,
                    )?
                else {
                    continue;
                };
                let x_placement = (i64::from(x_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_placement = (i64::from(y_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let x_advance = (i64::from(x_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_advance = (i64::from(y_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index].origin.x = output[index].origin.x.saturating_add(x_placement);
                output[index].origin.y = output[index].origin.y.saturating_add(y_placement);
                output[index].advance = output[index].advance.saturating_add(x_advance);
                for following in output.iter_mut().take(count).skip(index + 1) {
                    following.origin.x = following.origin.x.saturating_add(x_advance);
                    following.origin.y = following.origin.y.saturating_add(y_advance);
                }
            }
            for index in 0..count.saturating_sub(1) {
                let left = output[index].glyph_id;
                let right = output[index + 1].glyph_id;
                let previous = (index > 0).then(|| output[index - 1].glyph_id);
                let previous2 = (index > 1).then(|| output[index - 2].glyph_id);
                let next = (index + 2 < count).then(|| output[index + 2].glyph_id);
                let next2 = (index + 3 < count).then(|| output[index + 3].glyph_id);
                let Some((x_placement, y_placement, x_advance, y_advance)) = self
                    .gpos_contextual_adjustment(
                        left,
                        right,
                        GposContextOptions {
                            script_tag,
                            language_tag,
                            ppem,
                            previous,
                            previous2,
                            next,
                            next2,
                            third: None,
                            fourth: None,
                            fifth: None,
                            input_count: 2,
                            target_sequence_index: 1,
                        },
                    )?
                else {
                    continue;
                };
                let x_placement = (i64::from(x_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_placement = (i64::from(y_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let x_advance = (i64::from(x_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_advance = (i64::from(y_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index + 1].origin.x = output[index + 1].origin.x.saturating_add(x_placement);
                output[index + 1].origin.y = output[index + 1].origin.y.saturating_add(y_placement);
                output[index + 1].advance = output[index + 1].advance.saturating_add(x_advance);
                for following in output.iter_mut().take(count).skip(index + 2) {
                    following.origin.x = following.origin.x.saturating_add(x_advance);
                    following.origin.y = following.origin.y.saturating_add(y_advance);
                }
            }
            for index in 0..count {
                let glyph = output[index].glyph_id;
                let previous = (index > 0).then(|| output[index - 1].glyph_id);
                let previous2 = (index > 1).then(|| output[index - 2].glyph_id);
                let next = (index + 1 < count).then(|| output[index + 1].glyph_id);
                let next2 = (index + 2 < count).then(|| output[index + 2].glyph_id);
                let Some((x_placement, y_placement, x_advance, y_advance)) = self
                    .gpos_contextual_adjustment(
                        glyph,
                        u16::MAX,
                        GposContextOptions {
                            script_tag,
                            language_tag,
                            ppem,
                            previous,
                            previous2,
                            next,
                            next2,
                            third: None,
                            fourth: None,
                            fifth: None,
                            input_count: 1,
                            target_sequence_index: 0,
                        },
                    )?
                else {
                    continue;
                };
                let x_placement = (i64::from(x_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_placement = (i64::from(y_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let x_advance = (i64::from(x_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_advance = (i64::from(y_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index].origin.x = output[index].origin.x.saturating_add(x_placement);
                output[index].origin.y = output[index].origin.y.saturating_add(y_placement);
                output[index].advance = output[index].advance.saturating_add(x_advance);
                for following in output.iter_mut().take(count).skip(index + 1) {
                    following.origin.x = following.origin.x.saturating_add(x_advance);
                    following.origin.y = following.origin.y.saturating_add(y_advance);
                }
            }
            for index in 0..count.saturating_sub(2) {
                let left = output[index].glyph_id;
                let right = output[index + 1].glyph_id;
                let third = output[index + 2].glyph_id;
                let previous = (index > 0).then(|| output[index - 1].glyph_id);
                let previous2 = (index > 1).then(|| output[index - 2].glyph_id);
                let next = (index + 3 < count).then(|| output[index + 3].glyph_id);
                let next2 = (index + 4 < count).then(|| output[index + 4].glyph_id);
                let Some((x_placement, y_placement, x_advance, y_advance)) = self
                    .gpos_contextual_adjustment(
                        left,
                        right,
                        GposContextOptions {
                            script_tag,
                            language_tag,
                            ppem,
                            previous,
                            previous2,
                            next,
                            next2,
                            third: Some(third),
                            fourth: None,
                            fifth: None,
                            input_count: 3,
                            target_sequence_index: 2,
                        },
                    )?
                else {
                    continue;
                };
                let x_placement = (i64::from(x_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_placement = (i64::from(y_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let x_advance = (i64::from(x_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_advance = (i64::from(y_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index + 2].origin.x = output[index + 2].origin.x.saturating_add(x_placement);
                output[index + 2].origin.y = output[index + 2].origin.y.saturating_add(y_placement);
                output[index + 2].advance = output[index + 2].advance.saturating_add(x_advance);
                for following in output.iter_mut().take(count).skip(index + 3) {
                    following.origin.x = following.origin.x.saturating_add(x_advance);
                    following.origin.y = following.origin.y.saturating_add(y_advance);
                }
            }
            for index in 0..count.saturating_sub(3) {
                let left = output[index].glyph_id;
                let right = output[index + 1].glyph_id;
                let third = output[index + 2].glyph_id;
                let fourth = output[index + 3].glyph_id;
                let previous = (index > 0).then(|| output[index - 1].glyph_id);
                let previous2 = (index > 1).then(|| output[index - 2].glyph_id);
                let next = (index + 4 < count).then(|| output[index + 4].glyph_id);
                let next2 = (index + 5 < count).then(|| output[index + 5].glyph_id);
                let Some((x_placement, y_placement, x_advance, y_advance)) = self
                    .gpos_contextual_adjustment(
                        left,
                        right,
                        GposContextOptions {
                            script_tag,
                            language_tag,
                            ppem,
                            previous,
                            previous2,
                            next,
                            next2,
                            third: Some(third),
                            fourth: Some(fourth),
                            fifth: None,
                            input_count: 4,
                            target_sequence_index: 3,
                        },
                    )?
                else {
                    continue;
                };
                let x_placement = (i64::from(x_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_placement = (i64::from(y_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let x_advance = (i64::from(x_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_advance = (i64::from(y_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index + 3].origin.x = output[index + 3].origin.x.saturating_add(x_placement);
                output[index + 3].origin.y = output[index + 3].origin.y.saturating_add(y_placement);
                output[index + 3].advance = output[index + 3].advance.saturating_add(x_advance);
                for following in output.iter_mut().take(count).skip(index + 4) {
                    following.origin.x = following.origin.x.saturating_add(x_advance);
                    following.origin.y = following.origin.y.saturating_add(y_advance);
                }
            }
            for index in 0..count.saturating_sub(4) {
                let left = output[index].glyph_id;
                let right = output[index + 1].glyph_id;
                let third = output[index + 2].glyph_id;
                let fourth = output[index + 3].glyph_id;
                let fifth = output[index + 4].glyph_id;
                let previous = (index > 0).then(|| output[index - 1].glyph_id);
                let previous2 = (index > 1).then(|| output[index - 2].glyph_id);
                let next = (index + 5 < count).then(|| output[index + 5].glyph_id);
                let next2 = (index + 6 < count).then(|| output[index + 6].glyph_id);
                let Some((x_placement, y_placement, x_advance, y_advance)) = self
                    .gpos_contextual_adjustment(
                        left,
                        right,
                        GposContextOptions {
                            script_tag,
                            language_tag,
                            ppem,
                            previous,
                            previous2,
                            next,
                            next2,
                            third: Some(third),
                            fourth: Some(fourth),
                            fifth: Some(fifth),
                            input_count: 5,
                            target_sequence_index: 4,
                        },
                    )?
                else {
                    continue;
                };
                let x_placement = (i64::from(x_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_placement = (i64::from(y_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let x_advance = (i64::from(x_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_advance = (i64::from(y_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index + 4].origin.x = output[index + 4].origin.x.saturating_add(x_placement);
                output[index + 4].origin.y = output[index + 4].origin.y.saturating_add(y_placement);
                output[index + 4].advance = output[index + 4].advance.saturating_add(x_advance);
                for following in output.iter_mut().take(count).skip(index + 5) {
                    following.origin.x = following.origin.x.saturating_add(x_advance);
                    following.origin.y = following.origin.y.saturating_add(y_advance);
                }
            }
            for index in 0..count {
                let glyph = output[index].glyph_id;
                let Some((x_placement, y_placement, x_advance, y_advance)) = self
                    .gpos_pair_adjustment(
                        glyph,
                        u16::MAX,
                        GposContextNeighbors::default(),
                        script_tag,
                        language_tag,
                        ppem,
                    )?
                else {
                    continue;
                };
                let x_placement = (i64::from(x_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_placement = (i64::from(y_placement) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let x_advance = (i64::from(x_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_advance = (i64::from(y_advance) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index].origin.x = output[index].origin.x.saturating_add(x_placement);
                output[index].origin.y = output[index].origin.y.saturating_add(y_placement);
                output[index].advance = output[index].advance.saturating_add(x_advance);
                for following in output.iter_mut().take(count).skip(index + 1) {
                    following.origin.x = following.origin.x.saturating_add(x_advance);
                    following.origin.y = following.origin.y.saturating_add(y_advance);
                }
            }
            for index in 0..count.saturating_sub(1) {
                let ligature = output[index].glyph_id;
                let mark = output[index + 1].glyph_id;
                let Some((x_adjustment, y_adjustment)) = self.gpos_mark_to_ligature_adjustment(
                    ligature,
                    mark,
                    script_tag,
                    language_tag,
                    ppem,
                )?
                else {
                    continue;
                };
                let x_adjustment = (i64::from(x_adjustment) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_adjustment = (i64::from(y_adjustment) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index + 1].origin.x =
                    output[index + 1].origin.x.saturating_add(x_adjustment);
                output[index + 1].origin.y =
                    output[index + 1].origin.y.saturating_add(y_adjustment);
            }
            for index in 0..count.saturating_sub(1) {
                let base = output[index].glyph_id;
                let mark = output[index + 1].glyph_id;
                let Some((x_adjustment, y_adjustment)) =
                    self.gpos_mark_to_base_adjustment(base, mark, script_tag, language_tag, ppem)?
                else {
                    continue;
                };
                let x_adjustment = (i64::from(x_adjustment) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_adjustment = (i64::from(y_adjustment) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index + 1].origin.x =
                    output[index + 1].origin.x.saturating_add(x_adjustment);
                output[index + 1].origin.y =
                    output[index + 1].origin.y.saturating_add(y_adjustment);
            }
            for index in 0..count.saturating_sub(1) {
                let left = output[index].glyph_id;
                let right = output[index + 1].glyph_id;
                let Some((x_adjustment, y_adjustment)) =
                    self.gpos_cursive_adjustment(left, right, script_tag, language_tag, ppem)?
                else {
                    continue;
                };
                let x_adjustment = (i64::from(x_adjustment) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                let y_adjustment = (i64::from(y_adjustment) * scale)
                    .div_euclid(units)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                output[index + 1].origin.x =
                    output[index + 1].origin.x.saturating_add(x_adjustment);
                output[index + 1].origin.y =
                    output[index + 1].origin.y.saturating_add(y_adjustment);
            }
            Ok(())
        }

        /// Apply bounded OpenType liga/rlig substitutions and emit shaped
        /// glyph IDs with scaled advances. Fonts without those features fall
        /// back to one glyph per character; malformed tables fail closed.
        pub fn shape_text_ligatures(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            output: &mut [ShapedGlyph],
        ) -> Result<usize, TrueTypeError> {
            self.shape_text_ligatures_with_options(
                text,
                origin,
                scale,
                TextShapingOptions::default(),
                output,
            )
        }

        /// Apply GSUB and GPOS with an explicit ppem for device deltas.
        pub fn shape_text_ligatures_with_ppem(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            ppem: u16,
            output: &mut [ShapedGlyph],
        ) -> Result<usize, TrueTypeError> {
            self.shape_text_ligatures_with_options(
                text,
                origin,
                scale,
                TextShapingOptions {
                    ppem: Some(ppem),
                    ..TextShapingOptions::default()
                },
                output,
            )
        }

        /// Apply GSUB ligatures selected by the default language system of a
        /// bounded OpenType script record.
        pub fn shape_text_ligatures_with_script(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            script_tag: [u8; 4],
            output: &mut [ShapedGlyph],
        ) -> Result<usize, TrueTypeError> {
            self.shape_text_ligatures_with_options(
                text,
                origin,
                scale,
                TextShapingOptions {
                    script_tag: Some(script_tag),
                    ..TextShapingOptions::default()
                },
                output,
            )
        }

        /// Apply script-scoped GSUB and GPOS with an explicit ppem for
        /// device-table deltas.
        pub fn shape_text_ligatures_with_script_and_ppem(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            script_tag: [u8; 4],
            ppem: u16,
            output: &mut [ShapedGlyph],
        ) -> Result<usize, TrueTypeError> {
            self.shape_text_ligatures_with_options(
                text,
                origin,
                scale,
                TextShapingOptions {
                    script_tag: Some(script_tag),
                    ppem: Some(ppem),
                    ..TextShapingOptions::default()
                },
                output,
            )
        }

        /// Apply GSUB and GPOS using a named language system within a
        /// bounded OpenType script record.
        pub fn shape_text_ligatures_with_script_language(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            script_tag: [u8; 4],
            language_tag: [u8; 4],
            output: &mut [ShapedGlyph],
        ) -> Result<usize, TrueTypeError> {
            self.shape_text_ligatures_with_options(
                text,
                origin,
                scale,
                TextShapingOptions {
                    script_tag: Some(script_tag),
                    language_tag: Some(language_tag),
                    ..TextShapingOptions::default()
                },
                output,
            )
        }

        /// Apply GSUB/GPOS using explicit script, language-system and ppem
        /// options.
        pub fn shape_text_ligatures_with_options(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            options: TextShapingOptions,
            output: &mut [ShapedGlyph],
        ) -> Result<usize, TrueTypeError> {
            self.shape_text_ligatures_with_device_ppem(text, origin, scale, options, output)
        }

        fn shape_text_ligatures_with_device_ppem(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            options: TextShapingOptions,
            output: &mut [ShapedGlyph],
        ) -> Result<usize, TrueTypeError> {
            let mut glyphs = [0u16; 128];
            let mut characters = ['\0'; 128];
            let mut input_count = 0usize;
            for character in text.chars() {
                if input_count == glyphs.len() {
                    return Err(TrueTypeError::TooLarge);
                }
                glyphs[input_count] = self
                    .glyph_index(character)
                    .ok_or(TrueTypeError::UnsupportedGlyph)?;
                characters[input_count] = character;
                input_count += 1;
            }
            let scale = i64::from(scale.max(1));
            let units = i64::from(self.units_per_em.max(1));
            let mut cursor = origin.x;
            let mut input = 0usize;
            let mut output_count = 0usize;
            while input < input_count {
                if output_count == output.len() {
                    return Err(TrueTypeError::TooLarge);
                }
                let substitution = self.gsub_substitution_pipeline(
                    &glyphs,
                    input,
                    options.script_tag,
                    options.language_tag,
                )?;
                match substitution {
                    GsubSubstitution::Single { glyph, consumed } => {
                        let advance = self
                            .horizontal_metric(usize::from(glyph))
                            .ok_or(TrueTypeError::BadTableDirectory)?
                            .0;
                        let pixels = (i64::from(advance) * scale)
                            .div_euclid(units)
                            .clamp(1, i64::from(i32::MAX))
                            as i32;
                        output[output_count] = ShapedGlyph {
                            glyph_id: glyph,
                            source: characters[input],
                            source_count: u8::try_from(consumed)
                                .map_err(|_| TrueTypeError::TooLarge)?,
                            origin: Point::new(cursor, origin.y),
                            advance: pixels,
                        };
                        cursor = cursor.saturating_add(pixels);
                        input += consumed;
                        output_count += 1;
                    }
                    GsubSubstitution::Multiple {
                        glyphs: replacement,
                        count,
                    } => {
                        if output_count
                            .checked_add(count)
                            .is_none_or(|end| end > output.len())
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        for (index, glyph) in replacement.iter().copied().take(count).enumerate() {
                            let advance = self
                                .horizontal_metric(usize::from(glyph))
                                .ok_or(TrueTypeError::BadTableDirectory)?
                                .0;
                            let pixels = (i64::from(advance) * scale)
                                .div_euclid(units)
                                .clamp(1, i64::from(i32::MAX))
                                as i32;
                            output[output_count] = ShapedGlyph {
                                glyph_id: glyph,
                                source: if index == 0 { characters[input] } else { '\0' },
                                source_count: if index == 0 { 1 } else { 0 },
                                origin: Point::new(cursor, origin.y),
                                advance: pixels,
                            };
                            cursor = cursor.saturating_add(pixels);
                            output_count += 1;
                        }
                        input += 1;
                    }
                }
            }
            self.apply_gpos_adjustments(
                output,
                output_count,
                scale,
                options.script_tag,
                options.language_tag,
                options.ppem,
            )?;
            Ok(output_count)
        }

        /// Apply GSUB ligatures while deriving advances from bounded gvar
        /// phantom metrics at the supplied normalized coordinates.
        pub fn shape_text_ligatures_with_variation(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            normalized_coordinates: &[i16],
            output: &mut [ShapedGlyph],
        ) -> Result<usize, TrueTypeError> {
            let mut glyphs = [0u16; 128];
            let mut characters = ['\0'; 128];
            let mut input_count = 0usize;
            for character in text.chars() {
                if input_count == glyphs.len() {
                    return Err(TrueTypeError::TooLarge);
                }
                glyphs[input_count] = self
                    .glyph_index(character)
                    .ok_or(TrueTypeError::UnsupportedGlyph)?;
                characters[input_count] = character;
                input_count += 1;
            }
            let scale = i64::from(scale.max(1));
            let units = i64::from(self.units_per_em.max(1));
            let mut cursor = origin.x;
            let mut input = 0usize;
            let mut output_count = 0usize;
            while input < input_count {
                if output_count == output.len() {
                    return Err(TrueTypeError::TooLarge);
                }
                let substitution = self.gsub_substitution_pipeline(&glyphs, input, None, None)?;
                match substitution {
                    GsubSubstitution::Single { glyph, consumed } => {
                        let advance =
                            self.advance_width_glyph_with_variation(glyph, normalized_coordinates)?;
                        let pixels = (i64::from(advance) * scale)
                            .div_euclid(units)
                            .clamp(1, i64::from(i32::MAX))
                            as i32;
                        output[output_count] = ShapedGlyph {
                            glyph_id: glyph,
                            source: characters[input],
                            source_count: u8::try_from(consumed)
                                .map_err(|_| TrueTypeError::TooLarge)?,
                            origin: Point::new(cursor, origin.y),
                            advance: pixels,
                        };
                        cursor = cursor.saturating_add(pixels);
                        input += consumed;
                        output_count += 1;
                    }
                    GsubSubstitution::Multiple {
                        glyphs: replacement,
                        count,
                    } => {
                        if output_count
                            .checked_add(count)
                            .is_none_or(|end| end > output.len())
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        for (index, glyph) in replacement.iter().copied().take(count).enumerate() {
                            let advance = self.advance_width_glyph_with_variation(
                                glyph,
                                normalized_coordinates,
                            )?;
                            let pixels = (i64::from(advance) * scale)
                                .div_euclid(units)
                                .clamp(1, i64::from(i32::MAX))
                                as i32;
                            output[output_count] = ShapedGlyph {
                                glyph_id: glyph,
                                source: if index == 0 { characters[input] } else { '\0' },
                                source_count: if index == 0 { 1 } else { 0 },
                                origin: Point::new(cursor, origin.y),
                                advance: pixels,
                            };
                            cursor = cursor.saturating_add(pixels);
                            output_count += 1;
                        }
                        input += 1;
                    }
                }
            }
            self.apply_gpos_adjustments(output, output_count, scale, None, None, None)?;
            Ok(output_count)
        }

        /// Shape and rasterize one bounded single-line run, preserving GSUB
        /// glyph IDs through the outline pipeline.
        pub fn render_shaped_text(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            color: Color,
            renderer: &mut Renderer<'_, '_>,
        ) -> Result<usize, TrueTypeError> {
            let mut shaped = [ShapedGlyph {
                glyph_id: 0,
                source: '\0',
                source_count: 0,
                origin: Point::new(0, 0),
                advance: 0,
            }; 128];
            let count = self.shape_text_ligatures(text, origin, scale, &mut shaped)?;
            for glyph in shaped.iter().take(count) {
                self.render_glyph_id(glyph.glyph_id, glyph.origin, scale, color, renderer)?;
            }
            Ok(count)
        }

        /// Shape and rasterize with normalized variation coordinates carried
        /// through both metrics and direct glyph-ID outlines.
        pub fn render_shaped_text_with_variation(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            normalized_coordinates: &[i16],
            color: Color,
            renderer: &mut Renderer<'_, '_>,
        ) -> Result<usize, TrueTypeError> {
            let mut shaped = [ShapedGlyph {
                glyph_id: 0,
                source: '\0',
                source_count: 0,
                origin: Point::new(0, 0),
                advance: 0,
            }; 128];
            let count = self.shape_text_ligatures_with_variation(
                text,
                origin,
                scale,
                normalized_coordinates,
                &mut shaped,
            )?;
            for glyph in shaped.iter().take(count) {
                self.render_glyph_id_with_variation(
                    glyph.glyph_id,
                    glyph.origin,
                    scale,
                    normalized_coordinates,
                    color,
                    renderer,
                )?;
            }
            Ok(count)
        }

        /// Lay out text with explicit newline and character-wrap handling.
        /// The caller supplies the line height and maximum width in pixels.
        pub fn layout_text_box(
            &self,
            text: &str,
            origin: Point,
            max_width: i32,
            line_height: i32,
            scale: u16,
            output: &mut [GlyphPlacement],
        ) -> Result<usize, TrueTypeError> {
            if max_width <= 0 || line_height <= 0 {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let scale = i64::from(scale.max(1));
            let units = i64::from(self.units_per_em.max(1));
            let right = origin.x.saturating_add(max_width);
            let mut cursor_x = origin.x;
            let mut cursor_y = origin.y;
            let mut previous = None;
            let mut count = 0;
            for character in text.chars() {
                if character == '\n' {
                    cursor_x = origin.x;
                    cursor_y = cursor_y.saturating_add(line_height);
                    previous = None;
                    continue;
                }
                let advance = self
                    .advance_width(character)
                    .ok_or(TrueTypeError::UnsupportedGlyph)?;
                let pixels = (i64::from(advance) * scale)
                    .div_euclid(units)
                    .clamp(1, i64::from(i32::MAX)) as i32;
                let kern_pixels = previous
                    .and_then(|left| self.kerning(left, character))
                    .map(|kern| {
                        ((i64::from(kern) * scale).div_euclid(units))
                            .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                            as i32
                    })
                    .unwrap_or(0);
                let proposed = cursor_x.saturating_add(kern_pixels);
                if proposed != origin.x && proposed.saturating_add(pixels) > right {
                    cursor_x = origin.x;
                    cursor_y = cursor_y.saturating_add(line_height);
                } else {
                    cursor_x = proposed;
                }
                if count >= output.len() {
                    return Err(TrueTypeError::TooLarge);
                }
                output[count] = GlyphPlacement {
                    character,
                    origin: Point::new(cursor_x, cursor_y),
                    advance: pixels,
                };
                cursor_x = cursor_x.saturating_add(pixels);
                previous = Some(character);
                count += 1;
            }
            Ok(count)
        }

        /// Lay out text without splitting words at spaces. A word wider than
        /// the box is kept intact, matching bounded terminal-style behavior.
        pub fn layout_text_word_box(
            &self,
            text: &str,
            origin: Point,
            max_width: i32,
            line_height: i32,
            scale: u16,
            output: &mut [GlyphPlacement],
        ) -> Result<usize, TrueTypeError> {
            if max_width <= 0 || line_height <= 0 {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let scale = i64::from(scale.max(1));
            let units = i64::from(self.units_per_em.max(1));
            let right = origin.x.saturating_add(max_width);
            let mut cursor_x = origin.x;
            let mut cursor_y = origin.y;
            let mut previous = None;
            let mut count = 0;
            let mut offset = 0;
            while offset < text.len() {
                let character = text[offset..]
                    .chars()
                    .next()
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let character_len = character.len_utf8();
                if character == '\n' {
                    cursor_x = origin.x;
                    cursor_y = cursor_y.saturating_add(line_height);
                    previous = None;
                    offset += character_len;
                    continue;
                }
                if character == ' ' {
                    let advance = self
                        .advance_width(character)
                        .ok_or(TrueTypeError::UnsupportedGlyph)?;
                    let pixels = (i64::from(advance) * scale)
                        .div_euclid(units)
                        .clamp(1, i64::from(i32::MAX)) as i32;
                    if cursor_x != origin.x && cursor_x.saturating_add(pixels) > right {
                        cursor_x = origin.x;
                        cursor_y = cursor_y.saturating_add(line_height);
                        previous = None;
                    } else {
                        if count >= output.len() {
                            return Err(TrueTypeError::TooLarge);
                        }
                        output[count] = GlyphPlacement {
                            character,
                            origin: Point::new(cursor_x, cursor_y),
                            advance: pixels,
                        };
                        cursor_x = cursor_x.saturating_add(pixels);
                        previous = Some(character);
                        count += 1;
                    }
                    offset += character_len;
                    continue;
                }
                let word_start = offset;
                let mut word_end = offset;
                let mut word_width = 0i32;
                let mut word_previous = None;
                while word_end < text.len() {
                    let word_character = text[word_end..]
                        .chars()
                        .next()
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if word_character == ' ' || word_character == '\n' {
                        break;
                    }
                    let advance = self
                        .advance_width(word_character)
                        .ok_or(TrueTypeError::UnsupportedGlyph)?;
                    if let Some(left) = word_previous
                        && let Some(kern) = self.kerning(left, word_character)
                    {
                        word_width = word_width.saturating_add(
                            ((i64::from(kern) * scale).div_euclid(units))
                                .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                                as i32,
                        );
                    }
                    word_width = word_width.saturating_add(
                        ((i64::from(advance) * scale).div_euclid(units))
                            .clamp(1, i64::from(i32::MAX)) as i32,
                    );
                    word_previous = Some(word_character);
                    word_end += word_character.len_utf8();
                }
                if cursor_x != origin.x && cursor_x.saturating_add(word_width) > right {
                    cursor_x = origin.x;
                    cursor_y = cursor_y.saturating_add(line_height);
                    previous = None;
                }
                while offset < word_end {
                    let word_character = text[offset..]
                        .chars()
                        .next()
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let advance = self
                        .advance_width(word_character)
                        .ok_or(TrueTypeError::UnsupportedGlyph)?;
                    let kern_pixels = previous
                        .and_then(|left| self.kerning(left, word_character))
                        .map(|kern| {
                            ((i64::from(kern) * scale).div_euclid(units))
                                .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                                as i32
                        })
                        .unwrap_or(0);
                    cursor_x = cursor_x.saturating_add(kern_pixels);
                    let pixels = ((i64::from(advance) * scale).div_euclid(units))
                        .clamp(1, i64::from(i32::MAX)) as i32;
                    if count >= output.len() {
                        return Err(TrueTypeError::TooLarge);
                    }
                    output[count] = GlyphPlacement {
                        character: word_character,
                        origin: Point::new(cursor_x, cursor_y),
                        advance: pixels,
                    };
                    cursor_x = cursor_x.saturating_add(pixels);
                    previous = Some(word_character);
                    count += 1;
                    offset += word_character.len_utf8();
                }
                if word_start == word_end {
                    return Err(TrueTypeError::BadTableDirectory);
                }
            }
            Ok(count)
        }

        /// Lay out and rasterize a single-line string through the bounded
        /// antialiased glyph renderer.
        pub fn render_text(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            color: Color,
            renderer: &mut Renderer<'_, '_>,
        ) -> Result<usize, TrueTypeError> {
            let mut placements = [GlyphPlacement {
                character: '\0',
                origin: Point::new(0, 0),
                advance: 0,
            }; 128];
            let count = self.layout_text(text, origin, scale, &mut placements)?;
            for placement in placements.iter().take(count) {
                self.render_glyph(
                    placement.character,
                    placement.origin,
                    scale,
                    color,
                    renderer,
                )?;
            }
            Ok(count)
        }

        /// Render a single-line string, falling back to the shared bitmap
        /// font when this face does not map or cannot outline a character.
        /// `bitmap_scale` is independently bounded by the bitmap renderer.
        pub fn render_text_with_bitmap_fallback(
            &self,
            text: &str,
            origin: Point,
            scale: u16,
            bitmap_scale: usize,
            color: Color,
            renderer: &mut Renderer<'_, '_>,
        ) -> Result<usize, TrueTypeError> {
            let outline_scale = i64::from(scale.max(1));
            let units = i64::from(self.units_per_em.max(1));
            let mut cursor_x = origin.x;
            let mut count = 0;
            for character in text.chars() {
                if character == '\n' {
                    return Err(TrueTypeError::TooLarge);
                }
                let mut encoded = [0u8; 4];
                let encoded = character.encode_utf8(&mut encoded);
                let advance = self.advance_width(character);
                let mut used_outline = false;
                if advance.is_some() && self.glyph_index(character).is_some() {
                    match self.render_glyph(
                        character,
                        Point::new(cursor_x, origin.y),
                        scale,
                        color,
                        renderer,
                    ) {
                        Ok(_) => used_outline = true,
                        Err(TrueTypeError::UnsupportedGlyph) => {}
                        Err(error) => return Err(error),
                    }
                }
                if used_outline {
                    let pixels = (i64::from(advance.unwrap_or(0)) * outline_scale)
                        .div_euclid(units)
                        .clamp(1, i64::from(i32::MAX)) as i32;
                    cursor_x = cursor_x.saturating_add(pixels);
                } else {
                    renderer.text_scaled(cursor_x, origin.y, encoded, color, bitmap_scale);
                    cursor_x = cursor_x.saturating_add(
                        crate::font::ADVANCE.saturating_mul(bitmap_scale.clamp(1, 16) as i32),
                    );
                }
                count += 1;
            }
            Ok(count)
        }

        /// Return the glyph ID selected by the validated format-4 cmap.
        /// Non-BMP code points and unmapped values return `None`.
        pub fn glyph_index(&self, character: char) -> Option<u16> {
            let code = u32::from(character);
            if let Some((base, length)) = self.cmap12 {
                let groups = usize::try_from(read_u32(self.bytes, base + 12)?).ok()?;
                let groups_end = base.checked_add(16)?.checked_add(groups.checked_mul(12)?)?;
                if length >= 16 && groups_end <= base.checked_add(length)? {
                    for group in 0..groups {
                        let entry = base + 16 + group * 12;
                        let start = read_u32(self.bytes, entry)?;
                        let end = read_u32(self.bytes, entry + 4)?;
                        if code < start {
                            break;
                        }
                        if code <= end {
                            let glyph =
                                read_u32(self.bytes, entry + 8)?.checked_add(code - start)?;
                            return u16::try_from(glyph)
                                .ok()
                                .filter(|glyph| *glyph < self.glyph_count);
                        }
                    }
                }
            }
            if code > u32::from(u16::MAX) {
                return None;
            }
            let code = code as u16;
            let (base, cmap4_length) = self.cmap4?;
            let segment_count = usize::from(read_u16(self.bytes, base + 6)?) / 2;
            let end_codes = base.checked_add(14)?;
            let reserved_pad = end_codes.checked_add(segment_count.checked_mul(2)?)?;
            if reserved_pad.checked_add(2)? > base + cmap4_length {
                return None;
            }
            let start_codes = reserved_pad + 2;
            let deltas = start_codes.checked_add(segment_count.checked_mul(2)?)?;
            let range_offsets = deltas.checked_add(segment_count.checked_mul(2)?)?;
            for segment in 0..segment_count {
                let end = read_u16(self.bytes, end_codes + segment * 2)?;
                if code > end {
                    continue;
                }
                let start = read_u16(self.bytes, start_codes + segment * 2)?;
                if code < start {
                    return None;
                }
                let delta = read_i16(self.bytes, deltas + segment * 2)?;
                let range = read_u16(self.bytes, range_offsets + segment * 2)?;
                let glyph = if range == 0 {
                    code.wrapping_add(delta as u16)
                } else {
                    let location = range_offsets
                        .checked_add(segment * 2)?
                        .checked_add(usize::from(range))?
                        .checked_add(usize::from(code - start) * 2)?;
                    let raw = read_u16(self.bytes, location)?;
                    if raw == 0 {
                        0
                    } else {
                        raw.wrapping_add(delta as u16)
                    }
                };
                return (glyph < self.glyph_count).then_some(glyph);
            }
            None
        }

        pub const fn table_span(&self) -> (usize, usize) {
            (self.cmap_offset, self.cmap_length)
        }
    }

    fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
        Some(u16::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]))
    }

    fn read_i16(bytes: &[u8], offset: usize) -> Option<i16> {
        Some(i16::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]))
    }

    fn read_i32(bytes: &[u8], offset: usize) -> Option<i32> {
        Some(i32::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
            *bytes.get(offset + 2)?,
            *bytes.get(offset + 3)?,
        ]))
    }

    fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
            *bytes.get(offset + 2)?,
            *bytes.get(offset + 3)?,
        ]))
    }

    fn layout_lang_sys(
        bytes: &[u8],
        script_table: usize,
        language_tag: Option<[u8; 4]>,
        table_end: usize,
    ) -> Result<Option<usize>, TrueTypeError> {
        if script_table
            .checked_add(4)
            .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let default_offset =
            usize::from(read_u16(bytes, script_table).ok_or(TrueTypeError::BadTableDirectory)?);
        let language_count =
            usize::from(read_u16(bytes, script_table + 2).ok_or(TrueTypeError::BadTableDirectory)?);
        if language_count > 32
            || script_table
                .checked_add(
                    4usize
                        .checked_add(
                            language_count
                                .checked_mul(6)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        let language_offset = if let Some(language_tag) = language_tag {
            let mut selected = None;
            for index in 0..language_count {
                let record = script_table + 4 + index * 6;
                if bytes
                    .get(record..record + 4)
                    .ok_or(TrueTypeError::BadTableDirectory)?
                    == language_tag
                {
                    selected = Some(usize::from(
                        read_u16(bytes, record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    ));
                    break;
                }
            }
            let Some(selected) = selected else {
                return Ok(None);
            };
            selected
        } else {
            if default_offset == 0 {
                return Ok(None);
            }
            default_offset
        };
        let lang_sys = script_table
            .checked_add(language_offset)
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if lang_sys.checked_add(6).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        Ok(Some(lang_sys))
    }

    fn gsub_single_lookup(
        bytes: &[u8],
        lookup_table: usize,
        glyph: u16,
        table_end: usize,
    ) -> Result<Option<u16>, TrueTypeError> {
        if lookup_table
            .checked_add(6)
            .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::BadTableDirectory);
        }
        if read_u16(bytes, lookup_table) != Some(1) {
            return Ok(None);
        }
        let count =
            usize::from(read_u16(bytes, lookup_table + 4).ok_or(TrueTypeError::BadTableDirectory)?);
        if count > 32
            || lookup_table
                .checked_add(
                    6usize
                        .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        for index in 0..count {
            let subtable = lookup_table
                .checked_add(usize::from(
                    read_u16(bytes, lookup_table + 6 + index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if subtable.checked_add(6).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let format = read_u16(bytes, subtable).ok_or(TrueTypeError::BadTableDirectory)?;
            let coverage = subtable
                .checked_add(usize::from(
                    read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let Some(coverage_index) = gsub_coverage_index(bytes, coverage, table_end, glyph)?
            else {
                continue;
            };
            let substitute = match format {
                1 => {
                    let delta = i32::from(
                        read_i16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    let value = i32::from(glyph) + delta;
                    u16::try_from(value).map_err(|_| TrueTypeError::BadTableDirectory)?
                }
                2 => {
                    let count = usize::from(
                        read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if count > 128 || coverage_index >= count {
                        return Err(TrueTypeError::TooLarge);
                    }
                    let end = subtable
                        .checked_add(
                            6usize
                                .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?;
                    if end > table_end {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    read_u16(bytes, subtable + 6 + coverage_index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?
                }
                _ => return Err(TrueTypeError::UnsupportedCmap),
            };
            return Ok(Some(substitute));
        }
        Ok(None)
    }

    fn gsub_single_subtable(
        bytes: &[u8],
        subtable: usize,
        glyph: u16,
        table_end: usize,
    ) -> Result<Option<u16>, TrueTypeError> {
        if subtable.checked_add(6).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let format = read_u16(bytes, subtable).ok_or(TrueTypeError::BadTableDirectory)?;
        let coverage = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let Some(coverage_index) = gsub_coverage_index(bytes, coverage, table_end, glyph)? else {
            return Ok(None);
        };
        match format {
            1 => {
                let delta = i32::from(
                    read_i16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let substitute = i32::from(glyph) + delta;
                Ok(Some(
                    u16::try_from(substitute).map_err(|_| TrueTypeError::BadTableDirectory)?,
                ))
            }
            2 => {
                let count = usize::from(
                    read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if count > 128 || coverage_index >= count {
                    return Err(TrueTypeError::TooLarge);
                }
                let end = subtable
                    .checked_add(
                        6usize
                            .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if end > table_end {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                Ok(Some(
                    read_u16(bytes, subtable + 6 + coverage_index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                ))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn gsub_multiple_subtable(
        bytes: &[u8],
        subtable: usize,
        glyph: u16,
        table_end: usize,
    ) -> Result<Option<([u16; 16], usize)>, TrueTypeError> {
        if subtable.checked_add(6).is_none_or(|end| end > table_end)
            || read_u16(bytes, subtable) != Some(1)
        {
            return Ok(None);
        }
        let coverage = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let Some(coverage_index) = gsub_coverage_index(bytes, coverage, table_end, glyph)? else {
            return Ok(None);
        };
        let sequence_count =
            usize::from(read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?);
        if sequence_count == 0 || sequence_count > 128 || coverage_index >= sequence_count {
            return Err(TrueTypeError::TooLarge);
        }
        let offsets_end = subtable
            .checked_add(
                6usize
                    .checked_add(
                        sequence_count
                            .checked_mul(2)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        if offsets_end > table_end {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let sequence = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 6 + coverage_index * 2)
                    .ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if sequence.checked_add(2).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let count = usize::from(read_u16(bytes, sequence).ok_or(TrueTypeError::BadTableDirectory)?);
        if !(1..=16).contains(&count) {
            return Err(TrueTypeError::TooLarge);
        }
        let end = sequence
            .checked_add(
                2usize
                    .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        if end > table_end {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let mut result = [0u16; 16];
        for (index, value) in result.iter_mut().take(count).enumerate() {
            *value = read_u16(bytes, sequence + 2 + index * 2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
        }
        Ok(Some((result, count)))
    }

    pub(crate) fn gsub_alternate_subtable(
        bytes: &[u8],
        subtable: usize,
        glyph: u16,
        table_end: usize,
    ) -> Result<Option<u16>, TrueTypeError> {
        if subtable.checked_add(6).is_none_or(|end| end > table_end)
            || read_u16(bytes, subtable) != Some(1)
        {
            return Ok(None);
        }
        let coverage = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let Some(coverage_index) = gsub_coverage_index(bytes, coverage, table_end, glyph)? else {
            return Ok(None);
        };
        let set_count =
            usize::from(read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?);
        if set_count > 128 || coverage_index >= set_count {
            return Err(TrueTypeError::TooLarge);
        }
        let sets_end = subtable
            .checked_add(
                6usize
                    .checked_add(set_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        if sets_end > table_end {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let set = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 6 + coverage_index * 2)
                    .ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if set.checked_add(4).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let alternate_count =
            usize::from(read_u16(bytes, set).ok_or(TrueTypeError::BadTableDirectory)?);
        if alternate_count == 0 || alternate_count > 128 {
            return Err(TrueTypeError::TooLarge);
        }
        let end = set
            .checked_add(
                2usize
                    .checked_add(
                        alternate_count
                            .checked_mul(2)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        if end > table_end {
            return Err(TrueTypeError::BadTableDirectory);
        }
        Ok(Some(
            read_u16(bytes, set + 2).ok_or(TrueTypeError::BadTableDirectory)?,
        ))
    }

    pub(crate) fn gsub_reverse_chaining_substitution(
        bytes: &[u8],
        subtable: usize,
        glyphs: &[u16],
        position: usize,
        table_end: usize,
    ) -> Result<Option<u16>, TrueTypeError> {
        if subtable.checked_add(4).is_none_or(|end| end > table_end)
            || read_u16(bytes, subtable) != Some(1)
        {
            return Ok(None);
        }
        let coverage = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let Some(coverage_index) =
            gsub_coverage_index(bytes, coverage, table_end, glyphs[position])?
        else {
            return Ok(None);
        };
        let mut cursor = subtable + 4;
        let backtrack_count =
            usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
        cursor += 2;
        if backtrack_count > 16
            || cursor
                .checked_add(
                    backtrack_count
                        .checked_mul(2)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        let backtrack_start = cursor;
        cursor = cursor
            .checked_add(
                backtrack_count
                    .checked_mul(2)
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        let lookahead_count =
            usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
        cursor += 2;
        if lookahead_count > 16
            || cursor
                .checked_add(
                    lookahead_count
                        .checked_mul(2)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        let lookahead_start = cursor;
        cursor = cursor
            .checked_add(
                lookahead_count
                    .checked_mul(2)
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        let substitute_count =
            usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
        if substitute_count > 128 || coverage_index >= substitute_count {
            return Err(TrueTypeError::TooLarge);
        }
        let substitute_start = cursor.checked_add(2).ok_or(TrueTypeError::TooLarge)?;
        let substitute_end = substitute_start
            .checked_add(
                substitute_count
                    .checked_mul(2)
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        if substitute_end > table_end {
            return Err(TrueTypeError::BadTableDirectory);
        }
        for index in 0..backtrack_count {
            let Some(glyph_position) = position.checked_sub(index + 1) else {
                return Ok(None);
            };
            let coverage = subtable
                .checked_add(usize::from(
                    read_u16(bytes, backtrack_start + index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if gsub_coverage_index(bytes, coverage, table_end, glyphs[glyph_position])?.is_none() {
                return Ok(None);
            }
        }
        for index in 0..lookahead_count {
            let Some(glyph_position) = position
                .checked_add(1)
                .and_then(|value| value.checked_add(index))
            else {
                return Ok(None);
            };
            if glyph_position >= glyphs.len() {
                return Ok(None);
            }
            let coverage = subtable
                .checked_add(usize::from(
                    read_u16(bytes, lookahead_start + index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if gsub_coverage_index(bytes, coverage, table_end, glyphs[glyph_position])?.is_none() {
                return Ok(None);
            }
        }
        Ok(Some(
            read_u16(bytes, substitute_start + coverage_index * 2)
                .ok_or(TrueTypeError::BadTableDirectory)?,
        ))
    }

    fn gsub_ligature_subtable(
        bytes: &[u8],
        subtable: usize,
        glyphs: &[u16],
        position: usize,
        table_end: usize,
    ) -> Result<Option<(u16, usize)>, TrueTypeError> {
        if subtable.checked_add(6).is_none_or(|end| end > table_end)
            || read_u16(bytes, subtable) != Some(1)
        {
            return Ok(None);
        }
        let coverage = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let Some(coverage_index) =
            gsub_coverage_index(bytes, coverage, table_end, glyphs[position])?
        else {
            return Ok(None);
        };
        let set_count =
            usize::from(read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?);
        if coverage_index >= set_count
            || set_count > 128
            || subtable
                .checked_add(
                    6usize
                        .checked_add(set_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        let set = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 6 + coverage_index * 2)
                    .ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if set.checked_add(2).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let ligature_count =
            usize::from(read_u16(bytes, set).ok_or(TrueTypeError::BadTableDirectory)?);
        if ligature_count > 64
            || set
                .checked_add(
                    2usize
                        .checked_add(
                            ligature_count
                                .checked_mul(2)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        for ligature_index in 0..ligature_count {
            let ligature = set
                .checked_add(usize::from(
                    read_u16(bytes, set + 2 + ligature_index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if ligature.checked_add(4).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let ligature_glyph =
                read_u16(bytes, ligature).ok_or(TrueTypeError::BadTableDirectory)?;
            let component_count =
                usize::from(read_u16(bytes, ligature + 2).ok_or(TrueTypeError::BadTableDirectory)?);
            let component_bytes = (component_count.saturating_sub(1))
                .checked_mul(2)
                .ok_or(TrueTypeError::TooLarge)?;
            if !(2..=32).contains(&component_count)
                || ligature
                    .checked_add(
                        4usize
                            .checked_add(component_bytes)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            let Some(end_position) = position.checked_add(component_count) else {
                return Err(TrueTypeError::TooLarge);
            };
            if end_position > glyphs.len() {
                continue;
            }
            let mut matches = true;
            for component in 1..component_count {
                if read_u16(bytes, ligature + 4 + (component - 1) * 2)
                    != Some(glyphs[position + component])
                {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Ok(Some((ligature_glyph, component_count)));
            }
        }
        Ok(None)
    }

    fn gsub_chaining_class_substitution(
        bytes: &[u8],
        lookup_table: usize,
        lookup_base: usize,
        glyphs: &[u16],
        position: usize,
        table_end: usize,
        extension: Option<(usize, u16)>,
    ) -> Result<Option<u16>, TrueTypeError> {
        if lookup_table
            .checked_add(6)
            .is_none_or(|end| end > table_end)
            || (extension.is_none() && read_u16(bytes, lookup_table) != Some(6))
        {
            return Ok(None);
        }
        let lookup_count =
            usize::from(read_u16(bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?);
        let subtable_count = if extension.is_some() {
            1
        } else {
            usize::from(read_u16(bytes, lookup_table + 4).ok_or(TrueTypeError::BadTableDirectory)?)
        };
        if lookup_count > 128 || subtable_count > 32 {
            return Err(TrueTypeError::TooLarge);
        }
        let lookup_end = lookup_base
            .checked_add(
                2usize
                    .checked_add(lookup_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        let subtable_end = if let Some((subtable, _)) = extension {
            subtable.checked_add(2).ok_or(TrueTypeError::TooLarge)?
        } else {
            lookup_table
                .checked_add(
                    6usize
                        .checked_add(
                            subtable_count
                                .checked_mul(2)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::TooLarge)?
        };
        if lookup_end > table_end || subtable_end > table_end {
            return Err(TrueTypeError::BadTableDirectory);
        }
        for subtable_index in 0..subtable_count {
            let subtable = if let Some((subtable, _)) = extension {
                subtable
            } else {
                lookup_table
                    .checked_add(usize::from(
                        read_u16(bytes, lookup_table + 6 + subtable_index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?
            };
            if subtable.checked_add(12).is_none_or(|end| end > table_end)
                || extension.is_some_and(|(_, kind)| kind != 6)
                || read_u16(bytes, subtable) != Some(2)
            {
                continue;
            }
            let coverage = subtable
                .checked_add(usize::from(
                    read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if gsub_coverage_index(bytes, coverage, table_end, glyphs[position])?.is_none() {
                continue;
            }
            let backtrack_definition = subtable
                .checked_add(usize::from(
                    read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let input_definition = subtable
                .checked_add(usize::from(
                    read_u16(bytes, subtable + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let lookahead_definition = subtable
                .checked_add(usize::from(
                    read_u16(bytes, subtable + 8).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let class_set_count = usize::from(
                read_u16(bytes, subtable + 10).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if class_set_count > 128
                || subtable
                    .checked_add(
                        12usize
                            .checked_add(
                                class_set_count
                                    .checked_mul(2)
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            let current_class = usize::from(gpos_class_index(
                bytes,
                input_definition,
                glyphs[position],
                table_end,
            )?);
            if current_class >= class_set_count {
                continue;
            }
            let class_set_offset = usize::from(
                read_u16(bytes, subtable + 12 + current_class * 2)
                    .ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if class_set_offset == 0 {
                continue;
            }
            let class_set = subtable
                .checked_add(class_set_offset)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if class_set.checked_add(2).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let rule_count =
                usize::from(read_u16(bytes, class_set).ok_or(TrueTypeError::BadTableDirectory)?);
            if rule_count > 64
                || class_set
                    .checked_add(
                        2usize
                            .checked_add(rule_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for rule_index in 0..rule_count {
                let rule = class_set
                    .checked_add(usize::from(
                        read_u16(bytes, class_set + 2 + rule_index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let mut cursor = rule;
                let backtrack_count =
                    usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
                cursor += 2;
                if backtrack_count > 16
                    || cursor
                        .checked_add(
                            backtrack_count
                                .checked_mul(2)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                let backtrack_start = cursor;
                cursor += backtrack_count * 2;
                let mut matches = true;
                for index in 0..backtrack_count {
                    let Some(glyph_position) = position.checked_sub(index + 1) else {
                        matches = false;
                        break;
                    };
                    let expected = read_u16(bytes, backtrack_start + index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if usize::from(gpos_class_index(
                        bytes,
                        backtrack_definition,
                        glyphs[glyph_position],
                        table_end,
                    )?) != usize::from(expected)
                    {
                        matches = false;
                        break;
                    }
                }
                if !matches {
                    continue;
                }
                let input_count =
                    usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
                cursor += 2;
                if !(1..=16).contains(&input_count)
                    || position
                        .checked_add(input_count)
                        .is_none_or(|end| end > glyphs.len())
                {
                    return Err(TrueTypeError::TooLarge);
                }
                for index in 1..input_count {
                    let expected = read_u16(bytes, cursor + (index - 1) * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let actual = gpos_class_index(
                        bytes,
                        input_definition,
                        glyphs[position + index],
                        table_end,
                    )?;
                    if actual != expected {
                        matches = false;
                        break;
                    }
                }
                cursor = cursor
                    .checked_add(
                        (input_count - 1)
                            .checked_mul(2)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if !matches {
                    continue;
                }
                let lookahead_count =
                    usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
                cursor += 2;
                if lookahead_count > 16 {
                    return Err(TrueTypeError::TooLarge);
                }
                for index in 0..lookahead_count {
                    let Some(glyph_position) = position
                        .checked_add(input_count)
                        .and_then(|value| value.checked_add(index))
                    else {
                        matches = false;
                        break;
                    };
                    if glyph_position >= glyphs.len() {
                        matches = false;
                        break;
                    }
                    let expected = read_u16(bytes, cursor + index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let actual = gpos_class_index(
                        bytes,
                        lookahead_definition,
                        glyphs[glyph_position],
                        table_end,
                    )?;
                    if actual != expected {
                        matches = false;
                        break;
                    }
                }
                cursor = cursor
                    .checked_add(
                        lookahead_count
                            .checked_mul(2)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if !matches {
                    continue;
                }
                let substitution_count =
                    usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
                cursor += 2;
                if substitution_count > 16
                    || cursor
                        .checked_add(
                            substitution_count
                                .checked_mul(4)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .is_none_or(|end| end > table_end)
                {
                    return Err(TrueTypeError::TooLarge);
                }
                for index in 0..substitution_count {
                    let record = cursor + index * 4;
                    let sequence_index = usize::from(
                        read_u16(bytes, record).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    let nested_lookup = usize::from(
                        read_u16(bytes, record + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if sequence_index != 0 || nested_lookup >= lookup_count {
                        continue;
                    }
                    let nested_table = lookup_base
                        .checked_add(usize::from(
                            read_u16(bytes, lookup_base + 2 + nested_lookup * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if let Some(substitute) =
                        gsub_single_lookup(bytes, nested_table, glyphs[position], table_end)?
                    {
                        return Ok(Some(substitute));
                    }
                }
            }
        }
        Ok(None)
    }

    fn gsub_contextual_class_substitution(
        bytes: &[u8],
        lookup_table: usize,
        lookup_base: usize,
        glyphs: &[u16],
        position: usize,
        table_end: usize,
        extension: Option<(usize, u16)>,
    ) -> Result<Option<u16>, TrueTypeError> {
        if lookup_table
            .checked_add(6)
            .is_none_or(|end| end > table_end)
            || (extension.is_none() && read_u16(bytes, lookup_table) != Some(5))
        {
            return Ok(None);
        }
        let lookup_count =
            usize::from(read_u16(bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?);
        let subtable_count = if extension.is_some() {
            1
        } else {
            usize::from(read_u16(bytes, lookup_table + 4).ok_or(TrueTypeError::BadTableDirectory)?)
        };
        if lookup_count > 128 || subtable_count > 32 {
            return Err(TrueTypeError::TooLarge);
        }
        let lookup_end = lookup_base
            .checked_add(
                2usize
                    .checked_add(lookup_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        let subtable_end = if let Some((subtable, _)) = extension {
            subtable.checked_add(2).ok_or(TrueTypeError::TooLarge)?
        } else {
            lookup_table
                .checked_add(
                    6usize
                        .checked_add(
                            subtable_count
                                .checked_mul(2)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::TooLarge)?
        };
        if lookup_end > table_end || subtable_end > table_end {
            return Err(TrueTypeError::BadTableDirectory);
        }
        for subtable_index in 0..subtable_count {
            let subtable = if let Some((subtable, _)) = extension {
                subtable
            } else {
                lookup_table
                    .checked_add(usize::from(
                        read_u16(bytes, lookup_table + 6 + subtable_index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?
            };
            if subtable.checked_add(8).is_none_or(|end| end > table_end)
                || extension.is_some_and(|(_, kind)| kind != 5)
                || read_u16(bytes, subtable) != Some(2)
            {
                continue;
            }
            let coverage = subtable
                .checked_add(usize::from(
                    read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if gsub_coverage_index(bytes, coverage, table_end, glyphs[position])?.is_none() {
                continue;
            }
            let class_definition = subtable
                .checked_add(usize::from(
                    read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let class_set_count =
                usize::from(read_u16(bytes, subtable + 6).ok_or(TrueTypeError::BadTableDirectory)?);
            if class_set_count > 128
                || subtable
                    .checked_add(
                        8usize
                            .checked_add(
                                class_set_count
                                    .checked_mul(2)
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            let class = usize::from(gpos_class_index(
                bytes,
                class_definition,
                glyphs[position],
                table_end,
            )?);
            if class >= class_set_count {
                continue;
            }
            let class_set_offset = usize::from(
                read_u16(bytes, subtable + 8 + class * 2)
                    .ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if class_set_offset == 0 {
                continue;
            }
            let class_set = subtable
                .checked_add(class_set_offset)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if class_set.checked_add(2).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let rule_count =
                usize::from(read_u16(bytes, class_set).ok_or(TrueTypeError::BadTableDirectory)?);
            if rule_count > 64
                || class_set
                    .checked_add(
                        2usize
                            .checked_add(rule_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for rule_index in 0..rule_count {
                let rule = class_set
                    .checked_add(usize::from(
                        read_u16(bytes, class_set + 2 + rule_index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if rule.checked_add(4).is_none_or(|end| end > table_end) {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let glyph_count =
                    usize::from(read_u16(bytes, rule).ok_or(TrueTypeError::BadTableDirectory)?);
                let substitution_count =
                    usize::from(read_u16(bytes, rule + 2).ok_or(TrueTypeError::BadTableDirectory)?);
                if !(1..=16).contains(&glyph_count) || substitution_count > 16 {
                    return Err(TrueTypeError::TooLarge);
                }
                let input_classes = rule + 4;
                let records = input_classes
                    .checked_add(
                        (glyph_count - 1)
                            .checked_mul(2)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                let rule_end = records
                    .checked_add(
                        substitution_count
                            .checked_mul(4)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if rule_end > table_end
                    || position
                        .checked_add(glyph_count)
                        .is_none_or(|end| end > glyphs.len())
                {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let mut matches = true;
                for input_index in 1..glyph_count {
                    let expected = read_u16(bytes, input_classes + (input_index - 1) * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let actual = gpos_class_index(
                        bytes,
                        class_definition,
                        glyphs[position + input_index],
                        table_end,
                    )?;
                    if actual != expected {
                        matches = false;
                        break;
                    }
                }
                if !matches {
                    continue;
                }
                for substitution_index in 0..substitution_count {
                    let record = records + substitution_index * 4;
                    let sequence_index = usize::from(
                        read_u16(bytes, record).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    let nested_lookup = usize::from(
                        read_u16(bytes, record + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if sequence_index != 0 || nested_lookup >= lookup_count {
                        continue;
                    }
                    let nested_table = lookup_base
                        .checked_add(usize::from(
                            read_u16(bytes, lookup_base + 2 + nested_lookup * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if let Some(substitute) =
                        gsub_single_lookup(bytes, nested_table, glyphs[position], table_end)?
                    {
                        return Ok(Some(substitute));
                    }
                }
            }
        }
        Ok(None)
    }

    fn gsub_contextual_substitution(
        bytes: &[u8],
        lookup_table: usize,
        lookup_base: usize,
        glyphs: &[u16],
        position: usize,
        table_end: usize,
        extension: Option<(usize, u16)>,
    ) -> Result<Option<u16>, TrueTypeError> {
        if lookup_table
            .checked_add(6)
            .is_none_or(|end| end > table_end)
            || (extension.is_none()
                && read_u16(bytes, lookup_table) != Some(5)
                && read_u16(bytes, lookup_table) != Some(6))
        {
            return Ok(None);
        }
        let lookup_count =
            usize::from(read_u16(bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?);
        let subtable_count = if extension.is_some() {
            1
        } else {
            usize::from(read_u16(bytes, lookup_table + 4).ok_or(TrueTypeError::BadTableDirectory)?)
        };
        if lookup_count > 128 || subtable_count > 32 {
            return Err(TrueTypeError::TooLarge);
        }
        let lookup_end = lookup_base
            .checked_add(
                2usize
                    .checked_add(lookup_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                    .ok_or(TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::TooLarge)?;
        let subtable_end = if let Some((subtable, _)) = extension {
            subtable.checked_add(2).ok_or(TrueTypeError::TooLarge)?
        } else {
            lookup_table
                .checked_add(
                    6usize
                        .checked_add(
                            subtable_count
                                .checked_mul(2)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::TooLarge)?
        };
        if lookup_end > table_end || subtable_end > table_end {
            return Err(TrueTypeError::BadTableDirectory);
        }
        for subtable_index in 0..subtable_count {
            let subtable = if let Some((subtable, _)) = extension {
                subtable
            } else {
                lookup_table
                    .checked_add(usize::from(
                        read_u16(bytes, lookup_table + 6 + subtable_index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?
            };
            let expected_type = if let Some((_, extension_type)) = extension {
                extension_type
            } else {
                read_u16(bytes, lookup_table).unwrap_or(0)
            };
            if subtable.checked_add(2).is_none_or(|end| end > table_end)
                || expected_type != 5 && expected_type != 6
                || read_u16(bytes, subtable) != Some(3)
            {
                continue;
            }
            let mut cursor = subtable + 2;
            let backtrack_count =
                usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
            cursor += 2;
            if backtrack_count > 16 {
                return Err(TrueTypeError::TooLarge);
            }
            let backtrack_start = cursor;
            cursor = cursor
                .checked_add(
                    backtrack_count
                        .checked_mul(2)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::TooLarge)?;
            let input_count =
                usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
            cursor += 2;
            if !(1..=16).contains(&input_count) {
                return Err(TrueTypeError::TooLarge);
            }
            let input_start = cursor;
            cursor = cursor
                .checked_add(input_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                .ok_or(TrueTypeError::TooLarge)?;
            let lookahead_count =
                usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
            cursor += 2;
            if lookahead_count > 16 {
                return Err(TrueTypeError::TooLarge);
            }
            let lookahead_start = cursor;
            cursor = cursor
                .checked_add(
                    lookahead_count
                        .checked_mul(2)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .ok_or(TrueTypeError::TooLarge)?;
            let record_count =
                usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
            cursor += 2;
            if record_count > 16
                || cursor
                    .checked_add(record_count.checked_mul(4).ok_or(TrueTypeError::TooLarge)?)
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            let mut matches = true;
            for index in 0..backtrack_count {
                let Some(glyph_position) = position.checked_sub(index + 1) else {
                    matches = false;
                    break;
                };
                let coverage = subtable
                    .checked_add(usize::from(
                        read_u16(bytes, backtrack_start + index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if gsub_coverage_index(bytes, coverage, table_end, glyphs[glyph_position])?
                    .is_none()
                {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            if position
                .checked_add(input_count)
                .is_none_or(|end| end > glyphs.len())
            {
                continue;
            }
            matches = true;
            for index in 0..input_count {
                let coverage = subtable
                    .checked_add(usize::from(
                        read_u16(bytes, input_start + index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if gsub_coverage_index(bytes, coverage, table_end, glyphs[position + index])?
                    .is_none()
                {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            for index in 0..lookahead_count {
                let Some(glyph_position) = position
                    .checked_add(input_count)
                    .and_then(|value| value.checked_add(index))
                else {
                    matches = false;
                    break;
                };
                if glyph_position >= glyphs.len() {
                    matches = false;
                    break;
                }
                let coverage = subtable
                    .checked_add(usize::from(
                        read_u16(bytes, lookahead_start + index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if gsub_coverage_index(bytes, coverage, table_end, glyphs[glyph_position])?
                    .is_none()
                {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            let record_count_start = cursor - 2;
            for index in 0..record_count {
                let record = record_count_start + 2 + index * 4;
                let sequence_index =
                    usize::from(read_u16(bytes, record).ok_or(TrueTypeError::BadTableDirectory)?);
                let nested_lookup = usize::from(
                    read_u16(bytes, record + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if sequence_index != 0 || nested_lookup >= lookup_count {
                    continue;
                }
                let nested_table = lookup_base
                    .checked_add(usize::from(
                        read_u16(bytes, lookup_base + 2 + nested_lookup * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                if let Some(substitute) =
                    gsub_single_lookup(bytes, nested_table, glyphs[position], table_end)?
                {
                    return Ok(Some(substitute));
                }
            }
        }
        Ok(None)
    }

    fn gsub_coverage_index(
        bytes: &[u8],
        coverage: usize,
        table_end: usize,
        glyph: u16,
    ) -> Result<Option<usize>, TrueTypeError> {
        let format = read_u16(bytes, coverage).ok_or(TrueTypeError::BadTableDirectory)?;
        match format {
            1 => {
                let count = usize::from(
                    read_u16(bytes, coverage + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let end = coverage
                    .checked_add(
                        4usize
                            .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if count > 128 || end > table_end {
                    return Err(TrueTypeError::TooLarge);
                }
                for index in 0..count {
                    if read_u16(bytes, coverage + 4 + index * 2) == Some(glyph) {
                        return Ok(Some(index));
                    }
                }
                Ok(None)
            }
            2 => {
                let count = usize::from(
                    read_u16(bytes, coverage + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let end = coverage
                    .checked_add(
                        4usize
                            .checked_add(count.checked_mul(6).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if count > 128 || end > table_end {
                    return Err(TrueTypeError::TooLarge);
                }
                let glyph = u32::from(glyph);
                for index in 0..count {
                    let range = coverage + 4 + index * 6;
                    let start =
                        u32::from(read_u16(bytes, range).ok_or(TrueTypeError::BadTableDirectory)?);
                    let end_glyph = u32::from(
                        read_u16(bytes, range + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if (start..=end_glyph).contains(&glyph) {
                        let first = usize::from(
                            read_u16(bytes, range + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        return Ok(first.checked_add(
                            usize::try_from(glyph - start).map_err(|_| TrueTypeError::TooLarge)?,
                        ));
                    }
                }
                Ok(None)
            }
            _ => Err(TrueTypeError::UnsupportedCmap),
        }
    }

    fn gpos_selected_feature_indices(
        bytes: &[u8],
        table: usize,
        table_end: usize,
        script_tag: Option<[u8; 4]>,
        language_tag: Option<[u8; 4]>,
    ) -> Result<([u16; 64], usize, usize), TrueTypeError> {
        let feature_base = table
            .checked_add(usize::from(
                read_u16(bytes, table + 6).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if feature_base
            .checked_add(2)
            .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let feature_count =
            usize::from(read_u16(bytes, feature_base).ok_or(TrueTypeError::BadTableDirectory)?);
        if feature_count > 64
            || feature_base
                .checked_add(
                    2usize
                        .checked_add(
                            feature_count
                                .checked_mul(6)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        let mut indices = [0u16; 64];
        let mut count = 0usize;
        if let Some(script_tag) = script_tag {
            let script_offset =
                usize::from(read_u16(bytes, table + 4).ok_or(TrueTypeError::BadTableDirectory)?);
            if script_offset == 0 {
                return Ok((indices, 0, feature_count));
            }
            let script_list = table
                .checked_add(script_offset)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if script_list.checked_add(2).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let script_count =
                usize::from(read_u16(bytes, script_list).ok_or(TrueTypeError::BadTableDirectory)?);
            if script_count > 32
                || script_list
                    .checked_add(
                        2usize
                            .checked_add(
                                script_count.checked_mul(6).ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            let mut selected_script = None;
            for item in 0..script_count {
                let record = script_list + 2 + item * 6;
                if bytes
                    .get(record..record + 4)
                    .ok_or(TrueTypeError::BadTableDirectory)?
                    == script_tag
                {
                    selected_script = Some(
                        script_list
                            .checked_add(usize::from(
                                read_u16(bytes, record + 4)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    break;
                }
            }
            let Some(script_table) = selected_script else {
                return Ok((indices, 0, feature_count));
            };
            let Some(lang_sys) = layout_lang_sys(bytes, script_table, language_tag, table_end)?
            else {
                return Ok((indices, 0, feature_count));
            };
            let required = read_u16(bytes, lang_sys + 2).ok_or(TrueTypeError::BadTableDirectory)?;
            let lang_count =
                usize::from(read_u16(bytes, lang_sys + 4).ok_or(TrueTypeError::BadTableDirectory)?);
            if lang_count > 63
                || lang_sys
                    .checked_add(
                        6usize
                            .checked_add(lang_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            if required != u16::MAX {
                indices[count] = required;
                count += 1;
            }
            for item in 0..lang_count {
                indices[count] = read_u16(bytes, lang_sys + 6 + item * 2)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                count += 1;
            }
        } else {
            for item in 0..feature_count {
                indices[count] = u16::try_from(item).map_err(|_| TrueTypeError::TooLarge)?;
                count += 1;
            }
        }
        Ok((indices, count, feature_count))
    }

    fn gpos_attachment_lookup(
        bytes: &[u8],
        lookup: usize,
        expected_type: u16,
        table_end: usize,
    ) -> Result<Option<(bool, usize)>, TrueTypeError> {
        if lookup.checked_add(8).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let lookup_type = read_u16(bytes, lookup).ok_or(TrueTypeError::BadTableDirectory)?;
        if lookup_type == expected_type {
            return Ok(Some((false, lookup)));
        }
        if lookup_type != 9 {
            return Ok(None);
        }
        let extension = lookup
            .checked_add(usize::from(
                read_u16(bytes, lookup + 6).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if extension.checked_add(8).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        if read_u16(bytes, extension) != Some(1)
            || read_u16(bytes, extension + 2) != Some(expected_type)
        {
            return Ok(None);
        }
        let nested = extension
            .checked_add(
                usize::try_from(
                    read_u32(bytes, extension + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                )
                .map_err(|_| TrueTypeError::TooLarge)?,
            )
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if nested.checked_add(6).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        Ok(Some((true, nested)))
    }

    #[derive(Clone, Copy, Default)]
    struct GposContextNeighbors {
        previous: Option<u16>,
        previous2: Option<u16>,
        next: Option<u16>,
        next2: Option<u16>,
    }

    #[derive(Clone, Copy)]
    struct GposContextOptions {
        script_tag: Option<[u8; 4]>,
        language_tag: Option<[u8; 4]>,
        ppem: Option<u16>,
        previous: Option<u16>,
        previous2: Option<u16>,
        next: Option<u16>,
        next2: Option<u16>,
        third: Option<u16>,
        fourth: Option<u16>,
        fifth: Option<u16>,
        input_count: u16,
        target_sequence_index: u16,
    }

    #[derive(Clone, Copy)]
    struct GposContextTableBounds {
        table: usize,
        table_end: usize,
    }

    #[derive(Clone, Copy)]
    struct GposContextLookupBounds {
        lookup_base: usize,
        lookup_count: usize,
    }

    #[derive(Clone, Copy)]
    struct GposContextRecordTarget {
        target_glyph: u16,
        pair_right: Option<u16>,
    }

    fn gpos_contextual_chaining_apply_single_records(
        bytes: &[u8],
        record_count_offset: usize,
        table_bounds: GposContextTableBounds,
        lookup_bounds: GposContextLookupBounds,
        target: GposContextRecordTarget,
        options: GposContextOptions,
    ) -> Result<Option<(i32, i32, i32, i32)>, TrueTypeError> {
        let table = table_bounds.table;
        let table_end = table_bounds.table_end;
        let lookup_base = lookup_bounds.lookup_base;
        let lookup_count = lookup_bounds.lookup_count;
        if record_count_offset
            .checked_add(2)
            .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let record_count = usize::from(
            read_u16(bytes, record_count_offset).ok_or(TrueTypeError::BadTableDirectory)?,
        );
        if record_count > 8
            || record_count_offset
                .checked_add(
                    2usize
                        .checked_add(record_count.checked_mul(4).ok_or(TrueTypeError::TooLarge)?)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        for record_index in 0..record_count {
            let record = record_count_offset + 2 + record_index * 4;
            if read_u16(bytes, record) != Some(options.target_sequence_index) {
                continue;
            }
            let nested_index =
                usize::from(read_u16(bytes, record + 2).ok_or(TrueTypeError::BadTableDirectory)?);
            if nested_index >= lookup_count {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let nested_lookup = lookup_base
                .checked_add(usize::from(
                    read_u16(bytes, lookup_base + 2 + nested_index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let (nested_type, is_extension, nested_target) =
                if let Some((is_extension, nested_target)) =
                    gpos_attachment_lookup(bytes, nested_lookup, 1, table_end)?
                {
                    (1, is_extension, nested_target)
                } else if let Some((is_extension, nested_target)) =
                    gpos_attachment_lookup(bytes, nested_lookup, 2, table_end)?
                {
                    (2, is_extension, nested_target)
                } else {
                    continue;
                };
            let nested_count = if is_extension {
                1
            } else {
                usize::from(
                    read_u16(bytes, nested_lookup + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                )
            };
            if nested_count > 32 {
                return Err(TrueTypeError::TooLarge);
            }
            for nested_subtable_index in 0..nested_count {
                let nested_subtable = if is_extension {
                    nested_target
                } else {
                    nested_lookup
                        .checked_add(usize::from(
                            read_u16(bytes, nested_lookup + 6 + nested_subtable_index * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?
                };
                if nested_subtable
                    .checked_add(6)
                    .is_none_or(|end| end > table_end)
                {
                    continue;
                }
                if nested_type == 2 {
                    let Some(pair_right) = target.pair_right else {
                        continue;
                    };
                    if read_u16(bytes, nested_subtable) == Some(2) {
                        if nested_subtable
                            .checked_add(16)
                            .is_none_or(|end| end > table_end)
                        {
                            continue;
                        }
                        let coverage = nested_subtable
                            .checked_add(usize::from(
                                read_u16(bytes, nested_subtable + 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        if gsub_coverage_index(bytes, coverage, table_end, target.target_glyph)?
                            .is_none()
                        {
                            continue;
                        }
                        let value_format1 = read_u16(bytes, nested_subtable + 4)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let value_format2 = read_u16(bytes, nested_subtable + 6)
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let class_def1 = nested_subtable
                            .checked_add(usize::from(
                                read_u16(bytes, nested_subtable + 8)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let class_def2 = nested_subtable
                            .checked_add(usize::from(
                                read_u16(bytes, nested_subtable + 10)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let class1_count = usize::from(
                            read_u16(bytes, nested_subtable + 12)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        let class2_count = usize::from(
                            read_u16(bytes, nested_subtable + 14)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if class1_count == 0
                            || class2_count == 0
                            || class1_count > 128
                            || class2_count > 128
                        {
                            return Err(TrueTypeError::TooLarge);
                        }
                        let class1 = usize::from(gpos_class_index(
                            bytes,
                            class_def1,
                            target.target_glyph,
                            table_end,
                        )?);
                        let class2 = usize::from(gpos_class_index(
                            bytes, class_def2, pair_right, table_end,
                        )?);
                        if class1 >= class1_count || class2 >= class2_count {
                            continue;
                        }
                        let first_size = gpos_value_record_size(value_format1)?;
                        let second_size = gpos_value_record_size(value_format2)?;
                        let record_size = first_size
                            .checked_add(second_size)
                            .ok_or(TrueTypeError::TooLarge)?;
                        let records_start = nested_subtable
                            .checked_add(16)
                            .ok_or(TrueTypeError::TooLarge)?;
                        let record = records_start
                            .checked_add(
                                class1
                                    .checked_mul(class2_count)
                                    .and_then(|value| value.checked_add(class2))
                                    .and_then(|value| value.checked_mul(record_size))
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        let records_end = records_start
                            .checked_add(
                                class1_count
                                    .checked_mul(class2_count)
                                    .and_then(|value| value.checked_mul(record_size))
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?;
                        if records_end > table_end {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        return Ok(Some(gpos_value_adjustment(
                            bytes,
                            record,
                            value_format1,
                            table_end,
                            table,
                            options.ppem,
                        )?));
                    }
                    if read_u16(bytes, nested_subtable) != Some(1)
                        || nested_subtable
                            .checked_add(10)
                            .is_none_or(|end| end > table_end)
                    {
                        continue;
                    }
                    let coverage = nested_subtable
                        .checked_add(usize::from(
                            read_u16(bytes, nested_subtable + 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if gsub_coverage_index(bytes, coverage, table_end, target.target_glyph)?
                        .is_none()
                    {
                        continue;
                    }
                    let value_format1 = read_u16(bytes, nested_subtable + 4)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let value_format2 = read_u16(bytes, nested_subtable + 6)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let pair_set_count = usize::from(
                        read_u16(bytes, nested_subtable + 8)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if pair_set_count > 128
                        || nested_subtable
                            .checked_add(
                                10usize
                                    .checked_add(
                                        pair_set_count
                                            .checked_mul(2)
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .is_none_or(|end| end > table_end)
                    {
                        return Err(TrueTypeError::TooLarge);
                    }
                    let Some(coverage_index) =
                        gsub_coverage_index(bytes, coverage, table_end, target.target_glyph)?
                    else {
                        continue;
                    };
                    if coverage_index >= pair_set_count {
                        continue;
                    }
                    let pair_set = nested_subtable
                        .checked_add(usize::from(
                            read_u16(bytes, nested_subtable + 10 + coverage_index * 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if pair_set.checked_add(2).is_none_or(|end| end > table_end) {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    let pair_count = usize::from(
                        read_u16(bytes, pair_set).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if pair_count > 128 {
                        return Err(TrueTypeError::TooLarge);
                    }
                    let first_size = gpos_value_record_size(value_format1)?;
                    let second_size = gpos_value_record_size(value_format2)?;
                    let record_size = 2usize
                        .checked_add(first_size)
                        .and_then(|size| size.checked_add(second_size))
                        .ok_or(TrueTypeError::TooLarge)?;
                    let records_end = pair_set
                        .checked_add(
                            2usize
                                .checked_add(
                                    pair_count
                                        .checked_mul(record_size)
                                        .ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?;
                    if records_end > table_end {
                        return Err(TrueTypeError::BadTableDirectory);
                    }
                    for pair_index in 0..pair_count {
                        let pair_record = pair_set + 2 + pair_index * record_size;
                        if read_u16(bytes, pair_record) != Some(pair_right) {
                            continue;
                        }
                        return Ok(Some(gpos_value_adjustment(
                            bytes,
                            pair_record + 2,
                            value_format1,
                            table_end,
                            table,
                            options.ppem,
                        )?));
                    }
                    continue;
                }
                let coverage = nested_subtable
                    .checked_add(usize::from(
                        read_u16(bytes, nested_subtable + 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let Some(coverage_index) =
                    gsub_coverage_index(bytes, coverage, table_end, target.target_glyph)?
                else {
                    continue;
                };
                let value_format =
                    read_u16(bytes, nested_subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?;
                let record = if read_u16(bytes, nested_subtable) == Some(1) {
                    nested_subtable + 6
                } else if read_u16(bytes, nested_subtable) == Some(2) {
                    let count = usize::from(
                        read_u16(bytes, nested_subtable + 6)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    let record_size = gpos_value_record_size(value_format)?;
                    if count > 128 || coverage_index >= count {
                        return Err(TrueTypeError::TooLarge);
                    }
                    nested_subtable
                        .checked_add(
                            8usize
                                .checked_add(
                                    coverage_index
                                        .checked_mul(record_size)
                                        .ok_or(TrueTypeError::TooLarge)?,
                                )
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?
                } else {
                    continue;
                };
                return Ok(Some(gpos_value_adjustment(
                    bytes,
                    record,
                    value_format,
                    table_end,
                    table,
                    options.ppem,
                )?));
            }
        }
        Ok(None)
    }

    fn gpos_contextual_chaining_format1_single_adjustment(
        bytes: &[u8],
        subtable: usize,
        table_bounds: GposContextTableBounds,
        lookup_bounds: GposContextLookupBounds,
        left: u16,
        right: u16,
        options: GposContextOptions,
    ) -> Result<Option<(i32, i32, i32, i32)>, TrueTypeError> {
        let table = table_bounds.table;
        let table_end = table_bounds.table_end;
        let lookup_base = lookup_bounds.lookup_base;
        let lookup_count = lookup_bounds.lookup_count;
        let target_glyph = match options.target_sequence_index {
            0 => left,
            1 => right,
            2 => options.third.ok_or(TrueTypeError::BadTableDirectory)?,
            3 => options.fourth.ok_or(TrueTypeError::BadTableDirectory)?,
            4 => options.fifth.ok_or(TrueTypeError::BadTableDirectory)?,
            _ => return Ok(None),
        };
        if subtable.checked_add(6).is_none_or(|end| end > table_end)
            || read_u16(bytes, subtable) != Some(1)
        {
            return Ok(None);
        }
        let coverage = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let Some(coverage_index) = gsub_coverage_index(bytes, coverage, table_end, left)? else {
            return Ok(None);
        };
        let rule_set_count =
            usize::from(read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?);
        if rule_set_count > 128
            || subtable
                .checked_add(
                    6usize
                        .checked_add(
                            rule_set_count
                                .checked_mul(2)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        if coverage_index >= rule_set_count {
            return Ok(None);
        }
        let rule_set = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 6 + coverage_index * 2)
                    .ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if rule_set.checked_add(2).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let rule_count =
            usize::from(read_u16(bytes, rule_set).ok_or(TrueTypeError::BadTableDirectory)?);
        if rule_count > 128
            || rule_set
                .checked_add(
                    2usize
                        .checked_add(rule_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        for rule_index in 0..rule_count {
            let rule = rule_set
                .checked_add(usize::from(
                    read_u16(bytes, rule_set + 2 + rule_index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if rule.checked_add(2).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let backtrack_count =
                usize::from(read_u16(bytes, rule).ok_or(TrueTypeError::BadTableDirectory)?);
            if backtrack_count > 2 {
                continue;
            }
            let input_count_offset = rule
                .checked_add(2 + backtrack_count * 2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let input_count = usize::from(
                read_u16(bytes, input_count_offset).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if input_count == 0
                || input_count > 5
                || input_count != usize::from(options.input_count)
            {
                continue;
            }
            let lookahead_count_offset = input_count_offset
                .checked_add(2 + (input_count - 1) * 2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let lookahead_count = usize::from(
                read_u16(bytes, lookahead_count_offset).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if lookahead_count > 2 {
                continue;
            }
            let lookahead_glyphs = lookahead_count_offset
                .checked_add(2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let record_count_offset = lookahead_glyphs
                .checked_add(lookahead_count * 2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let mut matches = true;
            for index in 0..backtrack_count {
                let expected = match index {
                    0 => options.previous,
                    1 => options.previous2,
                    _ => None,
                };
                let Some(expected) = expected else {
                    matches = false;
                    break;
                };
                if read_u16(bytes, rule + 2 + index * 2) != Some(expected) {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            for index in 1..input_count {
                let expected = match index {
                    1 => Some(right),
                    2 => options.third,
                    3 => options.fourth,
                    4 => options.fifth,
                    _ => None,
                };
                let Some(expected) = expected else {
                    matches = false;
                    break;
                };
                if read_u16(bytes, input_count_offset + 2 + (index - 1) * 2) != Some(expected) {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            for index in 0..lookahead_count {
                let expected = match index {
                    0 => options.next,
                    1 => options.next2,
                    _ => None,
                };
                let Some(expected) = expected else {
                    matches = false;
                    break;
                };
                if read_u16(bytes, lookahead_glyphs + index * 2) != Some(expected) {
                    matches = false;
                    break;
                }
            }
            if !matches
                || record_count_offset
                    .checked_add(2)
                    .is_none_or(|end| end > table_end)
            {
                if matches {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                continue;
            }
            let pair_right = match options.target_sequence_index {
                0 => Some(right),
                1 if input_count >= 3 => options.third,
                1 => options.next,
                2 if input_count >= 4 => options.fourth,
                2 => options.next,
                3 if input_count >= 5 => options.fifth,
                3 => options.next,
                4 => options.next,
                _ => None,
            };
            if let Some(adjustment) = gpos_contextual_chaining_apply_single_records(
                bytes,
                record_count_offset,
                table_bounds,
                lookup_bounds,
                GposContextRecordTarget {
                    target_glyph,
                    pair_right,
                },
                options,
            )? {
                return Ok(Some(adjustment));
            }
            let record_count = usize::from(
                read_u16(bytes, record_count_offset).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if record_count > 8
                || record_count_offset
                    .checked_add(
                        2usize
                            .checked_add(
                                record_count.checked_mul(4).ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for record_index in 0..record_count {
                let record = record_count_offset + 2 + record_index * 4;
                if read_u16(bytes, record) != Some(options.target_sequence_index) {
                    continue;
                }
                let nested_index = usize::from(
                    read_u16(bytes, record + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if nested_index >= lookup_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let nested_lookup = lookup_base
                    .checked_add(usize::from(
                        read_u16(bytes, lookup_base + 2 + nested_index * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let Some((is_extension, nested_target)) =
                    gpos_attachment_lookup(bytes, nested_lookup, 1, table_end)?
                else {
                    continue;
                };
                let nested_count = if is_extension {
                    1
                } else {
                    usize::from(
                        read_u16(bytes, nested_lookup + 4)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    )
                };
                if nested_count > 32 {
                    return Err(TrueTypeError::TooLarge);
                }
                for nested_subtable_index in 0..nested_count {
                    let nested_subtable = if is_extension {
                        nested_target
                    } else {
                        nested_lookup
                            .checked_add(usize::from(
                                read_u16(bytes, nested_lookup + 6 + nested_subtable_index * 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?
                    };
                    if nested_subtable
                        .checked_add(6)
                        .is_none_or(|end| end > table_end)
                    {
                        continue;
                    }
                    let coverage = nested_subtable
                        .checked_add(usize::from(
                            read_u16(bytes, nested_subtable + 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let Some(coverage_index) =
                        gsub_coverage_index(bytes, coverage, table_end, target_glyph)?
                    else {
                        continue;
                    };
                    let value_format = read_u16(bytes, nested_subtable + 4)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let record = if read_u16(bytes, nested_subtable) == Some(1) {
                        nested_subtable + 6
                    } else if read_u16(bytes, nested_subtable) == Some(2) {
                        let count = usize::from(
                            read_u16(bytes, nested_subtable + 6)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        let record_size = gpos_value_record_size(value_format)?;
                        if count > 128 || coverage_index >= count {
                            return Err(TrueTypeError::TooLarge);
                        }
                        nested_subtable
                            .checked_add(
                                8usize
                                    .checked_add(
                                        coverage_index
                                            .checked_mul(record_size)
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?
                    } else {
                        continue;
                    };
                    return Ok(Some(gpos_value_adjustment(
                        bytes,
                        record,
                        value_format,
                        table_end,
                        table,
                        options.ppem,
                    )?));
                }
            }
            return Ok(None);
        }
        Ok(None)
    }

    fn gpos_contextual_chaining_format2_single_adjustment(
        bytes: &[u8],
        subtable: usize,
        table_bounds: GposContextTableBounds,
        lookup_bounds: GposContextLookupBounds,
        left: u16,
        right: u16,
        options: GposContextOptions,
    ) -> Result<Option<(i32, i32, i32, i32)>, TrueTypeError> {
        let table_end = table_bounds.table_end;
        if subtable.checked_add(12).is_none_or(|end| end > table_end)
            || read_u16(bytes, subtable) != Some(2)
        {
            return Ok(None);
        }
        let coverage = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if gsub_coverage_index(bytes, coverage, table_end, left)?.is_none() {
            return Ok(None);
        }
        let backtrack_class = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 4).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let input_class = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 6).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let lookahead_class = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 8).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let class_set_count =
            usize::from(read_u16(bytes, subtable + 10).ok_or(TrueTypeError::BadTableDirectory)?);
        if class_set_count > 128
            || subtable
                .checked_add(
                    12usize
                        .checked_add(
                            class_set_count
                                .checked_mul(2)
                                .ok_or(TrueTypeError::TooLarge)?,
                        )
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        let input_class_index = usize::from(gpos_class_index(bytes, input_class, left, table_end)?);
        if input_class_index >= class_set_count {
            return Ok(None);
        }
        let class_set = subtable
            .checked_add(usize::from(
                read_u16(bytes, subtable + 12 + input_class_index * 2)
                    .ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        if class_set.checked_add(2).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let rule_count =
            usize::from(read_u16(bytes, class_set).ok_or(TrueTypeError::BadTableDirectory)?);
        if rule_count > 128
            || class_set
                .checked_add(
                    2usize
                        .checked_add(rule_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        let target_glyph = match options.target_sequence_index {
            0 => left,
            1 => right,
            2 => options.third.ok_or(TrueTypeError::BadTableDirectory)?,
            3 => options.fourth.ok_or(TrueTypeError::BadTableDirectory)?,
            4 => options.fifth.ok_or(TrueTypeError::BadTableDirectory)?,
            _ => return Ok(None),
        };
        for rule_index in 0..rule_count {
            let rule = class_set
                .checked_add(usize::from(
                    read_u16(bytes, class_set + 2 + rule_index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if rule.checked_add(2).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let backtrack_count =
                usize::from(read_u16(bytes, rule).ok_or(TrueTypeError::BadTableDirectory)?);
            if backtrack_count > 2 {
                continue;
            }
            let input_count_offset = rule
                .checked_add(2 + backtrack_count * 2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let input_count = usize::from(
                read_u16(bytes, input_count_offset).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if input_count == 0
                || input_count > 5
                || input_count != usize::from(options.input_count)
            {
                continue;
            }
            let lookahead_count_offset = input_count_offset
                .checked_add(2 + (input_count - 1) * 2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let lookahead_count = usize::from(
                read_u16(bytes, lookahead_count_offset).ok_or(TrueTypeError::BadTableDirectory)?,
            );
            if lookahead_count > 2 {
                continue;
            }
            let lookahead_classes = lookahead_count_offset
                .checked_add(2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let record_count_offset = lookahead_classes
                .checked_add(lookahead_count * 2)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let mut matches = true;
            for index in 0..backtrack_count {
                let expected = usize::from(
                    read_u16(bytes, rule + 2 + index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let glyph = match index {
                    0 => options.previous,
                    1 => options.previous2,
                    _ => None,
                };
                let Some(glyph) = glyph else {
                    matches = false;
                    break;
                };
                if usize::from(gpos_class_index(bytes, backtrack_class, glyph, table_end)?)
                    != expected
                {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            for index in 1..input_count {
                let expected = usize::from(
                    read_u16(bytes, input_count_offset + 2 + (index - 1) * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let glyph = match index {
                    1 => Some(right),
                    2 => options.third,
                    3 => options.fourth,
                    4 => options.fifth,
                    _ => None,
                };
                let Some(glyph) = glyph else {
                    matches = false;
                    break;
                };
                if usize::from(gpos_class_index(bytes, input_class, glyph, table_end)?) != expected
                {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            for index in 0..lookahead_count {
                let expected = usize::from(
                    read_u16(bytes, lookahead_classes + index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let glyph = match index {
                    0 => options.next,
                    1 => options.next2,
                    _ => None,
                };
                let Some(glyph) = glyph else {
                    matches = false;
                    break;
                };
                if usize::from(gpos_class_index(bytes, lookahead_class, glyph, table_end)?)
                    != expected
                {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            let pair_right = match options.target_sequence_index {
                0 => Some(right),
                1 if input_count >= 3 => options.third,
                1 => options.next,
                2 if input_count >= 4 => options.fourth,
                2 => options.next,
                3 if input_count >= 5 => options.fifth,
                3 => options.next,
                4 => options.next,
                _ => None,
            };
            if let Some(adjustment) = gpos_contextual_chaining_apply_single_records(
                bytes,
                record_count_offset,
                table_bounds,
                lookup_bounds,
                GposContextRecordTarget {
                    target_glyph,
                    pair_right,
                },
                options,
            )? {
                return Ok(Some(adjustment));
            }
        }
        Ok(None)
    }

    fn gpos_contextual_single_adjustment(
        bytes: &[u8],
        table: usize,
        table_end: usize,
        left: u16,
        right: u16,
        options: GposContextOptions,
    ) -> Result<Option<(i32, i32, i32, i32)>, TrueTypeError> {
        if options.input_count == 0
            || options.input_count > 5
            || options.target_sequence_index >= options.input_count
        {
            return Ok(None);
        }
        let target_glyph = match options.target_sequence_index {
            0 => left,
            1 => right,
            2 => {
                let Some(third) = options.third else {
                    return Ok(None);
                };
                third
            }
            3 => {
                let Some(fourth) = options.fourth else {
                    return Ok(None);
                };
                fourth
            }
            4 => {
                let Some(fifth) = options.fifth else {
                    return Ok(None);
                };
                fifth
            }
            _ => return Ok(None),
        };
        let (feature_indices, feature_index_count, feature_count) = gpos_selected_feature_indices(
            bytes,
            table,
            table_end,
            options.script_tag,
            options.language_tag,
        )?;
        let lookup_base = table
            .checked_add(usize::from(
                read_u16(bytes, table + 8).ok_or(TrueTypeError::BadTableDirectory)?,
            ))
            .ok_or(TrueTypeError::BadTableDirectory)?;
        let lookup_count =
            usize::from(read_u16(bytes, lookup_base).ok_or(TrueTypeError::BadTableDirectory)?);
        if lookup_count > 128
            || lookup_base
                .checked_add(
                    2usize
                        .checked_add(lookup_count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                        .ok_or(TrueTypeError::TooLarge)?,
                )
                .is_none_or(|end| end > table_end)
        {
            return Err(TrueTypeError::TooLarge);
        }
        for feature_index in feature_indices.iter().copied().take(feature_index_count) {
            let feature_index = usize::from(feature_index);
            if feature_index >= feature_count {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let feature_record = table
                .checked_add(usize::from(
                    read_u16(bytes, table + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .and_then(|base| base.checked_add(2 + feature_index * 6))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let feature_base = feature_record
                .checked_sub(2 + feature_index * 6)
                .ok_or(TrueTypeError::BadTableDirectory)?;
            let feature = feature_base
                .checked_add(usize::from(
                    read_u16(bytes, feature_record + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                ))
                .ok_or(TrueTypeError::BadTableDirectory)?;
            if feature.checked_add(4).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let lookup_total =
                usize::from(read_u16(bytes, feature + 2).ok_or(TrueTypeError::BadTableDirectory)?);
            if lookup_total > 32
                || feature
                    .checked_add(
                        4usize
                            .checked_add(
                                lookup_total.checked_mul(2).ok_or(TrueTypeError::TooLarge)?,
                            )
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .is_none_or(|end| end > table_end)
            {
                return Err(TrueTypeError::TooLarge);
            }
            for item in 0..lookup_total {
                let selected = usize::from(
                    read_u16(bytes, feature + 4 + item * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if selected >= lookup_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let lookup = lookup_base
                    .checked_add(usize::from(
                        read_u16(bytes, lookup_base + 2 + selected * 2)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    ))
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                let contextual_lookup =
                    if let Some(result) = gpos_attachment_lookup(bytes, lookup, 7, table_end)? {
                        Some(result)
                    } else {
                        gpos_attachment_lookup(bytes, lookup, 8, table_end)?
                    };
                let Some((is_extension, lookup_target)) = contextual_lookup else {
                    continue;
                };
                let subtable_count = if is_extension {
                    1
                } else {
                    usize::from(
                        read_u16(bytes, lookup + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                    )
                };
                if subtable_count > 32 {
                    return Err(TrueTypeError::TooLarge);
                }
                for subtable_index in 0..subtable_count {
                    let subtable = if is_extension {
                        lookup_target
                    } else {
                        lookup
                            .checked_add(usize::from(
                                read_u16(bytes, lookup + 6 + subtable_index * 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?
                    };
                    if subtable.checked_add(4).is_none_or(|end| end > table_end) {
                        continue;
                    }
                    if read_u16(bytes, subtable) == Some(1) {
                        if let Some(adjustment) =
                            gpos_contextual_chaining_format1_single_adjustment(
                                bytes,
                                subtable,
                                GposContextTableBounds { table, table_end },
                                GposContextLookupBounds {
                                    lookup_base,
                                    lookup_count,
                                },
                                left,
                                right,
                                options,
                            )?
                        {
                            return Ok(Some(adjustment));
                        }
                        continue;
                    }
                    if read_u16(bytes, subtable) == Some(2) {
                        if let Some(adjustment) =
                            gpos_contextual_chaining_format2_single_adjustment(
                                bytes,
                                subtable,
                                GposContextTableBounds { table, table_end },
                                GposContextLookupBounds {
                                    lookup_base,
                                    lookup_count,
                                },
                                left,
                                right,
                                options,
                            )?
                        {
                            return Ok(Some(adjustment));
                        }
                        continue;
                    }
                    if read_u16(bytes, subtable) != Some(3) {
                        continue;
                    }
                    let backtrack_count = usize::from(
                        read_u16(bytes, subtable + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if backtrack_count > 2 {
                        continue;
                    }
                    let backtrack_offsets = subtable
                        .checked_add(4)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let input_count_offset = backtrack_offsets
                        .checked_add(backtrack_count * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let input_count = usize::from(
                        read_u16(bytes, input_count_offset)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if input_count != usize::from(options.input_count) {
                        continue;
                    }
                    let input_offsets = input_count_offset
                        .checked_add(2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let lookahead_count_offset = input_offsets
                        .checked_add(input_count * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let lookahead_count = usize::from(
                        read_u16(bytes, lookahead_count_offset)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if lookahead_count > 2 {
                        continue;
                    }
                    let lookahead_offsets = lookahead_count_offset
                        .checked_add(2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let record_count_offset = lookahead_offsets
                        .checked_add(lookahead_count * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let mut context_matches = true;
                    for context_index in 0..backtrack_count {
                        let backtrack = match context_index {
                            0 => options.previous,
                            1 => options.previous2,
                            _ => None,
                        };
                        let Some(backtrack) = backtrack else {
                            context_matches = false;
                            break;
                        };
                        if gsub_coverage_index(
                            bytes,
                            subtable
                                .checked_add(usize::from(
                                    read_u16(bytes, backtrack_offsets + context_index * 2)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                            table_end,
                            backtrack,
                        )?
                        .is_none()
                        {
                            context_matches = false;
                            break;
                        }
                    }
                    for context_index in 0..lookahead_count {
                        let lookahead = match context_index {
                            0 => options.next,
                            1 => options.next2,
                            _ => None,
                        };
                        let Some(lookahead) = lookahead else {
                            context_matches = false;
                            break;
                        };
                        if gsub_coverage_index(
                            bytes,
                            subtable
                                .checked_add(usize::from(
                                    read_u16(bytes, lookahead_offsets + context_index * 2)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                            table_end,
                            lookahead,
                        )?
                        .is_none()
                        {
                            context_matches = false;
                            break;
                        }
                    }
                    if !context_matches {
                        continue;
                    }
                    let record_count = usize::from(
                        read_u16(bytes, record_count_offset)
                            .ok_or(TrueTypeError::BadTableDirectory)?,
                    );
                    if record_count > 8
                        || record_count_offset
                            .checked_add(
                                2usize
                                    .checked_add(
                                        record_count
                                            .checked_mul(4)
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?,
                            )
                            .is_none_or(|end| end > table_end)
                    {
                        return Err(TrueTypeError::TooLarge);
                    }
                    let first_coverage = subtable
                        .checked_add(usize::from(
                            read_u16(bytes, input_offsets)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let second_coverage = subtable
                        .checked_add(usize::from(
                            read_u16(bytes, input_offsets + 2)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let third_coverage = subtable
                        .checked_add(usize::from(
                            read_u16(bytes, input_offsets + 4)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let fourth_coverage = subtable
                        .checked_add(usize::from(
                            read_u16(bytes, input_offsets + 6)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    let fifth_coverage = subtable
                        .checked_add(usize::from(
                            read_u16(bytes, input_offsets + 8)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                        ))
                        .ok_or(TrueTypeError::BadTableDirectory)?;
                    if gsub_coverage_index(bytes, first_coverage, table_end, left)?.is_none() {
                        continue;
                    }
                    if options.input_count == 2
                        && gsub_coverage_index(bytes, second_coverage, table_end, right)?.is_none()
                    {
                        continue;
                    }
                    if options.input_count == 3
                        && gsub_coverage_index(
                            bytes,
                            third_coverage,
                            table_end,
                            options.third.ok_or(TrueTypeError::BadTableDirectory)?,
                        )?
                        .is_none()
                    {
                        continue;
                    }
                    if options.input_count == 4
                        && gsub_coverage_index(
                            bytes,
                            fourth_coverage,
                            table_end,
                            options.fourth.ok_or(TrueTypeError::BadTableDirectory)?,
                        )?
                        .is_none()
                    {
                        continue;
                    }
                    if options.input_count == 5
                        && gsub_coverage_index(
                            bytes,
                            fifth_coverage,
                            table_end,
                            options.fifth.ok_or(TrueTypeError::BadTableDirectory)?,
                        )?
                        .is_none()
                    {
                        continue;
                    }
                    let mut accumulated = (0i32, 0i32, 0i32, 0i32);
                    let mut found = false;
                    for record_index in 0..record_count {
                        let record = record_count_offset + 2 + record_index * 4;
                        if read_u16(bytes, record) != Some(options.target_sequence_index) {
                            continue;
                        }
                        let nested_index = usize::from(
                            read_u16(bytes, record + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                        );
                        if nested_index >= lookup_count {
                            return Err(TrueTypeError::BadTableDirectory);
                        }
                        let nested_lookup = lookup_base
                            .checked_add(usize::from(
                                read_u16(bytes, lookup_base + 2 + nested_index * 2)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            ))
                            .ok_or(TrueTypeError::BadTableDirectory)?;
                        let (nested_type, is_extension, nested_target) =
                            if let Some((is_extension, nested_target)) =
                                gpos_attachment_lookup(bytes, nested_lookup, 1, table_end)?
                            {
                                (1, is_extension, nested_target)
                            } else if let Some((is_extension, nested_target)) =
                                gpos_attachment_lookup(bytes, nested_lookup, 2, table_end)?
                            {
                                (2, is_extension, nested_target)
                            } else {
                                continue;
                            };
                        let nested_count = if is_extension {
                            1
                        } else {
                            usize::from(
                                read_u16(bytes, nested_lookup + 4)
                                    .ok_or(TrueTypeError::BadTableDirectory)?,
                            )
                        };
                        if nested_count > 32 {
                            return Err(TrueTypeError::TooLarge);
                        }
                        for nested_subtable_index in 0..nested_count {
                            let nested_subtable = if is_extension {
                                nested_target
                            } else {
                                nested_lookup
                                    .checked_add(usize::from(
                                        read_u16(
                                            bytes,
                                            nested_lookup + 6 + nested_subtable_index * 2,
                                        )
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                    ))
                                    .ok_or(TrueTypeError::BadTableDirectory)?
                            };
                            if nested_subtable
                                .checked_add(6)
                                .is_none_or(|end| end > table_end)
                            {
                                continue;
                            }
                            if nested_type == 2 {
                                if options.input_count < 2 || options.target_sequence_index > 4 {
                                    continue;
                                }
                                let pair_right = match options.target_sequence_index {
                                    0 => Some(right),
                                    1 if options.input_count >= 3 => options.third,
                                    1 => options.next,
                                    2 if options.input_count >= 4 => options.fourth,
                                    2 => options.next,
                                    3 if options.input_count >= 5 => options.fifth,
                                    3 => options.next,
                                    4 => options.next,
                                    _ => None,
                                };
                                let Some(pair_right) = pair_right else {
                                    continue;
                                };
                                if read_u16(bytes, nested_subtable) == Some(2) {
                                    if nested_subtable
                                        .checked_add(16)
                                        .is_none_or(|end| end > table_end)
                                    {
                                        continue;
                                    }
                                    let coverage = nested_subtable
                                        .checked_add(usize::from(
                                            read_u16(bytes, nested_subtable + 2)
                                                .ok_or(TrueTypeError::BadTableDirectory)?,
                                        ))
                                        .ok_or(TrueTypeError::BadTableDirectory)?;
                                    if gsub_coverage_index(
                                        bytes,
                                        coverage,
                                        table_end,
                                        target_glyph,
                                    )?
                                    .is_none()
                                    {
                                        continue;
                                    }
                                    let value_format1 = read_u16(bytes, nested_subtable + 4)
                                        .ok_or(TrueTypeError::BadTableDirectory)?;
                                    let value_format2 = read_u16(bytes, nested_subtable + 6)
                                        .ok_or(TrueTypeError::BadTableDirectory)?;
                                    let class_def1 = nested_subtable
                                        .checked_add(usize::from(
                                            read_u16(bytes, nested_subtable + 8)
                                                .ok_or(TrueTypeError::BadTableDirectory)?,
                                        ))
                                        .ok_or(TrueTypeError::BadTableDirectory)?;
                                    let class_def2 = nested_subtable
                                        .checked_add(usize::from(
                                            read_u16(bytes, nested_subtable + 10)
                                                .ok_or(TrueTypeError::BadTableDirectory)?,
                                        ))
                                        .ok_or(TrueTypeError::BadTableDirectory)?;
                                    let class1_count = usize::from(
                                        read_u16(bytes, nested_subtable + 12)
                                            .ok_or(TrueTypeError::BadTableDirectory)?,
                                    );
                                    let class2_count = usize::from(
                                        read_u16(bytes, nested_subtable + 14)
                                            .ok_or(TrueTypeError::BadTableDirectory)?,
                                    );
                                    if class1_count == 0
                                        || class2_count == 0
                                        || class1_count > 128
                                        || class2_count > 128
                                    {
                                        return Err(TrueTypeError::TooLarge);
                                    }
                                    let class1 = usize::from(gpos_class_index(
                                        bytes,
                                        class_def1,
                                        target_glyph,
                                        table_end,
                                    )?);
                                    let class2 = usize::from(gpos_class_index(
                                        bytes, class_def2, pair_right, table_end,
                                    )?);
                                    if class1 >= class1_count || class2 >= class2_count {
                                        continue;
                                    }
                                    let first_size = gpos_value_record_size(value_format1)?;
                                    let second_size = gpos_value_record_size(value_format2)?;
                                    let record_size = first_size
                                        .checked_add(second_size)
                                        .ok_or(TrueTypeError::TooLarge)?;
                                    let records_start = nested_subtable
                                        .checked_add(16)
                                        .ok_or(TrueTypeError::TooLarge)?;
                                    let record = records_start
                                        .checked_add(
                                            class1
                                                .checked_mul(class2_count)
                                                .and_then(|value| value.checked_add(class2))
                                                .and_then(|value| value.checked_mul(record_size))
                                                .ok_or(TrueTypeError::TooLarge)?,
                                        )
                                        .ok_or(TrueTypeError::TooLarge)?;
                                    let records_end = records_start
                                        .checked_add(
                                            class1_count
                                                .checked_mul(class2_count)
                                                .and_then(|value| value.checked_mul(record_size))
                                                .ok_or(TrueTypeError::TooLarge)?,
                                        )
                                        .ok_or(TrueTypeError::TooLarge)?;
                                    if records_end > table_end {
                                        return Err(TrueTypeError::BadTableDirectory);
                                    }
                                    let adjustment = gpos_value_adjustment(
                                        bytes,
                                        record,
                                        value_format1,
                                        table_end,
                                        table,
                                        options.ppem,
                                    )?;
                                    accumulated.0 = accumulated.0.saturating_add(adjustment.0);
                                    accumulated.1 = accumulated.1.saturating_add(adjustment.1);
                                    accumulated.2 = accumulated.2.saturating_add(adjustment.2);
                                    accumulated.3 = accumulated.3.saturating_add(adjustment.3);
                                    found = true;
                                    continue;
                                }
                                if nested_subtable
                                    .checked_add(10)
                                    .is_none_or(|end| end > table_end)
                                    || read_u16(bytes, nested_subtable) != Some(1)
                                {
                                    continue;
                                }
                                let coverage = nested_subtable
                                    .checked_add(usize::from(
                                        read_u16(bytes, nested_subtable + 2)
                                            .ok_or(TrueTypeError::BadTableDirectory)?,
                                    ))
                                    .ok_or(TrueTypeError::BadTableDirectory)?;
                                if gsub_coverage_index(bytes, coverage, table_end, target_glyph)?
                                    .is_none()
                                {
                                    continue;
                                }
                                let value_format1 = read_u16(bytes, nested_subtable + 4)
                                    .ok_or(TrueTypeError::BadTableDirectory)?;
                                let value_format2 = read_u16(bytes, nested_subtable + 6)
                                    .ok_or(TrueTypeError::BadTableDirectory)?;
                                let pair_set_count = usize::from(
                                    read_u16(bytes, nested_subtable + 8)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                );
                                let Some(coverage_index) =
                                    gsub_coverage_index(bytes, coverage, table_end, target_glyph)?
                                else {
                                    continue;
                                };
                                if pair_set_count > 128 || coverage_index >= pair_set_count {
                                    return Err(TrueTypeError::TooLarge);
                                }
                                let pair_set = nested_subtable
                                    .checked_add(usize::from(
                                        read_u16(bytes, nested_subtable + 10 + coverage_index * 2)
                                            .ok_or(TrueTypeError::BadTableDirectory)?,
                                    ))
                                    .ok_or(TrueTypeError::BadTableDirectory)?;
                                let first_size = gpos_value_record_size(value_format1)?;
                                let second_size = gpos_value_record_size(value_format2)?;
                                let record_size = 2usize
                                    .checked_add(first_size)
                                    .and_then(|size| size.checked_add(second_size))
                                    .ok_or(TrueTypeError::TooLarge)?;
                                if pair_set.checked_add(2).is_none_or(|end| end > table_end) {
                                    return Err(TrueTypeError::BadTableDirectory);
                                }
                                let pair_count = usize::from(
                                    read_u16(bytes, pair_set)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                );
                                if pair_count > 128
                                    || pair_set
                                        .checked_add(
                                            2usize
                                                .checked_add(
                                                    pair_count
                                                        .checked_mul(record_size)
                                                        .ok_or(TrueTypeError::TooLarge)?,
                                                )
                                                .ok_or(TrueTypeError::TooLarge)?,
                                        )
                                        .is_none_or(|end| end > table_end)
                                {
                                    return Err(TrueTypeError::TooLarge);
                                }
                                for pair_index in 0..pair_count {
                                    let pair_record = pair_set + 2 + pair_index * record_size;
                                    if read_u16(bytes, pair_record) != Some(pair_right) {
                                        continue;
                                    }
                                    let adjustment = gpos_value_adjustment(
                                        bytes,
                                        pair_record + 2,
                                        value_format1,
                                        table_end,
                                        table,
                                        options.ppem,
                                    )?;
                                    accumulated.0 = accumulated.0.saturating_add(adjustment.0);
                                    accumulated.1 = accumulated.1.saturating_add(adjustment.1);
                                    accumulated.2 = accumulated.2.saturating_add(adjustment.2);
                                    accumulated.3 = accumulated.3.saturating_add(adjustment.3);
                                    found = true;
                                    break;
                                }
                                if found {
                                    break;
                                }
                                continue;
                            }
                            if nested_type != 1 {
                                continue;
                            }
                            let coverage = nested_subtable
                                .checked_add(usize::from(
                                    read_u16(bytes, nested_subtable + 2)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                ))
                                .ok_or(TrueTypeError::BadTableDirectory)?;
                            let Some(coverage_index) =
                                gsub_coverage_index(bytes, coverage, table_end, left)?
                            else {
                                continue;
                            };
                            let value_format = read_u16(bytes, nested_subtable + 4)
                                .ok_or(TrueTypeError::BadTableDirectory)?;
                            let record = if read_u16(bytes, nested_subtable) == Some(1) {
                                nested_subtable + 6
                            } else {
                                let count = usize::from(
                                    read_u16(bytes, nested_subtable + 6)
                                        .ok_or(TrueTypeError::BadTableDirectory)?,
                                );
                                let record_size = gpos_value_record_size(value_format)?;
                                if count > 128 || coverage_index >= count {
                                    return Err(TrueTypeError::TooLarge);
                                }
                                nested_subtable
                                    .checked_add(
                                        8usize
                                            .checked_add(
                                                coverage_index
                                                    .checked_mul(record_size)
                                                    .ok_or(TrueTypeError::TooLarge)?,
                                            )
                                            .ok_or(TrueTypeError::TooLarge)?,
                                    )
                                    .ok_or(TrueTypeError::TooLarge)?
                            };
                            let adjustment = gpos_value_adjustment(
                                bytes,
                                record,
                                value_format,
                                table_end,
                                table,
                                options.ppem,
                            )?;
                            accumulated.0 = accumulated.0.saturating_add(adjustment.0);
                            accumulated.1 = accumulated.1.saturating_add(adjustment.1);
                            accumulated.2 = accumulated.2.saturating_add(adjustment.2);
                            accumulated.3 = accumulated.3.saturating_add(adjustment.3);
                            found = true;
                        }
                    }
                    if found {
                        return Ok(Some(accumulated));
                    }
                }
            }
        }
        Ok(None)
    }

    fn gpos_anchor_coordinates(
        face: &TrueTypeFace<'_>,
        anchor: usize,
        ppem: Option<u16>,
        table_end: usize,
        glyph: u16,
    ) -> Result<Option<(i32, i32)>, TrueTypeError> {
        let bytes = face.bytes;
        if anchor.checked_add(6).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let format = read_u16(bytes, anchor).ok_or(TrueTypeError::BadTableDirectory)?;
        let mut x = i32::from(read_i16(bytes, anchor + 2).ok_or(TrueTypeError::BadTableDirectory)?);
        let mut y = i32::from(read_i16(bytes, anchor + 4).ok_or(TrueTypeError::BadTableDirectory)?);
        match format {
            1 => Ok(Some((x, y))),
            2 => {
                if anchor.checked_add(8).is_none_or(|end| end > table_end) {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let point_index = usize::from(
                    read_u16(bytes, anchor + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let mut points = [OutlinePoint {
                    x: 0,
                    y: 0,
                    on_curve: false,
                    contour_end: false,
                }; 128];
                let point_count = face.outline_points_glyph(glyph, &mut points)?;
                if point_index >= point_count {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                Ok(Some((
                    i32::from(points[point_index].x),
                    i32::from(points[point_index].y),
                )))
            }
            3 => {
                if anchor.checked_add(10).is_none_or(|end| end > table_end) {
                    return Err(TrueTypeError::BadTableDirectory);
                }
                let x_device = usize::from(
                    read_u16(bytes, anchor + 6).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let y_device = usize::from(
                    read_u16(bytes, anchor + 8).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                if let Some(ppem) = ppem {
                    if x_device != 0 {
                        x = x.saturating_add(gpos_device_delta(
                            bytes,
                            anchor
                                .checked_add(x_device)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                            ppem,
                            table_end,
                        )?);
                    }
                    if y_device != 0 {
                        y = y.saturating_add(gpos_device_delta(
                            bytes,
                            anchor
                                .checked_add(y_device)
                                .ok_or(TrueTypeError::BadTableDirectory)?,
                            ppem,
                            table_end,
                        )?);
                    }
                }
                Ok(Some((x, y)))
            }
            _ => Ok(None),
        }
    }

    fn gpos_value_record_size(format: u16) -> Result<usize, TrueTypeError> {
        let fields = usize::try_from(format.count_ones()).map_err(|_| TrueTypeError::TooLarge)?;
        fields.checked_mul(2).ok_or(TrueTypeError::TooLarge)
    }

    fn gpos_value_adjustment(
        bytes: &[u8],
        start: usize,
        format: u16,
        table_end: usize,
        table_base: usize,
        ppem: Option<u16>,
    ) -> Result<(i32, i32, i32, i32), TrueTypeError> {
        let mut cursor = start;
        let mut x_placement = 0i32;
        let mut y_placement = 0i32;
        let mut x_advance = 0i32;
        let mut y_advance = 0i32;
        for bit in 0..16 {
            if format & (1 << bit) == 0 {
                continue;
            }
            if cursor.checked_add(2).is_none_or(|end| end > table_end) {
                return Err(TrueTypeError::BadTableDirectory);
            }
            let value = if bit < 4 {
                i32::from(read_i16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?)
            } else {
                let offset =
                    usize::from(read_u16(bytes, cursor).ok_or(TrueTypeError::BadTableDirectory)?);
                let ppem = ppem.ok_or(TrueTypeError::UnsupportedVariation)?;
                let device = table_base
                    .checked_add(offset)
                    .ok_or(TrueTypeError::BadTableDirectory)?;
                gpos_device_delta(bytes, device, ppem, table_end)?
            };
            match bit {
                0 | 4 => x_placement = x_placement.saturating_add(value),
                1 | 5 => y_placement = y_placement.saturating_add(value),
                2 | 6 => x_advance = x_advance.saturating_add(value),
                3 | 7 => y_advance = y_advance.saturating_add(value),
                _ => {}
            }
            cursor += 2;
        }
        Ok((x_placement, y_placement, x_advance, y_advance))
    }

    fn gpos_device_delta(
        bytes: &[u8],
        device: usize,
        ppem: u16,
        table_end: usize,
    ) -> Result<i32, TrueTypeError> {
        if device.checked_add(6).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let start = read_u16(bytes, device).ok_or(TrueTypeError::BadTableDirectory)?;
        let end = read_u16(bytes, device + 2).ok_or(TrueTypeError::BadTableDirectory)?;
        let format = read_u16(bytes, device + 4).ok_or(TrueTypeError::BadTableDirectory)?;
        if start > end || ppem < start || ppem > end {
            return Ok(0);
        }
        let index = usize::from(ppem - start);
        let bits = match format {
            1 => 2,
            2 => 4,
            3 => 8,
            _ => return Err(TrueTypeError::UnsupportedVariation),
        };
        let per_word = 16 / bits;
        let word = device
            .checked_add(6)
            .and_then(|offset| offset.checked_add((index / per_word) * 2))
            .ok_or(TrueTypeError::TooLarge)?;
        if word.checked_add(2).is_none_or(|end| end > table_end) {
            return Err(TrueTypeError::BadTableDirectory);
        }
        let raw = u32::from(read_u16(bytes, word).ok_or(TrueTypeError::BadTableDirectory)?);
        let shift = (per_word - 1 - index % per_word) * bits;
        let mask = (1u32 << bits) - 1;
        let value = (raw >> shift) & mask;
        let sign = 1u32 << (bits - 1);
        if value & sign != 0 {
            Ok(i32::try_from(value).map_err(|_| TrueTypeError::TooLarge)? - (1i32 << bits))
        } else {
            i32::try_from(value).map_err(|_| TrueTypeError::TooLarge)
        }
    }

    fn gpos_class_index(
        bytes: &[u8],
        definition: usize,
        glyph: u16,
        table_end: usize,
    ) -> Result<u16, TrueTypeError> {
        match read_u16(bytes, definition).ok_or(TrueTypeError::BadTableDirectory)? {
            1 => {
                let start =
                    read_u16(bytes, definition + 2).ok_or(TrueTypeError::BadTableDirectory)?;
                let count = usize::from(
                    read_u16(bytes, definition + 4).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let end = definition
                    .checked_add(
                        6usize
                            .checked_add(count.checked_mul(2).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if count > 128 || end > table_end {
                    return Err(TrueTypeError::TooLarge);
                }
                let index = usize::from(glyph.saturating_sub(start));
                if glyph < start || index >= count {
                    Ok(0)
                } else {
                    read_u16(bytes, definition + 6 + index * 2)
                        .ok_or(TrueTypeError::BadTableDirectory)
                }
            }
            2 => {
                let count = usize::from(
                    read_u16(bytes, definition + 2).ok_or(TrueTypeError::BadTableDirectory)?,
                );
                let end = definition
                    .checked_add(
                        4usize
                            .checked_add(count.checked_mul(6).ok_or(TrueTypeError::TooLarge)?)
                            .ok_or(TrueTypeError::TooLarge)?,
                    )
                    .ok_or(TrueTypeError::TooLarge)?;
                if count > 128 || end > table_end {
                    return Err(TrueTypeError::TooLarge);
                }
                for index in 0..count {
                    let range = definition + 4 + index * 6;
                    let start = read_u16(bytes, range).ok_or(TrueTypeError::BadTableDirectory)?;
                    let end_glyph =
                        read_u16(bytes, range + 2).ok_or(TrueTypeError::BadTableDirectory)?;
                    if (start..=end_glyph).contains(&glyph) {
                        return read_u16(bytes, range + 4).ok_or(TrueTypeError::BadTableDirectory);
                    }
                }
                Ok(0)
            }
            _ => Err(TrueTypeError::UnsupportedCmap),
        }
    }

    fn find_table(bytes: &[u8], table_count: usize, tag: [u8; 4]) -> Option<(usize, usize)> {
        for index in 0..table_count {
            let record = 12usize.checked_add(index.checked_mul(16)?)?;
            if bytes.get(record..record + 4)? != tag {
                continue;
            }
            let offset = usize::try_from(read_u32(bytes, record + 8)?).ok()?;
            let length = usize::try_from(read_u32(bytes, record + 12)?).ok()?;
            if offset.checked_add(length)? <= bytes.len() {
                return Some((offset, length));
            }
            return None;
        }
        None
    }

    impl Rect {
        pub const fn new(x: i32, y: i32, width: usize, height: usize) -> Self {
            Self {
                x,
                y,
                width,
                height,
            }
        }

        pub const fn is_empty(self) -> bool {
            self.width == 0 || self.height == 0
        }

        pub fn intersect(self, other: Self) -> Self {
            let left = self.x.max(other.x);
            let top = self.y.max(other.y);
            let right = self
                .x
                .saturating_add(i32::try_from(self.width).unwrap_or(i32::MAX))
                .min(
                    other
                        .x
                        .saturating_add(i32::try_from(other.width).unwrap_or(i32::MAX)),
                );
            let bottom = self
                .y
                .saturating_add(i32::try_from(self.height).unwrap_or(i32::MAX))
                .min(
                    other
                        .y
                        .saturating_add(i32::try_from(other.height).unwrap_or(i32::MAX)),
                );
            if right <= left || bottom <= top {
                return Self::new(0, 0, 0, 0);
            }
            Self::new(left, top, (right - left) as usize, (bottom - top) as usize)
        }
    }

    /// A projected 3D vertex. Coordinates are in the target Canvas space;
    /// `depth` is compared front-to-back by the software depth buffer.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Vertex3 {
        pub x: i32,
        pub y: i32,
        pub depth: u16,
        pub color: Color,
    }

    impl Vertex3 {
        pub const fn new(x: i32, y: i32, depth: u16, color: Color) -> Self {
            Self { x, y, depth, color }
        }
    }

    /// A projected 3D vertex that samples a source surface instead of
    /// carrying its own colour: `u`/`v` are normalized 0-255 across the
    /// source's width/height, the same precision as a [`Color`] channel.
    /// See [`Renderer::textured_triangle_3d`].
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Vertex3Tex {
        pub x: i32,
        pub y: i32,
        pub depth: u16,
        pub u: u8,
        pub v: u8,
    }

    impl Vertex3Tex {
        pub const fn new(x: i32, y: i32, depth: u16, u: u8, v: u8) -> Self {
            Self { x, y, depth, u, v }
        }
    }

    /// A perspective camera, fixed at the world origin looking down +z, that
    /// projects a 3D point onto the Canvas a [`Vertex3`]/[`Vertex3Tex`]
    /// needs. World coordinates are signed 16.16 fixed-point, the same
    /// format [`Transform`] already uses in this renderer so no
    /// floating-point state enters freestanding userspace.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Camera {
        /// 16.16 fixed-point: larger narrows the field of view (things
        /// appear larger and flatter), smaller widens it.
        pub focal_length: i32,
        /// 16.16 fixed-point distance the camera sits behind the world
        /// origin along z, so a scene built around z=0 is in front of it
        /// rather than dividing by (near) zero.
        pub distance: i32,
        /// Screen pixel the world origin's (x=0, y=0) ray through the
        /// camera lands on -- ordinary integer pixels, not fixed-point.
        pub screen_x: i32,
        pub screen_y: i32,
    }

    impl Camera {
        pub const fn new(focal_length: i32, distance: i32, screen_x: i32, screen_y: i32) -> Self {
            Self {
                focal_length,
                distance,
                screen_x,
                screen_y,
            }
        }

        /// Project a 16.16 fixed-point world point to screen pixels and a
        /// depth-buffer value (larger is farther, matching
        /// [`DepthBuffer`]'s "smaller wins" test). `None` behind the camera.
        pub fn project(&self, x: i32, y: i32, z: i32) -> Option<(i32, i32, u16)> {
            let eye_z = i64::from(z.saturating_add(self.distance));
            if eye_z <= 0 {
                return None;
            }
            // real_screen = (real_x * real_focal) / real_eye_z, with every
            // term divided by 65536 for its fixed-point scale; two of those
            // three factors are in the numerator and one in the
            // denominator, so exactly one 65536 remains to divide out.
            let scale = |coordinate: i32| -> i32 {
                (i128::from(coordinate) * i128::from(self.focal_length)
                    / (i128::from(eye_z) * 65536))
                    .clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
            };
            let screen_x = self.screen_x.saturating_add(scale(x));
            let screen_y = self.screen_y.saturating_add(scale(y));
            // A coarser (shifted-down) range than the fixed-point input is
            // plenty for depth-buffer ordering, and keeps it in u16.
            let depth = (eye_z >> 8).clamp(0, i64::from(u16::MAX)) as u16;
            Some((screen_x, screen_y, depth))
        }
    }

    /// Caller-owned depth storage paired with a Canvas. Keeping it outside
    /// Renderer makes lifetime and memory ownership explicit for future GPU
    /// targets, while retaining a bounded software fallback today.
    pub struct DepthBuffer<'a> {
        values: &'a mut [u16],
        x: i32,
        y: i32,
        width: usize,
        height: usize,
    }

    impl<'a> DepthBuffer<'a> {
        pub fn new(
            values: &'a mut [u16],
            x: i32,
            y: i32,
            width: usize,
            height: usize,
        ) -> Option<Self> {
            if !crate::coordinate_extent_fits(x, width) || !crate::coordinate_extent_fits(y, height)
            {
                return None;
            }
            let length = width.checked_mul(height)?;
            values.get_mut(..length)?;
            Some(Self {
                values,
                x,
                y,
                width,
                height,
            })
        }

        pub fn clear(&mut self, depth: u16) {
            self.values.fill(depth);
        }

        fn test_and_set(&mut self, x: i32, y: i32, depth: u16) -> bool {
            let Some(column) = usize::try_from(x.saturating_sub(self.x)).ok() else {
                return false;
            };
            let Some(row) = usize::try_from(y.saturating_sub(self.y)).ok() else {
                return false;
            };
            if column >= self.width || row >= self.height {
                return false;
            }
            let slot = &mut self.values[row * self.width + column];
            if depth >= *slot {
                return false;
            }
            *slot = depth;
            true
        }
    }

    /// The first WovenGraphics renderer: a safe, clipped software renderer
    /// over a caller-owned Canvas. A later display driver can consume the same
    /// pixel stream without changing application scene code.
    pub struct Renderer<'canvas, 'pixels> {
        canvas: &'canvas mut crate::Canvas<'pixels>,
    }

    impl<'canvas, 'pixels> Renderer<'canvas, 'pixels> {
        pub fn new(canvas: &'canvas mut crate::Canvas<'pixels>) -> Self {
            Self { canvas }
        }

        pub fn fill(&mut self, rect: Rect, color: Color) {
            self.canvas
                .fill_rect(rect.x, rect.y, rect.width, rect.height, color.packed());
        }

        pub fn blend(&mut self, rect: Rect, color: Color, opacity: u8) {
            self.canvas.blend_rect(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                color.packed(),
                opacity,
            );
        }

        /// Fill a rounded rectangle with opaque colour. Radius is clamped to
        /// half the shorter dimension before rasterization.
        pub fn rounded_rect(&mut self, rect: Rect, radius: usize, color: Color) {
            self.rounded_rect_blended(rect, radius, color, u8::MAX);
        }

        /// Fill a rounded rectangle with source-over alpha. Iteration is
        /// clipped to the Canvas before any per-pixel corner calculation.
        pub fn rounded_rect_blended(
            &mut self,
            rect: Rect,
            radius: usize,
            color: Color,
            opacity: u8,
        ) {
            if rect.is_empty() {
                return;
            }
            let radius = radius.min(rect.width.min(rect.height) / 2);
            let Some((column, row, columns, rows)) =
                self.canvas.clip(rect.x, rect.y, rect.width, rect.height)
            else {
                return;
            };
            let right = rect_end(rect.x, rect.width);
            let bottom = rect_end(rect.y, rect.height);
            let radius = i32::try_from(radius).unwrap_or(i32::MAX);
            let r = radius as i64;
            let r2 = r * r;
            for canvas_row in row..row + rows {
                let y = self.canvas.y + canvas_row as i32;
                for canvas_column in column..column + columns {
                    let x = self.canvas.x + canvas_column as i32;
                    let corner_x = if x < rect.x.saturating_add(radius) {
                        rect.x.saturating_add(radius)
                    } else if x > right.saturating_sub(radius) {
                        right.saturating_sub(radius)
                    } else {
                        x
                    };
                    let corner_y = if y < rect.y.saturating_add(radius) {
                        rect.y.saturating_add(radius)
                    } else if y > bottom.saturating_sub(radius) {
                        bottom.saturating_sub(radius)
                    } else {
                        y
                    };
                    let dx = i64::from(x) - i64::from(corner_x);
                    let dy = i64::from(y) - i64::from(corner_y);
                    if dx * dx + dy * dy <= r2 {
                        self.canvas.blend_pixel(x, y, color.packed(), opacity);
                    }
                }
            }
        }

        /// Fill a rectangle with a clipped top-to-bottom colour gradient.
        pub fn vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color) {
            let Some((column, row, columns, rows)) =
                self.canvas.clip(rect.x, rect.y, rect.width, rect.height)
            else {
                return;
            };
            let last = i128::try_from(rect.height.saturating_sub(1).max(1)).unwrap_or(i128::MAX);
            for canvas_row in row..row + rows {
                let y = self.canvas.y + canvas_row as i32;
                let offset = (i128::from(y) - i128::from(rect.y)).clamp(0, last);
                let t = (offset * 255 / last) as u8;
                let color = Color::rgb(
                    lerp(top.red, bottom.red, t),
                    lerp(top.green, bottom.green, t),
                    lerp(top.blue, bottom.blue, t),
                );
                for canvas_column in column..column + columns {
                    self.canvas
                        .set(self.canvas.x + canvas_column as i32, y, color.packed());
                }
            }
        }

        /// Fill a clipped left-to-right colour gradient.
        pub fn horizontal_gradient(&mut self, rect: Rect, left: Color, right: Color) {
            let Some((column, row, columns, rows)) =
                self.canvas.clip(rect.x, rect.y, rect.width, rect.height)
            else {
                return;
            };
            let last = i128::try_from(rect.width.saturating_sub(1).max(1)).unwrap_or(i128::MAX);
            for canvas_column in column..column + columns {
                let x = self.canvas.x + canvas_column as i32;
                let offset = (i128::from(x) - i128::from(rect.x)).clamp(0, last);
                let t = (offset * 255 / last) as u8;
                let color = Color::rgb(
                    lerp(left.red, right.red, t),
                    lerp(left.green, right.green, t),
                    lerp(left.blue, right.blue, t),
                );
                for canvas_row in row..row + rows {
                    self.canvas
                        .set(x, self.canvas.y + canvas_row as i32, color.packed());
                }
            }
        }

        /// Draw a one-pixel rectangle outline, clipped by the Canvas.
        pub fn stroke(&mut self, rect: Rect, color: Color) {
            if rect.is_empty() {
                return;
            }
            self.fill(Rect::new(rect.x, rect.y, rect.width, 1), color);
            let right = rect
                .x
                .saturating_add(i32::try_from(rect.width.saturating_sub(1)).unwrap_or(i32::MAX));
            let bottom = rect
                .y
                .saturating_add(i32::try_from(rect.height.saturating_sub(1)).unwrap_or(i32::MAX));
            self.fill(Rect::new(rect.x, bottom, rect.width, 1), color);
            self.fill(Rect::new(rect.x, rect.y, 1, rect.height), color);
            self.fill(Rect::new(right, rect.y, 1, rect.height), color);
        }

        /// Blit a bounded row-major image into the Canvas. Invalid dimensions
        /// or a short source leave the target unchanged and return `false`.
        pub fn image(
            &mut self,
            x: i32,
            y: i32,
            width: usize,
            height: usize,
            pixels: &[u32],
        ) -> bool {
            let Some(length) = width.checked_mul(height) else {
                return false;
            };
            let Some(source) = pixels.get(..length) else {
                return false;
            };
            let Some((canvas_column, canvas_row, columns, rows)) =
                self.canvas.clip(x, y, width, height)
            else {
                return true;
            };
            let source_column = usize::try_from(
                i128::from(self.canvas.x) + i128::try_from(canvas_column).unwrap_or(i128::MAX)
                    - i128::from(x),
            )
            .unwrap_or(0);
            let source_row = usize::try_from(
                i128::from(self.canvas.y) + i128::try_from(canvas_row).unwrap_or(i128::MAX)
                    - i128::from(y),
            )
            .unwrap_or(0);
            for row in 0..rows {
                let source_start = source_row
                    .checked_add(row)
                    .and_then(|row| row.checked_mul(width))
                    .and_then(|row| row.checked_add(source_column));
                let Some(source_start) = source_start else {
                    return false;
                };
                for column in 0..columns {
                    let Some(pixel) = source.get(source_start + column).copied() else {
                        return false;
                    };
                    self.canvas.set(
                        self.canvas.x + canvas_column as i32 + column as i32,
                        self.canvas.y + canvas_row as i32 + row as i32,
                        pixel,
                    );
                }
            }
            true
        }

        /// Blit a bounded row-major image with uniform source-over opacity.
        /// Invalid dimensions or a short source leave the target unchanged.
        pub fn image_blended(
            &mut self,
            x: i32,
            y: i32,
            width: usize,
            height: usize,
            pixels: &[u32],
            opacity: u8,
        ) -> bool {
            let Some(length) = width.checked_mul(height) else {
                return false;
            };
            let Some(source) = pixels.get(..length) else {
                return false;
            };
            let Some((canvas_column, canvas_row, columns, rows)) =
                self.canvas.clip(x, y, width, height)
            else {
                return true;
            };
            let source_column = usize::try_from(
                i128::from(self.canvas.x) + i128::try_from(canvas_column).unwrap_or(i128::MAX)
                    - i128::from(x),
            )
            .unwrap_or(0);
            let source_row = usize::try_from(
                i128::from(self.canvas.y) + i128::try_from(canvas_row).unwrap_or(i128::MAX)
                    - i128::from(y),
            )
            .unwrap_or(0);
            for row in 0..rows {
                let source_start = source_row
                    .checked_add(row)
                    .and_then(|row| row.checked_mul(width))
                    .and_then(|row| row.checked_add(source_column));
                let Some(source_start) = source_start else {
                    return false;
                };
                for column in 0..columns {
                    let Some(pixel) = source.get(source_start + column).copied() else {
                        return false;
                    };
                    self.canvas.blend_pixel(
                        self.canvas.x + canvas_column as i32 + column as i32,
                        self.canvas.y + canvas_row as i32 + row as i32,
                        pixel,
                        opacity,
                    );
                }
            }
            true
        }

        /// Fill a transformed rectangle as two clipped triangles. This keeps
        /// transforms a scene-level concern while using the same safe Canvas
        /// pixel boundary as every other WovenGraphics primitive.
        pub fn transformed_rect(&mut self, rect: Rect, transform: Transform, color: Color) {
            if rect.is_empty() {
                return;
            }
            let right = rect
                .x
                .saturating_add(i32::try_from(rect.width).unwrap_or(i32::MAX));
            let bottom = rect
                .y
                .saturating_add(i32::try_from(rect.height).unwrap_or(i32::MAX));
            let (x0, y0) = transform.map(rect.x, rect.y);
            let (x1, y1) = transform.map(right, rect.y);
            let (x2, y2) = transform.map(right, bottom);
            let (x3, y3) = transform.map(rect.x, bottom);
            self.fill_triangle([(x0, y0), (x1, y1), (x2, y2)], color);
            self.fill_triangle([(x0, y0), (x2, y2), (x3, y3)], color);
        }

        fn fill_triangle(&mut self, points: [(i32, i32); 3], color: Color) {
            let [(ax, ay), (bx, by), (cx, cy)] = points;
            let area = edge(ax, ay, bx, by, cx, cy);
            if area == 0 {
                return;
            }
            let canvas_right = self.canvas.x.saturating_add(
                i32::try_from(self.canvas.width.saturating_sub(1)).unwrap_or(i32::MAX),
            );
            let canvas_bottom = self.canvas.y.saturating_add(
                i32::try_from(self.canvas.height.saturating_sub(1)).unwrap_or(i32::MAX),
            );
            let min_x = ax.min(bx).min(cx).max(self.canvas.x);
            let max_x = ax.max(bx).max(cx).min(canvas_right);
            let min_y = ay.min(by).min(cy).max(self.canvas.y);
            let max_y = ay.max(by).max(cy).min(canvas_bottom);
            if min_x > max_x || min_y > max_y {
                return;
            }
            let positive = area > 0;
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let first = edge(bx, by, cx, cy, x, y);
                    let second = edge(cx, cy, ax, ay, x, y);
                    let third = edge(ax, ay, bx, by, x, y);
                    let inside = if positive {
                        first >= 0 && second >= 0 && third >= 0
                    } else {
                        first <= 0 && second <= 0 && third <= 0
                    };
                    if inside {
                        self.canvas.set(x, y, color.packed());
                    }
                }
            }
        }

        /// Rasterize a clipped, flat-colour triangle with a caller-owned
        /// front-to-back depth buffer. This is the first software 3D path;
        /// projected vertices can later be supplied by a GPU backend without
        /// changing the scene-facing WovenGraphics types.
        pub fn triangle_3d(
            &mut self,
            depth: &mut DepthBuffer<'_>,
            first: Vertex3,
            second: Vertex3,
            third: Vertex3,
        ) {
            let area = edge(first.x, first.y, second.x, second.y, third.x, third.y);
            if area == 0 {
                return;
            }
            let canvas_right = self.canvas.x.saturating_add(
                i32::try_from(self.canvas.width.saturating_sub(1)).unwrap_or(i32::MAX),
            );
            let canvas_bottom = self.canvas.y.saturating_add(
                i32::try_from(self.canvas.height.saturating_sub(1)).unwrap_or(i32::MAX),
            );
            let depth_right = depth
                .x
                .saturating_add(i32::try_from(depth.width.saturating_sub(1)).unwrap_or(i32::MAX));
            let depth_bottom = depth
                .y
                .saturating_add(i32::try_from(depth.height.saturating_sub(1)).unwrap_or(i32::MAX));
            let min_x = first
                .x
                .min(second.x)
                .min(third.x)
                .max(self.canvas.x)
                .max(depth.x);
            let max_x = first
                .x
                .max(second.x)
                .max(third.x)
                .min(canvas_right)
                .min(depth_right);
            let min_y = first
                .y
                .min(second.y)
                .min(third.y)
                .max(self.canvas.y)
                .max(depth.y);
            let max_y = first
                .y
                .max(second.y)
                .max(third.y)
                .min(canvas_bottom)
                .min(depth_bottom);
            if min_x > max_x || min_y > max_y {
                return;
            }
            let positive = area > 0;
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let first_edge = edge(second.x, second.y, third.x, third.y, x, y);
                    let second_edge = edge(third.x, third.y, first.x, first.y, x, y);
                    let third_edge = edge(first.x, first.y, second.x, second.y, x, y);
                    let inside = if positive {
                        first_edge >= 0 && second_edge >= 0 && third_edge >= 0
                    } else {
                        first_edge <= 0 && second_edge <= 0 && third_edge <= 0
                    };
                    if !inside {
                        continue;
                    }
                    let depth_value = ((i128::from(first.depth) * first_edge.abs()
                        + i128::from(second.depth) * second_edge.abs()
                        + i128::from(third.depth) * third_edge.abs())
                        / area.abs())
                    .min(i128::from(u16::MAX)) as u16;
                    if depth.test_and_set(x, y, depth_value) {
                        let red = interpolate_channel(
                            first.color.red,
                            second.color.red,
                            third.color.red,
                            first_edge.abs(),
                            second_edge.abs(),
                            third_edge.abs(),
                            area.abs(),
                        );
                        let green = interpolate_channel(
                            first.color.green,
                            second.color.green,
                            third.color.green,
                            first_edge.abs(),
                            second_edge.abs(),
                            third_edge.abs(),
                            area.abs(),
                        );
                        let blue = interpolate_channel(
                            first.color.blue,
                            second.color.blue,
                            third.color.blue,
                            first_edge.abs(),
                            second_edge.abs(),
                            third_edge.abs(),
                            area.abs(),
                        );
                        self.canvas.set(x, y, Color::rgb(red, green, blue).packed());
                    }
                }
            }
        }

        /// As [`Self::triangle_3d`], but each covered pixel samples `source`
        /// (nearest-neighbour, by the affine-interpolated `u`/`v`) instead of
        /// interpolating a flat per-vertex colour. This is how an existing
        /// 2D surface -- a window's own rendered `Canvas`, unchanged -- becomes
        /// one textured face of a 3D scene: the desktop projects windows as
        /// textured planes, it does not re-render their contents in 3D.
        pub fn textured_triangle_3d(
            &mut self,
            depth: &mut DepthBuffer<'_>,
            first: Vertex3Tex,
            second: Vertex3Tex,
            third: Vertex3Tex,
            source: &crate::Canvas<'_>,
        ) {
            let area = edge(first.x, first.y, second.x, second.y, third.x, third.y);
            if area == 0 || source.width() == 0 || source.height() == 0 {
                return;
            }
            let canvas_right = self.canvas.x.saturating_add(
                i32::try_from(self.canvas.width.saturating_sub(1)).unwrap_or(i32::MAX),
            );
            let canvas_bottom = self.canvas.y.saturating_add(
                i32::try_from(self.canvas.height.saturating_sub(1)).unwrap_or(i32::MAX),
            );
            let depth_right = depth
                .x
                .saturating_add(i32::try_from(depth.width.saturating_sub(1)).unwrap_or(i32::MAX));
            let depth_bottom = depth
                .y
                .saturating_add(i32::try_from(depth.height.saturating_sub(1)).unwrap_or(i32::MAX));
            let min_x = first
                .x
                .min(second.x)
                .min(third.x)
                .max(self.canvas.x)
                .max(depth.x);
            let max_x = first
                .x
                .max(second.x)
                .max(third.x)
                .min(canvas_right)
                .min(depth_right);
            let min_y = first
                .y
                .min(second.y)
                .min(third.y)
                .max(self.canvas.y)
                .max(depth.y);
            let max_y = first
                .y
                .max(second.y)
                .max(third.y)
                .min(canvas_bottom)
                .min(depth_bottom);
            if min_x > max_x || min_y > max_y {
                return;
            }
            let positive = area > 0;
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let first_edge = edge(second.x, second.y, third.x, third.y, x, y);
                    let second_edge = edge(third.x, third.y, first.x, first.y, x, y);
                    let third_edge = edge(first.x, first.y, second.x, second.y, x, y);
                    let inside = if positive {
                        first_edge >= 0 && second_edge >= 0 && third_edge >= 0
                    } else {
                        first_edge <= 0 && second_edge <= 0 && third_edge <= 0
                    };
                    if !inside {
                        continue;
                    }
                    let depth_value = ((i128::from(first.depth) * first_edge.abs()
                        + i128::from(second.depth) * second_edge.abs()
                        + i128::from(third.depth) * third_edge.abs())
                        / area.abs())
                    .min(i128::from(u16::MAX)) as u16;
                    if !depth.test_and_set(x, y, depth_value) {
                        continue;
                    }
                    let u = interpolate_channel(
                        first.u,
                        second.u,
                        third.u,
                        first_edge.abs(),
                        second_edge.abs(),
                        third_edge.abs(),
                        area.abs(),
                    );
                    let v = interpolate_channel(
                        first.v,
                        second.v,
                        third.v,
                        first_edge.abs(),
                        second_edge.abs(),
                        third_edge.abs(),
                        area.abs(),
                    );
                    let sample_x =
                        source.x() + (usize::from(u) * (source.width() - 1) / 255) as i32;
                    let sample_y =
                        source.y() + (usize::from(v) * (source.height() - 1) / 255) as i32;
                    if let Some(sampled) = source.get(sample_x, sample_y) {
                        self.canvas.set(x, y, sampled);
                    }
                }
            }
        }

        pub fn text(&mut self, x: i32, y: i32, text: &str, color: Color) {
            self.canvas.draw_text(x, y, text, color.packed());
        }

        /// Draw text centered in a logical rectangle using the shared font's
        /// bounded metrics. Text remains clipped by the Canvas boundary.
        pub fn text_centered(&mut self, rect: Rect, text: &str, color: Color) {
            self.text_centered_scaled(rect, text, color, 1);
        }

        /// Center text using a bounded integer glyph scale.
        pub fn text_centered_scaled(&mut self, rect: Rect, text: &str, color: Color, scale: usize) {
            let metrics = Font::DEFAULT.measure_scaled(text, scale);
            let x = rect.x.saturating_add(
                i32::try_from(rect.width.saturating_sub(metrics.width) / 2).unwrap_or(i32::MAX),
            );
            let y = rect.y.saturating_add(
                i32::try_from(rect.height.saturating_sub(metrics.height) / 2).unwrap_or(i32::MAX),
            );
            self.text_scaled(x, y, text, color, scale);
        }

        /// Draw the shared bitmap font with an integer pixel scale. This is a
        /// bounded nearest-neighbour raster operation and preserves the same
        /// replacement-glyph behavior as the unscaled path.
        pub fn text_scaled(&mut self, x: i32, y: i32, text: &str, color: Color, scale: usize) {
            let scale = scale.clamp(1, 16);
            let mut cursor_x = x;
            let mut cursor_y = y;
            for character in text.chars() {
                if character == '\n' {
                    cursor_x = x;
                    cursor_y = cursor_y
                        .saturating_add(crate::font::LINE_HEIGHT.saturating_mul(scale as i32));
                    continue;
                }
                for (row, bits) in crate::font::glyph(character).iter().enumerate() {
                    for column in 0..crate::font::WIDTH {
                        if bits & (1 << (crate::font::WIDTH - 1 - column)) == 0 {
                            continue;
                        }
                        self.fill(
                            Rect::new(
                                cursor_x.saturating_add(column as i32 * scale as i32),
                                cursor_y.saturating_add(row as i32 * scale as i32),
                                scale,
                                scale,
                            ),
                            color,
                        );
                    }
                }
                cursor_x =
                    cursor_x.saturating_add(crate::font::ADVANCE.saturating_mul(scale as i32));
            }
        }

        /// Draw UTF-8 text with deterministic character wrapping inside a
        /// clipped rectangle. Wrapping is allocation-free and uses the same
        /// replacement glyph and integer scale as `text_scaled`.
        pub fn text_box(&mut self, rect: Rect, text: &str, color: Color, scale: usize) -> usize {
            if rect.is_empty() {
                return 0;
            }
            let scale = scale.clamp(1, 16);
            let advance = crate::font::ADVANCE.saturating_mul(scale as i32);
            let line_height = crate::font::LINE_HEIGHT.saturating_mul(scale as i32);
            let right = rect
                .x
                .saturating_add(i32::try_from(rect.width.saturating_sub(1)).unwrap_or(i32::MAX));
            let bottom = rect
                .y
                .saturating_add(i32::try_from(rect.height.saturating_sub(1)).unwrap_or(i32::MAX));
            let mut cursor_x = rect.x;
            let mut cursor_y = rect.y;
            let mut lines: usize = 1;
            for character in text.chars() {
                if character == '\n'
                    || cursor_x > rect.x && cursor_x.saturating_add(advance - 1) > right
                {
                    cursor_x = rect.x;
                    cursor_y = cursor_y.saturating_add(line_height);
                    lines = lines.saturating_add(1);
                    if character == '\n' {
                        continue;
                    }
                }
                if cursor_y.saturating_add(crate::font::HEIGHT as i32 * scale as i32 - 1) > bottom {
                    break;
                }
                let mut encoded = [0u8; 4];
                let glyph = character.encode_utf8(&mut encoded);
                self.text_scaled(cursor_x, cursor_y, glyph, color, scale);
                cursor_x = cursor_x.saturating_add(advance);
            }
            lines
        }

        /// Stroke every segment in a bounded path with a clipped integer line
        /// rasterizer. Closed paths add the final segment back to the origin.
        pub fn stroke_path(&mut self, path: &Path, color: Color) {
            let points = path.points();
            if points.len() < 2 {
                return;
            }
            for pair in points.windows(2) {
                self.stroke_line(pair[0], pair[1], color);
            }
            if path.closed {
                self.stroke_line(points[points.len() - 1], points[0], color);
            }
        }

        pub fn stroke_compound_path(&mut self, path: &CompoundPath, color: Color) {
            for contour in path.paths() {
                self.stroke_path(contour, color);
            }
        }

        /// Fill a closed path with an even-odd winding rule. The scan is
        /// clipped to the Canvas before iterating, so hostile or merely large
        /// scene coordinates cannot turn a bounded surface into unbounded
        /// work. Returns `false` for an open or too-short path.
        pub fn fill_path(&mut self, path: &Path, color: Color) -> bool {
            self.fill_path_blended(path, color, u8::MAX)
        }

        /// Fill a closed path with uniform alpha using the same source-over
        /// formula as rectangle and compositor blending.
        pub fn fill_path_blended(&mut self, path: &Path, color: Color, opacity: u8) -> bool {
            let points = path.points();
            if points.len() < 3 || !path.closed {
                return false;
            }
            let mut min_x = points[0].x;
            let mut max_x = points[0].x;
            let mut min_y = points[0].y;
            let mut max_y = points[0].y;
            for point in &points[1..] {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }
            let canvas_max_x = rect_end(self.canvas.x, self.canvas.width);
            let canvas_max_y = rect_end(self.canvas.y, self.canvas.height);
            min_x = min_x.max(self.canvas.x);
            max_x = max_x.min(canvas_max_x);
            min_y = min_y.max(self.canvas.y);
            max_y = max_y.min(canvas_max_y);
            if min_x > max_x || min_y > max_y {
                return true;
            }
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let mut inside = false;
                    for index in 0..points.len() {
                        let first = points[index];
                        let second = points[(index + 1) % points.len()];
                        if (first.y > y) != (second.y > y) {
                            let numerator = (i128::from(y) - i128::from(first.y))
                                * (i128::from(second.x) - i128::from(first.x));
                            let denominator = i128::from(second.y) - i128::from(first.y);
                            let intersection = i128::from(first.x) + numerator / denominator;
                            if i128::from(x) < intersection {
                                inside = !inside;
                            }
                        }
                    }
                    if inside {
                        self.canvas.blend_pixel(x, y, color.packed(), opacity);
                    }
                }
            }
            true
        }

        /// Fill a closed path with deterministic 4x4 supersampled coverage.
        pub fn fill_path_antialiased(&mut self, path: &Path, color: Color) -> bool {
            self.fill_path_antialiased_blended(path, color, u8::MAX)
        }

        pub fn fill_path_antialiased_blended(
            &mut self,
            path: &Path,
            color: Color,
            opacity: u8,
        ) -> bool {
            let points = path.points();
            if points.len() < 3 || !path.closed {
                return false;
            }
            let (mut min_x, mut max_x, mut min_y, mut max_y) = path_bounds(points);
            min_x = min_x.saturating_sub(1).max(self.canvas.x);
            max_x = max_x
                .saturating_add(1)
                .min(rect_end(self.canvas.x, self.canvas.width));
            min_y = min_y.saturating_sub(1).max(self.canvas.y);
            max_y = max_y
                .saturating_add(1)
                .min(rect_end(self.canvas.y, self.canvas.height));
            if min_x > max_x || min_y > max_y {
                return true;
            }
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let coverage = path_coverage(points, x, y);
                    if coverage != 0 {
                        self.canvas.blend_pixel(
                            x,
                            y,
                            color.packed(),
                            combine_opacity(opacity, coverage),
                        );
                    }
                }
            }
            true
        }

        pub fn fill_compound_path(&mut self, path: &CompoundPath, color: Color) -> bool {
            self.fill_compound_path_blended(path, color, u8::MAX)
        }

        pub fn fill_compound_path_blended(
            &mut self,
            path: &CompoundPath,
            color: Color,
            opacity: u8,
        ) -> bool {
            let mut contours: [Option<&Path>; MAX_CONTOURS] = [None; MAX_CONTOURS];
            let mut contour_count = 0;
            for contour in path.paths() {
                if contour.points().len() >= 3 && contour.closed && contour_count < MAX_CONTOURS {
                    contours[contour_count] = Some(contour);
                    contour_count += 1;
                }
            }
            if contour_count == 0 {
                return false;
            }
            let first = match contours[0] {
                Some(contour) => contour,
                None => return false,
            };
            let mut min_x = first.points()[0].x;
            let mut max_x = min_x;
            let mut min_y = first.points()[0].y;
            let mut max_y = min_y;
            for contour in contours.iter().take(contour_count).flatten() {
                for point in contour.points() {
                    min_x = min_x.min(point.x);
                    max_x = max_x.max(point.x);
                    min_y = min_y.min(point.y);
                    max_y = max_y.max(point.y);
                }
            }
            let canvas_max_x = rect_end(self.canvas.x, self.canvas.width);
            let canvas_max_y = rect_end(self.canvas.y, self.canvas.height);
            min_x = min_x.max(self.canvas.x);
            max_x = max_x.min(canvas_max_x);
            min_y = min_y.max(self.canvas.y);
            max_y = max_y.min(canvas_max_y);
            if min_x > max_x || min_y > max_y {
                return true;
            }
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let mut inside = false;
                    for contour in contours.iter().take(contour_count).flatten() {
                        let points = contour.points();
                        for index in 0..points.len() {
                            let first = points[index];
                            let second = points[(index + 1) % points.len()];
                            if (first.y > y) != (second.y > y) {
                                let numerator = (i128::from(y) - i128::from(first.y))
                                    * (i128::from(second.x) - i128::from(first.x));
                                let denominator = i128::from(second.y) - i128::from(first.y);
                                let intersection = i128::from(first.x) + numerator / denominator;
                                if i128::from(x) < intersection {
                                    inside = !inside;
                                }
                            }
                        }
                    }
                    if inside {
                        self.canvas.blend_pixel(x, y, color.packed(), opacity);
                    }
                }
            }
            true
        }

        pub fn fill_compound_path_antialiased(
            &mut self,
            path: &CompoundPath,
            color: Color,
        ) -> bool {
            self.fill_compound_path_antialiased_blended(path, color, u8::MAX)
        }

        pub fn fill_compound_path_antialiased_blended(
            &mut self,
            path: &CompoundPath,
            color: Color,
            opacity: u8,
        ) -> bool {
            let contours = path.paths();
            let contour_count = contours.len();
            if contour_count == 0
                || path
                    .paths()
                    .iter()
                    .any(|contour| contour.points().len() < 3 || !contour.closed)
            {
                return false;
            }
            let (mut min_x, mut max_x, mut min_y, mut max_y) = path_bounds(contours[0].points());
            for contour in contours.iter().skip(1) {
                let (left, right, top, bottom) = path_bounds(contour.points());
                min_x = min_x.min(left);
                max_x = max_x.max(right);
                min_y = min_y.min(top);
                max_y = max_y.max(bottom);
            }
            min_x = min_x.saturating_sub(1).max(self.canvas.x);
            max_x = max_x
                .saturating_add(1)
                .min(rect_end(self.canvas.x, self.canvas.width));
            min_y = min_y.saturating_sub(1).max(self.canvas.y);
            max_y = max_y
                .saturating_add(1)
                .min(rect_end(self.canvas.y, self.canvas.height));
            if min_x > max_x || min_y > max_y {
                return true;
            }
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let mut covered = 0u8;
                    for sample_y in 0..4 {
                        for sample_x in 0..4 {
                            let point_x = i128::from(x) * 16 + i128::from(sample_x * 4 + 2);
                            let point_y = i128::from(y) * 16 + i128::from(sample_y * 4 + 2);
                            let inside = contours.iter().fold(false, |inside, contour| {
                                inside ^ path_contains_sample(contour.points(), point_x, point_y)
                            });
                            if inside {
                                covered += 1;
                            }
                        }
                    }
                    if covered != 0 {
                        self.canvas.blend_pixel(
                            x,
                            y,
                            color.packed(),
                            combine_opacity(opacity, covered.saturating_mul(16)),
                        );
                    }
                }
            }
            true
        }

        fn stroke_line(&mut self, first: Point, second: Point, color: Color) {
            if self.canvas.width == 0 || self.canvas.height == 0 {
                return;
            }
            let Some((first, second)) = clip_line_to_bounds(
                first,
                second,
                self.canvas.x,
                self.canvas.y,
                rect_end(self.canvas.x, self.canvas.width),
                rect_end(self.canvas.y, self.canvas.height),
            ) else {
                return;
            };
            let mut x = first.x;
            let mut y = first.y;
            let dx = (i64::from(second.x) - i64::from(first.x)).abs();
            let sx = if first.x < second.x { 1 } else { -1 };
            let dy = -(i64::from(second.y) - i64::from(first.y)).abs();
            let sy = if first.y < second.y { 1 } else { -1 };
            let mut error = dx + dy;
            loop {
                self.canvas.set(x, y, color.packed());
                if x == second.x && y == second.y {
                    break;
                }
                let twice = error * 2;
                if twice >= dy {
                    error += dy;
                    x += sx;
                }
                if twice <= dx {
                    error += dx;
                    y += sy;
                }
            }
        }
    }

    fn path_bounds(points: &[Point]) -> (i32, i32, i32, i32) {
        let mut min_x = points[0].x;
        let mut max_x = min_x;
        let mut min_y = points[0].y;
        let mut max_y = min_y;
        for point in &points[1..] {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }
        (min_x, max_x, min_y, max_y)
    }

    fn path_contains_sample(points: &[Point], sample_x: i128, sample_y: i128) -> bool {
        let mut inside = false;
        for index in 0..points.len() {
            let first = points[index];
            let second = points[(index + 1) % points.len()];
            let first_y = i128::from(first.y) * 16;
            let second_y = i128::from(second.y) * 16;
            if (first_y > sample_y) != (second_y > sample_y) {
                let numerator =
                    (sample_y - first_y) * (i128::from(second.x) - i128::from(first.x)) * 16;
                let denominator = second_y - first_y;
                let intersection = i128::from(first.x) * 16 + numerator / denominator;
                if sample_x < intersection {
                    inside = !inside;
                }
            }
        }
        inside
    }

    fn path_coverage(points: &[Point], x: i32, y: i32) -> u8 {
        let mut covered = 0u8;
        for sample_y in 0..4 {
            for sample_x in 0..4 {
                let point_x = i128::from(x) * 16 + i128::from(sample_x * 4 + 2);
                let point_y = i128::from(y) * 16 + i128::from(sample_y * 4 + 2);
                if path_contains_sample(points, point_x, point_y) {
                    covered += 1;
                }
            }
        }
        covered.saturating_mul(16)
    }

    fn combine_opacity(opacity: u8, coverage: u8) -> u8 {
        ((u16::from(opacity) * u16::from(coverage) + 127) / 255) as u8
    }

    pub use crate::{Canvas, GraphicsSurface};

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Surface(pub u64);

    fn rect_end(origin: i32, extent: usize) -> i32 {
        origin.saturating_add(i32::try_from(extent.saturating_sub(1)).unwrap_or(i32::MAX))
    }

    fn clip_line_to_bounds(
        mut first: Point,
        mut second: Point,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    ) -> Option<(Point, Point)> {
        const LEFT: u8 = 1;
        const RIGHT: u8 = 2;
        const TOP: u8 = 4;
        const BOTTOM: u8 = 8;

        let code = |point: Point| {
            let mut result = 0;
            if point.x < left {
                result |= LEFT;
            } else if point.x > right {
                result |= RIGHT;
            }
            if point.y < top {
                result |= TOP;
            } else if point.y > bottom {
                result |= BOTTOM;
            }
            result
        };

        loop {
            let first_code = code(first);
            let second_code = code(second);
            if first_code | second_code == 0 {
                return Some((first, second));
            }
            if first_code & second_code != 0 {
                return None;
            }
            let outside_code = if first_code != 0 {
                first_code
            } else {
                second_code
            };
            let candidate = if outside_code & TOP != 0 {
                let y = top;
                let numerator = (i128::from(second.x) - i128::from(first.x))
                    * (i128::from(y) - i128::from(first.y));
                let denominator = i128::from(second.y) - i128::from(first.y);
                Point::new(
                    (i128::from(first.x) + numerator / denominator)
                        .clamp(i128::from(i32::MIN), i128::from(i32::MAX))
                        as i32,
                    y,
                )
            } else if outside_code & BOTTOM != 0 {
                let y = bottom;
                let numerator = (i128::from(second.x) - i128::from(first.x))
                    * (i128::from(y) - i128::from(first.y));
                let denominator = i128::from(second.y) - i128::from(first.y);
                Point::new(
                    (i128::from(first.x) + numerator / denominator)
                        .clamp(i128::from(i32::MIN), i128::from(i32::MAX))
                        as i32,
                    y,
                )
            } else if outside_code & RIGHT != 0 {
                let x = right;
                let numerator = (i128::from(second.y) - i128::from(first.y))
                    * (i128::from(x) - i128::from(first.x));
                let denominator = i128::from(second.x) - i128::from(first.x);
                Point::new(
                    x,
                    (i128::from(first.y) + numerator / denominator)
                        .clamp(i128::from(i32::MIN), i128::from(i32::MAX))
                        as i32,
                )
            } else {
                let x = left;
                let numerator = (i128::from(second.y) - i128::from(first.y))
                    * (i128::from(x) - i128::from(first.x));
                let denominator = i128::from(second.x) - i128::from(first.x);
                Point::new(
                    x,
                    (i128::from(first.y) + numerator / denominator)
                        .clamp(i128::from(i32::MIN), i128::from(i32::MAX))
                        as i32,
                )
            };
            if first_code != 0 {
                first = candidate;
            } else {
                second = candidate;
            }
        }
    }

    const fn lerp(start: u8, end: u8, amount: u8) -> u8 {
        let start = start as u16;
        let end = end as u16;
        ((start * (255 - amount) as u16 + end * amount as u16 + 127) / 255) as u8
    }

    fn edge(ax: i32, ay: i32, bx: i32, by: i32, px: i32, py: i32) -> i128 {
        (i128::from(bx) - i128::from(ax)) * (i128::from(py) - i128::from(ay))
            - (i128::from(by) - i128::from(ay)) * (i128::from(px) - i128::from(ax))
    }

    fn interpolate_channel(
        first: u8,
        second: u8,
        third: u8,
        first_weight: i128,
        second_weight: i128,
        third_weight: i128,
        total: i128,
    ) -> u8 {
        ((i128::from(first) * first_weight
            + i128::from(second) * second_weight
            + i128::from(third) * third_weight
            + total / 2)
            / total)
            .min(i128::from(u8::MAX)) as u8
    }
}
pub mod notifications {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Record {
        pub kind: u64,
        pub source: u64,
        pub payload: u64,
    }
}

/// Raw kernel entry points.
///
/// Every function here is `unsafe` for the same two reasons: pointers must be
/// valid readable or writable userspace addresses for the stated length, and
/// the call must originate from WovenHat userspace. Individual docs note any
/// additional contract.
pub mod syscall {
    /// Returned in place of a result when a call fails.
    pub const ERROR: u64 = u64::MAX;
    /// Returned by non-blocking calls that have no result yet.
    pub const WOULD_BLOCK: u64 = u64::MAX - 1;

    pub const STDIN: u64 = 0;
    pub const STDOUT: u64 = 1;
    pub const STDERR: u64 = 2;

    pub const READ: u64 = 0;
    pub const WRITE: u64 = 1;
    pub const FILE_WRITE: u64 = 10;
    pub const OPEN: u64 = 2;
    pub const EXIT: u64 = 3;
    pub const GETPID: u64 = 4;
    pub const WAITPID: u64 = 5;
    pub const CLOSE: u64 = 6;
    pub const YIELD: u64 = 7;
    pub const GETUID: u64 = 13;
    pub const GETGID: u64 = 14;
    pub const EXEC: u64 = 15;
    pub const FORK: u64 = 16;
    pub const STAT: u64 = 17;
    pub const MKDIR: u64 = 19;
    pub const CHDIR: u64 = 20;
    pub const GETCWD: u64 = 21;
    pub const GETPPID: u64 = 25;
    pub const LSEEK: u64 = 27;
    pub const UNLINK: u64 = 28;
    pub const SLEEP: u64 = 29;
    pub const RENAME: u64 = 30;
    pub const GETTICKS: u64 = 31;
    pub const SYNC: u64 = 32;
    pub const EXEC_COMMAND: u64 = 50;
    pub const ENV_GET: u64 = 51;
    pub const ENV_COUNT: u64 = 53;
    pub const SPAWN_COMMAND: u64 = 57;
    pub const READDIR: u64 = 18;
    pub const NOTIFICATION_POLL: u64 = 95;
    pub const NOTIFICATION_POST: u64 = 96;
    pub const GRAPHICS_SURFACE_CREATE: u64 = 98;
    pub const GRAPHICS_SURFACE_FILL: u64 = 99;
    pub const GRAPHICS_SURFACE_PIXEL: u64 = 100;
    pub const GRAPHICS_SURFACE_DESTROY: u64 = 101;
    pub const GRAPHICS_COMPOSITOR_BLIT: u64 = 102;
    pub const GRAPHICS_COMPOSITOR_PIXEL: u64 = 103;
    pub const GRAPHICS_SURFACE_STROKE: u64 = 104;
    pub const GRAPHICS_COMPOSITOR_CLAIM: u64 = 105;
    pub const GRAPHICS_COMPOSITOR_PRESENT: u64 = 106;
    pub const INPUT_POLL: u64 = 107;
    pub const INPUT_WAIT: u64 = 108;
    pub const GRAPHICS_SURFACE_SHARE: u64 = 109;
    pub const GRAPHICS_COMPOSITOR_BLIT_SHARED: u64 = 110;
    pub const WAIT_ANY: u64 = 111;
    pub const WAIT_ANY_OBJECTS: u64 = 206;
    pub const TIMER_CREATE: u64 = 87;
    pub const TIMER_WAIT: u64 = 88;
    pub const EVENT_CREATE: u64 = 89;
    pub const EVENT_SET: u64 = 90;
    pub const EVENT_WAIT: u64 = 91;
    pub const TIMER_CLOSE: u64 = 92;
    pub const EVENT_CLOSE: u64 = 93;
    pub const GRAPHICS_SURFACE_DAMAGE: u64 = 112;
    pub const GRAPHICS_COMPOSITOR_BLIT_DAMAGE: u64 = 113;
    pub const GRAPHICS_COMPOSITOR_BLEND: u64 = 114;
    pub const GRAPHICS_COMPOSITOR_BLEND_SHARED: u64 = 115;
    pub const GRAPHICS_COMPOSITOR_INFO: u64 = 116;
    pub const GRAPHICS_COMPOSITOR_SCALE: u64 = 117;
    pub const GRAPHICS_SURFACE_WRITE: u64 = 118;
    pub const CLOCK_READ: u64 = 119;
    pub const GRAPHICS_COMPOSITOR_BLIT_REGION: u64 = 120;
    pub const GRAPHICS_COMPOSITOR_BLIT_REGION_SHARED: u64 = 121;
    pub const PERMISSION_REQUEST: u64 = 122;
    pub const PERMISSION_POLL: u64 = 123;
    pub const PERMISSION_DECIDE: u64 = 124;
    pub const PERMISSION_REVOKE: u64 = 125;
    pub const AUDIT_READ: u64 = 126;
    pub const SECRET_STORE: u64 = 127;
    pub const SECRET_READ: u64 = 128;
    pub const SECRET_REVOKE: u64 = 129;
    pub const THREAD_CREATE: u64 = 130;
    pub const THREAD_EXIT: u64 = 131;
    pub const THREAD_JOIN: u64 = 132;
    pub const THREAD_SELF: u64 = 133;
    pub const THREAD_SET_TLS: u64 = 134;
    pub const THREAD_GET_TLS: u64 = 135;
    pub const THREAD_CANCEL: u64 = 136;
    pub const GRAPHICS_COMPOSITOR_BLEND_REGION: u64 = 200;
    pub const GRAPHICS_COMPOSITOR_BLEND_REGION_SHARED: u64 = 201;
    pub const GRAPHICS_COMPOSITOR_BLIT_KEYED: u64 = 202;
    pub const AUDIO_INFO: u64 = 203;
    pub const AUDIO_PLAY: u64 = 204;
    pub const AUDIO_SET_VOLUME: u64 = 205;
    pub const GRAPHICS_COMPOSITOR_OUTPUTS: u64 = 207;
    pub const POWER_CONTROL: u64 = 209;
    pub const POWER_CAPABILITIES: u64 = 235;
    pub const CPU_IDLE_CAPABILITIES: u64 = 236;
    pub const CPU_FREQUENCY_INFO: u64 = 237;
    pub const ACCOUNTS_CHANGE_PIN: u64 = 210;
    pub const IDENTITY_ADMIN_ADD_USER: u64 = 226;
    pub const IDENTITY_ADMIN_DELETE_USER: u64 = 227;
    pub const IDENTITY_ADMIN_ADD_GROUP: u64 = 228;
    pub const IDENTITY_ADMIN_ADD_MEMBER: u64 = 229;
    pub const IDENTITY_ADMIN_REMOVE_MEMBER: u64 = 230;
    pub const IDENTITY_ADMIN_QUERY: u64 = 231;
    pub const IDENTITY_ADMIN_SET_LOCKOUT: u64 = 232;
    pub const IDENTITY_CONSUME_HARDWARE: u64 = 233;
    pub const IDENTITY_REQUEST_HARDWARE: u64 = 234;
    pub const IPC_ENDPOINT_CREATE: u64 = 211;
    pub const IPC_SERVICE_PUBLISH: u64 = 212;
    pub const IPC_SERVICE_UNPUBLISH: u64 = 213;
    pub const IPC_SERVICE_DISCOVER: u64 = 214;
    pub const IPC_MESSAGE_SEND: u64 = 215;
    pub const IPC_MESSAGE_RECEIVE: u64 = 216;
    pub const PACKAGE_TRANSACTION: u64 = 217;
    pub const AI_MESSAGE_SEND: u64 = 218;
    pub const IDENTITY_OPEN: u64 = 219;
    pub const IDENTITY_LOCK: u64 = 220;
    pub const IDENTITY_UNLOCK: u64 = 221;
    pub const IDENTITY_ELEVATE: u64 = 222;
    pub const IDENTITY_CLOSE: u64 = 223;
    pub const IDENTITY_OPEN_VERIFIED: u64 = 224;
    pub const IDENTITY_UNLOCK_VERIFIED: u64 = 225;
    pub const MESSAGE_SEND: u64 = 11;
    pub const MESSAGE_RECEIVE: u64 = 12;

    /// Invoke the WovenHat Ring-3 ABI.
    ///
    /// # Safety
    /// Arguments and pointers must satisfy the contract of the syscall named by
    /// `number`, and the caller must be executing in WovenHat userspace.
    #[inline]
    pub unsafe fn invoke(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
        let result: u64;
        // SAFETY: The caller is executing in WovenHat userspace with the ABI
        // pointer contract satisfied; int 0x80 is the DPL3 syscall gate.
        unsafe {
            core::arch::asm!(
                "int 0x80",
                inlateout("rax") number => result,
                in("rdi") arg0,
                in("rsi") arg1,
                in("rdx") arg2,
                options(preserves_flags),
            );
        }
        result
    }

    /// Terminate the current process. Never returns.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn exit(status: u64) -> ! {
        // SAFETY: Exit takes no pointers and does not return.
        unsafe {
            invoke(EXIT, status, 0, 0);
        }
        // The kernel does not return from Exit. Spin rather than fall off the
        // end of a `-> !` function if it ever did.
        loop {
            core::hint::spin_loop();
        }
    }

    /// Write `len` bytes from `buffer` to `descriptor`.
    ///
    /// # Safety
    /// `buffer` must be readable for `len` bytes.
    pub unsafe fn write(descriptor: u64, buffer: *const u8, len: usize) -> u64 {
        unsafe { invoke(WRITE, descriptor, buffer as u64, len as u64) }
    }

    /// Write bytes to an opened filesystem descriptor.
    ///
    /// # Safety
    /// `buffer` must be readable for `len` bytes and the descriptor must be an
    /// opened file whose owner has the kernel's `FileWrite` capability.
    pub unsafe fn file_write(descriptor: u64, buffer: *const u8, len: usize) -> u64 {
        unsafe { invoke(FILE_WRITE, descriptor, buffer as u64, len as u64) }
    }

    /// Read up to `len` bytes from `descriptor` into `buffer`.
    ///
    /// # Safety
    /// `buffer` must be writable for `len` bytes.
    pub unsafe fn read(descriptor: u64, buffer: *mut u8, len: usize) -> u64 {
        unsafe { invoke(READ, descriptor, buffer as u64, len as u64) }
    }

    /// Open the path described by `path`/`path_len`, returning a descriptor.
    ///
    /// # Safety
    /// `path` must be readable for `path_len` bytes.
    pub unsafe fn open(path: *const u8, path_len: usize, flags: u64) -> u64 {
        unsafe { invoke(OPEN, path as u64, path_len as u64, flags) }
    }

    /// Release a descriptor.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn close(descriptor: u64) -> u64 {
        unsafe { invoke(CLOSE, descriptor, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn getpid() -> u64 {
        unsafe { invoke(GETPID, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn getppid() -> u64 {
        unsafe { invoke(GETPPID, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn getuid() -> u64 {
        unsafe { invoke(GETUID, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn getgid() -> u64 {
        unsafe { invoke(GETGID, 0, 0, 0) }
    }

    /// Create a thread at an RX image address with a private stack of
    /// `stack_size` bytes. The entry function takes no arguments and must
    /// eventually call [`thread_exit`].
    ///
    /// # Safety
    /// `entry` must be a valid WovenHat userspace code address and the size
    /// must satisfy the kernel's bounded stack contract.
    pub unsafe fn thread_create(entry: u64, stack_size: usize) -> u64 {
        unsafe { invoke(THREAD_CREATE, entry, stack_size as u64, 0) }
    }

    /// Terminate the calling thread. The process leader uses `exit` semantics.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn thread_exit(status: i32) -> ! {
        unsafe { invoke(THREAD_EXIT, status as u64, 0, 0) };
        loop {
            core::hint::spin_loop();
        }
    }

    /// Join a thread owned by the current process.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn thread_join(id: super::thread::Id) -> super::thread::Join {
        let result = unsafe { invoke(THREAD_JOIN, id.0, 0, 0) };
        super::thread::decode_join(result)
    }

    /// Return the current thread's generation-tagged handle.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn thread_self() -> u64 {
        unsafe { invoke(THREAD_SELF, 0, 0, 0) }
    }

    /// Set the current thread's kernel-visible TLS base value.
    ///
    /// # Safety
    /// The value must point at the caller's valid initial-exec TLS base.
    pub unsafe fn thread_set_tls(base: u64) -> u64 {
        unsafe { invoke(THREAD_SET_TLS, base, 0, 0) }
    }

    /// Read the current thread's kernel-visible TLS base value.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn thread_get_tls() -> u64 {
        unsafe { invoke(THREAD_GET_TLS, 0, 0, 0) }
    }

    /// Cancel an owned sibling thread. The sibling remains joinable and the
    /// supplied status is returned by [`thread_join`].
    ///
    /// # Safety
    /// Must be called from WovenHat userspace with a valid owned thread ID.
    pub unsafe fn thread_cancel(id: super::thread::Id, status: i32) -> u64 {
        unsafe { invoke(THREAD_CANCEL, id.0, status as u64, 0) }
    }

    /// Yield the remainder of this time slice.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn yield_now() -> u64 {
        unsafe { invoke(YIELD, 0, 0, 0) }
    }

    /// Block for `ticks` timer interrupts.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn sleep(ticks: u64) -> u64 {
        unsafe { invoke(SLEEP, ticks, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn ticks() -> u64 {
        unsafe { invoke(GETTICKS, 0, 0, 0) }
    }

    /// Wait for a child, returning its zero-extended 32-bit status payload.
    /// Full-width ERROR/WOULD_BLOCK remain distinct; decode success as `i32`.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn waitpid(pid: u64) -> u64 {
        unsafe { invoke(WAITPID, pid, 0, 0) }
    }

    /// Spawn a child from a command line, returning its pid.
    ///
    /// # Safety
    /// `command` must be readable for `len` bytes.
    pub unsafe fn spawn(command: *const u8, len: usize) -> u64 {
        unsafe { invoke(SPAWN_COMMAND, command as u64, len as u64, 0) }
    }

    /// Replace this process image from a command line. Returns only on failure.
    ///
    /// # Safety
    /// `command` must be readable for `len` bytes.
    pub unsafe fn exec(command: *const u8, len: usize) -> u64 {
        unsafe { invoke(EXEC_COMMAND, command as u64, len as u64, 0) }
    }

    /// Flush pending filesystem state to its device.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn sync() -> u64 {
        unsafe { invoke(SYNC, 0, 0, 0) }
    }

    /// # Safety
    /// `record` must be a valid writable userspace pointer.
    pub unsafe fn notification_poll(record: *mut super::notifications::Record) -> u64 {
        unsafe { invoke(NOTIFICATION_POLL, record as u64, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn notification_post(kind: u64, payload: u64) -> u64 {
        unsafe { invoke(NOTIFICATION_POST, kind, payload, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_create(width: u32, height: u32) -> u64 {
        unsafe {
            invoke(
                GRAPHICS_SURFACE_CREATE,
                u64::from(width),
                u64::from(height),
                0,
            )
        }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_fill(
        handle: u64,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: u32,
    ) -> u64 {
        let packed = u64::from(x)
            | (u64::from(y) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48);
        unsafe { invoke(GRAPHICS_SURFACE_FILL, handle, packed, u64::from(color)) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_pixel(handle: u64, x: u32, y: u32) -> u64 {
        let packed = u64::from(x) | (u64::from(y) << 32);
        unsafe { invoke(GRAPHICS_SURFACE_PIXEL, handle, packed, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_destroy(handle: u64) -> u64 {
        unsafe { invoke(GRAPHICS_SURFACE_DESTROY, handle, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit(handle: u64, x: i32, y: i32) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLIT, handle, packed, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_pixel(x: i32, y: i32) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_PIXEL, packed, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_claim() -> u64 {
        unsafe { invoke(GRAPHICS_COMPOSITOR_CLAIM, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_present() -> u64 {
        unsafe { invoke(GRAPHICS_COMPOSITOR_PRESENT, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn input_poll() -> u64 {
        unsafe { invoke(INPUT_POLL, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn input_wait(deadline: u64) -> u64 {
        unsafe { invoke(INPUT_WAIT, deadline, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_share(handle: u64, grantee: u64) -> u64 {
        unsafe { invoke(GRAPHICS_SURFACE_SHARE, handle, grantee, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit_shared(handle: u64, x: i32, y: i32) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLIT_SHARED, handle, packed, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn wait_any(deadline: u64, mask: u64) -> u64 {
        unsafe { invoke(WAIT_ANY, deadline, mask, 0) }
    }

    /// # Safety
    /// `sources` must point to `count` packed 24-byte wait-object records in
    /// readable WovenHat userspace memory.
    pub unsafe fn wait_any_objects(sources: *const u8, count: u64, deadline: u64) -> u64 {
        unsafe { invoke(WAIT_ANY_OBJECTS, deadline, sources as u64, count) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn timer_create(relative: u64, period: u64) -> u64 {
        unsafe { invoke(TIMER_CREATE, relative, period, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn event_create(initial: u64) -> u64 {
        unsafe { invoke(EVENT_CREATE, initial, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace with an owned event handle.
    pub unsafe fn event_set(handle: u64, state: u64) -> u64 {
        unsafe { invoke(EVENT_SET, handle, state, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace with an owned object handle.
    pub unsafe fn timer_close(handle: u64) -> u64 {
        unsafe { invoke(TIMER_CLOSE, handle, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace with an owned object handle.
    pub unsafe fn event_close(handle: u64) -> u64 {
        unsafe { invoke(EVENT_CLOSE, handle, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_damage(
        handle: u64,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> u64 {
        let packed = u64::from(x)
            | (u64::from(y) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48);
        unsafe { invoke(GRAPHICS_SURFACE_DAMAGE, handle, packed, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blend(handle: u64, x: i32, y: i32, opacity: u64) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLEND, handle, packed, opacity) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blend_shared(
        handle: u64,
        x: i32,
        y: i32,
        opacity: u64,
    ) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLEND_SHARED, handle, packed, opacity) }
    }

    /// # Safety
    /// `pixels` must be readable for `width * height` `u32`s.
    pub unsafe fn graphics_surface_write(
        handle: u64,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        pixels: *const u32,
    ) -> u64 {
        let packed = u64::from(x)
            | (u64::from(y) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48);
        unsafe { invoke(GRAPHICS_SURFACE_WRITE, handle, packed, pixels as u64) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit_region(
        handle: u64,
        x: i32,
        y: i32,
        region: u64,
        shared: bool,
    ) -> u64 {
        let destination = (x as u32 as u64) | ((y as u32 as u64) << 32);
        let number = if shared {
            GRAPHICS_COMPOSITOR_BLIT_REGION_SHARED
        } else {
            GRAPHICS_COMPOSITOR_BLIT_REGION
        };
        unsafe { invoke(number, handle, destination, region) }
    }

    /// # Safety
    /// `path` must be readable for `path_len` bytes and `name` writable for
    /// 255 bytes, the longest name a directory entry can carry.
    pub unsafe fn readdir(path: *const u8, path_len: usize, index: usize, name: *mut u8) -> u64 {
        let packed = (path_len as u64 & 0xffff) | ((index as u64) << 16);
        unsafe { invoke(READDIR, path as u64, packed, name as u64) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blend_region(
        handle: u64,
        x: i32,
        y: i32,
        opacity: u8,
        region: u64,
        shared: bool,
    ) -> u64 {
        let number = if shared {
            GRAPHICS_COMPOSITOR_BLEND_REGION_SHARED
        } else {
            GRAPHICS_COMPOSITOR_BLEND_REGION
        };
        unsafe {
            invoke(
                number,
                handle,
                super::pack_blend_destination(x, y, opacity),
                region,
            )
        }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit_keyed(handle: u64, x: i32, y: i32, region: u64) -> u64 {
        let destination = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLIT_KEYED, handle, destination, region) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn clock_read() -> u64 {
        unsafe { invoke(CLOCK_READ, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_info() -> u64 {
        unsafe { invoke(GRAPHICS_COMPOSITOR_INFO, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_scale(scale: u64) -> u64 {
        unsafe { invoke(GRAPHICS_COMPOSITOR_SCALE, scale, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit_damage(handle: u64, x: i32, y: i32) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLIT_DAMAGE, handle, packed, 0) }
    }

    /// # Safety
    /// `buffer` must be readable for `length` bytes.
    pub unsafe fn message_send(receiver: u64, buffer: *const u8, length: usize) -> u64 {
        unsafe { invoke(MESSAGE_SEND, receiver, buffer as u64, length as u64) }
    }

    /// # Safety
    /// `buffer` must be writable for `capacity` bytes and `sender` for 8.
    pub unsafe fn message_receive(buffer: *mut u8, capacity: usize, sender: *mut u64) -> u64 {
        unsafe {
            invoke(
                MESSAGE_RECEIVE,
                buffer as u64,
                capacity as u64,
                sender as u64,
            )
        }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_stroke(
        handle: u64,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: u32,
    ) -> u64 {
        let packed = u64::from(x)
            | (u64::from(y) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48);
        unsafe { invoke(GRAPHICS_SURFACE_STROKE, handle, packed, u64::from(color)) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn ipc_endpoint_create() -> u64 {
        unsafe { invoke(IPC_ENDPOINT_CREATE, 0, 0, 0) }
    }

    /// # Safety
    /// `name` must be readable for `name_len` bytes.
    pub unsafe fn ipc_service_publish(
        name: *const u8,
        name_len: usize,
        client_rights: u8,
        endpoint: u64,
    ) -> u64 {
        let packed = (name_len as u64) | (u64::from(client_rights) << 32);
        unsafe { invoke(IPC_SERVICE_PUBLISH, name as u64, packed, endpoint) }
    }

    /// # Safety
    /// `name` must be readable for `name_len` bytes.
    pub unsafe fn ipc_service_unpublish(name: *const u8, name_len: usize) -> u64 {
        unsafe { invoke(IPC_SERVICE_UNPUBLISH, name as u64, name_len as u64, 0) }
    }

    /// # Safety
    /// `name` must be readable for `name_len` bytes.
    pub unsafe fn ipc_service_discover(name: *const u8, name_len: usize) -> u64 {
        unsafe { invoke(IPC_SERVICE_DISCOVER, name as u64, name_len as u64, 0) }
    }

    /// # Safety
    /// `buffer` must be readable for `length` bytes.
    pub unsafe fn ipc_message_send(endpoint: u64, buffer: *const u8, length: usize) -> u64 {
        unsafe { invoke(IPC_MESSAGE_SEND, endpoint, buffer as u64, length as u64) }
    }

    /// # Safety
    /// `buffer` must be writable for `capacity` bytes.
    pub unsafe fn ipc_message_receive(endpoint: u64, buffer: *mut u8, capacity: usize) -> u64 {
        unsafe {
            invoke(
                IPC_MESSAGE_RECEIVE,
                endpoint,
                buffer as u64,
                capacity as u64,
            )
        }
    }
}

/// Pack a colour into the kernel's `0x00RRGGBB` representation.
pub const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
}

/// Typed wrapper for a client-owned drawing surface.
///
/// The underlying token packs a slot, a generation and the owning process, so
/// a `GraphicsSurface` held across a destroy is refused by the kernel rather
/// than driving a reused slot. Dropping a `GraphicsSurface` releases it.
pub struct GraphicsSurface {
    handle: u64,
    width: u32,
    height: u32,
}

impl GraphicsSurface {
    /// Allocate a surface. Caps enforced by the kernel.
    pub fn create(width: u32, height: u32) -> Result<Self, Error> {
        // SAFETY: both arguments are plain integers.
        let result = unsafe { syscall::graphics_surface_create(width, height) };
        check(result).map(|handle| Self {
            handle,
            width,
            height,
        })
    }

    pub const fn handle(&self) -> u64 {
        self.handle
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Fill `(x, y)` through `(x + w - 1, y + h - 1)` with `color`.
    pub fn fill_rect(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: u32,
    ) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result =
            unsafe { syscall::graphics_surface_fill(self.handle, x, y, width, height, color) };
        check(result).map(|_| ())
    }

    /// Read one pixel back for verification.
    pub fn pixel(&self, x: u32, y: u32) -> Result<u32, Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_pixel(self.handle, x, y) };
        check(result).map(|value| value as u32)
    }

    /// Stroke a one-pixel outline of a rectangle inside this surface.
    pub fn stroke_rect(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: u32,
    ) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result =
            unsafe { syscall::graphics_surface_stroke(self.handle, x, y, width, height, color) };
        check(result).map(|_| ())
    }

    /// Allow process `grantee` to read this surface, so a compositor process
    /// can draw it with [`compositor_blit_shared`]. Replaces any earlier
    /// grant; the grant ends when the surface is destroyed.
    pub fn share_with(&self, grantee: process::Id) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_share(self.handle, grantee.0) };
        check(result).map(|_| ())
    }

    /// Tell the compositor that `(x, y, width, height)` of this surface
    /// changed. If the surface is shared, the grantee is woken from
    /// [`wait_any`] with [`wait::DAMAGE`].
    pub fn damage(&self, x: u16, y: u16, width: u16, height: u16) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_damage(self.handle, x, y, width, height) };
        check(result).map(|_| ())
    }

    /// Withdraw any grant made with [`share_with`](Self::share_with).
    pub fn unshare(&self) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_share(self.handle, 0) };
        check(result).map(|_| ())
    }

    /// Upload `pixels` (`0x00RRGGBB`, row by row) into the rectangle at
    /// `(x, y)`. The rectangle must lie inside the surface and `pixels` must
    /// hold exactly `width * height` values. This is how anything fill and
    /// stroke cannot express reaches a surface: render into a [`Canvas`],
    /// then upload it.
    pub fn write(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        pixels: &[u32],
    ) -> Result<(), Error> {
        if pixels.len() != usize::from(width) * usize::from(height) {
            return Err(Error::Failed);
        }
        // SAFETY: `pixels` is readable for exactly `width * height` values.
        let result = unsafe {
            syscall::graphics_surface_write(self.handle, x, y, width, height, pixels.as_ptr())
        };
        check(result).map(|_| ())
    }

    /// Blit only `region` of this surface (in surface coordinates) so its
    /// top-left lands at `(x, y)` on the compositor. Lease holder only.
    pub fn present_region(&self, region: Region, x: i32, y: i32) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe {
            syscall::graphics_compositor_blit_region(self.handle, x, y, region.pack(), false)
        };
        check(result).map(|_| ())
    }

    /// Blend only `region` of this surface onto the compositor with uniform
    /// `opacity`, its top-left at `(x, y)`. Lease holder only. `x` and `y`
    /// travel as 24-bit signed values, far beyond any screen.
    pub fn present_region_blended(
        &self,
        region: Region,
        x: i32,
        y: i32,
        opacity: u8,
    ) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe {
            syscall::graphics_compositor_blend_region(
                self.handle,
                x,
                y,
                opacity,
                region.pack(),
                false,
            )
        };
        check(result).map(|_| ())
    }

    /// Blit only `region` of this surface to `(x, y)`, skipping pixels equal
    /// to [`TRANSPARENT`]: draw an overlay on a surface filled with it, and
    /// only the overlay lands. Lease holder only.
    pub fn present_region_keyed(&self, region: Region, x: i32, y: i32) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result =
            unsafe { syscall::graphics_compositor_blit_keyed(self.handle, x, y, region.pack()) };
        check(result).map(|_| ())
    }

    /// Upload a whole canvas to the top-left of this surface.
    pub fn write_canvas(&self, canvas: &Canvas<'_>) -> Result<(), Error> {
        let (Ok(width), Ok(height)) = (u16::try_from(canvas.width), u16::try_from(canvas.height))
        else {
            return Err(Error::Failed);
        };
        self.write(0, 0, width, height, canvas.pixels())
    }

    /// Blend this surface onto the compositor at `(x, y)` with uniform
    /// `opacity` (0 invisible, 255 opaque). Lease holder only; this is how a
    /// window manager draws shadows and translucent chrome.
    pub fn present_blended(&self, x: i32, y: i32, opacity: u8) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result =
            unsafe { syscall::graphics_compositor_blend(self.handle, x, y, u64::from(opacity)) };
        check(result).map(|_| ())
    }

    /// Blit this surface onto the compositor at `(x, y)`.
    ///
    /// Policy is up to the compositor: today it draws exactly where asked,
    /// which is the primitive a future window manager will drive.
    pub fn present(&self, x: i32, y: i32) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_compositor_blit(self.handle, x, y) };
        check(result).map(|_| ())
    }
}

/// Blit another process's surface onto the compositor at `(x, y)`.
///
/// `handle` is the owner's token, received out of band (for example over
/// IPC). It works only for the lease holder, and only if the owner shared the
/// surface with this process.
pub fn compositor_blit_shared(handle: u64, x: i32, y: i32) -> Result<(), Error> {
    // SAFETY: integer-only call.
    let result = unsafe { syscall::graphics_compositor_blit_shared(handle, x, y) };
    check(result).map(|_| ())
}

/// Sources for [`wait_any`].
pub mod wait {
    /// Input for the focused process (`InputIo`).
    pub const INPUT: u64 = 1 << 0;
    /// A message in this process's mailbox (`Ipc`).
    pub const MESSAGE: u64 = 1 << 1;
    /// Damage on a surface shared with this process (`DisplayIo`).
    pub const DAMAGE: u64 = 1 << 2;
}

/// A timer or manual-reset event that can participate in [`wait_any_objects`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WaitObject {
    pub handle: u64,
    pub timer: bool,
    pub mask: u64,
}

/// Create a one-shot or periodic userspace timer. Requires `TimerRead`.
pub fn timer_create(relative: time::Ticks, period: time::Ticks) -> Result<u64, Error> {
    check(unsafe { syscall::timer_create(relative, period) })
}

/// Create a manual-reset event. Requires `Ipc`.
pub fn event_create(initial: bool) -> Result<u64, Error> {
    check(unsafe { syscall::event_create(u64::from(initial)) })
}

/// Set or clear a manual-reset event.
pub fn event_set(handle: u64, state: bool) -> Result<(), Error> {
    check(unsafe { syscall::event_set(handle, u64::from(state)) }).map(|_| ())
}

pub fn timer_close(handle: u64) -> Result<(), Error> {
    check(unsafe { syscall::timer_close(handle) }).map(|_| ())
}

pub fn event_close(handle: u64) -> Result<(), Error> {
    check(unsafe { syscall::event_close(handle) }).map(|_| ())
}

/// Wait on up to eight timer/event objects. The source records are copied by
/// the kernel before registration; no userspace pointer is retained while the
/// task sleeps.
pub fn wait_any_objects(
    sources: &[WaitObject],
    deadline: Option<time::Ticks>,
) -> Result<u64, Error> {
    if sources.is_empty() || sources.len() > 8 {
        return Err(Error::Failed);
    }
    let mut packed = [0u8; 8 * 24];
    for (index, source) in sources.iter().enumerate() {
        let offset = index * 24;
        packed[offset..offset + 8].copy_from_slice(&source.handle.to_le_bytes());
        let kind = u64::from(!source.timer);
        packed[offset + 8..offset + 16].copy_from_slice(&kind.to_le_bytes());
        packed[offset + 16..offset + 24].copy_from_slice(&source.mask.to_le_bytes());
    }
    // SAFETY: `packed` is live and readable for the duration of the syscall.
    check(unsafe {
        syscall::wait_any_objects(
            packed.as_ptr(),
            sources.len() as u64,
            deadline.unwrap_or(u64::MAX),
        )
    })
}

/// Sleep until any source in `mask` is ready, or until the timer reaches
/// `deadline` (`None` waits indefinitely). Returns the ready sources, or 0 on
/// timeout. Nothing is consumed: follow up with [`input_poll`] or
/// [`message_receive`].
pub fn wait_any(mask: u64, deadline: Option<time::Ticks>) -> Result<u64, Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::wait_any(deadline.unwrap_or(u64::MAX), mask) })
}

/// A rectangle reported by [`compositor_blit_damage`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Region {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Region {
    /// `x | y << 16 | width << 32 | height << 48`, the kernel's rectangle
    /// packing.
    pub const fn pack(self) -> u64 {
        self.x as u64
            | (self.y as u64) << 16
            | (self.width as u64) << 32
            | (self.height as u64) << 48
    }
}

/// The region blend's destination word: `x` and `y` as 24-bit two's
/// complement fields, the opacity in the top byte, the byte between zero.
pub const fn pack_blend_destination(x: i32, y: i32, opacity: u8) -> u64 {
    (x as u32 as u64 & 0xff_ffff) | ((y as u32 as u64 & 0xff_ffff) << 24) | ((opacity as u64) << 56)
}

/// Blend only `region` of another process's shared surface with uniform
/// `opacity`. Same rules as [`compositor_blit_region_shared`].
pub fn compositor_blend_region_shared(
    handle: u64,
    region: Region,
    x: i32,
    y: i32,
    opacity: u8,
) -> Result<(), Error> {
    // SAFETY: integer-only call.
    let result = unsafe {
        syscall::graphics_compositor_blend_region(handle, x, y, opacity, region.pack(), true)
    };
    check(result).map(|_| ())
}

/// The process that owns the surface `token` names. A token packs its
/// owner's id in its high 32 bits, and the kernel checks that name against
/// the real owner on every operation, so a token naming process P can only
/// ever reach a surface P owns. A compositor uses this to accept a surface
/// only from its owner.
pub const fn surface_owner(token: u64) -> process::Id {
    process::Id(token >> 32)
}

/// Blit only `region` of another process's shared surface so its top-left
/// lands at `(x, y)`. Same rules as [`compositor_blit_shared`]; this is how a
/// window manager repaints an exposed rectangle without overdrawing windows
/// stacked above it.
pub fn compositor_blit_region_shared(
    handle: u64,
    region: Region,
    x: i32,
    y: i32,
) -> Result<(), Error> {
    // SAFETY: integer-only call.
    let result =
        unsafe { syscall::graphics_compositor_blit_region(handle, x, y, region.pack(), true) };
    check(result).map(|_| ())
}

/// Composite only what a client changed: blit the pending damage of a shared
/// surface into a window placed at `(x, y)`. Returns the region copied, in
/// surface coordinates, or `None` when nothing was pending. Lease holder only.
pub fn compositor_blit_damage(handle: u64, x: i32, y: i32) -> Result<Option<Region>, Error> {
    // SAFETY: integer-only call.
    let value = check(unsafe { syscall::graphics_compositor_blit_damage(handle, x, y) })?;
    if value == 0 {
        return Ok(None);
    }
    Ok(Some(Region {
        x: value as u16,
        y: (value >> 16) as u16,
        width: (value >> 32) as u16,
        height: (value >> 48) as u16,
    }))
}

/// Blend another process's shared surface onto the compositor with uniform
/// `opacity`. Same rules as [`compositor_blit_shared`].
pub fn compositor_blend_shared(handle: u64, x: i32, y: i32, opacity: u8) -> Result<(), Error> {
    // SAFETY: integer-only call.
    let result =
        unsafe { syscall::graphics_compositor_blend_shared(handle, x, y, u64::from(opacity)) };
    check(result).map(|_| ())
}

/// Send `bytes` to process `receiver`. Requires the `Ipc` capability.
pub fn message_send(receiver: process::Id, bytes: &[u8]) -> Result<(), Error> {
    // SAFETY: the pointer and length describe `bytes`.
    let result = unsafe { syscall::message_send(receiver.0, bytes.as_ptr(), bytes.len()) };
    check(result).map(|_| ())
}

/// Take the next handle-addressed message, returning its length and sender.
/// The first eight bytes of the caller buffer are reserved for the sender. Fails when no
/// message is queued, or when it does not fit in `buffer`.
pub fn message_receive(buffer: &mut [u8]) -> Result<(usize, process::Id), Error> {
    let mut sender = 0u64;
    // SAFETY: the pointers describe `buffer` and `sender`, both writable.
    let result =
        unsafe { syscall::message_receive(buffer.as_mut_ptr(), buffer.len(), &mut sender) };
    check(result).map(|length| (length as usize, process::Id(sender)))
}

pub fn ipc_endpoint_create() -> Result<u64, Error> {
    // SAFETY: argument-free syscall.
    check(unsafe { syscall::ipc_endpoint_create() })
}

pub fn ipc_service_publish(name: &[u8], client_rights: u8, endpoint: u64) -> Result<(), Error> {
    // SAFETY: the pointer and length describe `name`.
    check(unsafe {
        syscall::ipc_service_publish(name.as_ptr(), name.len(), client_rights, endpoint)
    })
    .map(|_| ())
}

pub fn ipc_service_unpublish(name: &[u8]) -> Result<(), Error> {
    // SAFETY: the pointer and length describe `name`.
    check(unsafe { syscall::ipc_service_unpublish(name.as_ptr(), name.len()) }).map(|_| ())
}

pub fn ipc_service_discover(name: &[u8]) -> Result<u64, Error> {
    // SAFETY: the pointer and length describe `name`.
    check(unsafe { syscall::ipc_service_discover(name.as_ptr(), name.len()) })
}

pub fn ipc_message_send(endpoint: u64, bytes: &[u8]) -> Result<(), Error> {
    // SAFETY: the pointer and length describe `bytes`.
    check(unsafe { syscall::ipc_message_send(endpoint, bytes.as_ptr(), bytes.len()) }).map(|_| ())
}

pub fn ipc_message_receive(
    endpoint: u64,
    buffer: &mut [u8],
) -> Result<(usize, process::Id), Error> {
    // SAFETY: the pointer and length describe writable `buffer` storage.
    let length = check(unsafe {
        syscall::ipc_message_receive(endpoint, buffer.as_mut_ptr(), buffer.len())
    })? as usize;
    if length < 8 {
        return Err(Error::Failed);
    }
    let sender = u64::from_le_bytes(buffer[..8].try_into().unwrap());
    buffer.copy_within(8..length, 0);
    Ok((length - 8, process::Id(sender)))
}

/// Run a package transaction through the authenticated WovenPackage authority.
/// The path identifies a broker-visible source/name; package bytes remain out
/// of the fixed-size IPC message.
pub fn package_transaction(operation: package_ipc::Operation, path: &str) -> Result<u64, Error> {
    // SAFETY: the path pointer and length describe immutable caller-owned
    // storage for the duration of this synchronous syscall.
    check(unsafe {
        syscall::invoke(
            syscall::PACKAGE_TRANSACTION,
            operation as u64,
            path.as_ptr() as u64,
            path.len() as u64,
        )
    })
}

/// Send a bounded WovenAI request. The kernel requires both `Ipc` and `Ai`
/// capability before the request can reach the broker.
pub fn ai_message_send(endpoint: u64, bytes: &[u8]) -> Result<(), Error> {
    // SAFETY: the pointer and length describe `bytes`.
    check(unsafe {
        syscall::invoke(
            syscall::AI_MESSAGE_SEND,
            endpoint,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
        )
    })
    .map(|_| ())
}

/// Take the compositor lease.
///
/// Requires the `DisplayIo` capability, and only one process holds the lease
/// at a time. Blitting with [`GraphicsSurface::present`], reading the screen
/// with [`screen_pixel`] and presenting with [`compositor_present`] all
/// require it. The kernel releases the lease when the holder exits.
pub fn compositor_claim() -> Result<(), Error> {
    // SAFETY: argument-free call.
    let result = unsafe { syscall::graphics_compositor_claim() };
    check(result).map(|_| ())
}

/// Copy compositor damage to the framebuffer, taking the display from the text
/// terminal on first use. Returns the number of pixels written.
pub fn compositor_present() -> Result<u64, Error> {
    // SAFETY: argument-free call.
    let result = unsafe { syscall::graphics_compositor_present() };
    check(result)
}

/// The colour [`GraphicsSurface::present_region_keyed`] treats as
/// transparent: magenta, matching the kernel's key.
pub const TRANSPARENT: u32 = rgb(255, 0, 255);

/// Blend `over` onto `under` with `opacity` (0 keeps `under`, 255 gives
/// `over`), per channel, rounding to nearest: the same formula the kernel
/// uses for translucent blits.
pub const fn blend(over: u32, under: u32, opacity: u8) -> u32 {
    let a = opacity as u32;
    let mut result = 0;
    let mut shift = 0;
    while shift <= 16 {
        let top = (over >> shift) & 0xff;
        let bottom = (under >> shift) & 0xff;
        result |= ((top * a + bottom * (255 - a) + 127) / 255) << shift;
        shift += 8;
    }
    result
}

/// A software render target: a caller-owned pixel buffer covering a
/// `width`×`height` rectangle whose top-left is `(x, y)` in the caller's
/// coordinates. Every drawing call takes those coordinates and clips to the
/// rectangle, so a scene can be drawn once per tile without offset
/// arithmetic in the scene code.
pub struct Canvas<'a> {
    pixels: &'a mut [u32],
    x: i32,
    y: i32,
    width: usize,
    height: usize,
}

fn coordinate_extent_fits(origin: i32, extent: usize) -> bool {
    extent == 0
        || (extent <= i32::MAX as usize
            && i128::from(origin) + i128::try_from(extent.saturating_sub(1)).unwrap_or(i128::MAX)
                <= i128::from(i32::MAX))
}

impl<'a> Canvas<'a> {
    /// `None` if `pixels` is shorter than `width * height`.
    pub fn new(pixels: &'a mut [u32], x: i32, y: i32, width: usize, height: usize) -> Option<Self> {
        if !coordinate_extent_fits(x, width) || !coordinate_extent_fits(y, height) {
            return None;
        }
        let length = width.checked_mul(height)?;
        let pixels = pixels.get_mut(..length)?;
        Some(Self {
            pixels,
            x,
            y,
            width,
            height,
        })
    }

    pub const fn x(&self) -> i32 {
        self.x
    }

    pub const fn y(&self) -> i32 {
        self.y
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    /// The rendered pixels, row by row: what [`GraphicsSurface::write`]
    /// uploads.
    pub fn pixels(&self) -> &[u32] {
        self.pixels
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let column = usize::try_from(x.checked_sub(self.x)?).ok()?;
        let row = usize::try_from(y.checked_sub(self.y)?).ok()?;
        (column < self.width && row < self.height).then_some(row * self.width + column)
    }

    pub fn get(&self, x: i32, y: i32) -> Option<u32> {
        self.index(x, y).map(|index| self.pixels[index])
    }

    pub fn set(&mut self, x: i32, y: i32, color: u32) {
        if let Some(index) = self.index(x, y) {
            self.pixels[index] = color & 0x00ff_ffff;
        }
    }

    pub fn blend_pixel(&mut self, x: i32, y: i32, color: u32, opacity: u8) {
        if let Some(index) = self.index(x, y) {
            self.pixels[index] = blend(color, self.pixels[index], opacity) & 0x00ff_ffff;
        }
    }

    /// Set every pixel from `shader(x, y)`, in caller coordinates.
    pub fn shade(&mut self, mut shader: impl FnMut(i32, i32) -> u32) {
        for row in 0..self.height {
            let y = self.y + row as i32;
            for column in 0..self.width {
                self.pixels[row * self.width + column] =
                    shader(self.x + column as i32, y) & 0x00ff_ffff;
            }
        }
    }

    /// The part of `(x, y, width, height)` inside the canvas, as buffer
    /// rows and columns.
    fn clip(
        &self,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
    ) -> Option<(usize, usize, usize, usize)> {
        let left = i128::from(x).max(i128::from(self.x));
        let top = i128::from(y).max(i128::from(self.y));
        let right = (i128::from(x) + i128::try_from(width).unwrap_or(i128::MAX))
            .min(i128::from(self.x) + i128::try_from(self.width).unwrap_or(i128::MAX));
        let bottom = (i128::from(y) + i128::try_from(height).unwrap_or(i128::MAX))
            .min(i128::from(self.y) + i128::try_from(self.height).unwrap_or(i128::MAX));
        if right <= left || bottom <= top {
            return None;
        }
        Some((
            (left - i128::from(self.x)) as usize,
            (top - i128::from(self.y)) as usize,
            (right - left) as usize,
            (bottom - top) as usize,
        ))
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: usize, height: usize, color: u32) {
        self.blend_rect(x, y, width, height, color, u8::MAX);
    }

    /// Blend `color` over a rectangle with uniform `opacity`.
    pub fn blend_rect(
        &mut self,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
        color: u32,
        opacity: u8,
    ) {
        let Some((column, row, columns, rows)) = self.clip(x, y, width, height) else {
            return;
        };
        for r in row..row + rows {
            for pixel in
                &mut self.pixels[r * self.width + column..r * self.width + column + columns]
            {
                *pixel = blend(color, *pixel, opacity) & 0x00ff_ffff;
            }
        }
    }

    /// Draw `text` with the kernel's font, top-left at `(x, y)`. Newlines
    /// start a new line at `x`.
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, color: u32) {
        let (mut cursor_x, mut cursor_y) = (x, y);
        for character in text.chars() {
            if character == '\n' {
                cursor_x = x;
                cursor_y = cursor_y.saturating_add(font::LINE_HEIGHT);
                continue;
            }
            for (row, bits) in font::glyph(character).iter().enumerate() {
                for column in 0..font::WIDTH as i32 {
                    if bits & (1 << (font::WIDTH as i32 - 1 - column)) != 0 {
                        self.set(
                            cursor_x.saturating_add(column),
                            cursor_y.saturating_add(row as i32),
                            color,
                        );
                    }
                }
            }
            cursor_x = cursor_x.saturating_add(font::ADVANCE);
        }
    }
}

/// A calendar date and time of day from the hardware clock, UTC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Read the wall clock. The hardware clock keeps UTC and carries no time
/// zone.
pub fn clock() -> Result<DateTime, Error> {
    // SAFETY: argument-free call.
    check(unsafe { syscall::clock_read() }).map(|value| DateTime {
        second: value as u8,
        minute: (value >> 8) as u8,
        hour: (value >> 16) as u8,
        day: (value >> 24) as u8,
        month: (value >> 32) as u8,
        year: (value >> 40) as u16,
    })
}

/// The compositor surface's geometry and the display scale.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CompositorInfo {
    /// Surface width in logical pixels: the framebuffer width over `scale`.
    pub width: u32,
    /// Surface height in logical pixels.
    pub height: u32,
    /// Each logical pixel is shown as a `scale`×`scale` block.
    pub scale: u32,
}

impl CompositorInfo {
    const fn unpack(value: u64) -> Self {
        Self {
            width: (value & 0xff_ffff) as u32,
            height: ((value >> 24) & 0xff_ffff) as u32,
            scale: ((value >> 48) & 0xffff) as u32,
        }
    }
}

/// Largest display scale the kernel accepts.
pub const MAX_DISPLAY_SCALE: u32 = 4;

/// The compositor's current geometry. Lease holder only.
pub fn compositor_info() -> Result<CompositorInfo, Error> {
    // SAFETY: argument-free call.
    check(unsafe { syscall::graphics_compositor_info() }).map(CompositorInfo::unpack)
}

/// Set the integer display scale (1 to [`MAX_DISPLAY_SCALE`]). The surface
/// becomes the framebuffer divided by `scale` and starts black, so redraw
/// everything at the returned geometry. Lease holder only; a new holder
/// always starts at scale 1.
pub fn compositor_set_scale(scale: u32) -> Result<CompositorInfo, Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::graphics_compositor_scale(u64::from(scale)) })
        .map(CompositorInfo::unpack)
}

/// A spring that eases a value toward a target, stepped once per frame:
/// motion that starts quickly and settles smoothly, without the abrupt stop
/// of a linear ramp.
///
/// The state is fixed-point (16 fractional bits), so it needs no floating
/// point and every run is identical. Stepping is semi-implicit Euler with
/// stiffness 0.08 and damping 0.5 per frame, which keeps both roots of the
/// discrete system real and positive: the value approaches its target
/// without oscillating or overshooting, and settles within about 30 frames
/// of a 255-step move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spring {
    position: i64,
    velocity: i64,
    target: i64,
}

impl Spring {
    const ONE: i64 = 1 << 16;
    const STIFFNESS: i64 = 5243; // 0.08
    const DAMPING: i64 = 32768; // 0.5
    /// Steps taken at most per call, so a long stall cannot loop for long.
    const MAX_STEPS: u64 = 256;

    /// A spring at rest at `value`.
    pub const fn new(value: i32) -> Self {
        let position = value as i64 * Self::ONE;
        Self {
            position,
            velocity: 0,
            target: position,
        }
    }

    /// Start moving toward `target` from wherever the spring is now.
    pub fn set_target(&mut self, target: i32) {
        self.target = i64::from(target) * Self::ONE;
    }

    pub const fn target(&self) -> i32 {
        (self.target / Self::ONE) as i32
    }

    /// The current value, rounded to the nearest whole unit.
    pub const fn value(&self) -> i32 {
        ((self.position + Self::ONE / 2).div_euclid(Self::ONE)) as i32
    }

    /// Whether the spring has come to rest on its target.
    pub const fn is_settled(&self) -> bool {
        self.position == self.target && self.velocity == 0
    }

    /// Advance by `frames` frames. Pass a [`FrameEvent::Frame`]'s
    /// `skipped + 1`, so motion stays on time when frames are dropped. Once
    /// within half a unit and nearly still, the spring snaps to its target.
    pub fn step(&mut self, frames: u64) {
        for _ in 0..frames.min(Self::MAX_STEPS) {
            if self.is_settled() {
                return;
            }
            let force = Self::STIFFNESS * (self.target - self.position) / Self::ONE
                - Self::DAMPING * self.velocity / Self::ONE;
            self.velocity += force;
            self.position += self.velocity;
            if (self.target - self.position).abs() < Self::ONE / 2
                && self.velocity.abs() < Self::ONE / 8
            {
                self.position = self.target;
                self.velocity = 0;
            }
        }
    }
}

/// Window layout for a tiling window manager: pure geometry, so it can be
/// tested on the host.
pub mod layout {
    /// A rectangle in screen coordinates.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Frame {
        pub x: i32,
        pub y: i32,
        pub width: i32,
        pub height: i32,
    }

    impl Frame {
        pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
            Self {
                x,
                y,
                width,
                height,
            }
        }

        /// The client area of a window filling this frame under a title bar
        /// `title` pixels tall.
        pub const fn client(self, title: i32) -> Self {
            Self::new(self.x, self.y + title, self.width, self.height - title)
        }
    }

    /// The most windows [`tile`] lays out.
    pub const MAX_TILES: usize = 8;

    /// Master and stack: the first window takes the left half of `area`, the
    /// rest share the right half in equal rows, the last taking any
    /// remainder. One window takes the whole area. Frames cover `area`
    /// exactly, without overlapping. Returns the frames and how many there
    /// are (`count`, capped at [`MAX_TILES`]).
    pub fn tile(count: usize, area: Frame) -> ([Frame; MAX_TILES], usize) {
        let count = count.min(MAX_TILES);
        let mut frames = [Frame::new(0, 0, 0, 0); MAX_TILES];
        match count {
            0 => {}
            1 => frames[0] = area,
            _ => {
                let master = area.width / 2;
                frames[0] = Frame::new(area.x, area.y, master, area.height);
                let stack = (count - 1) as i32;
                let row = area.height / stack;
                for index in 0..stack {
                    let y = area.y + index * row;
                    let height = if index == stack - 1 {
                        area.y + area.height - y
                    } else {
                        row
                    };
                    frames[1 + index as usize] =
                        Frame::new(area.x + master, y, area.width - master, height);
                }
            }
        }
        (frames, count)
    }

    /// The left or right half of `area`, for snapping a window dragged to
    /// an edge.
    pub const fn half(area: Frame, right: bool) -> Frame {
        let left = area.width / 2;
        if right {
            Frame::new(area.x + left, area.y, area.width - left, area.height)
        } else {
            Frame::new(area.x, area.y, left, area.height)
        }
    }
}

/// What ended a [`FrameClock::wait`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameEvent {
    /// The next frame is due. `skipped` counts whole frames that went by
    /// unrendered because the caller ran late; an animation advances by
    /// `skipped + 1` steps to stay on time.
    Frame { skipped: u64 },
    /// A [`wait_any`] source became ready before the frame was due.
    Ready(u64),
}

/// Paces an animation at a fixed period on the kernel timer.
///
/// Frame boundaries are absolute (`start + n * period`), so lateness in one
/// frame does not push back every later one, and no frame is ever early.
/// There is no vertical-sync source yet, so frames are not aligned to the
/// display's refresh; at the 100 Hz timer the shortest period is 10 ms.
pub struct FrameClock {
    period: time::Ticks,
    next: time::Ticks,
}

impl FrameClock {
    /// Start a clock whose first frame is due one `period` from now. A
    /// period of 0 is treated as 1 tick.
    pub fn new(period: time::Ticks) -> Self {
        let period = period.max(1);
        Self {
            period,
            next: ticks().saturating_add(period),
        }
    }

    pub const fn period(&self) -> time::Ticks {
        self.period
    }

    /// Tick at which the next frame is due.
    pub const fn deadline(&self) -> time::Ticks {
        self.next
    }

    /// Sleep until the next frame is due, or until a source in `mask` is
    /// ready (`mask` 0 waits for the frame alone). A ready source does not
    /// consume the frame: call again to keep waiting for it.
    pub fn wait(&mut self, mask: u64) -> Result<FrameEvent, Error> {
        loop {
            let now = ticks();
            if now >= self.next {
                let skipped = (now - self.next) / self.period;
                self.next = self
                    .next
                    .saturating_add(self.period.saturating_mul(skipped + 1));
                return Ok(FrameEvent::Frame { skipped });
            }
            if mask == 0 {
                // Relative sleep from a tick read earlier can only overshoot,
                // never wake before the deadline.
                sleep(self.next - now)?;
            } else {
                let ready = wait_any(mask, Some(self.next))?;
                if ready != 0 {
                    return Ok(FrameEvent::Ready(ready));
                }
            }
        }
    }
}

/// A keyboard event delivered to the focused process.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    F1,
    F2,
    F3,
    F4,
}

/// An input event delivered to the focused process.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputEvent {
    Key(Key),
    /// Relative motion; Y grows downwards.
    PointerMove {
        dx: i16,
        dy: i16,
    },
    /// Button 0 is left, 1 right, 2 middle.
    PointerButton {
        button: u8,
        pressed: bool,
    },
    /// Wheel motion; positive is away from the user.
    Scroll {
        vertical: i16,
    },
    /// An absolute pointer position (tablets): each axis 0..=32767.
    PointerAbsolute {
        x: u16,
        y: u16,
    },
    /// A touch contact at 0..=32767; `active` false is its lift.
    Touch {
        contact: u8,
        x: u16,
        y: u16,
        active: bool,
    },
    /// Pen position 0..=32767 and pressure 0..=1023.
    Pen {
        x: u16,
        y: u16,
        pressure: u16,
        touching: bool,
    },
    /// A game-controller button (its number; 0 or 1), axis (0x100 + axis;
    /// -32768..=32767) or hat (0x200; position or -1).
    GameController {
        control: u16,
        value: i16,
    },
}

/// Take the next input event of any kind, if any. Same authority as
/// [`input_poll`].
pub fn input_poll_event() -> Result<Option<InputEvent>, Error> {
    // SAFETY: argument-free call.
    decode_event(unsafe { syscall::input_poll() })
}

/// Take the next key, if any, discarding pointer events ahead of it.
///
/// Requires the `InputIo` capability and input focus, which the kernel grants
/// to the compositor lease holder while it owns the display. Returns
/// `Ok(None)` when no key is pending; the call never blocks.
pub fn input_poll() -> Result<Option<Key>, Error> {
    loop {
        match input_poll_event()? {
            None => return Ok(None),
            Some(InputEvent::Key(key)) => return Ok(Some(key)),
            Some(_) => {}
        }
    }
}

/// Sleep until the next input event, or until the timer reaches `deadline`
/// (`None` waits indefinitely). Returns `Ok(None)` on timeout.
///
/// Same authority as [`input_poll`]. Only one task of a process may wait at a
/// time; a second concurrent waiter is refused.
pub fn input_wait(deadline: Option<time::Ticks>) -> Result<Option<Key>, Error> {
    loop {
        // SAFETY: integer-only call.
        match decode_event(unsafe { syscall::input_wait(deadline.unwrap_or(u64::MAX)) })? {
            None => return Ok(None),
            Some(InputEvent::Key(key)) => return Ok(Some(key)),
            Some(_) => {}
        }
    }
}

fn decode_event(result: u64) -> Result<Option<InputEvent>, Error> {
    if result == syscall::WOULD_BLOCK {
        return Ok(None);
    }
    let value = check(result)?;
    let payload = value & 0x00ff_ffff_ffff_ffff;
    Ok(Some(match value >> 56 {
        1 => InputEvent::Key(Key::Char(
            char::from_u32(payload as u32).ok_or(Error::Failed)?,
        )),
        2 => InputEvent::Key(Key::Enter),
        3 => InputEvent::Key(Key::Backspace),
        4 => InputEvent::Key(Key::Tab),
        5 => InputEvent::Key(Key::F1),
        8 => InputEvent::Key(Key::F2),
        14 => InputEvent::Key(Key::F3),
        15 => InputEvent::Key(Key::F4),
        6 => InputEvent::PointerMove {
            dx: payload as u16 as i16,
            dy: (payload >> 16) as u16 as i16,
        },
        7 => InputEvent::PointerButton {
            button: payload as u8,
            pressed: (payload >> 8) & 1 != 0,
        },
        9 => InputEvent::Scroll {
            vertical: payload as u16 as i16,
        },
        10 => InputEvent::PointerAbsolute {
            x: payload as u16,
            y: (payload >> 16) as u16,
        },
        11 => InputEvent::Touch {
            contact: payload as u8,
            active: (payload >> 8) & 1 != 0,
            x: (payload >> 16) as u16,
            y: (payload >> 32) as u16,
        },
        12 => InputEvent::Pen {
            x: payload as u16,
            y: (payload >> 16) as u16,
            pressure: ((payload >> 32) & 0x3ff) as u16,
            touching: (payload >> 48) & 1 != 0,
        },
        13 => InputEvent::GameController {
            control: payload as u16,
            value: (payload >> 16) as u16 as i16,
        },
        _ => return Err(Error::Failed),
    }))
}

/// Read one pixel from the compositor surface. Requires the lease, because the
/// screen carries every client's pixels.
pub fn screen_pixel(x: i32, y: i32) -> Result<u32, Error> {
    // SAFETY: integer-only call.
    let result = unsafe { syscall::graphics_compositor_pixel(x, y) };
    check(result).map(|value| value as u32)
}

impl Drop for GraphicsSurface {
    fn drop(&mut self) {
        // SAFETY: destroying a surface the current process owns is always safe;
        // the kernel ignores a stale handle rather than hitting another slot.
        unsafe {
            let _ = syscall::graphics_surface_destroy(self.handle);
        }
    }
}

/// Failure reported by the safe API.
///
/// The kernel returns a single sentinel rather than an errno space, so this
/// distinguishes only what the ABI itself distinguishes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The kernel rejected the call.
    Failed,
    /// No result is available yet.
    WouldBlock,
}

/// Runtime authorities that may be requested from the permission broker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Permission {
    Camera = 0,
    Microphone = 1,
    Network = 2,
    FilesystemRead = 3,
    FilesystemWrite = 4,
    Gpu = 5,
    Usb = 6,
    Clipboard = 7,
    Location = 8,
    Processes = 9,
    Ipc = 10,
    Storage = 11,
    Display = 12,
    Input = 13,
    Ai = 14,
    Secrets = 15,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionState {
    Pending,
    Resolving,
    Granted,
    Denied,
    Revoked,
}

/// Ask the broker for one authority. `reason` is an application-defined
/// stable digest; user-facing brokers can resolve it to localized text.
pub fn permission_request(permission: Permission, reason: u64) -> Result<u64, Error> {
    check(unsafe { syscall::invoke(syscall::PERMISSION_REQUEST, permission as u64, reason, 0) })
}

pub fn permission_poll(handle: u64) -> Result<PermissionState, Error> {
    match check(unsafe { syscall::invoke(syscall::PERMISSION_POLL, handle, 0, 0) })? {
        0 => Ok(PermissionState::Pending),
        1 => Ok(PermissionState::Resolving),
        2 => Ok(PermissionState::Granted),
        3 => Ok(PermissionState::Denied),
        4 => Ok(PermissionState::Revoked),
        _ => Err(Error::Failed),
    }
}

/// Resolve a request. The kernel accepts this only from an authorized policy
/// service holding `TaskControl`.
pub fn permission_decide(handle: u64, allow: bool) -> Result<(), Error> {
    check(unsafe { syscall::invoke(syscall::PERMISSION_DECIDE, handle, u64::from(allow), 0) })
        .map(|_| ())
}

pub fn permission_revoke(handle: u64) -> Result<(), Error> {
    check(unsafe { syscall::invoke(syscall::PERMISSION_REVOKE, handle, 0, 0) }).map(|_| ())
}

/// Store a bounded secret in the caller-owned volatile WovenVault and return a
/// generation-tagged handle. The input is copied before the syscall returns.
/// A monitor's area of the compositor surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// The compositor's monitors, left to right, and how many there are. Same
/// authority as the other compositor calls (DisplayIo and the lease).
/// Turn the machine off. Returns only if it is still running, with the
/// reason: 1 no ACPI power-off, 2 the firmware refused ACPI mode, 3 the
/// machine ignored the request. Needs PowerControl (the desktop session).
pub fn power_off() -> Result<u64, Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::invoke(syscall::POWER_CONTROL, 0, 0, 0) })
}

/// Restart the machine. Returns only on refusal. Needs PowerControl.
pub fn restart() -> Result<u64, Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::invoke(syscall::POWER_CONTROL, 1, 0, 0) })
}

/// Firmware-advertised power transitions. This query performs no transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PowerCapabilities {
    pub power_off: bool,
    pub suspend: bool,
    pub restart: bool,
}

pub fn power_capabilities() -> Result<PowerCapabilities, Error> {
    let mut bytes = [0u8; 3];
    check(unsafe {
        syscall::invoke(
            syscall::POWER_CAPABILITIES,
            bytes.as_mut_ptr() as u64,
            bytes.len() as u64,
            0,
        )
    })?;
    Ok(PowerCapabilities {
        power_off: bytes[0] != 0,
        suspend: bytes[1] != 0,
        restart: bytes[2] != 0,
    })
}

/// Architectural CPU idle primitives. This query performs no idle action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuIdleCapabilities {
    pub hlt: bool,
    pub monitor_mwait: bool,
    pub invariant_tsc: bool,
}

pub fn cpu_idle_capabilities() -> Result<CpuIdleCapabilities, Error> {
    let mut bytes = [0u8; 3];
    check(unsafe {
        syscall::invoke(
            syscall::CPU_IDLE_CAPABILITIES,
            bytes.as_mut_ptr() as u64,
            bytes.len() as u64,
            0,
        )
    })?;
    Ok(CpuIdleCapabilities {
        hlt: bytes[0] != 0,
        monitor_mwait: bytes[1] != 0,
        invariant_tsc: bytes[2] != 0,
    })
}

/// CPUID-reported frequency inventory. This query performs no frequency
/// transition and may return `None` when the firmware omits leaf 0x16.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuFrequencyInfo {
    pub base_mhz: u32,
    pub maximum_mhz: u32,
    pub bus_mhz: u32,
}

pub fn cpu_frequency_info() -> Result<Option<CpuFrequencyInfo>, Error> {
    let mut bytes = [0u8; 13];
    check(unsafe {
        syscall::invoke(
            syscall::CPU_FREQUENCY_INFO,
            bytes.as_mut_ptr() as u64,
            bytes.len() as u64,
            0,
        )
    })?;
    if bytes[0] == 0 {
        return Ok(None);
    }
    Ok(Some(CpuFrequencyInfo {
        base_mhz: u32::from_le_bytes(bytes[1..5].try_into().unwrap_or([0; 4])),
        maximum_mhz: u32::from_le_bytes(bytes[5..9].try_into().unwrap_or([0; 4])),
        bus_mhz: u32::from_le_bytes(bytes[9..13].try_into().unwrap_or([0; 4])),
    }))
}

pub fn compositor_outputs() -> Result<([Output; 4], usize), Error> {
    let mut bytes = [0u8; 64];
    let count = check(unsafe {
        syscall::invoke(
            syscall::GRAPHICS_COMPOSITOR_OUTPUTS,
            bytes.as_mut_ptr() as u64,
            0,
            0,
        )
    })?;
    let mut outputs = [Output::default(); 4];
    for (output, chunk) in outputs.iter_mut().zip(bytes.as_chunks::<16>().0) {
        let field = |index: usize| {
            u32::from_le_bytes([
                chunk[index],
                chunk[index + 1],
                chunk[index + 2],
                chunk[index + 3],
            ])
        };
        *output = Output {
            x: field(0),
            y: field(4),
            width: field(8),
            height: field(12),
        };
    }
    Ok((outputs, (count as usize).min(4)))
}

/// What WovenAudio plays: 48 kHz 16-bit stereo today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioInfo {
    pub sample_rate: u32,
    /// The most PCM bytes one `audio_play` accepts.
    pub max_bytes: u32,
    pub playback_streams: u8,
    pub capture_streams: u8,
    pub channels: u8,
    pub bits: u8,
}

/// The audio output, if the system has one.
pub fn audio_info() -> Result<AudioInfo, Error> {
    let mut bytes = [0u8; 12];
    check(unsafe { syscall::invoke(syscall::AUDIO_INFO, bytes.as_mut_ptr() as u64, 0, 0) })?;
    Ok(AudioInfo {
        sample_rate: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        max_bytes: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        playback_streams: bytes[8],
        capture_streams: bytes[9],
        channels: bytes[10],
        bits: bytes[11],
    })
}

/// Play interleaved 16-bit little-endian stereo PCM at the output's sample
/// rate; returns once it has finished. Needs the AudioPlayback capability.
pub fn audio_play(pcm: &[u8]) -> Result<usize, Error> {
    check(unsafe {
        syscall::invoke(
            syscall::AUDIO_PLAY,
            pcm.as_ptr() as u64,
            pcm.len() as u64,
            0,
        )
    })
    .map(|played| played as usize)
}

/// Set the output volume, 0-100 (0 mutes); returns the amplifier gain set.
pub fn audio_set_volume(percent: u8) -> Result<u8, Error> {
    check(unsafe { syscall::invoke(syscall::AUDIO_SET_VOLUME, u64::from(percent), 0, 0) })
        .map(|gain| gain as u8)
}

pub fn secret_store(secret: &[u8]) -> Result<u64, Error> {
    check(unsafe {
        syscall::invoke(
            syscall::SECRET_STORE,
            secret.as_ptr() as u64,
            secret.len() as u64,
            0,
        )
    })
}

/// Copy a secret into a caller-owned buffer. The handle never exposes a kernel
/// pointer and is invalidated when the owner exits or revokes it.
pub fn secret_read(handle: u64, output: &mut [u8]) -> Result<usize, Error> {
    let length = check(unsafe {
        syscall::invoke(
            syscall::SECRET_READ,
            handle,
            output.as_mut_ptr() as u64,
            output.len() as u64,
        )
    })? as usize;
    if length > output.len() {
        return Err(Error::Failed);
    }
    Ok(length)
}

pub fn secret_revoke(handle: u64) -> Result<(), Error> {
    check(unsafe { syscall::invoke(syscall::SECRET_REVOKE, handle, 0, 0) }).map(|_| ())
}

fn check(value: u64) -> Result<u64, Error> {
    match value {
        syscall::ERROR => Err(Error::Failed),
        syscall::WOULD_BLOCK => Err(Error::WouldBlock),
        other => Ok(other),
    }
}

/// Read one retained security-ledger record, newest first. The caller needs
/// the kernel's `TaskInspect` authority; no user pointer is retained.
pub fn audit_read(offset: u64) -> Result<Option<security::AuditEvent>, Error> {
    let mut bytes = [0_u8; 48];
    let result = check(unsafe {
        syscall::invoke(syscall::AUDIT_READ, offset, bytes.as_mut_ptr() as u64, 0)
    })?;
    if result == 0 {
        return Ok(None);
    }
    if result != 1 {
        return Err(Error::Failed);
    }
    let action = match bytes[18] {
        0 => security::AuditAction::CapabilityGrant,
        1 => security::AuditAction::CapabilityRevoke,
        2 => security::AuditAction::IpcSend,
        3 => security::AuditAction::FileWrite,
        4 => security::AuditAction::ProcessFault,
        5 => security::AuditAction::ProcessExec,
        6 => security::AuditAction::ProcessFork,
        7 => security::AuditAction::WovenGuardAllow,
        8 => security::AuditAction::WovenGuardDeny,
        9 => security::AuditAction::CapabilityLineageIssue,
        10 => security::AuditAction::CapabilityDerived,
        11 => security::AuditAction::CapabilityLineageRelease,
        12 => security::AuditAction::CapabilityLineageRevoke,
        13 => security::AuditAction::PermissionRequest,
        14 => security::AuditAction::PermissionDecision,
        15 => security::AuditAction::PermissionRevoke,
        16 => security::AuditAction::SandboxProfileBind,
        17 => security::AuditAction::SandboxProfileDeny,
        18 => security::AuditAction::SandboxServicePublish,
        19 => security::AuditAction::SandboxServiceDiscover,
        20 => security::AuditAction::SandboxServiceUse,
        21 => security::AuditAction::SandboxFileRead,
        22 => security::AuditAction::SandboxFileWrite,
        23 => security::AuditAction::SandboxDeviceAccess,
        24 => security::AuditAction::SecretStore,
        25 => security::AuditAction::SecretRevoke,
        26 => security::AuditAction::IdentityOpen,
        27 => security::AuditAction::IdentityLock,
        28 => security::AuditAction::IdentityUnlock,
        29 => security::AuditAction::IdentityElevate,
        30 => security::AuditAction::IdentityClose,
        31 => security::AuditAction::IdentityAdminAddUser,
        32 => security::AuditAction::IdentityAdminDeleteUser,
        33 => security::AuditAction::IdentityAdminAddGroup,
        34 => security::AuditAction::IdentityAdminAddMember,
        35 => security::AuditAction::IdentityAdminRemoveMember,
        36 => security::AuditAction::IdentityAdminQuery,
        37 => security::AuditAction::IdentityAdminSetLockout,
        38 => security::AuditAction::IdentityHardware,
        _ => return Err(Error::Failed),
    };
    Ok(Some(security::AuditEvent {
        sequence: u64::from_le_bytes(bytes[0..8].try_into().unwrap_or([0; 8])),
        tick: u64::from_le_bytes(bytes[8..16].try_into().unwrap_or([0; 8])),
        cpu: u16::from_le_bytes(bytes[16..18].try_into().unwrap_or([0; 2])),
        action,
        actor: u64::from_le_bytes(bytes[20..28].try_into().unwrap_or([0; 8])),
        target: u64::from_le_bytes(bytes[28..36].try_into().unwrap_or([0; 8])),
        detail: u64::from_le_bytes(bytes[36..44].try_into().unwrap_or([0; 8])),
        allowed: bytes[19] != 0,
    }))
}

/// Write a byte slice to a descriptor, returning the count accepted.
pub fn write(descriptor: u64, bytes: &[u8]) -> Result<usize, Error> {
    // SAFETY: a slice is readable for its own length.
    let result = unsafe { syscall::write(descriptor, bytes.as_ptr(), bytes.len()) };
    check(result).map(|count| count as usize)
}

/// Write bytes to an opened filesystem descriptor. The kernel enforces the
/// descriptor's owner and `FileWrite` capability before mutating storage.
pub fn write_file(descriptor: u64, bytes: &[u8]) -> Result<usize, Error> {
    // SAFETY: a slice is readable for its own length.
    let result = unsafe { syscall::file_write(descriptor, bytes.as_ptr(), bytes.len()) };
    check(result).map(|count| count as usize)
}

/// Write a string to standard output.
pub fn print(text: &str) -> Result<usize, Error> {
    write(syscall::STDOUT, text.as_bytes())
}

/// Write a string to standard error.
pub fn eprint(text: &str) -> Result<usize, Error> {
    write(syscall::STDERR, text.as_bytes())
}

/// Read into a buffer, returning the count filled.
pub fn read(descriptor: u64, buffer: &mut [u8]) -> Result<usize, Error> {
    // SAFETY: a slice is writable for its own length.
    let result = unsafe { syscall::read(descriptor, buffer.as_mut_ptr(), buffer.len()) };
    check(result).map(|count| count as usize)
}

/// This process's identifier.
pub fn current_pid() -> process::Id {
    // SAFETY: getpid takes no pointers.
    process::Id(unsafe { syscall::getpid() })
}

/// This process's parent identifier.
pub fn parent_pid() -> process::Id {
    // SAFETY: getppid takes no pointers.
    process::Id(unsafe { syscall::getppid() })
}

/// Open a file, returning its descriptor. Requires `FileRead`.
pub fn open(path: &str) -> Result<u64, Error> {
    // SAFETY: a str is readable for its own length.
    check(unsafe { syscall::open(path.as_ptr(), path.len(), 0) })
}

/// Close a descriptor.
pub fn close(descriptor: u64) -> Result<(), Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::close(descriptor) }).map(|_| ())
}

/// Local accounts for the login manager: a name, a random salt and
/// `PBKDF2-HMAC-SHA256(PIN, salt, PBKDF2_ITERATIONS)`, one per line of an
/// accounts file: `name:salt-hex:hash-hex`. No PIN is ever stored.
pub mod accounts {
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    pub const MAX_NAME: usize = 16;
    pub const MAX_SALT: usize = 32;
    /// Must match `apps/woven-shell`'s own `MAX_PIN` and
    /// `kernel::accounts::MAX_PIN`.
    pub const MAX_PIN: usize = 16;
    /// Work factor for [`hash_pin`]. NIST SP 800-132 recommends at least
    /// 10,000; this keeps a wide margin while staying fast enough (tens of
    /// milliseconds, software-only, no AES-NI-style acceleration needed) for
    /// sign-in to feel instant on commodity hardware. The account line format
    /// does not record an iteration count, so this is a global constant, not
    /// a per-account parameter: changing it stops every hash already on disk
    /// from verifying, since it would be re-derived with a different work
    /// factor than it was created with. Raising it later means re-hashing
    /// every account, which needs the plaintext PIN and so forces each one
    /// through a password reset, not a silent migration.
    pub const PBKDF2_ITERATIONS: u32 = 100_000;

    /// One account, as read from its line.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Account {
        name: [u8; MAX_NAME],
        name_length: usize,
        salt: [u8; MAX_SALT],
        salt_length: usize,
        hash: [u8; 32],
    }

    /// `PBKDF2-HMAC-SHA256(pin, salt, PBKDF2_ITERATIONS)`.
    pub fn hash_pin(salt: &[u8], pin: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        pbkdf2_hmac::<Sha256>(pin, salt, PBKDF2_ITERATIONS, &mut out);
        out
    }

    fn hex_digit(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    /// Decode hex into `out`, returning the byte count; `None` for odd
    /// length, a non-hex digit, or too much input.
    fn hex(text: &[u8], out: &mut [u8]) -> Option<usize> {
        if !text.len().is_multiple_of(2) || text.len() / 2 > out.len() {
            return None;
        }
        for (slot, pair) in out.iter_mut().zip(text.as_chunks::<2>().0) {
            *slot = hex_digit(pair[0])? << 4 | hex_digit(pair[1])?;
        }
        Some(text.len() / 2)
    }

    impl Account {
        /// Parse `name:salt-hex:hash-hex`. The name is 1-16 printable ASCII
        /// characters other than `:`; the salt 1-32 bytes; the hash exactly
        /// 32. Anything else is refused.
        pub fn parse(line: &[u8]) -> Option<Self> {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let mut fields = line.split(|byte| *byte == b':');
            let (name, salt, hash) = (fields.next()?, fields.next()?, fields.next()?);
            if fields.next().is_some()
                || name.is_empty()
                || name.len() > MAX_NAME
                || !name.iter().all(|byte| (b' '..=b'~').contains(byte))
            {
                return None;
            }
            let mut account = Self {
                name: [0; MAX_NAME],
                name_length: name.len(),
                salt: [0; MAX_SALT],
                salt_length: 0,
                hash: [0; 32],
            };
            account.name[..name.len()].copy_from_slice(name);
            account.salt_length = hex(salt, &mut account.salt).filter(|length| *length > 0)?;
            if hex(hash, &mut account.hash)? != 32 {
                return None;
            }
            Some(account)
        }

        pub fn name(&self) -> &[u8] {
            &self.name[..self.name_length]
        }

        /// Return stored credential material without exposing plaintext.
        pub fn credential_material(&self) -> (&[u8], &[u8; 32]) {
            (&self.salt[..self.salt_length], &self.hash)
        }

        /// Whether `pin` is this account's PIN. The comparison of the two
        /// hashes takes the same time wherever they differ.
        pub fn verify(&self, pin: &[u8]) -> bool {
            let candidate = self.password_digest(pin);
            let mut difference = 0u8;
            for (a, b) in candidate.iter().zip(&self.hash) {
                difference |= a ^ b;
            }
            difference == 0
        }

        /// Derive the bounded candidate digest for the trusted kernel login
        /// manager. The digest is not the stored credential and is only
        /// handed to the kernel for constant-time comparison.
        pub fn password_digest(&self, pin: &[u8]) -> [u8; 32] {
            hash_pin(&self.salt[..self.salt_length], pin)
        }
    }

    /// Change a local account's own PIN: `name` must already have an
    /// account and `old_pin` must verify against it. Needs `AccountManage`
    /// (the desktop session only); every other account's line is left
    /// unchanged.
    pub fn change_pin(name: &[u8], old_pin: &[u8], new_pin: &[u8]) -> Result<(), crate::Error> {
        if name.is_empty()
            || name.len() > MAX_NAME
            || old_pin.len() > MAX_PIN
            || new_pin.len() > MAX_PIN
        {
            return Err(crate::Error::Failed);
        }
        let mut packed = [0u8; 3 + MAX_NAME + 2 * MAX_PIN];
        let mut offset = 0;
        for field in [name, old_pin, new_pin] {
            packed[offset] = field.len() as u8;
            offset += 1;
            packed[offset..offset + field.len()].copy_from_slice(field);
            offset += field.len();
        }
        // SAFETY: `packed` is a local, bounded, stack-owned buffer; its
        // length is `offset`, computed from the same field lengths just
        // copied into it.
        crate::check(unsafe {
            crate::syscall::invoke(
                crate::syscall::ACCOUNTS_CHANGE_PIN,
                packed.as_ptr() as u64,
                offset as u64,
                0,
            )
        })
        .map(|_| ())
    }
}

/// The longest name a directory entry can carry.
pub const MAX_NAME: usize = 255;

/// Entry `index` of directory `path`: its name, copied into `name`, and
/// whether it is itself a directory. Fails past the last entry. Requires
/// `FileRead`.
pub fn read_dir(
    path: &str,
    index: usize,
    name: &mut [u8; MAX_NAME],
) -> Result<(usize, bool), Error> {
    // SAFETY: the path is readable for its length, and `name` is writable
    // for the longest name the kernel copies.
    let value =
        check(unsafe { syscall::readdir(path.as_ptr(), path.len(), index, name.as_mut_ptr()) })?;
    Ok(((value & 0xff) as usize, (value >> 8) & 0xff == 1))
}

/// Timer ticks since boot.
pub fn ticks() -> time::Ticks {
    // SAFETY: getticks takes no pointers.
    unsafe { syscall::ticks() }
}

/// The CPU's timestamp counter: free-running cycles, not calibrated to any
/// time unit, but fine-grained enough to benchmark something as cheap as a
/// syscall round trip, which `ticks` (100 Hz) is not. No kernel involvement:
/// `RDTSC` is a plain instruction, not a syscall, and this kernel never sets
/// `CR4.TSD` to restrict it to Ring 0.
pub fn cycles() -> u64 {
    // SAFETY: RDTSC has no side effects and is permitted in Ring 3 here.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Yield the remainder of this time slice.
pub fn yield_now() {
    // SAFETY: yield takes no pointers.
    unsafe {
        syscall::yield_now();
    }
}

/// Block for `ticks` timer interrupts.
pub fn sleep(ticks: time::Ticks) -> Result<(), Error> {
    // SAFETY: sleep takes no pointers.
    check(unsafe { syscall::sleep(ticks) }).map(|_| ())
}

/// Spawn a child process from a command line.
pub fn spawn(command: &str) -> Result<process::Id, Error> {
    // SAFETY: a str is readable for its own length.
    let result = unsafe { syscall::spawn(command.as_ptr(), command.len()) };
    check(result).map(process::Id)
}

/// Wait for a child process, returning its zero-extended 32-bit exit payload.
/// Cast a successful value to `i32` to recover a signed status. A running child
/// returns `Error::WouldBlock`; an invalid/non-child wait returns `Error::Failed`.
pub fn wait(pid: process::Id) -> Result<u64, Error> {
    // SAFETY: waitpid takes no pointers.
    check(unsafe { syscall::waitpid(pid.0) })
}

/// Terminate this process. Never returns.
pub fn exit(status: u64) -> ! {
    // SAFETY: exit takes no pointers and does not return.
    unsafe { syscall::exit(status) }
}

/// Format `value` as decimal into `buffer`, returning the digits written.
///
/// Applications in this environment have no allocator and no `core::fmt`
/// machinery worth its size, so this covers the common case of reporting a
/// number without pulling in formatting.
pub fn format_u64(mut value: u64, buffer: &mut [u8]) -> &[u8] {
    if buffer.is_empty() {
        return &buffer[..0];
    }
    if value == 0 {
        buffer[0] = b'0';
        return &buffer[..1];
    }
    let mut digits = 0;
    while value > 0 && digits < buffer.len() {
        buffer[digits] = b'0' + (value % 10) as u8;
        value /= 10;
        digits += 1;
    }
    buffer[..digits].reverse();
    &buffer[..digits]
}

// Host unit tests for the parts of the API that are pure computation. The
// crate is freestanding everywhere else; under `cargo test` it links std so
// these run on the build machine.
#[cfg(test)]
mod tests {
    #[test]
    fn wovengraphics_colors_pack_and_rectangles_clip() {
        use crate::graphics::{Color, Rect};

        assert_eq!(Color::TITLE.packed(), crate::rgb(40, 220, 230));
        let clipped = Rect::new(4, 5, 10, 8).intersect(Rect::new(8, 9, 10, 10));
        assert_eq!(clipped, Rect::new(8, 9, 6, 4));
        assert!(Rect::new(0, 0, 0, 4).is_empty());
    }

    #[test]
    fn wovengraphics_renderer_clips_gradient_stroke_and_image() {
        use crate::graphics::{Color, Rect, Renderer};

        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            renderer.vertical_gradient(Rect::new(-1, -1, 6, 6), Color::BLACK, Color::WHITE);
            renderer.stroke(Rect::new(0, 0, 4, 4), Color::TITLE);
            assert!(renderer.image(1, 1, 2, 2, &[Color::WHITE.packed(); 4]));
            assert!(renderer.image_blended(10, 10, 1, 1, &[Color::BLACK.packed()], 128));
            assert!(!renderer.image(0, 0, 2, 2, &[Color::BLACK.packed(); 3]));
        }
        assert_eq!(pixels[0], Color::TITLE.packed());
        assert_eq!(pixels[5], Color::WHITE.packed());
        assert_eq!(pixels[10], Color::WHITE.packed());
        assert_eq!(pixels[15], Color::TITLE.packed());
    }

    #[test]
    fn wovengraphics_stroke_clips_extreme_rectangle() {
        use crate::graphics::{Color, Rect, Renderer};

        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            Renderer::new(&mut canvas).stroke(Rect::new(0, 0, usize::MAX, 4), Color::TITLE);
        }
        for column in 0..4 {
            assert_eq!(pixels[column], Color::TITLE.packed());
            assert_eq!(pixels[12 + column], Color::TITLE.packed());
        }
        for row in 1..3 {
            assert_eq!(pixels[row * 4], Color::TITLE.packed());
            assert_eq!(pixels[row * 4 + 1], 0);
        }
    }

    #[test]
    fn wovengraphics_image_clips_source_rows_and_columns() {
        use crate::graphics::Renderer;

        let source: Vec<u32> = (1..=12).collect();
        let mut pixels = [0u32; 8];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 2).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            assert!(renderer.image(-2, 0, 6, 2, &source));
        }
        assert_eq!(pixels, [3, 4, 5, 6, 9, 10, 11, 12]);
    }

    #[test]
    fn wovengraphics_horizontal_gradient_reaches_both_edges() {
        use crate::graphics::{Color, Rect, Renderer};

        let mut pixels = [0u32; 9];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 3, 3).unwrap();
            Renderer::new(&mut canvas).horizontal_gradient(
                Rect::new(-1, 0, 5, 3),
                Color::BLACK,
                Color::WHITE,
            );
        }
        assert_eq!(pixels[0], Color::rgb(63, 63, 63).packed());
        assert_eq!(pixels[2], Color::rgb(191, 191, 191).packed());
        assert_eq!(pixels[6], Color::rgb(63, 63, 63).packed());
        assert_eq!(pixels[8], Color::rgb(191, 191, 191).packed());
    }

    #[test]
    fn wovengraphics_gradients_clip_extreme_rectangles() {
        use crate::graphics::{Color, Rect, Renderer};

        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            renderer.vertical_gradient(
                Rect::new(0, i32::MIN, 4, usize::MAX),
                Color::BLACK,
                Color::WHITE,
            );
            renderer.horizontal_gradient(
                Rect::new(i32::MIN, 0, usize::MAX, 4),
                Color::BLACK,
                Color::WHITE,
            );
        }
        assert!(pixels.iter().all(|pixel| *pixel <= Color::WHITE.packed()));
    }

    #[test]
    fn wovengraphics_software_3d_depth_rejects_far_triangles() {
        use crate::graphics::{Color, DepthBuffer, Renderer, Vertex3};

        let mut pixels = [0u32; 16];
        let mut depths = [u16::MAX; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            let mut depth = DepthBuffer::new(&mut depths, 0, 0, 4, 4).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            renderer.triangle_3d(
                &mut depth,
                Vertex3::new(0, 0, 10, Color::WHITE),
                Vertex3::new(3, 0, 10, Color::WHITE),
                Vertex3::new(0, 3, 10, Color::WHITE),
            );
            renderer.triangle_3d(
                &mut depth,
                Vertex3::new(0, 0, 20, Color::BLACK),
                Vertex3::new(3, 0, 20, Color::BLACK),
                Vertex3::new(0, 3, 20, Color::BLACK),
            );
        }
        assert_eq!(pixels[0], Color::WHITE.packed());
    }

    #[test]
    fn wovengraphics_software_3d_interpolates_vertex_colour() {
        use crate::graphics::{Color, DepthBuffer, Renderer, Vertex3};

        let mut pixels = [0u32; 25];
        let mut depths = [u16::MAX; 25];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 5, 5).unwrap();
            let mut depth = DepthBuffer::new(&mut depths, 0, 0, 5, 5).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            renderer.triangle_3d(
                &mut depth,
                Vertex3::new(0, 0, 1, Color::rgb(255, 0, 0)),
                Vertex3::new(4, 0, 1, Color::rgb(0, 255, 0)),
                Vertex3::new(0, 4, 1, Color::rgb(0, 0, 255)),
            );
        }
        let center = pixels[6];
        assert_ne!(center, 0);
        assert!(center != Color::rgb(255, 0, 0).packed());
        assert!(center != Color::rgb(0, 255, 0).packed());
        assert!(center != Color::rgb(0, 0, 255).packed());
    }

    #[test]
    fn wovengraphics_software_3d_clips_extreme_coordinates() {
        use crate::graphics::{Color, DepthBuffer, Renderer, Vertex3};

        let mut pixels = [0u32; 16];
        let mut depths = [u16::MAX; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, -2, -2, 4, 4).unwrap();
            let mut depth = DepthBuffer::new(&mut depths, -2, -2, 4, 4).unwrap();
            Renderer::new(&mut canvas).triangle_3d(
                &mut depth,
                Vertex3::new(i32::MIN, i32::MIN, 1, Color::WHITE),
                Vertex3::new(i32::MAX, i32::MIN, 1, Color::WHITE),
                Vertex3::new(i32::MIN, i32::MAX, 1, Color::WHITE),
            );
        }
        assert_eq!(pixels[0], Color::WHITE.packed());
        assert_eq!(pixels[15], 0);
    }

    #[test]
    fn wovengraphics_textured_3d_samples_source_surface() {
        use crate::graphics::{DepthBuffer, Renderer, Vertex3Tex};

        // A 2x2 source: top-left red, the rest black, so sampling u=v=0
        // (top-left) must read differently from sampling the far corner.
        let mut source_pixels = [0x0000_0000u32, 0x0000_0000, 0x0000_0000, 0x0000_0000];
        source_pixels[0] = 0x00ff_0000;
        let source = Canvas::new(&mut source_pixels, 0, 0, 2, 2).unwrap();

        let mut pixels = [0u32; 16];
        let mut depths = [u16::MAX; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            let mut depth = DepthBuffer::new(&mut depths, 0, 0, 4, 4).unwrap();
            Renderer::new(&mut canvas).textured_triangle_3d(
                &mut depth,
                Vertex3Tex::new(0, 0, 1, 0, 0),
                Vertex3Tex::new(3, 0, 1, 255, 0),
                Vertex3Tex::new(0, 3, 1, 0, 255),
                &source,
            );
        }
        // The triangle's own top-left corner samples the source's top-left
        // (u=v=0): the red pixel.
        assert_eq!(pixels[0], 0x00ff_0000);
        // Its second vertex (u=255, v=0) samples the source's top-right
        // instead: black, not red, so u/v genuinely vary across the
        // triangle rather than a flat fill.
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn wovengraphics_textured_3d_depth_rejects_far_triangles() {
        use crate::graphics::{DepthBuffer, Renderer, Vertex3Tex};

        let mut near_pixels = [0x00ff_ffffu32; 4];
        let near = Canvas::new(&mut near_pixels, 0, 0, 2, 2).unwrap();
        let mut far_pixels = [0x0000_0000u32; 4];
        let far = Canvas::new(&mut far_pixels, 0, 0, 2, 2).unwrap();

        let mut pixels = [0u32; 16];
        let mut depths = [u16::MAX; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            let mut depth = DepthBuffer::new(&mut depths, 0, 0, 4, 4).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            renderer.textured_triangle_3d(
                &mut depth,
                Vertex3Tex::new(0, 0, 10, 0, 0),
                Vertex3Tex::new(3, 0, 10, 255, 0),
                Vertex3Tex::new(0, 3, 10, 0, 255),
                &near,
            );
            // Drawn after, at every covered pixel, but farther: the depth
            // test must keep the nearer triangle's (white) pixels.
            renderer.textured_triangle_3d(
                &mut depth,
                Vertex3Tex::new(0, 0, 20, 0, 0),
                Vertex3Tex::new(3, 0, 20, 255, 0),
                Vertex3Tex::new(0, 3, 20, 0, 255),
                &far,
            );
        }
        assert_eq!(pixels[0], 0x00ff_ffff);
    }

    #[test]
    fn wovengraphics_textured_3d_ignores_empty_source() {
        use crate::graphics::{DepthBuffer, Renderer, Vertex3Tex};

        let mut source_pixels: [u32; 0] = [];
        let source = Canvas::new(&mut source_pixels, 0, 0, 0, 0).unwrap();
        let mut pixels = [0u32; 16];
        let mut depths = [u16::MAX; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            let mut depth = DepthBuffer::new(&mut depths, 0, 0, 4, 4).unwrap();
            // Must not panic (divide by a zero source dimension) and must
            // draw nothing.
            Renderer::new(&mut canvas).textured_triangle_3d(
                &mut depth,
                Vertex3Tex::new(0, 0, 1, 0, 0),
                Vertex3Tex::new(3, 0, 1, 255, 0),
                Vertex3Tex::new(0, 3, 1, 0, 255),
                &source,
            );
        }
        assert!(pixels.iter().all(|pixel| *pixel == 0));
    }

    #[test]
    fn wovengraphics_camera_projects_origin_to_screen_center() {
        use crate::graphics::Camera;

        let camera = Camera::new(4 << 16, 10 << 16, 100, 60);
        let (x, y, depth) = camera.project(0, 0, 0).unwrap();
        assert_eq!((x, y), (100, 60));
        assert!(depth > 0);
    }

    #[test]
    fn wovengraphics_camera_shrinks_and_deepens_with_distance() {
        use crate::graphics::Camera;

        let camera = Camera::new(4 << 16, 10 << 16, 100, 60);
        // The same off-axis point, once at the world's z=0 plane and once
        // much farther away: perspective must shrink it toward screen
        // center and increase its depth value.
        let (near_x, _, near_depth) = camera.project(20 << 16, 0, 0).unwrap();
        let (far_x, _, far_depth) = camera.project(20 << 16, 0, 40 << 16).unwrap();
        assert!(
            far_x - 100 < near_x - 100,
            "farther should project closer to center"
        );
        assert!(
            far_depth > near_depth,
            "farther should have a larger depth value"
        );
    }

    #[test]
    fn wovengraphics_camera_rejects_points_behind_it() {
        use crate::graphics::Camera;

        let camera = Camera::new(4 << 16, 10 << 16, 100, 60);
        // z this far negative puts the point behind the camera, whatever is
        // in front of the world origin (distance = 10).
        assert!(camera.project(0, 0, -20 << 16).is_none());
    }

    #[test]
    fn wovengraphics_transformed_rect_is_clipped_and_filled() {
        use crate::graphics::{Color, Rect, Renderer, Transform};

        let mut pixels = [0u32; 36];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 6, 6).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            renderer.transformed_rect(
                Rect::new(-2, 1, 5, 3),
                Transform::new(1 << 16, 1 << 15, 0, 1 << 16, 0, 0),
                Color::TITLE,
            );
        }
        assert!(pixels.iter().any(|pixel| *pixel == Color::TITLE.packed()));
        assert_eq!(pixels[35], 0);
    }

    #[test]
    fn wovengraphics_transformed_rect_clips_extreme_vertices() {
        use crate::graphics::{Color, Rect, Renderer, Transform};

        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            Renderer::new(&mut canvas).transformed_rect(
                Rect::new(-1, -1, 4, 4),
                Transform::new(i32::MAX, 0, 0, i32::MAX, 0, 0),
                Color::TITLE,
            );
        }
        assert_eq!(pixels[0], Color::TITLE.packed());
        assert_eq!(pixels[15], Color::TITLE.packed());
    }

    #[test]
    fn wovengraphics_transformed_rect_bounds_extreme_dimensions() {
        use crate::graphics::{Color, Rect, Renderer, Transform};

        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            Renderer::new(&mut canvas).transformed_rect(
                Rect::new(0, 0, usize::MAX, 4),
                Transform::IDENTITY,
                Color::TITLE,
            );
        }
        assert_eq!(pixels[0], Color::TITLE.packed());
        assert_eq!(pixels[15], Color::TITLE.packed());
    }

    #[test]
    fn wovengraphics_transform_bounds_extreme_fixed_point_inputs() {
        use crate::graphics::{Color, Rect, Renderer, Transform};

        let scale = Transform::scale(i32::MAX, i32::MIN);
        let translate = Transform::translate(i32::MAX, i32::MIN);
        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            renderer.transformed_rect(Rect::new(0, 0, 4, 4), scale, Color::TITLE);
            renderer.transformed_rect(Rect::new(0, 0, 4, 4), translate, Color::WHITE);
        }
        assert_eq!(pixels.len(), 16);
    }

    #[test]
    fn wovengraphics_font_metrics_use_shared_fallback_and_lines() {
        use crate::graphics::Font;

        let metrics = Font::DEFAULT.measure("AB\n☃");
        assert_eq!(metrics.width, crate::font::text_width("AB\n☃"));
        assert_eq!(metrics.lines, 2);
        assert_eq!(metrics.height, 2 * crate::font::LINE_HEIGHT as usize);
    }

    #[test]
    fn wovengraphics_truetype_face_validates_tables_and_maps_bmp_cmap4() {
        use crate::graphics::{TrueTypeError, TrueTypeFace};

        let mut bytes = [0u8; 320];
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put32 = |bytes: &mut [u8], offset: usize, value: u32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        };
        put32(&mut bytes, 0, 0x0001_0000);
        put16(&mut bytes, 4, 5);
        for (record, (tag, offset, length)) in [
            (*b"head", 100u32, 20u32),
            (*b"maxp", 120u32, 6u32),
            (*b"cmap", 126u32, 44u32),
            (*b"hhea", 170u32, 36u32),
            (*b"hmtx", 206u32, 8u32),
        ]
        .into_iter()
        .enumerate()
        {
            let base = 12 + record * 16;
            bytes[base..base + 4].copy_from_slice(&tag);
            put32(&mut bytes, base + 8, offset);
            put32(&mut bytes, base + 12, length);
        }
        put16(&mut bytes, 118, 1000);
        put16(&mut bytes, 124, 10);
        put16(&mut bytes, 204, 2);
        put16(&mut bytes, 206, 500);
        put16(&mut bytes, 210, 700);
        put16(&mut bytes, 126, 0);
        put16(&mut bytes, 128, 1);
        put16(&mut bytes, 130, 3);
        put16(&mut bytes, 132, 1);
        put32(&mut bytes, 134, 12);
        put16(&mut bytes, 138, 4);
        put16(&mut bytes, 140, 32);
        put16(&mut bytes, 142, 0);
        put16(&mut bytes, 144, 4);
        put16(&mut bytes, 146, 0);
        put16(&mut bytes, 148, 0);
        put16(&mut bytes, 152, 0x0041);
        put16(&mut bytes, 154, 0xffff);
        put16(&mut bytes, 156, 0);
        put16(&mut bytes, 158, 0x0041);
        put16(&mut bytes, 160, 0xffff);
        put16(&mut bytes, 162, 0xffc2);
        put16(&mut bytes, 164, 1);
        put16(&mut bytes, 166, 0);
        put16(&mut bytes, 168, 0);

        let face = TrueTypeFace::from_bytes(&bytes).unwrap();
        assert_eq!(face.units_per_em(), 1000);
        assert_eq!(face.glyph_count(), 10);
        assert_eq!(face.glyph_index('A'), Some(3));
        assert_eq!(face.advance_width('A'), Some(700));
        assert_eq!(face.kerning('A', 'A'), None);
        let mut axes = [crate::graphics::VariationAxis {
            tag: [0; 4],
            minimum: 0,
            default: 0,
            maximum: 0,
        }; 1];
        assert_eq!(face.variation_axes(&mut axes).unwrap(), 0);
        assert_eq!(face.glyph_index('B'), None);
        assert_eq!(face.glyph_index('\u{1f600}'), None);
        assert_eq!(face.table_span(), (126, 44));
        assert!(matches!(
            TrueTypeFace::from_bytes(b"not-a-font"),
            Err(TrueTypeError::TooSmall)
        ));
    }

    #[test]
    fn wovengraphics_truetype_face_decodes_bounded_outline_and_composite() {
        use crate::graphics::{OutlinePoint, TrueTypeFace};

        let mut bytes = [0u8; 430];
        let put16_raw = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            put16_raw(bytes, offset + 24, value);
        };
        let put_i16 = |bytes: &mut [u8], offset: usize, value: i16| {
            let offset = offset + 24;
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put32_raw = |bytes: &mut [u8], offset: usize, value: u32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        };
        let put32 = |bytes: &mut [u8], offset: usize, value: u32| {
            put32_raw(bytes, offset + 24, value);
        };
        put32_raw(&mut bytes, 0, 0x0001_0000);
        put16_raw(&mut bytes, 4, 8);
        for (record, (tag, offset, length)) in [
            (*b"head", 124u32, 52u32),
            (*b"maxp", 176u32, 6u32),
            (*b"cmap", 182u32, 44u32),
            (*b"hhea", 226u32, 36u32),
            (*b"hmtx", 262u32, 8u32),
            (*b"loca", 270u32, 10u32),
            (*b"glyf", 280u32, 56u32),
            (*b"gvar", 336u32, 46u32),
        ]
        .into_iter()
        .enumerate()
        {
            let base = 12 + record * 16;
            bytes[base..base + 4].copy_from_slice(&tag);
            put32_raw(&mut bytes, base + 8, offset);
            put32_raw(&mut bytes, base + 12, length);
        }
        put16(&mut bytes, 118, 1000);
        put_i16(&mut bytes, 150, 0);
        put16(&mut bytes, 156, 4);
        put16(&mut bytes, 236, 2);
        put16(&mut bytes, 238, 500);
        put16(&mut bytes, 242, 700);
        put16(&mut bytes, 160, 1);
        put16(&mut bytes, 162, 3);
        put16(&mut bytes, 164, 1);
        put32(&mut bytes, 166, 12);
        put16(&mut bytes, 170, 4);
        put16(&mut bytes, 172, 32);
        put16(&mut bytes, 176, 4);
        put16(&mut bytes, 184, 0x0041);
        put16(&mut bytes, 186, 0xffff);
        put16(&mut bytes, 190, 0x0041);
        put16(&mut bytes, 192, 0xffff);
        put_i16(&mut bytes, 194, -62);
        put_i16(&mut bytes, 196, 1);
        put16(&mut bytes, 198, 0);
        put16(&mut bytes, 200, 0);
        for (index, value) in [0, 0, 0, 15, 28].into_iter().enumerate() {
            put16(&mut bytes, 246 + index * 2, value);
        }
        put_i16(&mut bytes, 256, 1);
        put16(&mut bytes, 266, 2);
        put16(&mut bytes, 268, 0);
        bytes[294..297].copy_from_slice(&[1, 1, 1]);
        put_i16(&mut bytes, 273, 0);
        put_i16(&mut bytes, 275, 100);
        put_i16(&mut bytes, 277, -100);
        put_i16(&mut bytes, 279, 0);
        put_i16(&mut bytes, 281, 0);
        put_i16(&mut bytes, 283, 100);
        put_i16(&mut bytes, 286, -1);
        put16(&mut bytes, 296, 0x0023);
        put16(&mut bytes, 298, 2);
        put_i16(&mut bytes, 300, 0);
        put_i16(&mut bytes, 302, 0);
        put16(&mut bytes, 304, 0x0001);
        put16(&mut bytes, 306, 2);
        put_i16(&mut bytes, 308, 1);
        put_i16(&mut bytes, 310, 0);

        put32_raw(&mut bytes, 336, 0x0001_0000);
        put16_raw(&mut bytes, 340, 1);
        put16_raw(&mut bytes, 342, 0);
        put32_raw(&mut bytes, 344, 20);
        put16_raw(&mut bytes, 348, 4);
        put16_raw(&mut bytes, 350, 0);
        put32_raw(&mut bytes, 352, 30);
        for index in 0..=3 {
            put16_raw(&mut bytes, 356 + index * 2, 0);
        }
        put16_raw(&mut bytes, 364, 8);
        put16_raw(&mut bytes, 366, 1);
        put16_raw(&mut bytes, 368, 8);
        put16_raw(&mut bytes, 370, 0x8000);
        put_i16(&mut bytes, 372, 0x4000);
        bytes[374] = 0x05;
        bytes[375..381].copy_from_slice(&[10, 20, 0, 0, 0, 0]);
        bytes[381] = 0x45;

        let face = TrueTypeFace::from_bytes(&bytes).unwrap();
        let mut points = [OutlinePoint {
            x: 0,
            y: 0,
            on_curve: false,
            contour_end: false,
        }; 6];
        assert_eq!(face.outline_points('A', &mut points).unwrap(), 6);
        let base_points = points;
        let mut glyph_points = [OutlinePoint {
            x: 0,
            y: 0,
            on_curve: false,
            contour_end: false,
        }; 6];
        assert_eq!(face.outline_points_glyph(3, &mut glyph_points).unwrap(), 6);
        assert_eq!(glyph_points, points);
        assert_eq!(
            points[0],
            OutlinePoint {
                x: 0,
                y: 0,
                on_curve: true,
                contour_end: false
            }
        );
        assert_eq!(
            points[1],
            OutlinePoint {
                x: 100,
                y: 0,
                on_curve: true,
                contour_end: false
            }
        );
        assert_eq!(
            points[2],
            OutlinePoint {
                x: 0,
                y: 100,
                on_curve: true,
                contour_end: true
            }
        );
        let mut varied_points = [OutlinePoint {
            x: 0,
            y: 0,
            on_curve: false,
            contour_end: false,
        }; 6];
        assert_eq!(
            face.glyph_variation_data(3).unwrap().map(<[u8]>::len),
            Some(16)
        );
        assert_eq!(
            face.outline_points_with_variation('A', &[0x4000], &mut varied_points)
                .unwrap(),
            6
        );
        for (index, (base, varied)) in base_points.iter().zip(varied_points.iter()).enumerate() {
            let expected_delta = if index < 3 { 10 } else { 20 };
            assert_eq!(varied.x, base.x + expected_delta);
            assert_eq!(varied.y, base.y);
        }
        let mut path = crate::graphics::Path::new();
        assert_eq!(
            face.outline_contour('A', 0, crate::graphics::Point::new(10, 20), 100, &mut path)
                .unwrap(),
            3
        );
        assert_eq!(path.len(), 3);
        let mut compound = crate::graphics::CompoundPath::new();
        assert_eq!(
            face.outline_compound('A', crate::graphics::Point::new(10, 20), 100, &mut compound)
                .unwrap(),
            2
        );
        assert_eq!(compound.len(), 2);
        let mut pixels = [0u32; 20 * 20];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 20, 20).unwrap();
            let mut renderer = crate::graphics::Renderer::new(&mut canvas);
            assert_eq!(
                face.render_glyph(
                    'A',
                    crate::graphics::Point::new(2, 15),
                    100,
                    crate::graphics::Color::WHITE,
                    &mut renderer,
                )
                .unwrap(),
                2
            );
            assert_eq!(
                face.render_glyph_id(
                    3,
                    crate::graphics::Point::new(2, 15),
                    100,
                    crate::graphics::Color::WHITE,
                    &mut renderer,
                )
                .unwrap(),
                2
            );
            assert_eq!(
                face.render_glyph_id_with_variation(
                    3,
                    crate::graphics::Point::new(2, 15),
                    100,
                    &[0x4000],
                    crate::graphics::Color::WHITE,
                    &mut renderer,
                )
                .unwrap(),
                2
            );
            assert_eq!(
                face.render_shaped_text(
                    "AA",
                    crate::graphics::Point::new(2, 15),
                    100,
                    crate::graphics::Color::WHITE,
                    &mut renderer,
                )
                .unwrap(),
                2
            );
        }
        assert!(pixels.iter().any(|pixel| *pixel != 0));
        let mut placements = [crate::graphics::GlyphPlacement {
            character: '\0',
            origin: crate::graphics::Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            face.layout_text(
                "AA",
                crate::graphics::Point::new(2, 15),
                10,
                &mut placements,
            )
            .unwrap(),
            2
        );
        assert_eq!(placements[0].origin.x, 2);
        assert_eq!(placements[1].origin.x, 9);
        let mut rtl = [crate::graphics::GlyphPlacement {
            character: '\0',
            origin: crate::graphics::Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            face.layout_text_direction(
                "AA",
                crate::graphics::Point::new(2, 15),
                10,
                crate::graphics::TextDirection::Rtl,
                &mut rtl,
            )
            .unwrap(),
            2
        );
        assert_eq!(rtl[0].origin.x, 9);
        assert_eq!(rtl[1].origin.x, 2);
        assert_eq!(
            crate::graphics::TextDirection::classify('\u{05d0}'),
            Some(crate::graphics::TextDirection::Rtl)
        );
        assert_eq!(
            crate::graphics::TextDirection::classify('A'),
            Some(crate::graphics::TextDirection::Ltr)
        );
        assert_eq!(crate::graphics::TextDirection::classify('-'), None);
        let mut bidi = [crate::graphics::GlyphPlacement {
            character: '\0',
            origin: crate::graphics::Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            face.layout_text_bidi(
                "AA",
                crate::graphics::Point::new(2, 15),
                10,
                crate::graphics::TextDirection::Rtl,
                &mut bidi,
            )
            .unwrap(),
            2
        );
        assert_eq!(bidi[0].origin.x, 2);
        assert_eq!(bidi[1].origin.x, 9);
        let mut wrapped = [crate::graphics::GlyphPlacement {
            character: '\0',
            origin: crate::graphics::Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            face.layout_text_box(
                "AA",
                crate::graphics::Point::new(2, 15),
                10,
                12,
                10,
                &mut wrapped,
            )
            .unwrap(),
            2
        );
        assert_eq!(wrapped[0].origin, crate::graphics::Point::new(2, 15));
        assert_eq!(wrapped[1].origin, crate::graphics::Point::new(2, 27));
        let mut whole_word = [crate::graphics::GlyphPlacement {
            character: '\0',
            origin: crate::graphics::Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            face.layout_text_word_box(
                "AA",
                crate::graphics::Point::new(2, 15),
                10,
                12,
                10,
                &mut whole_word,
            )
            .unwrap(),
            2
        );
        assert_eq!(whole_word[1].origin.y, 15);
        let mut fallback_pixels = [0u32; 32 * 16];
        {
            let mut canvas = Canvas::new(&mut fallback_pixels, 0, 0, 32, 16).unwrap();
            let mut renderer = crate::graphics::Renderer::new(&mut canvas);
            assert_eq!(
                face.render_text_with_bitmap_fallback(
                    "AB",
                    crate::graphics::Point::new(2, 12),
                    10,
                    1,
                    crate::graphics::Color::WHITE,
                    &mut renderer,
                )
                .unwrap(),
                2
            );
        }
        assert!(fallback_pixels.iter().any(|pixel| *pixel != 0));
    }

    #[test]
    fn wovengraphics_truetype_face_maps_non_bmp_cmap12_groups() {
        use crate::graphics::TrueTypeFace;

        let mut bytes = [0u8; 340];
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put32 = |bytes: &mut [u8], offset: usize, value: u32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        };
        put32(&mut bytes, 0, 0x0001_0000);
        put16(&mut bytes, 4, 5);
        for (record, (tag, offset, length)) in [
            (*b"head", 100u32, 20u32),
            (*b"maxp", 120u32, 6u32),
            (*b"cmap", 126u32, 40u32),
            (*b"hhea", 166u32, 36u32),
            (*b"hmtx", 202u32, 8u32),
        ]
        .into_iter()
        .enumerate()
        {
            let base = 12 + record * 16;
            bytes[base..base + 4].copy_from_slice(&tag);
            put32(&mut bytes, base + 8, offset);
            put32(&mut bytes, base + 12, length);
        }
        put16(&mut bytes, 118, 1000);
        put16(&mut bytes, 124, 10);
        put16(&mut bytes, 200, 2);
        put16(&mut bytes, 202, 500);
        put16(&mut bytes, 206, 700);
        put16(&mut bytes, 126, 0);
        put16(&mut bytes, 128, 1);
        put16(&mut bytes, 130, 3);
        put16(&mut bytes, 132, 10);
        put32(&mut bytes, 134, 12);
        put16(&mut bytes, 138, 12);
        put16(&mut bytes, 140, 0);
        put32(&mut bytes, 142, 28);
        put32(&mut bytes, 146, 0);
        put32(&mut bytes, 150, 1);
        put32(&mut bytes, 154, 0x1f600);
        put32(&mut bytes, 158, 0x1f600);
        put32(&mut bytes, 162, 3);

        let face = TrueTypeFace::from_bytes(&bytes).unwrap();
        assert_eq!(face.glyph_index('\u{1f600}'), Some(3));
        assert_eq!(face.glyph_index('A'), None);
        assert_eq!(face.advance_width('\u{1f600}'), Some(700));
    }

    #[test]
    fn wovengraphics_gsub_alternate_subtable_selects_first_bounded_alternate() {
        let mut bytes = [0u8; 26];
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        put16(&mut bytes, 0, 1);
        put16(&mut bytes, 2, 8);
        put16(&mut bytes, 4, 1);
        put16(&mut bytes, 6, 20);
        put16(&mut bytes, 8, 1);
        put16(&mut bytes, 10, 1);
        put16(&mut bytes, 12, 7);
        put16(&mut bytes, 20, 2);
        put16(&mut bytes, 22, 9);
        put16(&mut bytes, 24, 10);
        assert_eq!(
            crate::graphics::gsub_alternate_subtable(&bytes, 0, 7, bytes.len()),
            Ok(Some(9))
        );
        assert_eq!(
            crate::graphics::gsub_alternate_subtable(&bytes, 0, 8, bytes.len()),
            Ok(None)
        );
        put16(&mut bytes, 20, 0);
        assert_eq!(
            crate::graphics::gsub_alternate_subtable(&bytes, 0, 7, bytes.len()),
            Err(crate::graphics::TrueTypeError::TooLarge)
        );
    }

    #[test]
    fn wovengraphics_gsub_reverse_chaining_substitution_is_bounded() {
        let mut bytes = [0u8; 38];
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        put16(&mut bytes, 0, 1);
        put16(&mut bytes, 2, 32);
        put16(&mut bytes, 4, 1);
        put16(&mut bytes, 6, 20);
        put16(&mut bytes, 8, 1);
        put16(&mut bytes, 10, 26);
        put16(&mut bytes, 12, 1);
        put16(&mut bytes, 14, 9);
        for (offset, glyph) in [(20, 4), (26, 6), (32, 5)] {
            put16(&mut bytes, offset, 1);
            put16(&mut bytes, offset + 2, 1);
            put16(&mut bytes, offset + 4, glyph);
        }
        assert_eq!(
            crate::graphics::gsub_reverse_chaining_substitution(
                &bytes,
                0,
                &[4, 5, 6],
                1,
                bytes.len(),
            ),
            Ok(Some(9))
        );
        assert_eq!(
            crate::graphics::gsub_reverse_chaining_substitution(
                &bytes,
                0,
                &[4, 5, 8],
                1,
                bytes.len(),
            ),
            Ok(None)
        );
        put16(&mut bytes, 4, 17);
        assert_eq!(
            crate::graphics::gsub_reverse_chaining_substitution(
                &bytes,
                0,
                &[4, 5, 6],
                1,
                bytes.len(),
            ),
            Err(crate::graphics::TrueTypeError::TooLarge)
        );
    }

    #[test]
    fn wovengraphics_truetype_face_applies_bounded_gsub_ligature() {
        use crate::graphics::{Point, ShapedGlyph, TrueTypeFace};

        let mut bytes = [0u8; 400];
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put_i16 = |bytes: &mut [u8], offset: usize, value: i16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put32 = |bytes: &mut [u8], offset: usize, value: u32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        };
        put32(&mut bytes, 0, 0x0001_0000);
        put16(&mut bytes, 4, 6);
        for (record, (tag, offset, length)) in [
            (*b"head", 108u32, 52u32),
            (*b"maxp", 160u32, 6u32),
            (*b"cmap", 166u32, 44u32),
            (*b"hhea", 210u32, 36u32),
            (*b"hmtx", 246u32, 16u32),
            (*b"GSUB", 262u32, 110u32),
        ]
        .into_iter()
        .enumerate()
        {
            let base = 12 + record * 16;
            bytes[base..base + 4].copy_from_slice(&tag);
            put32(&mut bytes, base + 8, offset);
            put32(&mut bytes, base + 12, length);
        }
        put16(&mut bytes, 126, 1000);
        put16(&mut bytes, 158, 0);
        put16(&mut bytes, 164, 4);
        put16(&mut bytes, 244, 4);
        for (index, advance) in [500, 500, 500, 1000].into_iter().enumerate() {
            put16(&mut bytes, 246 + index * 4, advance);
        }
        put16(&mut bytes, 166, 0);
        put16(&mut bytes, 168, 1);
        put16(&mut bytes, 170, 3);
        put32(&mut bytes, 174, 12);
        put16(&mut bytes, 178, 4);
        put16(&mut bytes, 180, 32);
        put16(&mut bytes, 182, 0);
        put16(&mut bytes, 184, 4);
        put16(&mut bytes, 186, 4);
        put16(&mut bytes, 188, 1);
        put16(&mut bytes, 190, 0);
        put16(&mut bytes, 192, 67);
        put16(&mut bytes, 194, 0xffff);
        put16(&mut bytes, 196, 0);
        put16(&mut bytes, 198, 65);
        put16(&mut bytes, 200, 0xffff);
        put_i16(&mut bytes, 202, -64);
        put_i16(&mut bytes, 204, 1);
        put16(&mut bytes, 206, 0);
        put16(&mut bytes, 208, 0);

        put32(&mut bytes, 262, 0x0001_0000);
        put16(&mut bytes, 266, 72);
        put16(&mut bytes, 268, 10);
        put16(&mut bytes, 270, 30);
        put16(&mut bytes, 272, 1);
        bytes[274..278].copy_from_slice(b"liga");
        put16(&mut bytes, 278, 8);
        put16(&mut bytes, 280, 0);
        put16(&mut bytes, 282, 1);
        put16(&mut bytes, 284, 0);
        put16(&mut bytes, 292, 1);
        put16(&mut bytes, 294, 4);
        put16(&mut bytes, 296, 4);
        put16(&mut bytes, 298, 0);
        put16(&mut bytes, 300, 1);
        put16(&mut bytes, 302, 8);
        put16(&mut bytes, 304, 1);
        put16(&mut bytes, 306, 24);
        put16(&mut bytes, 308, 1);
        put16(&mut bytes, 310, 10);
        put16(&mut bytes, 314, 1);
        put16(&mut bytes, 316, 8);
        put16(&mut bytes, 322, 3);
        put16(&mut bytes, 324, 2);
        put16(&mut bytes, 326, 2);
        put16(&mut bytes, 328, 1);
        put16(&mut bytes, 330, 1);
        put16(&mut bytes, 332, 1);
        put16(&mut bytes, 334, 1);
        bytes[336..340].copy_from_slice(b"DFLT");
        put16(&mut bytes, 340, 10);
        put16(&mut bytes, 344, 10);
        put16(&mut bytes, 346, 1);
        bytes[348..352].copy_from_slice(b"TRK ");
        put16(&mut bytes, 352, 18);
        put16(&mut bytes, 354, 0);
        put16(&mut bytes, 356, u16::MAX);
        put16(&mut bytes, 358, 1);
        put16(&mut bytes, 360, 0);
        put16(&mut bytes, 362, 0);
        put16(&mut bytes, 364, u16::MAX);
        put16(&mut bytes, 366, 1);
        put16(&mut bytes, 368, 0);
        assert_eq!(u16::from_be_bytes([bytes[328], bytes[329]]), 1);
        assert_eq!(u16::from_be_bytes([bytes[170], bytes[171]]), 3);
        assert_eq!(u16::from_be_bytes([bytes[178], bytes[179]]), 4);
        assert_eq!(u16::from_be_bytes([bytes[180], bytes[181]]), 32);

        let face = TrueTypeFace::from_bytes(&bytes).unwrap();
        let mut output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            face.shape_text_ligatures("AB", Point::new(2, 15), 10, &mut output)
                .unwrap(),
            1
        );
        assert_eq!(output[0].glyph_id, 3);
        let mut single_bytes = bytes;
        put32(&mut single_bytes, 104, 64);
        put32(&mut single_bytes, 262, 0x0001_0000);
        put16(&mut single_bytes, 266, 0);
        put16(&mut single_bytes, 268, 10);
        put16(&mut single_bytes, 270, 36);
        put16(&mut single_bytes, 272, 1);
        single_bytes[274..278].copy_from_slice(b"smcp");
        put16(&mut single_bytes, 278, 8);
        put16(&mut single_bytes, 280, 0);
        put16(&mut single_bytes, 282, 1);
        put16(&mut single_bytes, 284, 0);
        put16(&mut single_bytes, 298, 1);
        put16(&mut single_bytes, 300, 4);
        put16(&mut single_bytes, 302, 1);
        put16(&mut single_bytes, 304, 0);
        put16(&mut single_bytes, 306, 1);
        put16(&mut single_bytes, 308, 8);
        put16(&mut single_bytes, 310, 2);
        put16(&mut single_bytes, 312, 10);
        put16(&mut single_bytes, 314, 1);
        put16(&mut single_bytes, 316, 3);
        put16(&mut single_bytes, 320, 1);
        put16(&mut single_bytes, 322, 1);
        put16(&mut single_bytes, 324, 1);
        let single_face = TrueTypeFace::from_bytes(&single_bytes).unwrap();
        assert_eq!(
            single_face
                .shape_text_ligatures("A", Point::new(2, 15), 10, &mut output)
                .unwrap(),
            1
        );
        assert_eq!(output[0].glyph_id, 3);
        assert_eq!(output[0].source_count, 1);
        let mut multiple_bytes = single_bytes;
        put32(&mut multiple_bytes, 104, 70);
        multiple_bytes[274..278].copy_from_slice(b"subs");
        put16(&mut multiple_bytes, 310, 1);
        put16(&mut multiple_bytes, 302, 2);
        put16(&mut multiple_bytes, 312, 16);
        put16(&mut multiple_bytes, 314, 1);
        put16(&mut multiple_bytes, 316, 10);
        put16(&mut multiple_bytes, 320, 2);
        put16(&mut multiple_bytes, 322, 3);
        put16(&mut multiple_bytes, 324, 2);
        put16(&mut multiple_bytes, 326, 1);
        put16(&mut multiple_bytes, 328, 1);
        put16(&mut multiple_bytes, 330, 1);
        let parsed_multiple =
            crate::graphics::gsub_multiple_subtable(&multiple_bytes, 310, 1, 372).unwrap();
        assert_eq!(parsed_multiple.as_ref().map(|(_, count)| *count), Some(2));
        let multiple_face = TrueTypeFace::from_bytes(&multiple_bytes).unwrap();
        let mut multiple_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 128];
        assert_eq!(
            multiple_face
                .shape_text_ligatures("A", Point::new(2, 15), 10, &mut multiple_output)
                .unwrap(),
            2
        );
        assert_eq!(multiple_output[0].glyph_id, 3);
        assert_eq!(multiple_output[1].glyph_id, 2);
        assert_eq!(multiple_output[0].source, 'A');
        assert_eq!(multiple_output[0].source_count, 1);
        assert_eq!(multiple_output[1].source, '\0');
        assert_eq!(multiple_output[1].source_count, 0);
        let mut extension_multiple_bytes = multiple_bytes;
        put32(&mut extension_multiple_bytes, 104, 78);
        put16(&mut extension_multiple_bytes, 302, 7);
        put16(&mut extension_multiple_bytes, 310, 1);
        put16(&mut extension_multiple_bytes, 312, 2);
        put32(&mut extension_multiple_bytes, 314, 8);
        extension_multiple_bytes[318..340].copy_from_slice(&multiple_bytes[310..332]);
        let extension_multiple_face = TrueTypeFace::from_bytes(&extension_multiple_bytes).unwrap();
        let mut extension_multiple_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            extension_multiple_face
                .shape_text_ligatures("A", Point::new(2, 15), 10, &mut extension_multiple_output,)
                .unwrap(),
            2
        );
        assert_eq!(extension_multiple_output[0].glyph_id, 3);
        assert_eq!(extension_multiple_output[1].glyph_id, 2);
        let mut malformed_multiple = multiple_bytes;
        put16(&mut malformed_multiple, 320, 17);
        assert_eq!(
            crate::graphics::gsub_multiple_subtable(&malformed_multiple, 310, 1, 372),
            Err(crate::graphics::TrueTypeError::TooLarge)
        );
        let mut context_bytes = single_bytes;
        put32(&mut context_bytes, 104, 116);
        put32(&mut context_bytes, 262, 0x0001_0000);
        put16(&mut context_bytes, 266, 0);
        put16(&mut context_bytes, 268, 10);
        put16(&mut context_bytes, 270, 36);
        put16(&mut context_bytes, 272, 1);
        context_bytes[274..278].copy_from_slice(b"calt");
        put16(&mut context_bytes, 278, 8);
        put16(&mut context_bytes, 280, 0);
        put16(&mut context_bytes, 282, 1);
        put16(&mut context_bytes, 284, 0);
        put16(&mut context_bytes, 298, 2);
        put16(&mut context_bytes, 300, 6);
        put16(&mut context_bytes, 302, 54);
        put16(&mut context_bytes, 304, 5);
        put16(&mut context_bytes, 306, 0);
        put16(&mut context_bytes, 308, 1);
        put16(&mut context_bytes, 310, 8);
        put16(&mut context_bytes, 312, 3);
        put16(&mut context_bytes, 314, 1);
        put16(&mut context_bytes, 316, 20);
        put16(&mut context_bytes, 318, 1);
        put16(&mut context_bytes, 320, 26);
        put16(&mut context_bytes, 322, 1);
        put16(&mut context_bytes, 324, 32);
        put16(&mut context_bytes, 326, 1);
        put16(&mut context_bytes, 328, 0);
        put16(&mut context_bytes, 330, 1);
        put16(&mut context_bytes, 332, 1);
        put16(&mut context_bytes, 334, 1);
        put16(&mut context_bytes, 336, 1);
        put16(&mut context_bytes, 338, 1);
        put16(&mut context_bytes, 340, 1);
        put16(&mut context_bytes, 342, 2);
        put16(&mut context_bytes, 344, 1);
        put16(&mut context_bytes, 346, 1);
        put16(&mut context_bytes, 348, 3);
        put16(&mut context_bytes, 350, 1);
        put16(&mut context_bytes, 352, 1);
        put16(&mut context_bytes, 354, 0);
        put16(&mut context_bytes, 356, 1);
        put16(&mut context_bytes, 358, 8);
        put16(&mut context_bytes, 360, 2);
        put16(&mut context_bytes, 362, 10);
        put16(&mut context_bytes, 364, 1);
        put16(&mut context_bytes, 366, 3);
        put16(&mut context_bytes, 370, 1);
        put16(&mut context_bytes, 372, 1);
        put16(&mut context_bytes, 374, 2);
        let context_face = TrueTypeFace::from_bytes(&context_bytes).unwrap();
        let mut context_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 3];
        let context_count = context_face
            .shape_text_ligatures("ABC", Point::new(2, 15), 10, &mut context_output)
            .unwrap();
        assert_eq!(context_count, 3);
        assert_eq!(context_output[1].glyph_id, 3);
        assert_eq!(context_output[1].source, 'B');
        let mut chain_bytes = context_bytes;
        put16(&mut chain_bytes, 304, 6);
        let chain_face = TrueTypeFace::from_bytes(&chain_bytes).unwrap();
        let mut chain_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 3];
        assert_eq!(
            chain_face
                .shape_text_ligatures("ABC", Point::new(2, 15), 10, &mut chain_output)
                .unwrap(),
            3
        );
        assert_eq!(chain_output[1].glyph_id, 3);
        let mut class_bytes = single_bytes;
        put32(&mut class_bytes, 104, 138);
        put32(&mut class_bytes, 262, 0x0001_0000);
        put16(&mut class_bytes, 266, 0);
        put16(&mut class_bytes, 268, 10);
        put16(&mut class_bytes, 270, 36);
        put16(&mut class_bytes, 272, 1);
        class_bytes[274..278].copy_from_slice(b"calt");
        put16(&mut class_bytes, 278, 8);
        put16(&mut class_bytes, 280, 0);
        put16(&mut class_bytes, 282, 1);
        put16(&mut class_bytes, 284, 0);
        put16(&mut class_bytes, 298, 2);
        put16(&mut class_bytes, 300, 6);
        put16(&mut class_bytes, 302, 78);
        put16(&mut class_bytes, 304, 5);
        put16(&mut class_bytes, 306, 0);
        put16(&mut class_bytes, 308, 1);
        put16(&mut class_bytes, 310, 8);
        put16(&mut class_bytes, 312, 2);
        put16(&mut class_bytes, 314, 40);
        put16(&mut class_bytes, 316, 46);
        put16(&mut class_bytes, 318, 1);
        put16(&mut class_bytes, 320, 12);
        put16(&mut class_bytes, 324, 1);
        put16(&mut class_bytes, 326, 8);
        put16(&mut class_bytes, 332, 2);
        put16(&mut class_bytes, 334, 1);
        put16(&mut class_bytes, 336, 1);
        put16(&mut class_bytes, 338, 0);
        put16(&mut class_bytes, 340, 1);
        put16(&mut class_bytes, 352, 1);
        put16(&mut class_bytes, 354, 1);
        put16(&mut class_bytes, 356, 1);
        put16(&mut class_bytes, 358, 1);
        put16(&mut class_bytes, 360, 1);
        put16(&mut class_bytes, 362, 2);
        put16(&mut class_bytes, 364, 0);
        put16(&mut class_bytes, 366, 1);
        put16(&mut class_bytes, 376, 1);
        put16(&mut class_bytes, 378, 0);
        put16(&mut class_bytes, 380, 1);
        put16(&mut class_bytes, 382, 8);
        put16(&mut class_bytes, 384, 2);
        put16(&mut class_bytes, 386, 10);
        put16(&mut class_bytes, 388, 1);
        put16(&mut class_bytes, 390, 3);
        put16(&mut class_bytes, 394, 1);
        put16(&mut class_bytes, 396, 1);
        put16(&mut class_bytes, 398, 1);
        let class_face = TrueTypeFace::from_bytes(&class_bytes).unwrap();
        let mut class_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            class_face
                .shape_text_ligatures("AB", Point::new(2, 15), 10, &mut class_output)
                .unwrap(),
            2
        );
        assert_eq!(class_output[0].glyph_id, 3);
        let mut extension_class_bytes = [0u8; 512];
        extension_class_bytes[..class_bytes.len()].copy_from_slice(&class_bytes);
        extension_class_bytes[420..508].copy_from_slice(&class_bytes[312..400]);
        put32(&mut extension_class_bytes, 104, 250);
        put16(&mut extension_class_bytes, 304, 7);
        put16(&mut extension_class_bytes, 312, 1);
        put16(&mut extension_class_bytes, 314, 5);
        put32(&mut extension_class_bytes, 316, 108);
        let extension_class_face = TrueTypeFace::from_bytes(&extension_class_bytes).unwrap();
        let mut extension_class_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            extension_class_face
                .shape_text_ligatures("AB", Point::new(2, 15), 10, &mut extension_class_output)
                .unwrap(),
            2
        );
        assert_eq!(extension_class_output[0].glyph_id, 3);
        let mut class_chain_bytes = [0u8; 480];
        class_chain_bytes[..single_bytes.len()].copy_from_slice(&single_bytes);
        put32(&mut class_chain_bytes, 104, 210);
        put32(&mut class_chain_bytes, 262, 0x0001_0000);
        put16(&mut class_chain_bytes, 266, 0);
        put16(&mut class_chain_bytes, 268, 10);
        put16(&mut class_chain_bytes, 270, 36);
        put16(&mut class_chain_bytes, 272, 1);
        class_chain_bytes[274..278].copy_from_slice(b"calt");
        put16(&mut class_chain_bytes, 278, 8);
        put16(&mut class_chain_bytes, 280, 0);
        put16(&mut class_chain_bytes, 282, 1);
        put16(&mut class_chain_bytes, 284, 0);
        put16(&mut class_chain_bytes, 298, 2);
        put16(&mut class_chain_bytes, 300, 6);
        put16(&mut class_chain_bytes, 302, 62);
        put16(&mut class_chain_bytes, 304, 6);
        put16(&mut class_chain_bytes, 306, 0);
        put16(&mut class_chain_bytes, 308, 1);
        put16(&mut class_chain_bytes, 310, 8);
        put16(&mut class_chain_bytes, 312, 2);
        put16(&mut class_chain_bytes, 314, 100);
        put16(&mut class_chain_bytes, 316, 112);
        put16(&mut class_chain_bytes, 318, 124);
        put16(&mut class_chain_bytes, 320, 136);
        put16(&mut class_chain_bytes, 322, 1);
        put16(&mut class_chain_bytes, 324, 14);
        put16(&mut class_chain_bytes, 326, 1);
        put16(&mut class_chain_bytes, 328, 8);
        put16(&mut class_chain_bytes, 334, 1);
        put16(&mut class_chain_bytes, 336, 1);
        put16(&mut class_chain_bytes, 338, 1);
        put16(&mut class_chain_bytes, 340, 1);
        put16(&mut class_chain_bytes, 342, 1);
        put16(&mut class_chain_bytes, 344, 1);
        put16(&mut class_chain_bytes, 346, 0);
        put16(&mut class_chain_bytes, 348, 1);
        put16(&mut class_chain_bytes, 360, 1);
        put16(&mut class_chain_bytes, 362, 0);
        put16(&mut class_chain_bytes, 364, 1);
        put16(&mut class_chain_bytes, 366, 8);
        put16(&mut class_chain_bytes, 368, 2);
        put16(&mut class_chain_bytes, 370, 10);
        put16(&mut class_chain_bytes, 372, 1);
        put16(&mut class_chain_bytes, 374, 3);
        put16(&mut class_chain_bytes, 378, 1);
        put16(&mut class_chain_bytes, 380, 1);
        put16(&mut class_chain_bytes, 382, 2);
        put16(&mut class_chain_bytes, 412, 1);
        put16(&mut class_chain_bytes, 414, 1);
        put16(&mut class_chain_bytes, 416, 2);
        put16(&mut class_chain_bytes, 424, 1);
        put16(&mut class_chain_bytes, 426, 1);
        put16(&mut class_chain_bytes, 428, 3);
        put16(&mut class_chain_bytes, 430, 1);
        put16(&mut class_chain_bytes, 432, 0);
        put16(&mut class_chain_bytes, 434, 1);
        put16(&mut class_chain_bytes, 436, 1);
        put16(&mut class_chain_bytes, 438, 1);
        put16(&mut class_chain_bytes, 440, 3);
        put16(&mut class_chain_bytes, 442, 0);
        put16(&mut class_chain_bytes, 444, 0);
        put16(&mut class_chain_bytes, 446, 0);
        put16(&mut class_chain_bytes, 448, 1);
        put16(&mut class_chain_bytes, 450, 1);
        put16(&mut class_chain_bytes, 452, 3);
        put16(&mut class_chain_bytes, 454, 1);
        put16(&mut class_chain_bytes, 456, 0);
        put16(&mut class_chain_bytes, 458, 1);
        let class_chain_face = TrueTypeFace::from_bytes(&class_chain_bytes).unwrap();
        let mut class_chain_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 3];
        assert_eq!(
            class_chain_face
                .shape_text_ligatures("ABC", Point::new(2, 15), 10, &mut class_chain_output)
                .unwrap(),
            3
        );
        assert_eq!(class_chain_output[1].glyph_id, 3);
        let mut extension_class_chain_bytes = [0u8; 600];
        extension_class_chain_bytes[..class_chain_bytes.len()].copy_from_slice(&class_chain_bytes);
        extension_class_chain_bytes[450..598].copy_from_slice(&class_chain_bytes[312..460]);
        put32(&mut extension_class_chain_bytes, 104, 338);
        put16(&mut extension_class_chain_bytes, 304, 7);
        put16(&mut extension_class_chain_bytes, 312, 1);
        put16(&mut extension_class_chain_bytes, 314, 6);
        put32(&mut extension_class_chain_bytes, 316, 138);
        let extension_class_chain_face =
            TrueTypeFace::from_bytes(&extension_class_chain_bytes).unwrap();
        let mut extension_class_chain_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 3];
        assert_eq!(
            extension_class_chain_face
                .shape_text_ligatures(
                    "ABC",
                    Point::new(2, 15),
                    10,
                    &mut extension_class_chain_output,
                )
                .unwrap(),
            3
        );
        assert_eq!(extension_class_chain_output[1].glyph_id, 3);
        let mut pipeline_bytes = single_bytes;
        put32(&mut pipeline_bytes, 104, 100);
        put32(&mut pipeline_bytes, 262, 0x0001_0000);
        put16(&mut pipeline_bytes, 266, 0);
        put16(&mut pipeline_bytes, 268, 10);
        put16(&mut pipeline_bytes, 270, 36);
        put16(&mut pipeline_bytes, 272, 1);
        pipeline_bytes[274..278].copy_from_slice(b"smcp");
        put16(&mut pipeline_bytes, 278, 8);
        put16(&mut pipeline_bytes, 280, 0);
        put16(&mut pipeline_bytes, 282, 2);
        put16(&mut pipeline_bytes, 284, 0);
        put16(&mut pipeline_bytes, 286, 1);
        put16(&mut pipeline_bytes, 298, 2);
        put16(&mut pipeline_bytes, 300, 6);
        put16(&mut pipeline_bytes, 302, 32);
        put16(&mut pipeline_bytes, 304, 1);
        put16(&mut pipeline_bytes, 306, 0);
        put16(&mut pipeline_bytes, 308, 1);
        put16(&mut pipeline_bytes, 310, 8);
        put16(&mut pipeline_bytes, 312, 2);
        put16(&mut pipeline_bytes, 314, 10);
        put16(&mut pipeline_bytes, 316, 1);
        put16(&mut pipeline_bytes, 318, 2);
        put16(&mut pipeline_bytes, 322, 1);
        put16(&mut pipeline_bytes, 324, 1);
        put16(&mut pipeline_bytes, 326, 1);
        put16(&mut pipeline_bytes, 330, 1);
        put16(&mut pipeline_bytes, 332, 0);
        put16(&mut pipeline_bytes, 334, 1);
        put16(&mut pipeline_bytes, 336, 8);
        put16(&mut pipeline_bytes, 338, 2);
        put16(&mut pipeline_bytes, 340, 10);
        put16(&mut pipeline_bytes, 342, 1);
        put16(&mut pipeline_bytes, 344, 3);
        put16(&mut pipeline_bytes, 348, 1);
        put16(&mut pipeline_bytes, 350, 1);
        put16(&mut pipeline_bytes, 352, 2);
        let pipeline_face = TrueTypeFace::from_bytes(&pipeline_bytes).unwrap();
        let mut pipeline_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 1];
        assert_eq!(
            pipeline_face
                .shape_text_ligatures("A", Point::new(2, 15), 10, &mut pipeline_output)
                .unwrap(),
            1
        );
        assert_eq!(pipeline_output[0].glyph_id, 3);
        let mut extension_bytes = pipeline_bytes;
        put16(&mut extension_bytes, 304, 7);
        put16(&mut extension_bytes, 312, 1);
        put16(&mut extension_bytes, 314, 1);
        put32(&mut extension_bytes, 316, 8);
        put16(&mut extension_bytes, 320, 2);
        put16(&mut extension_bytes, 322, 8);
        put16(&mut extension_bytes, 324, 1);
        put16(&mut extension_bytes, 326, 2);
        put16(&mut extension_bytes, 328, 1);
        put16(&mut extension_bytes, 330, 1);
        put16(&mut extension_bytes, 332, 1);
        let extension_face = TrueTypeFace::from_bytes(&extension_bytes).unwrap();
        let mut extension_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 1];
        assert_eq!(
            extension_face
                .shape_text_ligatures("A", Point::new(2, 15), 10, &mut extension_output)
                .unwrap(),
            1
        );
        assert_eq!(extension_output[0].glyph_id, 3);
        let mut extension_ligature_bytes = pipeline_bytes;
        put16(&mut extension_ligature_bytes, 304, 7);
        put16(&mut extension_ligature_bytes, 312, 1);
        put16(&mut extension_ligature_bytes, 314, 4);
        put32(&mut extension_ligature_bytes, 316, 8);
        put16(&mut extension_ligature_bytes, 320, 1);
        put16(&mut extension_ligature_bytes, 322, 8);
        put16(&mut extension_ligature_bytes, 324, 1);
        put16(&mut extension_ligature_bytes, 326, 14);
        put16(&mut extension_ligature_bytes, 328, 1);
        put16(&mut extension_ligature_bytes, 330, 1);
        put16(&mut extension_ligature_bytes, 332, 1);
        put16(&mut extension_ligature_bytes, 334, 1);
        put16(&mut extension_ligature_bytes, 336, 6);
        put16(&mut extension_ligature_bytes, 340, 9);
        put16(&mut extension_ligature_bytes, 342, 2);
        put16(&mut extension_ligature_bytes, 344, 2);
        let extension_ligature_face = TrueTypeFace::from_bytes(&extension_ligature_bytes).unwrap();
        let mut extension_ligature_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            extension_ligature_face
                .shape_text_ligatures("AB", Point::new(2, 15), 10, &mut extension_ligature_output)
                .unwrap(),
            1
        );
        assert_eq!(extension_ligature_output[0].glyph_id, 9);
        let mut extension_contextual_bytes = pipeline_bytes;
        put32(&mut extension_contextual_bytes, 104, 138);
        put16(&mut extension_contextual_bytes, 302, 52);
        put16(&mut extension_contextual_bytes, 304, 7);
        put16(&mut extension_contextual_bytes, 312, 1);
        put16(&mut extension_contextual_bytes, 314, 5);
        put32(&mut extension_contextual_bytes, 316, 8);
        put16(&mut extension_contextual_bytes, 320, 3);
        put16(&mut extension_contextual_bytes, 322, 0);
        put16(&mut extension_contextual_bytes, 324, 2);
        put16(&mut extension_contextual_bytes, 326, 52);
        put16(&mut extension_contextual_bytes, 328, 58);
        put16(&mut extension_contextual_bytes, 330, 0);
        put16(&mut extension_contextual_bytes, 332, 1);
        put16(&mut extension_contextual_bytes, 334, 0);
        put16(&mut extension_contextual_bytes, 336, 1);
        put16(&mut extension_contextual_bytes, 350, 1);
        put16(&mut extension_contextual_bytes, 352, 0);
        put16(&mut extension_contextual_bytes, 354, 1);
        put16(&mut extension_contextual_bytes, 356, 8);
        put16(&mut extension_contextual_bytes, 358, 2);
        put16(&mut extension_contextual_bytes, 360, 8);
        put16(&mut extension_contextual_bytes, 362, 1);
        put16(&mut extension_contextual_bytes, 364, 3);
        put16(&mut extension_contextual_bytes, 366, 1);
        put16(&mut extension_contextual_bytes, 368, 1);
        put16(&mut extension_contextual_bytes, 370, 1);
        put16(&mut extension_contextual_bytes, 372, 1);
        put16(&mut extension_contextual_bytes, 374, 1);
        put16(&mut extension_contextual_bytes, 376, 2);
        put16(&mut extension_contextual_bytes, 378, 1);
        put16(&mut extension_contextual_bytes, 380, 1);
        put16(&mut extension_contextual_bytes, 382, 2);
        let extension_contextual_face =
            TrueTypeFace::from_bytes(&extension_contextual_bytes).unwrap();
        let mut extension_contextual_output = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            extension_contextual_face
                .shape_text_ligatures(
                    "AB",
                    Point::new(2, 15),
                    10,
                    &mut extension_contextual_output,
                )
                .unwrap(),
            2
        );
        assert_eq!(extension_contextual_output[0].glyph_id, 3);
        assert_eq!(
            face.shape_text_ligatures_with_script_language(
                "AB",
                Point::new(2, 15),
                10,
                *b"DFLT",
                *b"TRK ",
                &mut output,
            )
            .unwrap(),
            1
        );
        assert_eq!(
            face.shape_text_ligatures_with_script_language(
                "AB",
                Point::new(2, 15),
                10,
                *b"DFLT",
                *b"MISS",
                &mut output,
            )
            .unwrap(),
            2
        );
        assert_eq!(output[0].source_count, 1);
        assert_eq!(output[0].origin, Point::new(2, 15));
        assert_eq!(output[0].advance, 5);
        assert_eq!(
            face.shape_text_ligatures_with_script(
                "AB",
                Point::new(2, 15),
                10,
                *b"DFLT",
                &mut output,
            )
            .unwrap(),
            1
        );
        assert_eq!(output[0].glyph_id, 3);
    }

    #[test]
    fn wovengraphics_truetype_face_applies_bounded_gpos_pair_adjustment() {
        use crate::graphics::{Point, ShapedGlyph, TrueTypeFace};

        let mut bytes = [0u8; 420];
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put_i16 = |bytes: &mut [u8], offset: usize, value: i16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put32 = |bytes: &mut [u8], offset: usize, value: u32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        };
        put32(&mut bytes, 0, 0x0001_0000);
        put16(&mut bytes, 4, 6);
        for (record, (tag, offset, length)) in [
            (*b"head", 108u32, 52u32),
            (*b"maxp", 160u32, 6u32),
            (*b"cmap", 166u32, 44u32),
            (*b"hhea", 210u32, 36u32),
            (*b"hmtx", 246u32, 12u32),
            (*b"GPOS", 258u32, 156u32),
        ]
        .into_iter()
        .enumerate()
        {
            let base = 12 + record * 16;
            bytes[base..base + 4].copy_from_slice(&tag);
            put32(&mut bytes, base + 8, offset);
            put32(&mut bytes, base + 12, length);
        }
        put16(&mut bytes, 126, 1000);
        put16(&mut bytes, 164, 3);
        put16(&mut bytes, 244, 3);
        for (index, advance) in [500, 500, 500].into_iter().enumerate() {
            put16(&mut bytes, 246 + index * 4, advance);
        }
        put16(&mut bytes, 166, 0);
        put16(&mut bytes, 168, 1);
        put16(&mut bytes, 170, 3);
        put16(&mut bytes, 172, 1);
        put32(&mut bytes, 174, 12);
        put16(&mut bytes, 178, 4);
        put16(&mut bytes, 180, 32);
        put16(&mut bytes, 182, 0);
        put16(&mut bytes, 184, 4);
        put16(&mut bytes, 186, 4);
        put16(&mut bytes, 188, 1);
        put16(&mut bytes, 190, 0);
        put16(&mut bytes, 192, 66);
        put16(&mut bytes, 194, 0xffff);
        put16(&mut bytes, 196, 0);
        put16(&mut bytes, 198, 65);
        put16(&mut bytes, 200, 0xffff);
        put_i16(&mut bytes, 202, -64);
        put_i16(&mut bytes, 204, 1);
        put16(&mut bytes, 206, 0);
        put16(&mut bytes, 208, 0);

        put32(&mut bytes, 258, 0x0001_0000);
        put16(&mut bytes, 262, 120);
        put16(&mut bytes, 264, 10);
        put16(&mut bytes, 266, 30);
        put16(&mut bytes, 268, 1);
        bytes[270..274].copy_from_slice(b"kern");
        put16(&mut bytes, 274, 8);
        put16(&mut bytes, 276, 0);
        put16(&mut bytes, 278, 1);
        put16(&mut bytes, 280, 0);
        put16(&mut bytes, 288, 1);
        put16(&mut bytes, 290, 4);
        put16(&mut bytes, 292, 2);
        put16(&mut bytes, 294, 0);
        put16(&mut bytes, 296, 1);
        put16(&mut bytes, 298, 8);
        put16(&mut bytes, 300, 2);
        put16(&mut bytes, 302, 64);
        put16(&mut bytes, 304, 0x0017);
        put16(&mut bytes, 306, 0);
        put16(&mut bytes, 308, 48);
        put16(&mut bytes, 310, 56);
        put16(&mut bytes, 312, 2);
        put16(&mut bytes, 314, 2);
        put16(&mut bytes, 316, 0);
        put16(&mut bytes, 318, 0);
        put16(&mut bytes, 320, 0);
        put16(&mut bytes, 322, 0);
        put16(&mut bytes, 324, 0);
        put16(&mut bytes, 326, 0);
        put16(&mut bytes, 328, 0);
        put16(&mut bytes, 330, 0);
        put16(&mut bytes, 332, 0);
        put16(&mut bytes, 334, 0);
        put16(&mut bytes, 336, 0);
        put16(&mut bytes, 338, 0);
        put16(&mut bytes, 340, 0);
        put16(&mut bytes, 342, 100);
        put_i16(&mut bytes, 344, -80);
        put16(&mut bytes, 346, 112);
        put16(&mut bytes, 348, 1);
        put16(&mut bytes, 350, 1);
        put16(&mut bytes, 352, 1);
        put16(&mut bytes, 354, 1);
        put16(&mut bytes, 356, 1);
        put16(&mut bytes, 358, 2);
        put16(&mut bytes, 360, 1);
        put16(&mut bytes, 362, 1);
        put16(&mut bytes, 364, 1);
        put16(&mut bytes, 366, 1);
        put16(&mut bytes, 368, 1);
        put16(&mut bytes, 370, 10);
        put16(&mut bytes, 372, 10);
        put16(&mut bytes, 374, 3);
        put16(&mut bytes, 376, 0x0100);
        put16(&mut bytes, 378, 1);
        bytes[380..384].copy_from_slice(b"DFLT");
        put16(&mut bytes, 384, 10);
        put16(&mut bytes, 388, 10);
        put16(&mut bytes, 390, 1);
        bytes[392..396].copy_from_slice(b"TRK ");
        put16(&mut bytes, 396, 18);
        put16(&mut bytes, 398, 0);
        put16(&mut bytes, 400, u16::MAX);
        put16(&mut bytes, 402, 1);
        put16(&mut bytes, 404, 0);
        put16(&mut bytes, 406, 0);
        put16(&mut bytes, 408, u16::MAX);
        put16(&mut bytes, 410, 1);
        put16(&mut bytes, 412, 0);

        let face = TrueTypeFace::from_bytes(&bytes).unwrap();
        let mut shaped = [ShapedGlyph {
            glyph_id: 0,
            source: '\0',
            source_count: 0,
            origin: Point::new(0, 0),
            advance: 0,
        }; 2];
        assert_eq!(
            face.shape_text_ligatures("AB", Point::new(2, 15), 10, &mut shaped),
            Err(crate::graphics::TrueTypeError::UnsupportedVariation)
        );
        assert_eq!(
            face.shape_text_ligatures_with_ppem("AB", Point::new(2, 15), 1000, 10, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].origin, Point::new(3, 115));
        assert_eq!(
            face.shape_text_ligatures_with_options(
                "AB",
                Point::new(2, 15),
                1000,
                crate::graphics::TextShapingOptions {
                    script_tag: Some(*b"DFLT"),
                    language_tag: Some(*b"TRK "),
                    ppem: Some(10),
                },
                &mut shaped,
            )
            .unwrap(),
            2
        );
        assert_eq!(shaped[0].origin, Point::new(3, 115));
        assert_eq!(
            face.shape_text_ligatures_with_options(
                "AB",
                Point::new(2, 15),
                1000,
                crate::graphics::TextShapingOptions {
                    script_tag: Some(*b"DFLT"),
                    language_tag: Some(*b"MISS"),
                    ppem: Some(10),
                },
                &mut shaped,
            )
            .unwrap(),
            2
        );
        assert_eq!(shaped[0].origin, Point::new(2, 15));
        assert_eq!(shaped[0].advance, 500);
        assert_eq!(shaped[1].origin, Point::new(502, 15));
        assert_eq!(
            face.shape_text_ligatures_with_script_and_ppem(
                "AB",
                Point::new(2, 15),
                1000,
                *b"DFLT",
                10,
                &mut shaped,
            )
            .unwrap(),
            2
        );
        assert_eq!(shaped[0].origin, Point::new(3, 115));
        let mut contextual_gpos_bytes = bytes;
        put32(&mut contextual_gpos_bytes, 104, 102);
        contextual_gpos_bytes[270..274].copy_from_slice(b"calt");
        put16(&mut contextual_gpos_bytes, 288, 2);
        put16(&mut contextual_gpos_bytes, 290, 8);
        put16(&mut contextual_gpos_bytes, 292, 48);
        put16(&mut contextual_gpos_bytes, 296, 7);
        put16(&mut contextual_gpos_bytes, 298, 0);
        put16(&mut contextual_gpos_bytes, 300, 1);
        put16(&mut contextual_gpos_bytes, 302, 8);
        put16(&mut contextual_gpos_bytes, 304, 3);
        put16(&mut contextual_gpos_bytes, 306, 0);
        put16(&mut contextual_gpos_bytes, 308, 2);
        put16(&mut contextual_gpos_bytes, 310, 20);
        put16(&mut contextual_gpos_bytes, 312, 26);
        put16(&mut contextual_gpos_bytes, 314, 0);
        put16(&mut contextual_gpos_bytes, 316, 1);
        put16(&mut contextual_gpos_bytes, 318, 0);
        put16(&mut contextual_gpos_bytes, 320, 1);
        put16(&mut contextual_gpos_bytes, 324, 1);
        put16(&mut contextual_gpos_bytes, 326, 1);
        put16(&mut contextual_gpos_bytes, 328, 1);
        put16(&mut contextual_gpos_bytes, 330, 1);
        put16(&mut contextual_gpos_bytes, 332, 1);
        put16(&mut contextual_gpos_bytes, 334, 2);
        put16(&mut contextual_gpos_bytes, 336, 1);
        put16(&mut contextual_gpos_bytes, 338, 0);
        put16(&mut contextual_gpos_bytes, 340, 1);
        put16(&mut contextual_gpos_bytes, 342, 8);
        put16(&mut contextual_gpos_bytes, 344, 1);
        put16(&mut contextual_gpos_bytes, 346, 10);
        put16(&mut contextual_gpos_bytes, 348, 4);
        put_i16(&mut contextual_gpos_bytes, 350, 25);
        put16(&mut contextual_gpos_bytes, 354, 1);
        put16(&mut contextual_gpos_bytes, 356, 1);
        put16(&mut contextual_gpos_bytes, 358, 1);
        let contextual_gpos_face = TrueTypeFace::from_bytes(&contextual_gpos_bytes).unwrap();
        assert_eq!(
            contextual_gpos_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_chaining_bytes = contextual_gpos_bytes;
        put16(&mut contextual_chaining_bytes, 296, 8);
        let contextual_chaining_face =
            TrueTypeFace::from_bytes(&contextual_chaining_bytes).unwrap();
        assert_eq!(
            contextual_chaining_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_class_bytes = [0u8; 512];
        contextual_class_bytes[..contextual_gpos_bytes.len()]
            .copy_from_slice(&contextual_gpos_bytes);
        put32(&mut contextual_class_bytes, 104, 250);
        put16(&mut contextual_class_bytes, 304, 2);
        put16(&mut contextual_class_bytes, 306, 156);
        put16(&mut contextual_class_bytes, 308, 162);
        put16(&mut contextual_class_bytes, 310, 162);
        put16(&mut contextual_class_bytes, 312, 162);
        put16(&mut contextual_class_bytes, 314, 1);
        put16(&mut contextual_class_bytes, 316, 176);
        put16(&mut contextual_class_bytes, 460, 1);
        put16(&mut contextual_class_bytes, 462, 1);
        put16(&mut contextual_class_bytes, 464, 1);
        put16(&mut contextual_class_bytes, 466, 1);
        put16(&mut contextual_class_bytes, 468, 1);
        put16(&mut contextual_class_bytes, 470, 2);
        put16(&mut contextual_class_bytes, 472, 0);
        put16(&mut contextual_class_bytes, 474, 1);
        put16(&mut contextual_class_bytes, 480, 1);
        put16(&mut contextual_class_bytes, 482, 4);
        put16(&mut contextual_class_bytes, 484, 0);
        put16(&mut contextual_class_bytes, 486, 2);
        put16(&mut contextual_class_bytes, 488, 1);
        put16(&mut contextual_class_bytes, 490, 0);
        put16(&mut contextual_class_bytes, 492, 1);
        put16(&mut contextual_class_bytes, 494, 0);
        put16(&mut contextual_class_bytes, 496, 1);
        let contextual_class_face = TrueTypeFace::from_bytes(&contextual_class_bytes).unwrap();
        assert_eq!(
            contextual_class_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_class_pair_bytes = contextual_class_bytes;
        put16(&mut contextual_class_pair_bytes, 336, 2);
        put16(&mut contextual_class_pair_bytes, 344, 1);
        put16(&mut contextual_class_pair_bytes, 346, 26);
        put16(&mut contextual_class_pair_bytes, 348, 4);
        put16(&mut contextual_class_pair_bytes, 350, 0);
        put16(&mut contextual_class_pair_bytes, 352, 1);
        put16(&mut contextual_class_pair_bytes, 354, 18);
        put16(&mut contextual_class_pair_bytes, 362, 1);
        put16(&mut contextual_class_pair_bytes, 364, 2);
        put_i16(&mut contextual_class_pair_bytes, 366, 25);
        put16(&mut contextual_class_pair_bytes, 370, 1);
        put16(&mut contextual_class_pair_bytes, 372, 1);
        put16(&mut contextual_class_pair_bytes, 374, 1);
        let contextual_class_pair_face =
            TrueTypeFace::from_bytes(&contextual_class_pair_bytes).unwrap();
        assert_eq!(
            contextual_class_pair_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_class_matrix_bytes = contextual_class_bytes;
        put16(&mut contextual_class_matrix_bytes, 336, 2);
        put16(&mut contextual_class_matrix_bytes, 344, 2);
        put16(&mut contextual_class_matrix_bytes, 346, 26);
        put16(&mut contextual_class_matrix_bytes, 348, 4);
        put16(&mut contextual_class_matrix_bytes, 350, 0);
        put16(&mut contextual_class_matrix_bytes, 352, 62);
        put16(&mut contextual_class_matrix_bytes, 354, 70);
        put16(&mut contextual_class_matrix_bytes, 356, 1);
        put16(&mut contextual_class_matrix_bytes, 358, 1);
        put_i16(&mut contextual_class_matrix_bytes, 360, 25);
        put16(&mut contextual_class_matrix_bytes, 370, 1);
        put16(&mut contextual_class_matrix_bytes, 372, 1);
        put16(&mut contextual_class_matrix_bytes, 374, 1);
        put16(&mut contextual_class_matrix_bytes, 406, 1);
        put16(&mut contextual_class_matrix_bytes, 408, 1);
        put16(&mut contextual_class_matrix_bytes, 410, 1);
        put16(&mut contextual_class_matrix_bytes, 412, 0);
        put16(&mut contextual_class_matrix_bytes, 414, 1);
        put16(&mut contextual_class_matrix_bytes, 416, 2);
        put16(&mut contextual_class_matrix_bytes, 418, 1);
        put16(&mut contextual_class_matrix_bytes, 420, 0);
        let contextual_class_matrix_face =
            TrueTypeFace::from_bytes(&contextual_class_matrix_bytes).unwrap();
        assert_eq!(
            contextual_class_matrix_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_class_matrix_extension_bytes = contextual_class_bytes;
        put16(&mut contextual_class_matrix_extension_bytes, 336, 9);
        put16(&mut contextual_class_matrix_extension_bytes, 344, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 346, 2);
        put32(&mut contextual_class_matrix_extension_bytes, 348, 36);
        put16(&mut contextual_class_matrix_extension_bytes, 380, 2);
        put16(&mut contextual_class_matrix_extension_bytes, 382, 40);
        put16(&mut contextual_class_matrix_extension_bytes, 384, 4);
        put16(&mut contextual_class_matrix_extension_bytes, 386, 0);
        put16(&mut contextual_class_matrix_extension_bytes, 388, 24);
        put16(&mut contextual_class_matrix_extension_bytes, 390, 32);
        put16(&mut contextual_class_matrix_extension_bytes, 392, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 394, 1);
        put_i16(&mut contextual_class_matrix_extension_bytes, 396, 25);
        put16(&mut contextual_class_matrix_extension_bytes, 404, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 406, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 408, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 410, 0);
        put16(&mut contextual_class_matrix_extension_bytes, 412, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 414, 2);
        put16(&mut contextual_class_matrix_extension_bytes, 416, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 418, 0);
        put16(&mut contextual_class_matrix_extension_bytes, 420, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 422, 1);
        put16(&mut contextual_class_matrix_extension_bytes, 424, 1);
        let contextual_class_matrix_extension_face =
            TrueTypeFace::from_bytes(&contextual_class_matrix_extension_bytes).unwrap();
        assert_eq!(
            contextual_class_matrix_extension_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_class_target_bytes = contextual_class_bytes;
        put16(&mut contextual_class_target_bytes, 494, 1);
        put16(&mut contextual_class_target_bytes, 358, 2);
        let contextual_class_target_face =
            TrueTypeFace::from_bytes(&contextual_class_target_bytes).unwrap();
        assert_eq!(
            contextual_class_target_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 500);
        assert_eq!(shaped[1].advance, 525);
        let mut contextual_glyph_bytes = contextual_gpos_bytes;
        put16(&mut contextual_glyph_bytes, 296, 8);
        put16(&mut contextual_glyph_bytes, 304, 1);
        put16(&mut contextual_glyph_bytes, 306, 26);
        put16(&mut contextual_glyph_bytes, 308, 1);
        put16(&mut contextual_glyph_bytes, 310, 8);
        put16(&mut contextual_glyph_bytes, 312, 1);
        put16(&mut contextual_glyph_bytes, 314, 4);
        put16(&mut contextual_glyph_bytes, 316, 0);
        put16(&mut contextual_glyph_bytes, 318, 2);
        put16(&mut contextual_glyph_bytes, 320, 2);
        put16(&mut contextual_glyph_bytes, 322, 0);
        put16(&mut contextual_glyph_bytes, 324, 1);
        put16(&mut contextual_glyph_bytes, 326, 0);
        put16(&mut contextual_glyph_bytes, 328, 1);
        put16(&mut contextual_glyph_bytes, 330, 1);
        put16(&mut contextual_glyph_bytes, 332, 1);
        put16(&mut contextual_glyph_bytes, 334, 1);
        let contextual_glyph_face = TrueTypeFace::from_bytes(&contextual_glyph_bytes).unwrap();
        assert_eq!(
            contextual_glyph_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_glyph_target_bytes = contextual_glyph_bytes;
        put16(&mut contextual_glyph_target_bytes, 326, 1);
        put16(&mut contextual_glyph_target_bytes, 358, 2);
        let contextual_glyph_target_face =
            TrueTypeFace::from_bytes(&contextual_glyph_target_bytes).unwrap();
        assert_eq!(
            contextual_glyph_target_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 500);
        assert_eq!(shaped[1].advance, 525);
        let mut contextual_glyph_pair_bytes = contextual_glyph_bytes;
        put32(&mut contextual_glyph_pair_bytes, 104, 120);
        put16(&mut contextual_glyph_pair_bytes, 336, 2);
        put16(&mut contextual_glyph_pair_bytes, 344, 1);
        put16(&mut contextual_glyph_pair_bytes, 346, 26);
        put16(&mut contextual_glyph_pair_bytes, 348, 4);
        put16(&mut contextual_glyph_pair_bytes, 350, 0);
        put16(&mut contextual_glyph_pair_bytes, 352, 1);
        put16(&mut contextual_glyph_pair_bytes, 354, 18);
        put16(&mut contextual_glyph_pair_bytes, 362, 1);
        put16(&mut contextual_glyph_pair_bytes, 364, 2);
        put_i16(&mut contextual_glyph_pair_bytes, 366, 25);
        put16(&mut contextual_glyph_pair_bytes, 370, 1);
        put16(&mut contextual_glyph_pair_bytes, 372, 1);
        put16(&mut contextual_glyph_pair_bytes, 374, 1);
        let contextual_glyph_pair_face =
            TrueTypeFace::from_bytes(&contextual_glyph_pair_bytes).unwrap();
        assert_eq!(
            contextual_glyph_pair_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_glyph_pair_extension_bytes = [0u8; 512];
        contextual_glyph_pair_extension_bytes[..contextual_glyph_pair_bytes.len()]
            .copy_from_slice(&contextual_glyph_pair_bytes);
        put32(&mut contextual_glyph_pair_extension_bytes, 104, 250);
        put16(&mut contextual_glyph_pair_extension_bytes, 336, 9);
        put16(&mut contextual_glyph_pair_extension_bytes, 344, 1);
        put16(&mut contextual_glyph_pair_extension_bytes, 346, 2);
        put32(&mut contextual_glyph_pair_extension_bytes, 348, 36);
        put16(&mut contextual_glyph_pair_extension_bytes, 380, 1);
        put16(&mut contextual_glyph_pair_extension_bytes, 382, 26);
        put16(&mut contextual_glyph_pair_extension_bytes, 384, 4);
        put16(&mut contextual_glyph_pair_extension_bytes, 386, 0);
        put16(&mut contextual_glyph_pair_extension_bytes, 388, 1);
        put16(&mut contextual_glyph_pair_extension_bytes, 390, 18);
        put16(&mut contextual_glyph_pair_extension_bytes, 398, 1);
        put16(&mut contextual_glyph_pair_extension_bytes, 400, 2);
        put_i16(&mut contextual_glyph_pair_extension_bytes, 402, 25);
        put16(&mut contextual_glyph_pair_extension_bytes, 406, 1);
        put16(&mut contextual_glyph_pair_extension_bytes, 408, 1);
        put16(&mut contextual_glyph_pair_extension_bytes, 410, 1);
        let contextual_glyph_pair_extension_face =
            TrueTypeFace::from_bytes(&contextual_glyph_pair_extension_bytes).unwrap();
        assert_eq!(
            contextual_glyph_pair_extension_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_extension_bytes = [0u8; 512];
        contextual_extension_bytes[..contextual_gpos_bytes.len()]
            .copy_from_slice(&contextual_gpos_bytes);
        put32(&mut contextual_extension_bytes, 104, 250);
        contextual_extension_bytes[420..452].copy_from_slice(&contextual_gpos_bytes[304..336]);
        put16(&mut contextual_extension_bytes, 296, 9);
        put16(&mut contextual_extension_bytes, 304, 1);
        put16(&mut contextual_extension_bytes, 306, 7);
        put32(&mut contextual_extension_bytes, 308, 116);
        let contextual_extension_face =
            TrueTypeFace::from_bytes(&contextual_extension_bytes).unwrap();
        assert_eq!(
            contextual_extension_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_pair_bytes = contextual_gpos_bytes;
        put32(&mut contextual_pair_bytes, 104, 120);
        put16(&mut contextual_pair_bytes, 336, 2);
        put16(&mut contextual_pair_bytes, 344, 1);
        put16(&mut contextual_pair_bytes, 346, 26);
        put16(&mut contextual_pair_bytes, 348, 4);
        put16(&mut contextual_pair_bytes, 350, 0);
        put16(&mut contextual_pair_bytes, 352, 1);
        put16(&mut contextual_pair_bytes, 354, 18);
        put16(&mut contextual_pair_bytes, 362, 1);
        put16(&mut contextual_pair_bytes, 364, 2);
        put_i16(&mut contextual_pair_bytes, 366, 25);
        put16(&mut contextual_pair_bytes, 370, 1);
        put16(&mut contextual_pair_bytes, 372, 1);
        put16(&mut contextual_pair_bytes, 374, 1);
        let contextual_pair_face = TrueTypeFace::from_bytes(&contextual_pair_bytes).unwrap();
        assert_eq!(
            contextual_pair_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_class_pair_bytes = contextual_pair_bytes;
        put32(&mut contextual_class_pair_bytes, 104, 156);
        put16(&mut contextual_class_pair_bytes, 344, 2);
        put16(&mut contextual_class_pair_bytes, 346, 26);
        put16(&mut contextual_class_pair_bytes, 348, 4);
        put16(&mut contextual_class_pair_bytes, 350, 0);
        put16(&mut contextual_class_pair_bytes, 352, 40);
        put16(&mut contextual_class_pair_bytes, 354, 48);
        put16(&mut contextual_class_pair_bytes, 356, 1);
        put16(&mut contextual_class_pair_bytes, 358, 1);
        put_i16(&mut contextual_class_pair_bytes, 360, 25);
        put16(&mut contextual_class_pair_bytes, 370, 1);
        put16(&mut contextual_class_pair_bytes, 372, 1);
        put16(&mut contextual_class_pair_bytes, 374, 1);
        for (offset, glyph) in [(384, 1), (392, 2)] {
            put16(&mut contextual_class_pair_bytes, offset, 1);
            put16(&mut contextual_class_pair_bytes, offset + 2, glyph);
            put16(&mut contextual_class_pair_bytes, offset + 4, 1);
            put16(&mut contextual_class_pair_bytes, offset + 6, 0);
        }
        let contextual_class_pair_face =
            TrueTypeFace::from_bytes(&contextual_class_pair_bytes).unwrap();
        assert_eq!(
            contextual_class_pair_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_pair_second_bytes = contextual_pair_bytes;
        put16(&mut contextual_pair_second_bytes, 318, 1);
        put16(&mut contextual_pair_second_bytes, 374, 2);
        let contextual_pair_second_face =
            TrueTypeFace::from_bytes(&contextual_pair_second_bytes).unwrap();
        let mut shaped_pair_second = [shaped[0]; 3];
        assert_eq!(
            contextual_pair_second_face
                .shape_text_ligatures("ABB", Point::new(2, 15), 1000, &mut shaped_pair_second)
                .unwrap(),
            3
        );
        assert_eq!(shaped_pair_second[0].advance, 500);
        assert_eq!(shaped_pair_second[1].origin, Point::new(502, 15));
        assert_eq!(shaped_pair_second[1].advance, 525);
        assert_eq!(shaped_pair_second[2].origin, Point::new(1027, 15));
        let mut contextual_context2_bytes = contextual_gpos_bytes;
        put32(&mut contextual_context2_bytes, 104, 156);
        put16(&mut contextual_context2_bytes, 306, 2);
        put16(&mut contextual_context2_bytes, 308, 60);
        put16(&mut contextual_context2_bytes, 310, 66);
        put16(&mut contextual_context2_bytes, 312, 2);
        put16(&mut contextual_context2_bytes, 314, 72);
        put16(&mut contextual_context2_bytes, 316, 78);
        put16(&mut contextual_context2_bytes, 318, 2);
        put16(&mut contextual_context2_bytes, 320, 84);
        put16(&mut contextual_context2_bytes, 322, 90);
        put16(&mut contextual_context2_bytes, 324, 1);
        put16(&mut contextual_context2_bytes, 326, 0);
        put16(&mut contextual_context2_bytes, 328, 1);
        for offset in [364, 370, 376, 382, 388, 394] {
            put16(&mut contextual_context2_bytes, offset, 1);
            put16(&mut contextual_context2_bytes, offset + 2, 1);
            put16(&mut contextual_context2_bytes, offset + 4, 1);
        }
        let contextual_context2_face =
            TrueTypeFace::from_bytes(&contextual_context2_bytes).unwrap();
        let mut shaped_context2 = [shaped[0]; 6];
        assert_eq!(
            contextual_context2_face
                .shape_text_ligatures("AAAAAA", Point::new(2, 15), 1000, &mut shaped_context2)
                .unwrap(),
            6
        );
        assert_eq!(shaped_context2[2].advance, 525);
        assert_eq!(shaped_context2[3].origin, Point::new(1527, 15));
        let mut contextual_multi_bytes = contextual_gpos_bytes;
        put32(&mut contextual_multi_bytes, 104, 148);
        put16(&mut contextual_multi_bytes, 288, 3);
        put16(&mut contextual_multi_bytes, 290, 8);
        put16(&mut contextual_multi_bytes, 292, 62);
        put16(&mut contextual_multi_bytes, 294, 92);
        put16(&mut contextual_multi_bytes, 308, 2);
        put16(&mut contextual_multi_bytes, 310, 30);
        put16(&mut contextual_multi_bytes, 312, 36);
        put16(&mut contextual_multi_bytes, 316, 2);
        put16(&mut contextual_multi_bytes, 318, 0);
        put16(&mut contextual_multi_bytes, 320, 1);
        put16(&mut contextual_multi_bytes, 322, 0);
        put16(&mut contextual_multi_bytes, 324, 2);
        put16(&mut contextual_multi_bytes, 334, 1);
        put16(&mut contextual_multi_bytes, 336, 1);
        put16(&mut contextual_multi_bytes, 338, 1);
        put16(&mut contextual_multi_bytes, 340, 1);
        put16(&mut contextual_multi_bytes, 342, 1);
        put16(&mut contextual_multi_bytes, 344, 2);
        put16(&mut contextual_multi_bytes, 350, 1);
        put16(&mut contextual_multi_bytes, 352, 0);
        put16(&mut contextual_multi_bytes, 354, 1);
        put16(&mut contextual_multi_bytes, 356, 8);
        put16(&mut contextual_multi_bytes, 358, 1);
        put16(&mut contextual_multi_bytes, 360, 10);
        put16(&mut contextual_multi_bytes, 362, 4);
        put_i16(&mut contextual_multi_bytes, 364, 25);
        put16(&mut contextual_multi_bytes, 368, 1);
        put16(&mut contextual_multi_bytes, 370, 1);
        put16(&mut contextual_multi_bytes, 372, 1);
        put16(&mut contextual_multi_bytes, 380, 1);
        put16(&mut contextual_multi_bytes, 382, 0);
        put16(&mut contextual_multi_bytes, 384, 1);
        put16(&mut contextual_multi_bytes, 386, 8);
        put16(&mut contextual_multi_bytes, 388, 1);
        put16(&mut contextual_multi_bytes, 390, 10);
        put16(&mut contextual_multi_bytes, 392, 4);
        put_i16(&mut contextual_multi_bytes, 394, 15);
        put16(&mut contextual_multi_bytes, 398, 1);
        put16(&mut contextual_multi_bytes, 400, 1);
        put16(&mut contextual_multi_bytes, 402, 1);
        let contextual_multi_face = TrueTypeFace::from_bytes(&contextual_multi_bytes).unwrap();
        assert_eq!(
            contextual_multi_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 540);
        assert_eq!(shaped[1].origin, Point::new(542, 15));
        let mut contextual_second_bytes = contextual_multi_bytes;
        put16(&mut contextual_second_bytes, 322, 1);
        let contextual_second_face = TrueTypeFace::from_bytes(&contextual_second_bytes).unwrap();
        assert_eq!(
            contextual_second_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        assert_eq!(shaped[1].advance, 515);
        let mut contextual_single_input_bytes = contextual_gpos_bytes;
        put16(&mut contextual_single_input_bytes, 308, 1);
        put16(&mut contextual_single_input_bytes, 310, 20);
        put16(&mut contextual_single_input_bytes, 312, 0);
        put16(&mut contextual_single_input_bytes, 314, 1);
        put16(&mut contextual_single_input_bytes, 316, 0);
        put16(&mut contextual_single_input_bytes, 318, 1);
        let contextual_single_input_face =
            TrueTypeFace::from_bytes(&contextual_single_input_bytes).unwrap();
        assert_eq!(
            contextual_single_input_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 525);
        assert_eq!(shaped[1].origin, Point::new(527, 15));
        let mut contextual_three_bytes = contextual_gpos_bytes;
        put16(&mut contextual_three_bytes, 308, 3);
        put16(&mut contextual_three_bytes, 310, 20);
        put16(&mut contextual_three_bytes, 312, 26);
        put16(&mut contextual_three_bytes, 314, 50);
        put16(&mut contextual_three_bytes, 316, 0);
        put16(&mut contextual_three_bytes, 318, 1);
        put16(&mut contextual_three_bytes, 320, 2);
        put16(&mut contextual_three_bytes, 322, 1);
        let contextual_three_face = TrueTypeFace::from_bytes(&contextual_three_bytes).unwrap();
        let mut shaped_three = [shaped[0]; 3];
        assert_eq!(
            contextual_three_face
                .shape_text_ligatures("AAA", Point::new(2, 15), 1000, &mut shaped_three)
                .unwrap(),
            3
        );
        assert_eq!(shaped_three[0].advance, 500);
        assert_eq!(shaped_three[1].origin, Point::new(502, 15));
        assert_eq!(shaped_three[2].origin, Point::new(1002, 15));
        assert_eq!(shaped_three[2].advance, 525);
        let mut contextual_three_pair_bytes = contextual_three_bytes;
        put32(&mut contextual_three_pair_bytes, 104, 120);
        put16(&mut contextual_three_pair_bytes, 314, 66);
        put16(&mut contextual_three_pair_bytes, 336, 2);
        put16(&mut contextual_three_pair_bytes, 344, 1);
        put16(&mut contextual_three_pair_bytes, 346, 26);
        put16(&mut contextual_three_pair_bytes, 348, 4);
        put16(&mut contextual_three_pair_bytes, 350, 0);
        put16(&mut contextual_three_pair_bytes, 352, 1);
        put16(&mut contextual_three_pair_bytes, 354, 18);
        put16(&mut contextual_three_pair_bytes, 362, 1);
        put16(&mut contextual_three_pair_bytes, 364, 1);
        put_i16(&mut contextual_three_pair_bytes, 366, 25);
        put16(&mut contextual_three_pair_bytes, 370, 1);
        put16(&mut contextual_three_pair_bytes, 372, 1);
        put16(&mut contextual_three_pair_bytes, 374, 1);
        let contextual_three_pair_face =
            TrueTypeFace::from_bytes(&contextual_three_pair_bytes).unwrap();
        let mut shaped_three_pair = [shaped[0]; 4];
        assert_eq!(
            contextual_three_pair_face
                .shape_text_ligatures("AAAA", Point::new(2, 15), 1000, &mut shaped_three_pair)
                .unwrap(),
            4
        );
        assert_eq!(shaped_three_pair[2].advance, 525);
        assert_eq!(shaped_three_pair[3].origin, Point::new(1527, 15));
        let mut contextual_four_bytes = contextual_gpos_bytes;
        put32(&mut contextual_four_bytes, 104, 156);
        put16(&mut contextual_four_bytes, 308, 4);
        put16(&mut contextual_four_bytes, 310, 60);
        put16(&mut contextual_four_bytes, 312, 66);
        put16(&mut contextual_four_bytes, 314, 72);
        put16(&mut contextual_four_bytes, 316, 78);
        put16(&mut contextual_four_bytes, 318, 0);
        put16(&mut contextual_four_bytes, 320, 1);
        put16(&mut contextual_four_bytes, 322, 3);
        put16(&mut contextual_four_bytes, 324, 1);
        for offset in [364, 370, 376, 382] {
            put16(&mut contextual_four_bytes, offset, 1);
            put16(&mut contextual_four_bytes, offset + 2, 1);
            put16(&mut contextual_four_bytes, offset + 4, 1);
        }
        let contextual_four_face = TrueTypeFace::from_bytes(&contextual_four_bytes).unwrap();
        let mut shaped_four = [shaped[0]; 4];
        assert_eq!(
            contextual_four_face
                .shape_text_ligatures("AAAA", Point::new(2, 15), 1000, &mut shaped_four)
                .unwrap(),
            4
        );
        assert_eq!(shaped_four[0].advance, 500);
        assert_eq!(shaped_four[1].origin, Point::new(502, 15));
        assert_eq!(shaped_four[2].origin, Point::new(1002, 15));
        assert_eq!(shaped_four[3].origin, Point::new(1502, 15));
        assert_eq!(shaped_four[3].advance, 525);
        let mut contextual_five_bytes = contextual_gpos_bytes;
        put32(&mut contextual_five_bytes, 104, 156);
        put16(&mut contextual_five_bytes, 308, 5);
        put16(&mut contextual_five_bytes, 310, 60);
        put16(&mut contextual_five_bytes, 312, 66);
        put16(&mut contextual_five_bytes, 314, 72);
        put16(&mut contextual_five_bytes, 316, 78);
        put16(&mut contextual_five_bytes, 318, 84);
        put16(&mut contextual_five_bytes, 320, 0);
        put16(&mut contextual_five_bytes, 322, 1);
        put16(&mut contextual_five_bytes, 324, 4);
        put16(&mut contextual_five_bytes, 326, 1);
        for offset in [364, 370, 376, 382, 388] {
            put16(&mut contextual_five_bytes, offset, 1);
            put16(&mut contextual_five_bytes, offset + 2, 1);
            put16(&mut contextual_five_bytes, offset + 4, 1);
        }
        let contextual_five_face = TrueTypeFace::from_bytes(&contextual_five_bytes).unwrap();
        let mut shaped_five = [shaped[0]; 5];
        assert_eq!(
            contextual_five_face
                .shape_text_ligatures("AAAAA", Point::new(2, 15), 1000, &mut shaped_five)
                .unwrap(),
            5
        );
        assert_eq!(shaped_five[4].origin, Point::new(2002, 15));
        assert_eq!(shaped_five[4].advance, 525);
        let mut single_adjustment_bytes = [0u8; 420];
        single_adjustment_bytes.copy_from_slice(&bytes);
        put32(&mut single_adjustment_bytes, 104, 156);
        put16(&mut single_adjustment_bytes, 278, 2);
        put16(&mut single_adjustment_bytes, 280, 0);
        put16(&mut single_adjustment_bytes, 282, 1);
        put16(&mut single_adjustment_bytes, 288, 2);
        put16(&mut single_adjustment_bytes, 290, 8);
        put16(&mut single_adjustment_bytes, 292, 56);
        put16(&mut single_adjustment_bytes, 296, 1);
        put16(&mut single_adjustment_bytes, 298, 0);
        put16(&mut single_adjustment_bytes, 300, 1);
        put16(&mut single_adjustment_bytes, 302, 8);
        put16(&mut single_adjustment_bytes, 304, 1);
        put16(&mut single_adjustment_bytes, 306, 10);
        put16(&mut single_adjustment_bytes, 308, 4);
        put_i16(&mut single_adjustment_bytes, 310, 50);
        put16(&mut single_adjustment_bytes, 314, 1);
        put16(&mut single_adjustment_bytes, 316, 1);
        put16(&mut single_adjustment_bytes, 318, 1);
        put16(&mut single_adjustment_bytes, 344, 1);
        put16(&mut single_adjustment_bytes, 346, 0);
        put16(&mut single_adjustment_bytes, 348, 1);
        put16(&mut single_adjustment_bytes, 350, 8);
        put16(&mut single_adjustment_bytes, 352, 1);
        put16(&mut single_adjustment_bytes, 354, 10);
        put16(&mut single_adjustment_bytes, 356, 4);
        put_i16(&mut single_adjustment_bytes, 358, 25);
        put16(&mut single_adjustment_bytes, 362, 1);
        put16(&mut single_adjustment_bytes, 364, 1);
        put16(&mut single_adjustment_bytes, 366, 1);
        let single_adjustment_face = TrueTypeFace::from_bytes(&single_adjustment_bytes).unwrap();
        assert_eq!(
            single_adjustment_face
                .shape_text_ligatures_with_ppem("AB", Point::new(2, 15), 1000, 10, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].advance, 575);
        assert_eq!(shaped[1].origin, Point::new(577, 15));
        let mut mark_bytes = bytes;
        put32(&mut mark_bytes, 104, 94);
        mark_bytes[270..274].copy_from_slice(b"mark");
        put16(&mut mark_bytes, 292, 4);
        put16(&mut mark_bytes, 294, 0);
        put16(&mut mark_bytes, 296, 1);
        put16(&mut mark_bytes, 298, 8);
        put16(&mut mark_bytes, 300, 1);
        put16(&mut mark_bytes, 302, 12);
        put16(&mut mark_bytes, 304, 20);
        put16(&mut mark_bytes, 306, 1);
        put16(&mut mark_bytes, 308, 26);
        put16(&mut mark_bytes, 310, 40);
        put16(&mut mark_bytes, 312, 1);
        put16(&mut mark_bytes, 314, 1);
        put16(&mut mark_bytes, 316, 2);
        put16(&mut mark_bytes, 320, 1);
        put16(&mut mark_bytes, 322, 1);
        put16(&mut mark_bytes, 324, 1);
        put16(&mut mark_bytes, 326, 1);
        put16(&mut mark_bytes, 328, 0);
        put16(&mut mark_bytes, 330, 6);
        put16(&mut mark_bytes, 332, 1);
        put_i16(&mut mark_bytes, 334, 0);
        put_i16(&mut mark_bytes, 336, 0);
        put16(&mut mark_bytes, 340, 1);
        put16(&mut mark_bytes, 342, 4);
        put16(&mut mark_bytes, 344, 1);
        put_i16(&mut mark_bytes, 346, 100);
        put_i16(&mut mark_bytes, 348, 200);
        let mark_face = TrueTypeFace::from_bytes(&mark_bytes).unwrap();
        assert_eq!(
            mark_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[1].origin, Point::new(602, 215));
        assert_eq!(shaped[1].advance, 500);
        let mut mark_script_bytes = mark_bytes;
        put32(&mut mark_script_bytes, 104, 156);
        let mark_script_face = TrueTypeFace::from_bytes(&mark_script_bytes).unwrap();
        assert_eq!(
            mark_script_face
                .shape_text_ligatures_with_options(
                    "AB",
                    Point::new(2, 15),
                    1000,
                    crate::graphics::TextShapingOptions {
                        script_tag: Some(*b"DFLT"),
                        language_tag: Some(*b"MISS"),
                        ppem: None,
                    },
                    &mut shaped,
                )
                .unwrap(),
            2
        );
        assert_eq!(shaped[1].origin, Point::new(502, 15));
        let mut mark_ligature_bytes = mark_bytes;
        put32(&mut mark_ligature_bytes, 104, 100);
        put16(&mut mark_ligature_bytes, 292, 5);
        put16(&mut mark_ligature_bytes, 300, 1);
        put16(&mut mark_ligature_bytes, 302, 12);
        put16(&mut mark_ligature_bytes, 304, 20);
        put16(&mut mark_ligature_bytes, 306, 1);
        put16(&mut mark_ligature_bytes, 308, 26);
        put16(&mut mark_ligature_bytes, 310, 40);
        put16(&mut mark_ligature_bytes, 340, 1);
        put16(&mut mark_ligature_bytes, 342, 4);
        put16(&mut mark_ligature_bytes, 344, 1);
        put16(&mut mark_ligature_bytes, 346, 8);
        put16(&mut mark_ligature_bytes, 352, 1);
        put_i16(&mut mark_ligature_bytes, 354, 100);
        put_i16(&mut mark_ligature_bytes, 356, 200);
        let mark_ligature_face = TrueTypeFace::from_bytes(&mark_ligature_bytes).unwrap();
        assert_eq!(
            mark_ligature_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[1].origin, Point::new(602, 215));
        let mut mark_format3_bytes = mark_bytes;
        put32(&mut mark_format3_bytes, 104, 120);
        put16(&mut mark_format3_bytes, 332, 3);
        put16(&mut mark_format3_bytes, 338, 0);
        put16(&mut mark_format3_bytes, 340, 0);
        put16(&mut mark_format3_bytes, 310, 60);
        put16(&mut mark_format3_bytes, 360, 1);
        put16(&mut mark_format3_bytes, 362, 4);
        put16(&mut mark_format3_bytes, 364, 3);
        put_i16(&mut mark_format3_bytes, 366, 100);
        put_i16(&mut mark_format3_bytes, 368, 200);
        put16(&mut mark_format3_bytes, 370, 0);
        put16(&mut mark_format3_bytes, 372, 0);
        let mark_format3_face = TrueTypeFace::from_bytes(&mark_format3_bytes).unwrap();
        assert_eq!(
            mark_format3_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[1].origin, Point::new(602, 215));
        let mut cursive_bytes = bytes;
        put32(&mut cursive_bytes, 104, 78);
        cursive_bytes[270..274].copy_from_slice(b"curs");
        put16(&mut cursive_bytes, 292, 3);
        put16(&mut cursive_bytes, 294, 0);
        put16(&mut cursive_bytes, 296, 1);
        put16(&mut cursive_bytes, 298, 8);
        put16(&mut cursive_bytes, 300, 1);
        put16(&mut cursive_bytes, 302, 16);
        put16(&mut cursive_bytes, 304, 2);
        put16(&mut cursive_bytes, 306, 0);
        put16(&mut cursive_bytes, 308, 24);
        put16(&mut cursive_bytes, 310, 30);
        put16(&mut cursive_bytes, 312, 0);
        put16(&mut cursive_bytes, 316, 1);
        put16(&mut cursive_bytes, 318, 2);
        put16(&mut cursive_bytes, 320, 1);
        put16(&mut cursive_bytes, 322, 2);
        put16(&mut cursive_bytes, 324, 1);
        put_i16(&mut cursive_bytes, 326, 100);
        put_i16(&mut cursive_bytes, 328, 200);
        put16(&mut cursive_bytes, 330, 1);
        put_i16(&mut cursive_bytes, 332, 50);
        put_i16(&mut cursive_bytes, 334, 100);
        let cursive_face = TrueTypeFace::from_bytes(&cursive_bytes).unwrap();
        assert_eq!(
            cursive_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[1].origin, Point::new(552, 115));
        assert_eq!(shaped[1].advance, 500);
        let mut mark_mark_bytes = bytes;
        put32(&mut mark_mark_bytes, 104, 94);
        mark_mark_bytes[270..274].copy_from_slice(b"mkmk");
        put16(&mut mark_mark_bytes, 292, 6);
        put16(&mut mark_mark_bytes, 294, 0);
        put16(&mut mark_mark_bytes, 296, 1);
        put16(&mut mark_mark_bytes, 298, 8);
        put16(&mut mark_mark_bytes, 300, 1);
        put16(&mut mark_mark_bytes, 302, 12);
        put16(&mut mark_mark_bytes, 304, 20);
        put16(&mut mark_mark_bytes, 306, 1);
        put16(&mut mark_mark_bytes, 308, 26);
        put16(&mut mark_mark_bytes, 310, 40);
        put16(&mut mark_mark_bytes, 312, 1);
        put16(&mut mark_mark_bytes, 314, 1);
        put16(&mut mark_mark_bytes, 316, 2);
        put16(&mut mark_mark_bytes, 320, 1);
        put16(&mut mark_mark_bytes, 322, 1);
        put16(&mut mark_mark_bytes, 324, 2);
        put16(&mut mark_mark_bytes, 326, 1);
        put16(&mut mark_mark_bytes, 328, 0);
        put16(&mut mark_mark_bytes, 330, 6);
        put16(&mut mark_mark_bytes, 332, 1);
        put_i16(&mut mark_mark_bytes, 334, 0);
        put_i16(&mut mark_mark_bytes, 336, 0);
        put16(&mut mark_mark_bytes, 340, 1);
        put16(&mut mark_mark_bytes, 342, 4);
        put16(&mut mark_mark_bytes, 344, 1);
        put_i16(&mut mark_mark_bytes, 346, 100);
        put_i16(&mut mark_mark_bytes, 348, 200);
        let mark_mark_face = TrueTypeFace::from_bytes(&mark_mark_bytes).unwrap();
        assert_eq!(
            mark_mark_face
                .shape_text_ligatures("BB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[1].origin, Point::new(402, -185));
        assert_eq!(shaped[1].advance, 500);
        let mut extension_bytes = [0u8; 520];
        extension_bytes[..bytes.len()].copy_from_slice(&bytes);
        extension_bytes[390..504].copy_from_slice(&bytes[300..414]);
        put32(&mut extension_bytes, 104, 246);
        put16(&mut extension_bytes, 292, 9);
        put16(&mut extension_bytes, 300, 1);
        put16(&mut extension_bytes, 302, 2);
        put32(&mut extension_bytes, 304, 90);
        let extension_face = TrueTypeFace::from_bytes(&extension_bytes).unwrap();
        assert_eq!(
            extension_face
                .shape_text_ligatures_with_ppem("AB", Point::new(2, 15), 1000, 10, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[0].origin, Point::new(3, 115));
        let mut extension_mark_bytes = [0u8; 520];
        extension_mark_bytes[..mark_bytes.len()].copy_from_slice(&mark_bytes);
        extension_mark_bytes[390..440].copy_from_slice(&mark_bytes[300..350]);
        put32(&mut extension_mark_bytes, 104, 246);
        put16(&mut extension_mark_bytes, 292, 9);
        put16(&mut extension_mark_bytes, 300, 1);
        put16(&mut extension_mark_bytes, 302, 4);
        put32(&mut extension_mark_bytes, 304, 90);
        let extension_mark_face = TrueTypeFace::from_bytes(&extension_mark_bytes).unwrap();
        assert_eq!(
            extension_mark_face
                .shape_text_ligatures("AB", Point::new(2, 15), 1000, &mut shaped)
                .unwrap(),
            2
        );
        assert_eq!(shaped[1].origin, Point::new(602, 215));
    }

    #[test]
    fn wovengraphics_truetype_face_reads_fvar_axis_metadata() {
        use crate::graphics::{TrueTypeFace, VariationAxis};

        let mut bytes = [0u8; 340];
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put32 = |bytes: &mut [u8], offset: usize, value: u32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        };
        let put_i32 = |bytes: &mut [u8], offset: usize, value: i32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        };
        put32(&mut bytes, 0, 0x0001_0000);
        put16(&mut bytes, 4, 7);
        for (record, (tag, offset, length)) in [
            (*b"head", 120u32, 20u32),
            (*b"maxp", 140u32, 6u32),
            (*b"cmap", 146u32, 40u32),
            (*b"hhea", 186u32, 36u32),
            (*b"hmtx", 222u32, 8u32),
            (*b"fvar", 230u32, 36u32),
            (*b"gvar", 266u32, 48u32),
        ]
        .into_iter()
        .enumerate()
        {
            let base = 12 + record * 16;
            bytes[base..base + 4].copy_from_slice(&tag);
            put32(&mut bytes, base + 8, offset);
            put32(&mut bytes, base + 12, length);
        }
        put16(&mut bytes, 138, 1000);
        put16(&mut bytes, 144, 10);
        put16(&mut bytes, 184, 2);
        put16(&mut bytes, 220, 2);
        put16(&mut bytes, 222, 500);
        put16(&mut bytes, 226, 700);
        put16(&mut bytes, 146, 0);
        put16(&mut bytes, 148, 1);
        put16(&mut bytes, 150, 3);
        put16(&mut bytes, 152, 10);
        put32(&mut bytes, 154, 12);
        put16(&mut bytes, 158, 12);
        put32(&mut bytes, 162, 28);
        put32(&mut bytes, 166, 0);
        put32(&mut bytes, 170, 1);
        put32(&mut bytes, 174, 0x1f600);
        put32(&mut bytes, 178, 0x1f600);
        put32(&mut bytes, 182, 3);
        put16(&mut bytes, 230, 1);
        put16(&mut bytes, 232, 0);
        put16(&mut bytes, 238, 16);
        put16(&mut bytes, 242, 1);
        put16(&mut bytes, 244, 20);
        bytes[246..250].copy_from_slice(b"wght");
        put_i32(&mut bytes, 250, 0x0000_8000);
        put_i32(&mut bytes, 254, 0x0001_0000);
        put_i32(&mut bytes, 258, 0x0002_0000);
        put32(&mut bytes, 266, 0x0001_0000);
        put16(&mut bytes, 270, 1);
        put16(&mut bytes, 272, 0);
        put32(&mut bytes, 274, 20);
        put16(&mut bytes, 278, 10);
        put16(&mut bytes, 280, 0);
        put32(&mut bytes, 282, 42);
        for index in 0..=3 {
            put16(&mut bytes, 286 + index * 2, 0);
        }
        for index in 4..=10 {
            put16(&mut bytes, 286 + index * 2, 1);
        }
        bytes[308..310].copy_from_slice(&[0xaa, 0xbb]);

        let face = TrueTypeFace::from_bytes(&bytes).unwrap();
        let mut axes = [VariationAxis {
            tag: [0; 4],
            minimum: 0,
            default: 0,
            maximum: 0,
        }];
        assert_eq!(face.variation_axes(&mut axes).unwrap(), 1);
        assert_eq!(axes[0].tag, *b"wght");
        assert_eq!(axes[0].minimum, 0x0000_8000);
        assert_eq!(axes[0].default, 0x0001_0000);
        assert_eq!(axes[0].maximum, 0x0002_0000);
        assert_eq!(face.glyph_variation_data(0).unwrap(), Some(&[][..]));
        assert_eq!(
            face.glyph_variation_data(3).unwrap(),
            Some(&[0xaa, 0xbb][..])
        );
    }

    #[test]
    fn wovengraphics_truetype_face_applies_simple_gvar_tuple() {
        use crate::graphics::{OutlinePoint, TrueTypeFace};

        let mut bytes = [0u8; 400];
        let put16 = |bytes: &mut [u8], offset: usize, value: u16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put_i16 = |bytes: &mut [u8], offset: usize, value: i16| {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        };
        let put32 = |bytes: &mut [u8], offset: usize, value: u32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        };
        put32(&mut bytes, 0, 0x0001_0000);
        put16(&mut bytes, 4, 8);
        for (record, (tag, offset, length)) in [
            (*b"head", 140u32, 52u32),
            (*b"maxp", 192u32, 6u32),
            (*b"cmap", 198u32, 44u32),
            (*b"hhea", 242u32, 36u32),
            (*b"hmtx", 278u32, 4u32),
            (*b"loca", 282u32, 4u32),
            (*b"glyf", 286u32, 48u32),
            (*b"gvar", 334u32, 48u32),
        ]
        .into_iter()
        .enumerate()
        {
            let base = 12 + record * 16;
            bytes[base..base + 4].copy_from_slice(&tag);
            put32(&mut bytes, base + 8, offset);
            put32(&mut bytes, base + 12, length);
        }
        put16(&mut bytes, 158, 1000);
        put16(&mut bytes, 190, 0);
        put16(&mut bytes, 196, 1);
        put16(&mut bytes, 276, 1);
        put16(&mut bytes, 278, 500);
        put16(&mut bytes, 282, 0);
        put16(&mut bytes, 284, 24);

        put16(&mut bytes, 198, 0);
        put16(&mut bytes, 200, 1);
        put16(&mut bytes, 202, 3);
        put16(&mut bytes, 204, 1);
        put32(&mut bytes, 206, 12);
        put16(&mut bytes, 210, 4);
        put16(&mut bytes, 212, 32);
        put16(&mut bytes, 214, 0);
        put16(&mut bytes, 216, 4);
        put16(&mut bytes, 218, 4);
        put16(&mut bytes, 220, 1);
        put16(&mut bytes, 222, 0);
        put16(&mut bytes, 224, 65);
        put16(&mut bytes, 226, 0xffff);
        put16(&mut bytes, 228, 0);
        put16(&mut bytes, 230, 65);
        put16(&mut bytes, 232, 0xffff);
        put_i16(&mut bytes, 234, -65);
        put_i16(&mut bytes, 236, 1);
        put16(&mut bytes, 238, 0);
        put16(&mut bytes, 240, 0);

        put_i16(&mut bytes, 286, 1);
        put_i16(&mut bytes, 288, 0);
        put_i16(&mut bytes, 290, 0);
        put_i16(&mut bytes, 292, 10);
        put_i16(&mut bytes, 294, 10);
        put16(&mut bytes, 296, 2);
        put16(&mut bytes, 298, 26);
        bytes[300..326].copy_from_slice(&[
            0xb0, 0, 0x43, 0x58, 0xb0, 0, 0xb0, 42, 0x42, 0x1b, 0xb0, 0, 0xb0, 99, 0x42, 0x59,
            0xb0, 0, 0x43, 0x21, 0xb0, 0, 0xb0, 0, 0x3f, 0x22,
        ]);
        bytes[326..329].copy_from_slice(&[0x31, 0x33, 0x27]);
        bytes[329..331].copy_from_slice(&[10, 10]);
        bytes[331] = 10;

        put32(&mut bytes, 334, 0x0001_0000);
        put16(&mut bytes, 338, 1);
        put16(&mut bytes, 340, 0);
        put32(&mut bytes, 342, 20);
        put16(&mut bytes, 346, 1);
        put16(&mut bytes, 348, 0);
        put32(&mut bytes, 350, 24);
        put16(&mut bytes, 354, 0);
        put16(&mut bytes, 356, 12);
        put16(&mut bytes, 358, 0x8001);
        put16(&mut bytes, 360, 0);
        put16(&mut bytes, 362, 10);
        put16(&mut bytes, 364, 0xc000);
        put_i16(&mut bytes, 366, 0x4000);
        put_i16(&mut bytes, 368, 0x0000);
        put_i16(&mut bytes, 370, 0x4000);
        bytes[372] = 0x06;
        bytes[373..380].copy_from_slice(&[10, 10, 10, 2, 12, 0, 0]);
        bytes[380] = 0x40;
        bytes[381] = 0x45;

        let face = TrueTypeFace::from_bytes(&bytes).unwrap();
        let mut points = [OutlinePoint {
            x: 0,
            y: 0,
            on_curve: false,
            contour_end: false,
        }; 3];
        assert_eq!(
            face.outline_points_with_variation('A', &[0x4000], &mut points)
                .unwrap(),
            3
        );
        assert_eq!([points[0].x, points[1].x, points[2].x], [10, 20, 10]);
        assert_eq!([points[0].y, points[1].y, points[2].y], [0, 0, 10]);
        assert_eq!(
            face.advance_width_with_variation('A', &[0x4000]).unwrap(),
            510
        );
        assert_eq!(
            face.advance_width_glyph_with_variation(0, &[0x4000])
                .unwrap(),
            510
        );
        assert_eq!(
            face.outline_points_with_variation('A', &[0x2000], &mut points)
                .unwrap(),
            3
        );
        assert_eq!([points[0].x, points[1].x, points[2].x], [5, 15, 5]);
        assert_eq!(
            face.advance_width_with_variation('A', &[0x2000]).unwrap(),
            505
        );
        let mut instructions = [0u8; 26];
        assert_eq!(face.glyph_hint_program('A', &mut instructions).unwrap(), 26);
        assert_eq!(
            &instructions[..26],
            &[
                0xb0, 0, 0x43, 0x58, 0xb0, 0, 0xb0, 42, 0x42, 0x1b, 0xb0, 0, 0xb0, 99, 0x42, 0x59,
                0xb0, 0, 0x43, 0x21, 0xb0, 0, 0xb0, 0, 0x3f, 0x22
            ]
        );
        let mut hint_state = crate::graphics::HintState::default();
        let mut hinted_points = points;
        assert!(matches!(
            face.execute_glyph_hints('A', &mut hinted_points, &mut hint_state),
            Err(crate::graphics::TrueTypeError::UnsupportedHinting)
        ));
        assert!(matches!(
            face.execute_glyph_hints_with_cvt('A', &mut hinted_points, &[42], &mut hint_state,),
            Err(crate::graphics::TrueTypeError::UnsupportedHinting)
        ));
        let mut storage = [0i32; 1];
        storage[0] = 1;
        assert_eq!(
            face.execute_glyph_hints_with_resources(
                'A',
                &mut hinted_points,
                &[42],
                &mut storage,
                &mut hint_state,
            )
            .unwrap(),
            26
        );
        assert_eq!(storage[0], 42);
        assert!(hint_state.round_to_grid);
        assert_eq!(hint_state.stack_depth, 0);
        assert_eq!(hinted_points[0].x, 42);
        storage[0] = 0;
        hinted_points = points;
        hint_state = crate::graphics::HintState::default();
        assert_eq!(
            face.execute_glyph_hints_with_resources(
                'A',
                &mut hinted_points,
                &[42],
                &mut storage,
                &mut hint_state,
            )
            .unwrap(),
            26
        );
        assert_eq!(storage[0], 99);
        assert_eq!(hinted_points[0].x, 42);
        let mut pixels = [0u32; 20 * 20];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 20, 20).unwrap();
            let mut renderer = crate::graphics::Renderer::new(&mut canvas);
            assert_eq!(
                face.render_shaped_text_with_variation(
                    "A",
                    crate::graphics::Point::new(2, 15),
                    100,
                    &[0x4000],
                    crate::graphics::Color::WHITE,
                    &mut renderer,
                )
                .unwrap(),
                1
            );
        }
        assert!(pixels.iter().any(|pixel| *pixel != 0));
    }

    #[test]
    fn wovengraphics_scaled_font_is_bounded_and_rasterizes_blocks() {
        use crate::graphics::{Color, Font, Renderer};

        assert_eq!(
            Font::DEFAULT.measure_scaled("A", 0).width,
            crate::font::WIDTH
        );
        assert_eq!(
            Font::DEFAULT.measure_scaled("A", 100).width,
            crate::font::text_width("A") * 16
        );
        let mut pixels = [0u32; 64];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 8, 8).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            renderer.text_scaled(0, 0, "A", Color::WHITE, 2);
        }
        assert_eq!(pixels[2], Color::WHITE.packed());
        assert_eq!(pixels[3], Color::WHITE.packed());
        assert_eq!(pixels[10], Color::WHITE.packed());
        assert_eq!(pixels[11], Color::WHITE.packed());
    }

    #[test]
    fn wovengraphics_paths_stroke_and_close_with_bounded_storage() {
        use crate::graphics::{Color, Path, Point, Renderer};

        let mut path = Path::new();
        assert!(path.move_to(Point::new(-1, 1)));
        assert!(path.line_to(Point::new(3, 1)));
        assert!(path.line_to(Point::new(3, 3)));
        assert!(path.close());
        assert!(!path.line_to(Point::new(0, 0)));
        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            Renderer::new(&mut canvas).stroke_path(&path, Color::TITLE);
        }
        assert_eq!(pixels[5], Color::TITLE.packed());
        assert_eq!(pixels[3 * 4 + 3], Color::TITLE.packed());
    }

    #[test]
    fn wovengraphics_stroke_path_clips_extreme_endpoints() {
        use crate::graphics::{Color, Path, Point, Renderer};

        let mut path = Path::new();
        assert!(path.move_to(Point::new(i32::MIN, 1)));
        assert!(path.line_to(Point::new(i32::MAX, 1)));
        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            Renderer::new(&mut canvas).stroke_path(&path, Color::TITLE);
        }
        assert_eq!(&pixels[4..8], &[Color::TITLE.packed(); 4]);
    }

    #[test]
    fn wovengraphics_quadratic_path_flattens_deterministically_and_is_atomic() {
        use crate::graphics::{Path, Point};

        let mut path = Path::new();
        assert!(path.move_to(Point::new(0, 0)));
        assert!(path.quadratic_to(Point::new(8, 16), Point::new(16, 0)));
        for index in 0..57 {
            assert!(path.line_to(Point::new(index, 0)));
        }
        assert!(!path.quadratic_to(Point::new(24, 16), Point::new(32, 0)));
    }

    #[test]
    fn wovengraphics_fill_path_clips_closed_polygon() {
        use crate::graphics::{Color, Path, Point, Renderer};

        let mut path = Path::new();
        assert!(path.move_to(Point::new(-2, 0)));
        assert!(path.line_to(Point::new(5, 0)));
        assert!(path.line_to(Point::new(1, 5)));
        assert!(path.close());
        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            assert!(Renderer::new(&mut canvas).fill_path(&path, Color::TITLE));
        }
        assert!(pixels.iter().any(|pixel| *pixel == Color::TITLE.packed()));
        assert_eq!(pixels.len(), 16);
    }

    #[test]
    fn wovengraphics_fill_path_widens_extreme_intersections() {
        use crate::graphics::{Color, Path, Point, Renderer};

        let mut path = Path::new();
        assert!(path.move_to(Point::new(i32::MIN, i32::MIN)));
        assert!(path.line_to(Point::new(i32::MAX, i32::MAX)));
        assert!(path.line_to(Point::new(i32::MAX, i32::MIN)));
        assert!(path.close());
        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            assert!(Renderer::new(&mut canvas).fill_path(&path, Color::TITLE));
        }
        assert!(pixels.iter().any(|pixel| *pixel == Color::TITLE.packed()));
    }

    #[test]
    fn wovengraphics_fill_path_blends_source_over_canvas() {
        use crate::graphics::{Color, Path, Point, Renderer};

        let mut path = Path::new();
        assert!(path.move_to(Point::new(0, 0)));
        assert!(path.line_to(Point::new(2, 0)));
        assert!(path.line_to(Point::new(0, 2)));
        assert!(path.close());
        let mut pixels = [Color::BLACK.packed(); 4];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 2, 2).unwrap();
            assert!(Renderer::new(&mut canvas).fill_path_blended(&path, Color::WHITE, 128));
        }
        assert_eq!(
            pixels[0],
            blend(Color::WHITE.packed(), Color::BLACK.packed(), 128)
        );
    }

    #[test]
    fn wovengraphics_antialiased_path_uses_partial_coverage() {
        use crate::graphics::{Color, Path, Point, Renderer};

        let mut path = Path::new();
        assert!(path.move_to(Point::new(0, 0)));
        assert!(path.line_to(Point::new(4, 0)));
        assert!(path.line_to(Point::new(0, 4)));
        assert!(path.close());
        let mut pixels = [Color::BLACK.packed(); 25];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 5, 5).unwrap();
            assert!(Renderer::new(&mut canvas).fill_path_antialiased(&path, Color::WHITE));
        }
        assert!(
            pixels
                .iter()
                .any(|pixel| *pixel != Color::BLACK.packed() && *pixel != Color::WHITE.packed())
        );
    }

    #[test]
    fn wovengraphics_compound_path_uses_even_odd_holes() {
        use crate::graphics::{Color, CompoundPath, Path, Point, Renderer};

        let mut outer = Path::new();
        assert!(outer.move_to(Point::new(0, 0)));
        assert!(outer.line_to(Point::new(5, 0)));
        assert!(outer.line_to(Point::new(5, 5)));
        assert!(outer.line_to(Point::new(0, 5)));
        assert!(outer.close());
        let mut inner = Path::new();
        assert!(inner.move_to(Point::new(1, 1)));
        assert!(inner.line_to(Point::new(4, 1)));
        assert!(inner.line_to(Point::new(4, 4)));
        assert!(inner.line_to(Point::new(1, 4)));
        assert!(inner.close());
        let mut compound = CompoundPath::new();
        assert!(compound.push(outer));
        assert!(compound.push(inner));
        let mut pixels = [Color::BLACK.packed(); 25];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 5, 5).unwrap();
            assert!(Renderer::new(&mut canvas).fill_compound_path(&compound, Color::WHITE));
        }
        assert_eq!(pixels[0], Color::WHITE.packed());
        assert_eq!(pixels[2 * 5 + 2], Color::BLACK.packed());
    }

    #[test]
    fn wovengraphics_text_box_wraps_and_clips() {
        use crate::graphics::{Color, Rect, Renderer};

        let mut pixels = [0u32; 144];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 8, 18).unwrap();
            let lines = Renderer::new(&mut canvas).text_box(
                Rect::new(0, 0, 8, 18),
                "ABCD",
                Color::WHITE,
                1,
            );
            assert_eq!(lines, 3);
        }
        assert!(pixels.iter().any(|pixel| *pixel == Color::WHITE.packed()));
    }

    #[test]
    fn wovengraphics_text_layout_bounds_extreme_rectangles() {
        use crate::graphics::{Color, Rect, Renderer};

        let mut pixels = [0u32; 64];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 8, 8).unwrap();
            let mut renderer = Renderer::new(&mut canvas);
            assert_eq!(
                renderer.text_box(Rect::new(0, 0, usize::MAX, 8), "A", Color::WHITE, 1),
                1
            );
            renderer.text_centered(Rect::new(0, 0, usize::MAX, usize::MAX), "A", Color::TITLE);
        }
        assert!(pixels.iter().any(|pixel| *pixel != 0));
    }

    #[test]
    fn wovengraphics_rounded_rect_keeps_corners_clipped_and_blended() {
        use crate::graphics::{Color, Rect, Renderer};

        let mut pixels = [Color::BLACK.packed(); 25];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 5, 5).unwrap();
            Renderer::new(&mut canvas).rounded_rect_blended(
                Rect::new(-1, -1, 6, 6),
                2,
                Color::WHITE,
                128,
            );
        }
        assert_eq!(
            pixels[0],
            blend(Color::WHITE.packed(), Color::BLACK.packed(), 128)
        );
        assert_eq!(
            pixels[2],
            blend(Color::WHITE.packed(), Color::BLACK.packed(), 128)
        );
        assert_eq!(
            pixels[12],
            blend(Color::WHITE.packed(), Color::BLACK.packed(), 128)
        );
    }

    #[test]
    fn wovengraphics_rounded_rect_bounds_extreme_radius() {
        use crate::graphics::{Color, Rect, Renderer};

        let mut pixels = [0u32; 16];
        {
            let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
            Renderer::new(&mut canvas).rounded_rect(
                Rect::new(0, 0, 4, 4),
                usize::MAX,
                Color::TITLE,
            );
        }
        assert!(pixels.iter().any(|pixel| *pixel == Color::TITLE.packed()));
    }

    use super::*;

    #[test]
    fn kernel_signed_thread_statuses_decode_without_sentinel_aliasing() {
        for status in [-1, -2, i32::MIN, i32::MAX, 0, 37] {
            assert_eq!(
                thread::decode_join(kernel_exit_status::encode(status)),
                thread::Join::Exited(status)
            );
        }
        assert_eq!(thread::decode_join(syscall::ERROR), thread::Join::Invalid);
        assert_eq!(
            thread::decode_join(syscall::WOULD_BLOCK),
            thread::Join::Running
        );
    }

    #[test]
    fn kernel_signed_process_statuses_are_successful_wait_payloads() {
        for status in [-1, -2, i32::MIN, i32::MAX, 0, 37] {
            assert_eq!(
                check(kernel_exit_status::encode(status)),
                Ok(u64::from(status as u32))
            );
        }
        assert_eq!(check(syscall::ERROR), Err(Error::Failed));
        assert_eq!(check(syscall::WOULD_BLOCK), Err(Error::WouldBlock));
    }

    #[test]
    fn blend_is_exact_at_both_ends_and_rounds_to_nearest() {
        let (over, under) = (rgb(200, 100, 0), rgb(0, 50, 255));
        assert_eq!(blend(over, under, 0), under);
        assert_eq!(blend(over, under, 255), over);
        // Per channel: (over * a + under * (255 - a) + 127) / 255.
        let mid = blend(over, under, 128);
        assert_eq!(mid >> 16, (200 * 128 + 127) / 255);
        assert_eq!(mid >> 8 & 0xff, (100 * 128 + 50 * 127 + 127) / 255);
        assert_eq!(mid & 0xff, (255 * 127 + 127) / 255);
        for opacity in [1, 77, 128, 254] {
            assert_eq!(blend(over, over, opacity), over, "a colour over itself");
        }
    }

    #[test]
    fn canvas_needs_a_buffer_large_enough_for_its_rectangle() {
        let mut pixels = [0u32; 12];
        assert!(Canvas::new(&mut pixels, 0, 0, 4, 3).is_some());
        assert!(Canvas::new(&mut pixels, 0, 0, 4, 4).is_none());
        assert!(
            Canvas::new(&mut pixels, 0, 0, usize::MAX, 2).is_none(),
            "overflow refused"
        );
        assert!(Canvas::new(&mut pixels, i32::MAX, 0, 2, 1).is_none());
        assert!(Canvas::new(&mut pixels, 0, i32::MAX, 1, 2).is_none());
        assert!(Canvas::new(&mut pixels, i32::MIN, 0, i32::MAX as usize + 1, 1).is_none());
        let canvas = Canvas::new(&mut pixels, 0, 0, 2, 2).unwrap();
        assert_eq!(canvas.pixels().len(), 4, "only the rectangle is exposed");
    }

    #[test]
    fn depth_buffer_rejects_unrepresentable_coordinates() {
        use crate::graphics::DepthBuffer;

        let mut depths = [0u16; 4];
        assert!(DepthBuffer::new(&mut depths, i32::MAX, 0, 2, 1).is_none());
        assert!(DepthBuffer::new(&mut depths, 0, i32::MAX, 1, 2).is_none());
    }

    #[test]
    fn canvas_uses_caller_coordinates_and_clips_to_its_rectangle() {
        let mut pixels = [0u32; 16];
        let mut canvas = Canvas::new(&mut pixels, 100, 50, 4, 4).unwrap();
        canvas.set(101, 52, 0xffab_cdef);
        assert_eq!(canvas.get(101, 52), Some(0x00ab_cdef), "high byte cleared");
        // Outside the rectangle: ignored on write, absent on read.
        for (x, y) in [
            (99, 50),
            (104, 50),
            (100, 49),
            (100, 54),
            (i32::MIN, i32::MAX),
        ] {
            canvas.set(x, y, 1);
            assert_eq!(canvas.get(x, y), None);
        }
        assert_eq!(
            canvas.pixels().iter().filter(|pixel| **pixel != 0).count(),
            1
        );
        assert_eq!(
            pixels[2 * 4 + 1],
            0x00ab_cdef,
            "row-major from the top-left"
        );
    }

    #[test]
    fn fill_and_blend_rect_touch_only_the_overlap() {
        let mut pixels = [0u32; 16];
        let mut canvas = Canvas::new(&mut pixels, 10, 10, 4, 4).unwrap();
        canvas.fill_rect(0, 0, 100, 100, rgb(9, 9, 9));
        assert!(canvas.pixels().iter().all(|pixel| *pixel == rgb(9, 9, 9)));
        // Hanging off the bottom-right corner: a 2x2 overlap.
        canvas.fill_rect(12, 12, 10, 10, rgb(1, 2, 3));
        let changed = canvas
            .pixels()
            .iter()
            .filter(|pixel| **pixel == rgb(1, 2, 3))
            .count();
        assert_eq!(changed, 4);
        assert_eq!(canvas.get(11, 11), Some(rgb(9, 9, 9)));
        // Entirely outside, or empty: nothing.
        canvas.fill_rect(20, 20, 5, 5, 0);
        canvas.fill_rect(10, 10, 0, 5, 0);
        assert!(canvas.pixels().iter().all(|pixel| *pixel != 0));
        // Blending is the formula over what is there.
        canvas.blend_rect(10, 10, 1, 1, rgb(255, 255, 255), 100);
        assert_eq!(
            canvas.get(10, 10),
            Some(blend(rgb(255, 255, 255), rgb(9, 9, 9), 100))
        );
        assert_eq!(canvas.get(11, 10), Some(rgb(9, 9, 9)));
    }

    #[test]
    fn shade_passes_caller_coordinates() {
        let mut pixels = [0u32; 6];
        let mut canvas = Canvas::new(&mut pixels, -1, 7, 3, 2).unwrap();
        canvas.shade(|x, y| ((x + 1) as u32) << 8 | y as u32);
        assert_eq!(canvas.get(-1, 7), Some(7));
        assert_eq!(canvas.get(1, 8), Some(2 << 8 | 8));
    }

    #[test]
    fn draw_text_renders_the_kernel_font_and_honours_newlines() {
        let (width, height) = (16usize, 20usize);
        let mut pixels = vec![0u32; width * height];
        let mut canvas = Canvas::new(&mut pixels, 0, 0, width, height).unwrap();
        canvas.draw_text(1, 2, "Ab\nc", 7);
        let lit = |canvas: &Canvas<'_>, x: i32, y: i32| canvas.get(x, y) == Some(7);
        for (character, x0, y0) in [
            ('A', 1, 2),
            ('b', 1 + font::ADVANCE, 2),
            ('c', 1, 2 + font::LINE_HEIGHT),
        ] {
            for (row, bits) in font::glyph(character).iter().enumerate() {
                for column in 0..font::WIDTH as i32 {
                    let set = bits & (1 << (font::WIDTH as i32 - 1 - column)) != 0;
                    assert_eq!(
                        lit(&canvas, x0 + column, y0 + row as i32),
                        set,
                        "{character} ({column}, {row})"
                    );
                }
            }
        }
        // The blank column between glyphs stays clear.
        assert!((2..2 + font::HEIGHT as i32).all(|y| !lit(&canvas, 1 + font::WIDTH as i32, y)));
    }

    #[test]
    fn canvas_draw_text_bounds_extreme_origins() {
        let mut pixels = [0u32; 16];
        let mut canvas = Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
        canvas.draw_text(i32::MAX, i32::MAX, "A\nB", 7);
        assert_eq!(pixels, [0; 16]);
    }

    #[test]
    fn region_packing_matches_the_kernel_rectangle_layout() {
        let region = Region {
            x: 0x0102,
            y: 0x0304,
            width: 0x0506,
            height: 0x0708,
        };
        assert_eq!(region.pack(), 0x0708_0506_0304_0102);
    }

    #[test]
    fn compositor_info_unpacks_the_kernel_packing() {
        // The kernel packs width | height << 24 | scale << 48.
        let packed = 640 | (400 << 24) | (2 << 48);
        assert_eq!(
            CompositorInfo::unpack(packed),
            CompositorInfo {
                width: 640,
                height: 400,
                scale: 2
            }
        );
        let largest = 1920 | (1200 << 24) | (4 << 48);
        assert_eq!(
            CompositorInfo::unpack(largest),
            CompositorInfo {
                width: 1920,
                height: 1200,
                scale: 4
            }
        );
    }

    #[test]
    fn blend_destination_packs_signed_coordinates_and_opacity() {
        assert_eq!(
            pack_blend_destination(5, 7, 0x80),
            5 | (7 << 24) | (0x80 << 56)
        );
        // Negative coordinates are 24-bit two's complement; the reserved
        // byte stays zero.
        let packed = pack_blend_destination(-1, -2, 255);
        assert_eq!(packed & 0xff_ffff, 0xff_ffff);
        assert_eq!((packed >> 24) & 0xff_ffff, 0xff_fffe);
        assert_eq!((packed >> 48) & 0xff, 0);
        assert_eq!(packed >> 56, 255);
    }

    #[test]
    fn spring_eases_to_its_target_without_overshoot() {
        for (from, to) in [(0, 255), (255, 0), (-40, 300)] {
            let mut spring = Spring::new(from);
            spring.set_target(to);
            let mut previous = from;
            let mut frames = 0;
            let mut intermediate = 0;
            while !spring.is_settled() {
                spring.step(1);
                frames += 1;
                let value = spring.value();
                // Monotone toward the target, never past it.
                if to > from {
                    assert!(
                        value >= previous && value <= to,
                        "{from}->{to} frame {frames}: {value}"
                    );
                } else {
                    assert!(
                        value <= previous && value >= to,
                        "{from}->{to} frame {frames}: {value}"
                    );
                }
                if value != from && value != to {
                    intermediate += 1;
                }
                previous = value;
                assert!(frames < 60, "{from}->{to} settles");
            }
            assert_eq!(spring.value(), to);
            assert!(intermediate >= 10, "{from}->{to} is a motion, not a jump");
        }
    }

    #[test]
    fn spring_steps_compose_and_are_deterministic() {
        let mut one_by_one = Spring::new(0);
        let mut batched = Spring::new(0);
        one_by_one.set_target(255);
        batched.set_target(255);
        for _ in 0..7 {
            one_by_one.step(1);
        }
        batched.step(7);
        assert_eq!(one_by_one, batched, "skipped frames advance like real ones");
        let mut again = Spring::new(0);
        again.set_target(255);
        again.step(7);
        assert_eq!(again, batched);
        // A spring at rest stays at rest, and retargeting starts from where
        // it is.
        let mut resting = Spring::new(12);
        resting.step(100);
        assert!(resting.is_settled() && resting.value() == 12);
        batched.set_target(0);
        let before = batched.value();
        batched.step(1);
        assert!(batched.value() <= before);
    }

    #[test]
    fn tiling_covers_the_area_exactly_without_overlap() {
        use layout::{Frame, MAX_TILES, tile};
        let area = Frame::new(0, 0, 640, 376);
        assert_eq!(tile(0, area).1, 0);
        assert_eq!(tile(1, area).0[0], area, "one window takes everything");
        let (two, _) = tile(2, area);
        assert_eq!(two[0], Frame::new(0, 0, 320, 376), "master on the left");
        assert_eq!(two[1], Frame::new(320, 0, 320, 376), "stack on the right");
        for count in 1..=MAX_TILES + 2 {
            let (frames, placed) = tile(count, Frame::new(3, 5, 641, 377));
            assert_eq!(placed, count.min(MAX_TILES));
            let frames = &frames[..placed];
            let area: i64 = frames
                .iter()
                .map(|f| i64::from(f.width) * i64::from(f.height))
                .sum();
            assert_eq!(area, 641 * 377, "{count} windows cover the area");
            for (i, a) in frames.iter().enumerate() {
                assert!(a.width > 0 && a.height > 0);
                for b in &frames[i + 1..] {
                    let overlap = a.x < b.x + b.width
                        && b.x < a.x + a.width
                        && a.y < b.y + b.height
                        && b.y < a.y + a.height;
                    assert!(!overlap, "{count} windows: {a:?} and {b:?} overlap");
                }
            }
        }
        assert_eq!(
            Frame::new(0, 0, 320, 376).client(14),
            Frame::new(0, 14, 320, 362)
        );
    }

    #[test]
    fn halves_split_the_area() {
        use layout::{Frame, half};
        let area = Frame::new(0, 0, 641, 376);
        assert_eq!(half(area, false), Frame::new(0, 0, 320, 376));
        assert_eq!(half(area, true), Frame::new(320, 0, 321, 376));
    }

    #[test]
    fn pin_hash_is_pbkdf2_hmac_sha256() {
        use pbkdf2::pbkdf2_hmac;
        use sha2::{Digest, Sha256};

        // Matches a reference PBKDF2-HMAC-SHA256 computation at the same
        // iteration count, not a one-round SHA-256 (and not some other
        // count): this is what would silently regress to plain SHA-256, or
        // to an accidentally-reduced work factor, if someone "simplified"
        // `hash_pin` later.
        let mut reference = [0u8; 32];
        pbkdf2_hmac::<Sha256>(b"bc", b"a", accounts::PBKDF2_ITERATIONS, &mut reference);
        assert_eq!(accounts::hash_pin(b"a", b"bc"), reference);
        let mut one_round = Sha256::new();
        one_round.update(b"a");
        one_round.update(b"bc");
        let one_round: [u8; 32] = one_round.finalize().into();
        assert_ne!(
            accounts::hash_pin(b"a", b"bc"),
            one_round,
            "work factor was not applied"
        );

        // Deterministic, and distinguishes both inputs.
        assert_eq!(
            accounts::hash_pin(b"a", b"bc"),
            accounts::hash_pin(b"a", b"bc")
        );
        assert_ne!(
            accounts::hash_pin(b"a", b"bc"),
            accounts::hash_pin(b"a", b"bd")
        );
        assert_ne!(
            accounts::hash_pin(b"a", b"bc"),
            accounts::hash_pin(b"z", b"bc")
        );

        // Empty salt and PIN still hash deterministically to something
        // distinct from a non-empty input, rather than panicking or
        // short-circuiting to a fixed value.
        assert_eq!(accounts::hash_pin(b"", b""), accounts::hash_pin(b"", b""));
        assert_ne!(
            accounts::hash_pin(b"", b""),
            accounts::hash_pin(b"a", b"bc")
        );
    }

    #[test]
    fn accounts_parse_and_verify() {
        let salt = b"\x01\x02\xfe\xff";
        let hash = accounts::hash_pin(salt, b"2468");
        let mut line = std::string::String::from("ada:0102feff:");
        for byte in hash {
            line.push_str(&std::format!("{byte:02x}"));
        }
        let account = accounts::Account::parse(line.as_bytes()).expect("valid line");
        assert_eq!(account.name(), b"ada");
        assert!(account.verify(b"2468"));
        assert!(!account.verify(b"2469") && !account.verify(b"") && !account.verify(b"24680"));
        // A trailing CR is tolerated; malformed lines are refused.
        assert!(accounts::Account::parse(std::format!("{line}\r").as_bytes()).is_some());
        let refused = [
            "ada:0102feff",                // no hash
            ":0102feff:00",                // no name
            "a:b:c:d",                     // too many fields
            "ada::00",                     // empty salt
            "ada:010:00",                  // odd salt hex
            "ada:0102feff:zz",             // not hex
            "this-name-is-too-long:00:00", // name over 16
        ];
        for bad in refused {
            assert!(accounts::Account::parse(bad.as_bytes()).is_none(), "{bad}");
        }
        let short_hash = std::format!("ada:0102feff:{}", &line[13..line.len() - 2]);
        assert!(
            accounts::Account::parse(short_hash.as_bytes()).is_none(),
            "31-byte hash"
        );
    }

    #[test]
    fn surface_owner_reads_the_high_half_of_a_token() {
        // slot | generation << 8 | owner << 32, as the kernel packs it.
        let token = 3 | (0x00ab_cdef << 8) | (4242u64 << 32);
        assert_eq!(surface_owner(token), process::Id(4242));
        assert_eq!(surface_owner(0xffff_ffff), process::Id(0));
    }
}
