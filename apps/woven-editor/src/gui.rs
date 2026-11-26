use libwoven::layout::Frame;
use libwoven::ui::{Button, Label, TextField, Theme};
use libwoven::{rgb, Canvas, GraphicsSurface, InputEvent, Key};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 220;
const BACKGROUND: u32 = rgb(14, 18, 30);
const POINTER_MARKER: u32 = rgb(255, 80, 80);
const THEME: Theme = Theme::DARK;
const BUTTON_X: i32 = 250;
const BUTTON_Y: i32 = 170;
const BUTTON_WIDTH: usize = 80;
const BUTTON_HEIGHT: usize = 20;
const SAVE_X: i32 = 160;
const SAVE_WIDTH: usize = 80;
const MAX_TEXT: usize = 256;
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 198;
const OPEN_X: i32 = 22;
const SAVE_AS_X: i32 = 128;
const DISCARD_X: i32 = 234;
const ACTION_Y: i32 = 190;
const ACTION_WIDTH: usize = 96;
const PATH_Y: i32 = 170;
const MAX_ENTRIES: usize = 32;
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;

struct EditorView<'a, 'b> {
    refresh_count: u32,
    refresh: &'a Button<'b>,
    save: &'a Button<'b>,
    open: &'a Button<'b>,
    save_as: &'a Button<'b>,
    discard: &'a Button<'b>,
    editor: &'a TextField<MAX_TEXT>,
    path: &'a TextField<MAX_TEXT>,
    status: &'a str,
    saved: bool,
    confirm_unsaved: bool,
}

macro_rules! render_editor {
    ($window:expr, $pointer:expr, $pressed:expr, $refresh_count:expr, $refresh:expr, $save:expr, $open:expr, $save_as:expr, $discard:expr, $editor:expr, $path:expr, $status:expr, $saved:expr, $confirm:expr) => {{
        let view = EditorView {
            refresh_count: $refresh_count,
            refresh: $refresh,
            save: $save,
            open: $open,
            save_as: $save_as,
            discard: $discard,
            editor: $editor,
            path: $path,
            status: $status,
            saved: $saved,
            confirm_unsaved: $confirm,
        };
        render($window, $pointer, $pressed, &view)
    }};
}

struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: the editor GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer = PixelBuffer(core::cell::UnsafeCell::new(
    [0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT],
));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenEditor: failure\n");
    libwoven::exit(code)
}

fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn entry_count() -> usize {
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut count = 0usize;
    for index in 0..MAX_ENTRIES {
        if libwoven::read_dir("/mnt", index, &mut name).is_err() {
            break;
        }
        count += 1;
    }
    count
}

fn draw_editor(
    canvas: &mut Canvas<'_>,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    view: &EditorView<'_, '_>,
) {
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, THEME.surface);
    stroke_rect(
        canvas,
        PANEL_X,
        PANEL_Y,
        PANEL_WIDTH,
        PANEL_HEIGHT,
        THEME.accent,
    );
    Label::new(
        Frame::new(PANEL_X + 10, PANEL_Y + 8, 180, 16),
        "WovenEditor",
    )
    .draw(canvas, THEME);
    Label::new(Frame::new(PANEL_X + 10, PANEL_Y + 36, 180, 16), "Workspace").draw(
        canvas,
        Theme {
            text: THEME.accent,
            ..THEME
        },
    );
    let _ = entry_count();
    canvas.draw_text_system(PANEL_X + 10, PANEL_Y + 54, "/mnt", THEME.text);
    view.editor.draw(canvas, THEME);
    canvas.draw_text_system(
        PANEL_X + 10,
        PANEL_Y + 133,
        if view.confirm_unsaved {
            "Unsaved changes - Discard to close"
        } else {
            view.status
        },
        if view.saved { THEME.accent } else { THEME.text },
    );
    canvas.draw_text_system(PANEL_X + 10, PANEL_Y + 148, "Path", THEME.text);
    view.path.draw(canvas, THEME);
    if view.confirm_unsaved {
        view.discard.draw(canvas, THEME);
    }
    let _ = view.refresh_count;
    view.refresh.draw(canvas, THEME);
    view.save.draw(canvas, THEME);
    view.open.draw(canvas, THEME);
    view.save_as.draw(canvas, THEME);
    if let Some((x, y)) = pointer {
        canvas.fill_rect(
            i32::from(x).saturating_sub(2),
            i32::from(y).saturating_sub(2),
            5,
            5,
            if pressed { THEME.text } else { POINTER_MARKER },
        );
    }
}

fn render(
    window: &GraphicsSurface,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    view: &EditorView<'_, '_>,
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
    draw_editor(&mut canvas, pointer, pressed, view);
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

/// Open a bounded interactive editor workspace window through WovenShell.
/// The GUI owns text editing state and uses the capability-checked filesystem
/// ABI only when the user explicitly activates Save.
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
    let mut save = Button::new(
        Frame::new(SAVE_X, BUTTON_Y, SAVE_WIDTH as i32, BUTTON_HEIGHT as i32),
        "Save",
    );
    let mut open_button = Button::new(
        Frame::new(OPEN_X, ACTION_Y, ACTION_WIDTH as i32, BUTTON_HEIGHT as i32),
        "Open",
    );
    let mut save_as = Button::new(
        Frame::new(SAVE_AS_X, ACTION_Y, ACTION_WIDTH as i32, BUTTON_HEIGHT as i32),
        "Save As",
    );
    let mut discard = Button::new(
        Frame::new(DISCARD_X, ACTION_Y, ACTION_WIDTH as i32, BUTTON_HEIGHT as i32),
        "Discard",
    );
    let mut editor = TextField::<MAX_TEXT>::new(Frame::new(PANEL_X + 10, PANEL_Y + 108, 316, 18));
    editor.focused = true;
    let mut path = TextField::<MAX_TEXT>::new(Frame::new(PANEL_X + 10, PATH_Y, 130, 20));
    if !path.set_text(wovenhat_editor::DEFAULT_NOTE) {
        fail(9)
    }
    let mut saved = false;
    let mut status = "Ready";
    let mut confirm_unsaved = false;
    if !render_editor!(
        &window,
        None,
        false,
        0,
        &refresh,
        &save,
        &open_button,
        &save_as,
        &discard,
        &editor,
        &path,
        status,
        saved,
        confirm_unsaved
    ) {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Editor";
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
    let _ = libwoven::print("WovenEditor: window opened\n");
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
                if saved || confirm_unsaved {
                    break;
                }
                confirm_unsaved = true;
                status = "Unsaved changes - Discard to close";
                let _ = libwoven::print("WovenEditor: unsaved close held\n");
                let _ = render_editor!(
                    &window,
                    pointer,
                    pointer_pressed,
                    refresh_count,
                    &refresh,
                    &save,
                    &open_button,
                    &save_as,
                    &discard,
                    &editor,
                    &path,
                    status,
                    saved,
                    confirm_unsaved
                );
                continue;
            }
            if message.get(..length) == Some(&b"WCAN"[..]) {
                refresh.cancel_pointer();
                pointer_pressed = false;
                if !render_editor!(
                    &window,
                    pointer,
                    pointer_pressed,
                    refresh_count,
                    &refresh,
                    &save,
                    &open_button,
                    &save_as,
                    &discard,
                    &editor,
                    &path,
                    status,
                    saved,
                    confirm_unsaved
                ) {
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
                        if render_editor!(
                            &resized,
                            pointer,
                            pointer_pressed,
                            refresh_count,
                            &refresh,
                            &save,
                            &open_button,
                            &save_as,
                            &discard,
                            &editor,
                            &path,
                            status,
                            saved,
                            confirm_unsaved
                        ) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenEditor: window resized\n");
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
                    save.pointer_move(i32::from(x), i32::from(y));
                    open_button.pointer_move(i32::from(x), i32::from(y));
                    save_as.pointer_move(i32::from(x), i32::from(y));
                    discard.pointer_move(i32::from(x), i32::from(y));
                    editor.pointer(i32::from(x), i32::from(y), false);
                    path.pointer(i32::from(x), i32::from(y), false);
                }
                if !render_editor!(
                    &window,
                    pointer,
                    pointer_pressed,
                    refresh_count,
                    &refresh,
                    &save,
                    &open_button,
                    &save_as,
                    &discard,
                    &editor,
                    &path,
                    status,
                    saved,
                    confirm_unsaved
                ) {
                    fail(8)
                }
                log_pointer(pointer);
            } else if message.get(..4) == Some(&b"WPBT"[..]) && length == 10 && message[4] == 0 {
                pointer_pressed = message[5] != 0;
                let local_x = u16::from_le_bytes([message[6], message[7]]);
                let local_y = u16::from_le_bytes([message[8], message[9]]);
                pointer = Some((local_x, local_y));
                // Focusing a field is not an edit; only character and
                // backspace input marks the buffer dirty.
                let _ = editor.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed);
                let _ = path.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed);
                if refresh.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    refresh_count = refresh_count.saturating_add(1);
                    let _ = libwoven::print("WovenEditor: pointer refreshed\n");
                }
                if save.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    match save_to(wovenhat_editor::DEFAULT_NOTE, editor.as_str()) {
                        Ok(()) => {
                            saved = true;
                            status = "Saved";
                        }
                        Err(error) => status = error_status(error),
                    }
                }
                if save_as.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    match save_to(path.as_str(), editor.as_str()) {
                        Ok(()) => {
                            saved = true;
                            status = "Saved as path";
                        }
                        Err(error) => status = error_status(error),
                    }
                }
                if open_button.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    match open_note(path.as_str(), &mut editor) {
                        Ok(()) => {
                            saved = true;
                            status = "Opened";
                        }
                        Err(error) => status = error_status(error),
                    }
                }
                if confirm_unsaved
                    && discard.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed)
                {
                    let _ = libwoven::print("WovenEditor: unsaved changes discarded\n");
                    break;
                }
                if !render_editor!(
                    &window,
                    pointer,
                    pointer_pressed,
                    refresh_count,
                    &refresh,
                    &save,
                    &open_button,
                    &save_as,
                    &discard,
                    &editor,
                    &path,
                    status,
                    saved,
                    confirm_unsaved
                ) {
                    fail(8)
                }
                log_pointer(pointer);
            } else if message.get(..4) == Some(&b"WKEY"[..]) && length >= 5 {
                let event = match message[4] {
                    1 => Some(InputEvent::Key(Key::Enter)),
                    2 => Some(InputEvent::Key(Key::Backspace)),
                    _ => None,
                };
                let changed = if let Some(event) = event {
                    if matches!(event, InputEvent::Key(Key::Enter)) {
                        let target = if path.focused {
                            path.as_str()
                        } else {
                            wovenhat_editor::DEFAULT_NOTE
                        };
                        match save_to(target, editor.as_str()) {
                            Ok(()) => {
                                saved = true;
                                status = if path.focused { "Saved as path" } else { "Saved" };
                            }
                            Err(error) => status = error_status(error),
                        }
                        true
                    } else if path.focused {
                        path.handle(event)
                    } else {
                        editor.handle(event)
                    }
                } else {
                    false
                };
                if changed {
                    if !matches!(event, Some(InputEvent::Key(Key::Enter))) && !path.focused {
                        saved = false;
                    }
                    let _ = render_editor!(
                        &window,
                        pointer,
                        pointer_pressed,
                        refresh_count,
                        &refresh,
                        &save,
                        &open_button,
                        &save_as,
                        &discard,
                        &editor,
                        &path,
                        status,
                        saved,
                        confirm_unsaved
                    );
                }
            } else if length == 1 && message[0].is_ascii_graphic() {
                let event = InputEvent::Key(Key::Char(message[0] as char));
                let changed = if path.focused {
                    path.handle(event)
                } else {
                    editor.handle(event)
                };
                if changed {
                    if !path.focused {
                        saved = false;
                    }
                    let _ = render_editor!(
                        &window,
                        pointer,
                        pointer_pressed,
                        refresh_count,
                        &refresh,
                        &save,
                        &open_button,
                        &save_as,
                        &discard,
                        &editor,
                        &path,
                        status,
                        saved,
                        confirm_unsaved
                    );
                }
            }
        }
    }
    drop(window);
    libwoven::exit(0)
}

fn error_status(error: wovenhat_editor::SaveError) -> &'static str {
    match error {
        wovenhat_editor::SaveError::InvalidPath => "Error: invalid path",
        wovenhat_editor::SaveError::OpenDenied => "Error: open denied",
        wovenhat_editor::SaveError::ReadFailed => "Error: read failed",
        wovenhat_editor::SaveError::WriteFailed => "Error: write failed",
        wovenhat_editor::SaveError::SyncFailed => "Error: sync failed",
    }
}

fn log_failure(action: &str, error: wovenhat_editor::SaveError) {
    let _ = libwoven::print("WovenEditor: ");
    let _ = libwoven::print(action);
    let _ = libwoven::print(" failed: ");
    let _ = libwoven::print(error.as_str());
    let _ = libwoven::print("\n");
}

/// Save through the existing capability-checked filesystem ABI. The kernel
/// still enforces `FileWrite`, the mounted-path policy and the VFS bounds.
fn save_to(path: &str, text: &str) -> Result<(), wovenhat_editor::SaveError> {
    use wovenhat_editor::SaveError;
    if !wovenhat_editor::valid_note_path(path) {
        log_failure("save", SaveError::InvalidPath);
        return Err(SaveError::InvalidPath);
    }
    let Ok(descriptor) = libwoven::open_access(path, 6) else {
        log_failure("save", SaveError::OpenDenied);
        return Err(SaveError::OpenDenied);
    };
    let written = libwoven::write_file(descriptor, text.as_bytes());
    let _ = libwoven::close(descriptor);
    if written != Ok(text.len()) {
        log_failure("save", SaveError::WriteFailed);
        return Err(SaveError::WriteFailed);
    }
    // The syscall sentinel for failure is u64::MAX; zero is a valid count.
    if unsafe { libwoven::syscall::sync() } == u64::MAX {
        log_failure("save", SaveError::SyncFailed);
        return Err(SaveError::SyncFailed);
    }
    let _ = libwoven::print("WovenEditor: note saved\n");
    Ok(())
}

/// Load a bounded note into the editor field, trimming at a UTF-8 boundary.
fn open_note(
    path: &str,
    editor: &mut TextField<MAX_TEXT>,
) -> Result<(), wovenhat_editor::SaveError> {
    use wovenhat_editor::SaveError;
    if !wovenhat_editor::valid_note_path(path) {
        log_failure("open", SaveError::InvalidPath);
        return Err(SaveError::InvalidPath);
    }
    let Ok(descriptor) = libwoven::open_access(path, 1) else {
        log_failure("open", SaveError::OpenDenied);
        return Err(SaveError::OpenDenied);
    };
    let mut bytes = [0u8; MAX_TEXT];
    let result = libwoven::read(descriptor, &mut bytes);
    let _ = libwoven::close(descriptor);
    let Ok(length) = result else {
        log_failure("open", SaveError::ReadFailed);
        return Err(SaveError::ReadFailed);
    };
    let text = wovenhat_editor::trim_utf8(&bytes[..length.min(bytes.len())]);
    if !editor.set_text(text) {
        log_failure("open", SaveError::ReadFailed);
        return Err(SaveError::ReadFailed);
    }
    let _ = libwoven::print("WovenEditor: note opened\n");
    Ok(())
}

fn log_pointer(pointer: Option<(u16, u16)>) {
    let Some((x, y)) = pointer else {
        let _ = libwoven::print("WovenEditor: pointer updated\n");
        return;
    };
    let mut digits = [0u8; 20];
    let x_text = libwoven::format_u64(u64::from(x), &mut digits);
    let mut y_digits = [0u8; 20];
    let y_text = libwoven::format_u64(u64::from(y), &mut y_digits);
    let _ = libwoven::print("WovenEditor: pointer updated x=");
    if let Ok(text) = core::str::from_utf8(x_text) { let _ = libwoven::print(text); }
    let _ = libwoven::print(" y=");
    if let Ok(text) = core::str::from_utf8(y_text) { let _ = libwoven::print(text); }
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
        value = value
            .checked_mul(10)?
            .checked_add(u64::from(digit - b'0'))?;
    }
    Some(value)
}
