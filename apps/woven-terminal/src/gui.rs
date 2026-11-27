use libwoven::gui_protocol;
use libwoven::layout::Frame;
use libwoven::ui::{Button, Label, Theme};
use libwoven::{rgb, Canvas, GraphicsSurface};
use wovenhat_terminal::{complete, parse, Command};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 220;
const BACKGROUND: u32 = rgb(10, 14, 22);
const MUTED: u32 = rgb(150, 170, 190);
const POINTER_MARKER: u32 = rgb(255, 80, 80);
const THEME: Theme = Theme::DARK;
const BUTTON_X: i32 = 250;
const BUTTON_Y: i32 = 170;
const BUTTON_WIDTH: usize = 80;
const BUTTON_HEIGHT: usize = 20;
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 196;
// Keep the static render target below the userspace image/stack window while
// still covering the common tiled-client geometry. Larger WSIZ requests are
// answered with this bounded surface size.
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;

struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: the terminal GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer = PixelBuffer(core::cell::UnsafeCell::new(
    [0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT],
));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenTerminal: failure\n");
    libwoven::exit(code)
}

fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

const MAX_INPUT: usize = 48;
const MAX_ROWS: usize = 10;
const ROW_HEIGHT: i32 = 12;

struct TerminalView<'a, 'b> {
    line: &'a [u8],
    status: &'a [u8],
    clear: &'a Button<'b>,
    history: &'a [[u8; MAX_INPUT]; MAX_ROWS],
    results: &'a [u8; MAX_ROWS],
    history_len: usize,
}

fn draw(
    canvas: &mut Canvas<'_>,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    view: &TerminalView<'_, '_>,
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
        "WovenTerminal",
    )
    .draw(canvas, THEME);
    canvas.draw_text_system(PANEL_X + 10, PANEL_Y + 32, "Command history", THEME.accent);
    let start = view.history_len.saturating_sub(8);
    for row in start..view.history_len {
        let slot = row % MAX_ROWS;
        let y = PANEL_Y + 46 + ((row - start) as i32) * ROW_HEIGHT;
        let used = view.history[slot]
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(MAX_INPUT);
        if let Ok(text) = core::str::from_utf8(&view.history[slot][..used]) {
            canvas.draw_text_system(PANEL_X + 10, y, text, THEME.text);
        }
        let mark = if view.results[slot] == 0 {
            b"ok"
        } else {
            b"er"
        };
        canvas.draw_text_system(
            PANEL_X + 300,
            y,
            core::str::from_utf8(mark).unwrap_or(""),
            THEME.accent,
        );
    }
    canvas.draw_text_system(PANEL_X + 10, PANEL_Y + 150, ">", THEME.accent);
    if let Ok(text) = core::str::from_utf8(view.line) {
        canvas.draw_text_system(PANEL_X + 20, PANEL_Y + 150, text, THEME.accent);
    }
    if let Ok(text) = core::str::from_utf8(view.status) {
        canvas.draw_text_system(PANEL_X + 10, PANEL_Y + 164, text, THEME.text);
    }
    view.clear.draw(canvas, THEME);
    canvas.draw_text_system(
        PANEL_X + 10,
        PANEL_Y + 180,
        "cd | pwd | export | ls | cat | calc | jobs | fg",
        MUTED,
    );
    if let Some((x, y)) = pointer {
        let color = if pressed { THEME.text } else { POINTER_MARKER };
        canvas.fill_rect(
            i32::from(x).saturating_sub(2),
            i32::from(y).saturating_sub(2),
            5,
            5,
            color,
        );
    }
}

fn render(
    window: &GraphicsSurface,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    view: &TerminalView<'_, '_>,
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
    draw(&mut canvas, pointer, pressed, view);
    window.write_canvas(&canvas).is_ok()
}

macro_rules! render_terminal {
    ($window:expr, $pointer:expr, $pressed:expr, $line:expr, $status:expr, $clear:expr, $history:expr, $results:expr, $count:expr) => {{
        let view = TerminalView {
            line: $line,
            status: $status,
            clear: $clear,
            history: $history,
            results: $results,
            history_len: $count,
        };
        render($window, $pointer, $pressed, &view)
    }};
}

/// Ask the shell to show the last command's bounded result in the title bar.
fn request_title(shell: libwoven::process::Id, result: u64, command: &[u8]) {
    let mut payload = [0u8; gui_protocol::request::MAX_TITLE];
    let prefix: &[u8] = if result == 0 { b"ok " } else { b"er " };
    payload[..prefix.len()].copy_from_slice(prefix);
    let used = command.len().min(payload.len() - prefix.len());
    payload[prefix.len()..prefix.len() + used].copy_from_slice(&command[..used]);
    let mut request = [0u8; 5 + gui_protocol::request::MAX_TITLE];
    if let Some(length) = gui_protocol::encode_request(
        gui_protocol::request::SET_TITLE,
        &payload[..prefix.len() + used],
        &mut request,
    ) {
        let _ = libwoven::message_send(shell, &request[..length]);
    }
}

fn send_resize(window: &GraphicsSurface, shell: libwoven::process::Id) -> bool {
    let mut response = [0u8; 16];
    response[..4].copy_from_slice(b"WRSZ");
    response[4..12].copy_from_slice(&window.handle().to_le_bytes());
    response[12..14].copy_from_slice(&(window.width() as u16).to_le_bytes());
    response[14..].copy_from_slice(&(window.height() as u16).to_le_bytes());
    libwoven::message_send(shell, &response).is_ok()
}

/// Open the interactive terminal through WovenShell. ASCII input, Enter, and
/// Backspace are accepted through the bounded shell key protocol. Submitted
/// commands use the same bounded command executor as `--interactive`; their
/// output is visible on QEMU's serial/monitor console.
pub fn run(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };
    let mut window = GraphicsSurface::create(WIDTH, HEIGHT).unwrap_or_else(|_| fail(1));
    let mut clear = Button::new(
        Frame::new(
            BUTTON_X,
            BUTTON_Y,
            BUTTON_WIDTH as i32,
            BUTTON_HEIGHT as i32,
        ),
        "Clear",
    );
    let mut history = [[0u8; MAX_INPUT]; MAX_ROWS];
    let mut results = [0u8; MAX_ROWS];
    let mut history_count = 0usize;
    let line = [0u8; MAX_INPUT];
    let status: &[u8] = b"Ready for a bounded command preview";
    if !render_terminal!(
        &window,
        None,
        false,
        &line,
        status,
        &clear,
        &history,
        &results,
        history_count
    ) {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Terminal";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err()
        || libwoven::message_send(shell, b"WCAP\x01").is_err()
        || libwoven::message_send(shell, gui_protocol::OPT_IN).is_err()
    {
        fail(5)
    }
    let _ = libwoven::print("WovenTerminal: window opened\n");
    let mut message = [0u8; 64];
    let mut line = [0u8; MAX_INPUT];
    let mut length = 0usize;
    let mut status: &[u8] = b"Ready for a bounded command preview";
    let mut pointer = None;
    let mut pointer_pressed = false;
    loop {
        if libwoven::wait_any(libwoven::wait::MESSAGE, None).is_err() {
            fail(6)
        }
        if let Ok((message_length, sender)) = libwoven::message_receive(&mut message) {
            if sender != shell {
                continue;
            }
            if message.get(..message_length.min(4)) == Some(&b"WCLS"[..]) {
                break;
            }
            if message.get(..message_length) == Some(&b"WCAN"[..]) {
                clear.cancel_pointer();
                pointer_pressed = false;
                if !render_terminal!(
                    &window,
                    pointer,
                    pointer_pressed,
                    &line[..length],
                    status,
                    &clear,
                    &history,
                    &results,
                    history_count
                ) {
                    fail(9)
                }
                continue;
            }
            if message.get(..4) == Some(&b"WSIZ"[..]) && message_length == 8 {
                let width = u32::from(u16::from_le_bytes([message[4], message[5]]))
                    .min(MAX_SURFACE_WIDTH as u32);
                let height = u32::from(u16::from_le_bytes([message[6], message[7]]))
                    .min(MAX_SURFACE_HEIGHT as u32);
                if let Ok(resized) = GraphicsSurface::create(width, height) {
                    // Publish the replacement token before the potentially
                    // expensive pixel upload, so the shell's bounded resize
                    // grace window cannot expire while copying the canvas.
                    if resized.share_with(shell).is_ok() && send_resize(&resized, shell) {
                        if render_terminal!(
                            &resized,
                            pointer,
                            pointer_pressed,
                            &line[..length],
                            status,
                            &clear,
                            &history,
                            &results,
                            history_count
                        ) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenTerminal: window resized\n");
                        } else {
                            drop(resized);
                        }
                    } else {
                        drop(resized);
                    }
                }
                continue;
            }
            if message.get(..4) == Some(&b"WPMV"[..]) && message_length == 8 {
                pointer = Some((
                    u16::from_le_bytes([message[4], message[5]]),
                    u16::from_le_bytes([message[6], message[7]]),
                ));
                if let Some((x, y)) = pointer {
                    clear.pointer_move(i32::from(x), i32::from(y));
                }
                if !render_terminal!(
                    &window,
                    pointer,
                    pointer_pressed,
                    &line[..length],
                    status,
                    &clear,
                    &history,
                    &results,
                    history_count
                ) {
                    fail(9)
                }
                let _ = libwoven::print("WovenTerminal: pointer updated\n");
                continue;
            }
            if message.get(..4) == Some(&b"WPBT"[..]) && message_length == 10 && message[4] == 0 {
                let local_x = u16::from_le_bytes([message[6], message[7]]);
                let local_y = u16::from_le_bytes([message[8], message[9]]);
                pointer_pressed = message[5] != 0;
                pointer = Some((local_x, local_y));
                if clear.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    length = 0;
                    history_count = 0;
                    status = b"Input and history cleared by pointer control";
                    let _ = libwoven::print("WovenTerminal: pointer cleared\n");
                }
                if !render_terminal!(
                    &window,
                    pointer,
                    pointer_pressed,
                    &line[..length],
                    status,
                    &clear,
                    &history,
                    &results,
                    history_count
                ) {
                    fail(9)
                }
                let _ = libwoven::print("WovenTerminal: pointer updated\n");
                continue;
            }
            // Both key protocols normalize to the same bounded code set; v2
            // adds Tab completion and a uniform key layout. Legacy messages
            // remain accepted so an older shell still drives this client.
            let key = if let Some(decoded) = gui_protocol::decode_key(&message[..message_length]) {
                Some(decoded)
            } else if message.get(..4) == Some(&b"WKEY"[..]) && message_length >= 5 {
                Some((message[4], None))
            } else if message_length == 1 && message[0].is_ascii_graphic() {
                Some((gui_protocol::key::ASCII, Some(message[0])))
            } else {
                None
            };
            if let Some((code, ascii)) = key {
                match code {
                    gui_protocol::key::ENTER => {
                        let command = parse(&line[..length]);
                        if command == Command::Quit {
                            drop(window);
                            libwoven::exit(0)
                        }
                        let result = crate::run(command);
                        let slot = history_count % MAX_ROWS;
                        let used = length.min(MAX_INPUT);
                        history[slot][..used].copy_from_slice(&line[..used]);
                        history[slot][used..].fill(0);
                        results[slot] = if result == 0 { 0 } else { 1 };
                        history_count += 1;
                        request_title(shell, result, &line[..used]);
                        status = if result == 0 {
                            b"Command executed; result shown in window title"
                        } else {
                            b"Command failed; result shown in window title"
                        };
                        length = 0;
                    }
                    gui_protocol::key::BACKSPACE => {
                        length = length.saturating_sub(1);
                        status = b"Editing command preview";
                    }
                    gui_protocol::key::TAB => {
                        let mut completed = [0u8; MAX_INPUT];
                        if let Some(count) = complete(&line[..length], &mut completed) {
                            line[..count].copy_from_slice(&completed[..count]);
                            length = count;
                        }
                        status = b"Tab completed a builtin";
                    }
                    gui_protocol::key::ASCII => {
                        if let Some(byte) = ascii {
                            if byte.is_ascii_graphic() && length < MAX_INPUT {
                                line[length] = byte;
                                length += 1;
                            }
                        }
                        status = b"Editing command preview";
                    }
                    _ => {}
                }
                // Redraw after every bounded edit; the shared surface keeps
                // the compositor's ownership and lifecycle unchanged.
                if !render_terminal!(
                    &window,
                    pointer,
                    pointer_pressed,
                    &line[..length],
                    status,
                    &clear,
                    &history,
                    &results,
                    history_count
                ) {
                    fail(9)
                }
                let _ = libwoven::print("WovenTerminal: input updated\n");
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
