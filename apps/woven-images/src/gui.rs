use libwoven::layout::Frame;
use libwoven::media::ThumbnailCache;
use libwoven::ui::{Button, Label, Theme};
use libwoven::{rgb, Canvas, GraphicsSurface};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 220;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const POINTER_MARKER: u32 = rgb(255, 80, 80);
const BUTTON: u32 = rgb(45, 65, 90);
const BUTTON_X: i32 = 250;
const BUTTON_Y: i32 = 170;
const BUTTON_WIDTH: usize = 80;
const BUTTON_HEIGHT: usize = 20;
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 196;
const MAX_ENTRIES: usize = 128;
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;
const WALLPAPER_WIDTH: usize = 160;
const WALLPAPER_HEIGHT: usize = 90;
const WALLPAPER_CACHE_KEY: u64 = 0x574f5645_4e484154;
// Leave one sentinel byte because load_vfs_asset reserves the full buffer
// length to distinguish an exact-size asset from a truncated read.
const WALLPAPER_BYTES: usize = 54 + ((WALLPAPER_WIDTH * 3 + 3) & !3) * WALLPAPER_HEIGHT + 1;

struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: the images GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer = PixelBuffer(core::cell::UnsafeCell::new(
    [0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT],
));

struct WallpaperBytes(core::cell::UnsafeCell<[u8; WALLPAPER_BYTES]>);
unsafe impl Sync for WallpaperBytes {}
static WALLPAPER_DATA: WallpaperBytes =
    WallpaperBytes(core::cell::UnsafeCell::new([0; WALLPAPER_BYTES]));

struct WallpaperPixels(core::cell::UnsafeCell<[u32; WALLPAPER_WIDTH * WALLPAPER_HEIGHT]>);
unsafe impl Sync for WallpaperPixels {}
static WALLPAPER_PIXELS: WallpaperPixels = WallpaperPixels(core::cell::UnsafeCell::new(
    [0; WALLPAPER_WIDTH * WALLPAPER_HEIGHT],
));

struct WallpaperThumbnailCache(core::cell::UnsafeCell<ThumbnailCache<1>>);
unsafe impl Sync for WallpaperThumbnailCache {}
static WALLPAPER_THUMBNAILS: WallpaperThumbnailCache =
    WallpaperThumbnailCache(core::cell::UnsafeCell::new(ThumbnailCache::new()));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenImages: failure\n");
    libwoven::exit(code)
}

fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn image_count() -> usize {
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut count = 0usize;
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir("/mnt", index, &mut name) {
            Ok((_, directory)) if !directory => count += 1,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    count
}

fn load_wallpaper() -> Option<(&'static [u32], usize, usize)> {
    let cache: &'static mut ThumbnailCache<1> = unsafe { &mut *WALLPAPER_THUMBNAILS.0.get() };
    if cache.get(WALLPAPER_CACHE_KEY).is_some() {
        let thumbnail = cache.get(WALLPAPER_CACHE_KEY)?;
        return Some((
            &thumbnail.pixels,
            usize::from(thumbnail.width),
            usize::from(thumbnail.height),
        ));
    }
    let bytes = unsafe { &mut *WALLPAPER_DATA.0.get() };
    let pixels = unsafe { &mut *WALLPAPER_PIXELS.0.get() };
    let (length, metadata) =
        match libwoven::media::load_vfs_asset("/share/wallpapers/wovenhat-gradient.bmp", bytes) {
            Ok(asset) => asset,
            Err(libwoven::media::VfsAssetError::Open) => {
                let _ = libwoven::print("WovenImages: wallpaper open denied\n");
                return None;
            }
            Err(libwoven::media::VfsAssetError::Read) => {
                let _ = libwoven::print("WovenImages: wallpaper read failed\n");
                return None;
            }
            Err(libwoven::media::VfsAssetError::TooLarge) => {
                let _ = libwoven::print("WovenImages: wallpaper too large\n");
                return None;
            }
            Err(libwoven::media::VfsAssetError::InvalidImage(_)) => {
                let _ = libwoven::print("WovenImages: wallpaper header invalid\n");
                return None;
            }
        };
    if metadata.width as usize != WALLPAPER_WIDTH || metadata.height as usize != WALLPAPER_HEIGHT {
        return None;
    }
    libwoven::media::decode_bmp_rgba(&bytes[..length], pixels).ok()?;
    let mut thumbnail = [0u32; libwoven::media::THUMBNAIL_PIXELS];
    for y in 0..libwoven::media::THUMBNAIL_SIDE {
        let source_y = y * WALLPAPER_HEIGHT / libwoven::media::THUMBNAIL_SIDE;
        for x in 0..libwoven::media::THUMBNAIL_SIDE {
            let source_x = x * WALLPAPER_WIDTH / libwoven::media::THUMBNAIL_SIDE;
            thumbnail[y * libwoven::media::THUMBNAIL_SIDE + x] =
                pixels[source_y * WALLPAPER_WIDTH + source_x];
        }
    }
    if !cache.insert(
        WALLPAPER_CACHE_KEY,
        libwoven::media::THUMBNAIL_SIDE,
        libwoven::media::THUMBNAIL_SIDE,
        &thumbnail,
    ) {
        return None;
    }
    let thumbnail = cache.get(WALLPAPER_CACHE_KEY)?;
    Some((
        &thumbnail.pixels,
        usize::from(thumbnail.width),
        usize::from(thumbnail.height),
    ))
}

fn draw_thumbnail(canvas: &mut Canvas<'_>, image: Option<(&[u32], usize, usize)>) {
    let Some((pixels, width, height)) = image else {
        canvas.draw_text(182, 72, "Wallpaper unavailable", TEXT);
        return;
    };
    let x0 = 182i32;
    let y0 = 70i32;
    let target_width = 140usize;
    let target_height = 78usize;
    canvas.fill_rect(x0 - 2, y0 - 2, target_width + 4, target_height + 4, ACCENT);
    for y in 0..target_height {
        let source_y = y * height / target_height;
        for x in 0..target_width {
            let source_x = x * width / target_width;
            canvas.set(
                x0 + x as i32,
                y0 + y as i32,
                pixels[source_y * width + source_x],
            );
        }
    }
}

fn draw_images(
    canvas: &mut Canvas<'_>,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    refresh_count: u32,
    refresh: &Button<'_>,
    wallpaper: Option<(&[u32], usize, usize)>,
) {
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, PANEL);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, ACCENT);
    let theme = Theme {
        surface: PANEL,
        accent: ACCENT,
        text: TEXT,
        disabled: BUTTON,
        focus: ACCENT,
    };
    Label::new(
        Frame::new(PANEL_X + 10, PANEL_Y + 8, 180, 16),
        "WovenImages",
    )
    .draw(canvas, theme);
    Label::new(
        Frame::new(PANEL_X + 10, PANEL_Y + 36, 180, 16),
        "Image workspace",
    )
    .draw(
        canvas,
        Theme {
            text: ACCENT,
            ..theme
        },
    );
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 54, "/mnt", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 88, "Image files", ACCENT);
    canvas.draw_text(182, 54, "Licensed wallpaper", ACCENT);
    draw_thumbnail(canvas, wallpaper);
    let mut digits = [0u8; 20];
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 106, "Entries ", TEXT);
    let count_text = libwoven::format_u64(image_count() as u64, &mut digits);
    if let Ok(text) = core::str::from_utf8(count_text) {
        canvas.draw_text(PANEL_X + 58, PANEL_Y + 106, text, ACCENT);
    }
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 138, "PNG  JPEG  GIF  BMP", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 160, "CLI: --list PATH", TEXT);
    let mut refresh_digits = [0u8; 20];
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 178, "Refreshes ", TEXT);
    let refresh_text = libwoven::format_u64(u64::from(refresh_count), &mut refresh_digits);
    if let Ok(text) = core::str::from_utf8(refresh_text) {
        canvas.draw_text(PANEL_X + 70, PANEL_Y + 178, text, ACCENT);
    }
    refresh.draw(canvas, theme);
    if let Some((x, y)) = pointer {
        canvas.fill_rect(
            i32::from(x).saturating_sub(2),
            i32::from(y).saturating_sub(2),
            5,
            5,
            if pressed { TEXT } else { POINTER_MARKER },
        );
    }
}

fn render(
    window: &GraphicsSurface,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    refresh_count: u32,
    refresh: &Button<'_>,
) -> bool {
    let pixels = unsafe { &mut *CANVAS_PIXELS.0.get() };
    let Some(mut canvas) = Canvas::new(
        pixels,
        0,
        0,
        window.width() as usize,
        window.height() as usize,
    ) else {
        return false;
    };
    let wallpaper = load_wallpaper();
    draw_images(
        &mut canvas,
        pointer,
        pressed,
        refresh_count,
        refresh,
        wallpaper,
    );
    window.write_canvas(&canvas).is_ok()
}

fn send_resize(window: &GraphicsSurface, shell: libwoven::process::Id) -> bool {
    let mut response = [0u8; 16];
    response[..4].copy_from_slice(b"WRSZ");
    response[4..12].copy_from_slice(&window.handle().to_le_bytes());
    response[12..14].copy_from_slice(&(window.width() as u16).to_le_bytes());
    response[14..].copy_from_slice(&(window.height() as u16).to_le_bytes());
    libwoven::message_send(shell, &response).is_ok()
}

/// Open a bounded image-workspace preview through WovenShell. Image decoding
/// and input-driven selection remain explicit follow-up capabilities.
pub fn run(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };
    let mut window = GraphicsSurface::create(WIDTH, HEIGHT).unwrap_or_else(|_| fail(1));
    let mut refresh = Button::new(
        Frame::new(
            BUTTON_X,
            BUTTON_Y,
            BUTTON_WIDTH as i32,
            BUTTON_HEIGHT as i32,
        ),
        "Refresh",
    );
    match load_wallpaper() {
        Some((_, width, height)) => {
            let _ = libwoven::print("WovenImages: wallpaper decoded 160x90\n");
            if width != WALLPAPER_WIDTH || height != WALLPAPER_HEIGHT {
                fail(9)
            }
        }
        None => {
            let _ = libwoven::print("WovenImages: wallpaper unavailable\n");
        }
    }
    if !render(&window, None, false, 0, &refresh) {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Images";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err()
        || libwoven::message_send(shell, b"WCAP\x01").is_err()
    {
        fail(5)
    }
    let _ = libwoven::print("WovenImages: window opened\n");
    let mut message = [0u8; 64];
    let mut pointer = None;
    let mut pointer_pressed = false;
    let mut refresh_count = 0u32;
    loop {
        if libwoven::wait_any(libwoven::wait::MESSAGE, None).is_err() {
            fail(6)
        }
        if let Ok((length, sender)) = libwoven::message_receive(&mut message) {
            if sender != shell {
                continue;
            }
            if message.get(..length.min(4)) == Some(&b"WCLS"[..]) {
                break;
            }
            if message.get(..length) == Some(&b"WCAN"[..]) {
                refresh.cancel_pointer();
                pointer_pressed = false;
                if !render(&window, pointer, pointer_pressed, refresh_count, &refresh) {
                    fail(8)
                }
                continue;
            }
            if message.get(..4) == Some(&b"WSIZ"[..]) && length == 8 {
                let width = u32::from(u16::from_le_bytes([message[4], message[5]]))
                    .min(MAX_SURFACE_WIDTH as u32);
                let height = u32::from(u16::from_le_bytes([message[6], message[7]]))
                    .min(MAX_SURFACE_HEIGHT as u32);
                if let Ok(resized) = GraphicsSurface::create(width, height) {
                    if resized.share_with(shell).is_ok() && send_resize(&resized, shell) {
                        if render(&resized, pointer, pointer_pressed, refresh_count, &refresh) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenImages: window resized\n");
                        } else {
                            drop(resized);
                        }
                    } else {
                        drop(resized);
                    }
                }
            } else if message.get(..4) == Some(&b"WPMV"[..]) && length == 8 {
                pointer = Some((
                    u16::from_le_bytes([message[4], message[5]]),
                    u16::from_le_bytes([message[6], message[7]]),
                ));
                if let Some((x, y)) = pointer {
                    refresh.pointer_move(i32::from(x), i32::from(y));
                }
                if !render(&window, pointer, pointer_pressed, refresh_count, &refresh) {
                    fail(8)
                }
                let _ = libwoven::print("WovenImages: pointer updated\n");
            } else if message.get(..4) == Some(&b"WPBT"[..]) && length == 10 && message[4] == 0 {
                pointer_pressed = message[5] != 0;
                let local_x = u16::from_le_bytes([message[6], message[7]]);
                let local_y = u16::from_le_bytes([message[8], message[9]]);
                pointer = Some((local_x, local_y));
                if refresh.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    refresh_count = refresh_count.saturating_add(1);
                    let _ = libwoven::print("WovenImages: pointer refreshed\n");
                }
                if !render(&window, pointer, pointer_pressed, refresh_count, &refresh) {
                    fail(8)
                }
                let _ = libwoven::print("WovenImages: pointer updated\n");
            }
        }
    }
    drop(window);
    libwoven::exit(0)
}

fn parse_u64(text: &[u8]) -> Option<u64> {
    if text.is_empty() {
        return None;
    }
    let mut value = 0u64;
    for digit in text {
        if !digit.is_ascii_digit() {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(u64::from(digit - b'0'))?;
    }
    Some(value)
}
