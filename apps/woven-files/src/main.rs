#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::layout::Frame;
use libwoven::ui::{Button, Label, TextField, Theme};
use libwoven::{rgb, Args, Canvas, GraphicsSurface, InputEvent, Key};

libwoven::entry!(main);

const MAX_ENTRIES: usize = 128;
const MAX_PREVIEW: usize = 2048;

const WIDTH: u32 = 440;
const HEIGHT: u32 = 300;
const BACKGROUND: u32 = rgb(14, 18, 30);
/// The shared WovenUI theme (tokens.rs-equivalent for apps): surface,
/// accent, text, disabled and focus colors, unified with
/// apps/woven-shell's own palette so every first-party app reads as one
/// system rather than each choosing its own accent independently.
const THEME: Theme = Theme::DARK;
const POINTER_MARKER: u32 = rgb(255, 80, 80);
const BUTTON_X: i32 = 344;
const BUTTON_Y: i32 = 266;
const BUTTON_WIDTH: usize = 80;
const BUTTON_HEIGHT: usize = 20;
const PATH_X: i32 = 18;
const PATH_Y: i32 = 154;
const PATH_WIDTH: usize = 270;
const ACTION_Y: i32 = 180;
const ACTION_WIDTH: usize = 68;
const MAX_PATH: usize = 256;
const MAX_ROWS: usize = 12;

const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 296;
const PANEL_HEIGHT: usize = 216;
const LINE_HEIGHT: i32 = 10;
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;

/// In .bss rather than the 8 KiB task stack: too large to carry as a stack
/// local. Sized to the whole window: `GraphicsSurface::write_canvas` always
/// uploads to the surface's (0, 0), regardless of the canvas's own x/y
/// origin (that's purely for the canvas's own coordinate addressing), so
/// everything must be drawn into one full-window canvas rather than a
/// separately-placed inset one. See woven-calculator for the same pattern.
struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: woven-files is single-threaded and run_gui takes the only reference.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]));

/// A one-pixel-thick rectangle outline, since `Canvas` has no stroke
/// primitive of its own.
fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenFiles: failure\n");
    libwoven::exit(code)
}

struct FilesView<'a, 'b> {
    refresh: &'a Button<'b>,
    path: &'a TextField<MAX_PATH>,
    new: &'a Button<'b>,
    delete: &'a Button<'b>,
    rename: &'a Button<'b>,
    status: &'a str,
    selected: usize,
}

fn draw_files(
    canvas: &mut Canvas<'_>,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    refresh_count: u32,
    view: &FilesView<'_, '_>,
) {
    // First modernization slice: a consistent app header, navigation rail,
    // content area and status bar. Navigation is deliberately visual only;
    // directory operations remain read-only and scoped to /mnt.
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(0, 0, canvas.width(), 46, THEME.surface);
    canvas.fill_rect(0, 45, canvas.width(), 1, THEME.accent);
    canvas.draw_text_system(16, 13, "WovenFiles", THEME.text);
    canvas.draw_text_system(16, 30, "Browse files", THEME.disabled);

    canvas.fill_rect(0, 46, 114, 254, THEME.surface);
    canvas.fill_rect(113, 46, 1, 254, THEME.disabled);
    canvas.draw_text_system(14, 62, "PLACES", THEME.accent);
    canvas.fill_rect(8, 84, 98, 28, BACKGROUND);
    canvas.fill_rect(8, 84, 3, 28, THEME.accent);
    canvas.draw_text_system(18, 94, "Mounted files", THEME.text);
    canvas.draw_text_system(14, 128, "Read-only", THEME.disabled);

    canvas.draw_text_system(PANEL_X, 57, "Location: /mnt", THEME.text);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, THEME.surface);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, THEME.disabled);
    canvas.fill_rect(PANEL_X + 1, PANEL_Y + 1, PANEL_WIDTH - 2, 22, BACKGROUND);
    Label::new(Frame::new(PANEL_X + 9, PANEL_Y + 6, 180, 16), "Name").draw(canvas, THEME);
    canvas.draw_text_system(PANEL_X + 220, PANEL_Y + 6, "Type", THEME.text);

    let mut name = [0u8; libwoven::MAX_NAME];
    let mut shown = 0usize;
    for index in 0..MAX_ENTRIES {
        if shown >= MAX_ROWS {
            break;
        }
        match libwoven::read_dir("/mnt", index, &mut name) {
            Ok((length, directory)) => {
                let mut line = [0u8; libwoven::MAX_NAME + 1];
                let text_length = length.min(libwoven::MAX_NAME);
                line[..text_length].copy_from_slice(&name[..text_length]);
                if directory && text_length < line.len() {
                    line[text_length] = b'/';
                }
                let shown_length = text_length + usize::from(directory && text_length < line.len());
                let text = core::str::from_utf8(&line[..shown_length]).unwrap_or("?");
                let row_y = PANEL_Y + 6 + (shown as i32 + 1) * LINE_HEIGHT;
                if shown == view.selected {
                    canvas.fill_rect(PANEL_X + 4, row_y - 1, 180, LINE_HEIGHT as usize, THEME.accent);
                }
                canvas.draw_text_system(PANEL_X + 6, row_y, text, THEME.text);
                shown += 1;
            }
            Err(_) => break,
        }
    }
    if shown == 0 {
        canvas.draw_text_system(PANEL_X + 9, PANEL_Y + 35, "No entries available", THEME.text);
    }
    canvas.draw_text_system(PANEL_X + 6, PANEL_Y + 130, view.status, THEME.accent);
    view.path.draw(canvas, THEME);
    view.new.draw(canvas, THEME);
    view.delete.draw(canvas, THEME);
    view.rename.draw(canvas, THEME);
    let mut digits = [0; 10];
    let count = libwoven::format_u64(u64::from(refresh_count), &mut digits);
    canvas.draw_text_system(174, BUTTON_Y + 5, "Refreshes:", THEME.disabled);
    canvas.draw_text_system(
        258,
        BUTTON_Y + 5,
        core::str::from_utf8(count).unwrap_or("?"),
        THEME.text,
    );
    view.refresh.draw(canvas, THEME);
    if let Some((x, y)) = pointer {
        let color = if pressed { THEME.text } else { POINTER_MARKER };
        canvas.fill_rect(i32::from(x).saturating_sub(2), i32::from(y).saturating_sub(2), 5, 5, color);
    }
}

fn render(
    window: &GraphicsSurface,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    refresh_count: u32,
    view: &FilesView<'_, '_>,
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
    draw_files(&mut canvas, pointer, pressed, refresh_count, view);
    window.write_canvas(&canvas).is_ok()
}

macro_rules! render_files {
    ($window:expr, $pointer:expr, $pressed:expr, $count:expr, $refresh:expr, $path:expr, $new:expr, $delete:expr, $rename:expr, $status:expr, $selected:expr) => {{
        let view = FilesView {
            refresh: $refresh,
            path: $path,
            new: $new,
            delete: $delete,
            rename: $rename,
            status: $status,
            selected: $selected,
        };
        render($window, $pointer, $pressed, $count, &view)
    }};
}

fn send_resize(window: &GraphicsSurface, shell: libwoven::process::Id) -> bool {
    let mut response = [0u8; 16];
    response[..4].copy_from_slice(b"WRSZ");
    response[4..12].copy_from_slice(&window.handle().to_le_bytes());
    response[12..14].copy_from_slice(&(window.width() as u16).to_le_bytes());
    response[14..].copy_from_slice(&(window.height() as u16).to_le_bytes());
    libwoven::message_send(shell, &response).is_ok()
}

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        run_gui(args.get(2))
    }
    let status = match args.get(1) {
        Some(b"--view") => match args.get(2) {
            Some(path) => view(path),
            None => 2,
        },
        Some(b"--help") => {
            let _ = libwoven::print(
                "WovenFiles: --path PATH | --view PATH | --mkdir PATH | --rm PATH | --rename OLD NEW | --write PATH TEXT\n",
            );
            0
        }
        Some(b"--path") => match args.get(2) {
            Some(path) => list(path),
            None => 2,
        },
        Some(b"--mkdir") => match args.get(2) {
            Some(path) => create_dir(path),
            None => 2,
        },
        Some(b"--rm") => match args.get(2) {
            Some(path) => remove(path),
            None => 2,
        },
        Some(b"--rename") => match (args.get(2), args.get(3)) {
            (Some(old), Some(new)) => move_path(old, new),
            _ => 2,
        },
        Some(b"--write") => match (args.get(2), args.get(3)) {
            (Some(path), Some(text)) => write(path, text),
            _ => 2,
        },
        None => list(b"/mnt"),
        Some(_) => 2,
    };
    libwoven::exit(status)
}

/// Opens a window on the WovenShell desktop showing the `/mnt` listing.
/// `shell` is the shell's process id, passed as `%p` from the app catalog's
/// `Exec=` line; see woven-calculator's `run_gui` for the protocol details.
fn run_gui(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };

    let mut window = match GraphicsSurface::create(WIDTH, HEIGHT) {
        Ok(window) => window,
        Err(_) => fail(1),
    };
    let mut refresh_button = Button::new(
        Frame::new(BUTTON_X, BUTTON_Y, BUTTON_WIDTH as i32, BUTTON_HEIGHT as i32),
        "Refresh",
    );
    let mut path = TextField::<MAX_PATH>::new(Frame::new(PATH_X, PATH_Y, PATH_WIDTH as i32, 18));
    let mut new_button = Button::new(
        Frame::new(PATH_X, ACTION_Y, ACTION_WIDTH as i32, BUTTON_HEIGHT as i32),
        "New",
    );
    let mut delete_button = Button::new(
        Frame::new(PATH_X + 76, ACTION_Y, ACTION_WIDTH as i32, BUTTON_HEIGHT as i32),
        "Delete",
    );
    let mut rename_button = Button::new(
        Frame::new(PATH_X + 152, ACTION_Y, ACTION_WIDTH as i32, BUTTON_HEIGHT as i32),
        "Rename",
    );
    let mut status = "Select a row, then edit the path";
    let mut selected = usize::MAX;
    let mut selected_path = [0u8; MAX_PATH];
    let mut selected_len = 0usize;

    if !render_files!(&window, None, false, 0, &refresh_button, &path, &new_button, &delete_button, &rename_button, status, selected) {
        fail(4);
    }

    if window.share_with(shell).is_err() {
        fail(5);
    }
    const TITLE: &[u8] = b"Files";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err()
        || libwoven::message_send(shell, b"WCAP\x01").is_err()
    {
        fail(6);
    }
    let _ = libwoven::print("WovenFiles: window opened\n");

    let mut message = [0u8; 64];
    let mut pointer = None;
    let mut pointer_pressed = false;
    let mut refresh_count = 0u32;
    loop {
        match libwoven::wait_any(libwoven::wait::MESSAGE, None) {
            Ok(_) => {}
            Err(_) => fail(8),
        }
        if let Ok((length, sender)) = libwoven::message_receive(&mut message) {
            if sender != shell {
                continue;
            }
            if message.get(..length.min(4)) == Some(&b"WCLS"[..]) {
                break;
            }
            if message.get(..length) == Some(&b"WCAN"[..]) {
                refresh_button.cancel_pointer();
                pointer_pressed = false;
                let _ = libwoven::print("WovenFiles: pointer cancelled\n");
                if !render_files!(&window, pointer, pointer_pressed, refresh_count, &refresh_button, &path, &new_button, &delete_button, &rename_button, status, selected) {
                    fail(8)
                }
                continue;
            }
            if message.get(..4) == Some(&b"WSIZ"[..]) && length == 8 {
                let width = u32::from(u16::from_le_bytes([message[4], message[5]])).min(MAX_SURFACE_WIDTH as u32);
                let height = u32::from(u16::from_le_bytes([message[6], message[7]])).min(MAX_SURFACE_HEIGHT as u32);
                if let Ok(resized) = GraphicsSurface::create(width, height) {
                    if resized.share_with(shell).is_ok() && send_resize(&resized, shell) {
                        if render_files!(&resized, pointer, pointer_pressed, refresh_count, &refresh_button, &path, &new_button, &delete_button, &rename_button, status, selected) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenFiles: window resized\n");
                        } else {
                            drop(resized);
                        }
                    } else {
                        drop(resized);
                    }
                }
            } else if (message.get(..length) == Some(&b"WKEY\x01"[..])
                || (!path.focused
                    && (message.get(..length) == Some(&b"r"[..])
                        || message.get(..length) == Some(&b" "[..]))))
                && refresh_button.activate()
            {
                refresh_button.focused = true;
                refresh_count = refresh_count.saturating_add(1);
                if !render_files!(&window, pointer, pointer_pressed, refresh_count, &refresh_button, &path, &new_button, &delete_button, &rename_button, status, selected) {
                    fail(8)
                }
                let _ = libwoven::print("WovenFiles: keyboard refreshed\n");
            } else if message.get(..4) == Some(&b"WPMV"[..]) && length == 8 {
                let local_x = u16::from_le_bytes([message[4], message[5]]);
                let local_y = u16::from_le_bytes([message[6], message[7]]);
                pointer = Some((local_x, local_y));
                refresh_button.pointer_move(i32::from(local_x), i32::from(local_y));
                new_button.pointer_move(i32::from(local_x), i32::from(local_y));
                delete_button.pointer_move(i32::from(local_x), i32::from(local_y));
                rename_button.pointer_move(i32::from(local_x), i32::from(local_y));
                path.pointer(i32::from(local_x), i32::from(local_y), false);
                if !render_files!(&window, pointer, pointer_pressed, refresh_count, &refresh_button, &path, &new_button, &delete_button, &rename_button, status, selected) {
                    fail(8)
                }
                let _ = libwoven::print("WovenFiles: pointer updated\n");
            } else if message.get(..4) == Some(&b"WPBT"[..]) && length == 10 && message[4] == 0 {
                let local_x = u16::from_le_bytes([message[6], message[7]]);
                let local_y = u16::from_le_bytes([message[8], message[9]]);
                pointer_pressed = message[5] != 0;
                pointer = Some((local_x, local_y));
                if refresh_button.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    refresh_count = refresh_count.saturating_add(1);
                    let _ = libwoven::print("WovenFiles: pointer refreshed\n");
                }
                let _ = path.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed);
                // Row hit test: selecting a row fills the path field with its
                // `/mnt/<name>` path so Delete/Rename act on that entry.
                if i32::from(local_x) < PANEL_X + 190 {
                    let row = (i32::from(local_y) - (PANEL_Y + 6 + LINE_HEIGHT)) / LINE_HEIGHT;
                    if row >= 0 && (row as usize) < MAX_ROWS {
                        let index = row as usize;
                        let mut name = [0u8; libwoven::MAX_NAME];
                        if let Ok((length, _)) = libwoven::read_dir("/mnt", index, &mut name) {
                            let mut full = [0u8; MAX_PATH];
                            full[..5].copy_from_slice(b"/mnt/");
                            let length = length.min(libwoven::MAX_NAME).min(MAX_PATH - 5);
                            full[5..5 + length].copy_from_slice(&name[..length]);
                            if let Ok(text) = core::str::from_utf8(&full[..5 + length]) {
                                let _ = path.set_text(text);
                                selected = index;
                                selected_path[..5 + length].copy_from_slice(&full[..5 + length]);
                                selected_len = 5 + length;
                                status = "Entry selected";
                            }
                        }
                    }
                }
                if new_button.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    apply(&mut status, path.as_str(), Operation::Mkdir);
                }
                if delete_button.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    apply(&mut status, path.as_str(), Operation::Unlink);
                }
                if rename_button.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    let old = core::str::from_utf8(&selected_path[..selected_len]).unwrap_or("");
                    apply_rename(&mut status, old, path.as_str());
                }
                if !render_files!(&window, pointer, pointer_pressed, refresh_count, &refresh_button, &path, &new_button, &delete_button, &rename_button, status, selected) {
                    fail(8)
                }
                let _ = libwoven::print("WovenFiles: pointer updated\n");
            } else if message.get(..4) == Some(&b"WKEY"[..]) && length >= 5 && message[4] == 2 {
                if path.focused && path.handle(InputEvent::Key(Key::Backspace)) {
                    let _ = render_files!(&window, pointer, pointer_pressed, refresh_count, &refresh_button, &path, &new_button, &delete_button, &rename_button, status, selected);
                }
            } else if length == 1
                && message[0].is_ascii_graphic()
                && path.focused
                && path.handle(InputEvent::Key(Key::Char(message[0] as char)))
            {
                let _ = render_files!(&window, pointer, pointer_pressed, refresh_count, &refresh_button, &path, &new_button, &delete_button, &rename_button, status, selected);
            }
        }
    }
    drop(window);
    libwoven::exit(0)
}

enum Operation {
    Mkdir,
    Unlink,
}

fn bounded_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH
        && path.starts_with("/mnt/")
        && !path.as_bytes().contains(&0)
        && !path.contains('\\')
        && !path.split('/').skip(1).any(|part| part.is_empty() || part == "." || part == "..")
}

fn apply(status: &mut &str, path: &str, operation: Operation) {
    if !bounded_path(path) {
        *status = "Invalid /mnt path";
        return;
    }
    let result = match operation {
        Operation::Mkdir => libwoven::mkdir(path),
        Operation::Unlink => libwoven::unlink(path),
    };
    *status = match result {
        Ok(()) => "Operation completed",
        Err(_) => "Operation denied or unavailable",
    };
    let _ = libwoven::print("WovenFiles: gui operation ");
    let _ = libwoven::print(status);
    let _ = libwoven::print("\n");
}

fn apply_rename(status: &mut &str, old: &str, new: &str) {
    if !bounded_path(old) || !bounded_path(new) {
        *status = "Invalid /mnt path";
        return;
    }
    *status = match libwoven::rename(old, new) {
        Ok(()) => "Entry renamed",
        Err(_) => "Rename denied or unavailable",
    };
    let _ = libwoven::print("WovenFiles: gui rename ");
    let _ = libwoven::print(status);
    let _ = libwoven::print("\n");
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
        value = value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))?;
    }
    Some(value)
}

fn list(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let _ = libwoven::print("WovenFiles: ");
    let _ = libwoven::print(path);
    let _ = libwoven::print("\n");
    let mut name = [0u8; libwoven::MAX_NAME];
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir(path, index, &mut name) {
            Ok((length, directory)) => {
                let _ = libwoven::write(libwoven::syscall::STDOUT, &name[..length]);
                let _ = libwoven::print(if directory { "/\n" } else { "\n" });
            }
            Err(_) => return 0,
        }
    }
    0
}

fn view(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let Ok(descriptor) = libwoven::open(path) else {
        let _ = libwoven::print("WovenFiles: cannot open file\n");
        return 2;
    };
    let mut bytes = [0u8; MAX_PREVIEW];
    let result = match libwoven::read(descriptor, &mut bytes) {
        Ok(length) => {
            let _ = libwoven::write(libwoven::syscall::STDOUT, &bytes[..length]);
            if length == MAX_PREVIEW {
                let _ = libwoven::print("\n[preview truncated]\n");
            }
            0
        }
        Err(_) => 2,
    };
    let _ = libwoven::close(descriptor);
    result
}

const MAX_MUTATION_PATH: usize = 256;

/// Confine app-level mutations to bounded `/mnt/...` or `/tmp/...` paths. The
/// kernel still enforces `FileWrite`, the sandbox file scope and the mounted-
/// path policy; this check keeps the CLI from silently targeting a system path.
fn mutation_path(raw: &[u8]) -> Option<&str> {
    let path = core::str::from_utf8(raw).ok()?;
    let confined = path.starts_with("/mnt/") || path.starts_with("/tmp/");
    if path.is_empty() || path.len() > MAX_MUTATION_PATH || !confined {
        return None;
    }
    if path.contains('\0') || path.contains('\\') {
        return None;
    }
    path.split('/').skip(1).all(|part| {
        !part.is_empty() && part != "." && part != ".." && part.len() <= 64
    }).then_some(path)
}

fn create_dir(raw_path: &[u8]) -> u64 {
    let Some(path) = mutation_path(raw_path) else {
        let _ = libwoven::print("WovenFiles: mutation rejected: invalid path\n");
        return 2;
    };
    match libwoven::mkdir(path) {
        Ok(()) => {
            let _ = libwoven::print("WovenFiles: directory created\n");
            0
        }
        Err(_) => {
            let _ = libwoven::print("WovenFiles: mutation denied: mkdir\n");
            3
        }
    }
}

fn remove(raw_path: &[u8]) -> u64 {
    let Some(path) = mutation_path(raw_path) else {
        let _ = libwoven::print("WovenFiles: mutation rejected: invalid path\n");
        return 2;
    };
    match libwoven::unlink(path) {
        Ok(()) => {
            let _ = libwoven::print("WovenFiles: entry removed\n");
            0
        }
        Err(_) => {
            let _ = libwoven::print("WovenFiles: mutation denied: unlink\n");
            3
        }
    }
}

fn move_path(raw_old: &[u8], raw_new: &[u8]) -> u64 {
    let (Some(old), Some(new)) = (mutation_path(raw_old), mutation_path(raw_new)) else {
        let _ = libwoven::print("WovenFiles: mutation rejected: invalid path\n");
        return 2;
    };
    match libwoven::rename(old, new) {
        Ok(()) => {
            let _ = libwoven::print("WovenFiles: entry renamed\n");
            0
        }
        Err(_) => {
            let _ = libwoven::print("WovenFiles: mutation denied: rename\n");
            3
        }
    }
}

fn write(raw_path: &[u8], raw_text: &[u8]) -> u64 {
    let Some(path) = mutation_path(raw_path) else {
        let _ = libwoven::print("WovenFiles: mutation rejected: invalid path\n");
        return 2;
    };
    let Ok(descriptor) = libwoven::open_access(path, 6) else {
        let _ = libwoven::print("WovenFiles: mutation denied: open\n");
        return 3;
    };
    let written = libwoven::write_file_all(descriptor, raw_text);
    let _ = libwoven::close(descriptor);
    if written != Ok(raw_text.len()) {
        let _ = libwoven::print("WovenFiles: mutation denied: write\n");
        return 3;
    }
    if libwoven::sync_all().is_err() {
        let _ = libwoven::print("WovenFiles: mutation denied: sync\n");
        return 4;
    }
    let _ = libwoven::print("WovenFiles: entry written\n");
    0
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
