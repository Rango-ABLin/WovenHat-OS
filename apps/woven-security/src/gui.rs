use libwoven::{rgb, Canvas, GraphicsSurface};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 200;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const POINTER_MARKER: u32 = rgb(255, 80, 80);
const BUTTON: u32 = rgb(45, 65, 90);
const BUTTON_X: i32 = 250;
const BUTTON_Y: i32 = 145;
const BUTTON_WIDTH: usize = 80;
const BUTTON_HEIGHT: usize = 20;
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 176;
const MAX_EVENTS: u64 = 16;
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;
const MAX_ADMIN_ROWS: usize = 16;

#[derive(Clone, Copy)]
struct AdminRow {
    name: [u8; libwoven::accounts::MAX_NAME],
    length: usize,
    administrator: bool,
}

impl AdminRow {
    const EMPTY: Self = Self {
        name: [0; libwoven::accounts::MAX_NAME],
        length: 0,
        administrator: false,
    };
}

#[derive(Clone, Copy)]
struct AdminUi {
    mode: u8,
    session: Option<libwoven::identity::SessionId>,
    name: [u8; libwoven::accounts::MAX_NAME],
    name_length: usize,
    pin: [u8; libwoven::accounts::MAX_PIN],
    pin_length: usize,
    users: [AdminRow; MAX_ADMIN_ROWS],
    groups: [AdminRow; MAX_ADMIN_ROWS],
    user_count: usize,
    group_count: usize,
}

impl AdminUi {
    const EMPTY: Self = Self {
        mode: 0,
        session: None,
        name: [0; libwoven::accounts::MAX_NAME],
        name_length: 0,
        pin: [0; libwoven::accounts::MAX_PIN],
        pin_length: 0,
        users: [AdminRow::EMPTY; MAX_ADMIN_ROWS],
        groups: [AdminRow::EMPTY; MAX_ADMIN_ROWS],
        user_count: 0,
        group_count: 0,
    };

    fn clear_secret(&mut self) {
        for byte in &mut self.pin {
            // SAFETY: volatile clearing prevents the compiler from eliding
            // removal of the administrator credential from this UI state.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        self.pin_length = 0;
    }

    fn close(&mut self) {
        if let Some(session) = self.session.take() {
            let _ = libwoven::identity::session_close(session);
        }
        self.clear_secret();
        self.name.fill(0);
        self.name_length = 0;
        self.users = [AdminRow::EMPTY; MAX_ADMIN_ROWS];
        self.groups = [AdminRow::EMPTY; MAX_ADMIN_ROWS];
        self.user_count = 0;
        self.group_count = 0;
        self.mode = 0;
    }
}

struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: the security GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenSecurity: failure\n");
    libwoven::exit(code)
}

fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn draw_number(
    canvas: &mut Canvas<'_>,
    x: i32,
    y: i32,
    label: &str,
    value: u64,
    digits: &mut [u8; 20],
) {
    canvas.draw_text(x, y, label, TEXT);
    let text = libwoven::format_u64(value, digits);
    if let Ok(text) = core::str::from_utf8(text) {
        canvas.draw_text(x + label.len() as i32 * 6, y, text, ACCENT);
    }
}

fn audit_count() -> u64 {
    let mut count = 0;
    for offset in 0..MAX_EVENTS {
        match libwoven::audit_read(offset) {
            Ok(Some(_)) => count += 1,
            Ok(None) | Err(_) => break,
        }
    }
    count
}

fn draw_security(
    canvas: &mut Canvas<'_>,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    refresh_count: u32,
    admin: &AdminUi,
) {
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, PANEL);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, ACCENT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 8, "WovenSecurity", TEXT);
    if admin.mode != 0 {
        draw_admin(canvas, admin);
        if let Some((x, y)) = pointer {
            canvas.fill_rect(i32::from(x).saturating_sub(2), i32::from(y).saturating_sub(2), 5, 5, if pressed { TEXT } else { POINTER_MARKER });
        }
        return;
    }
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 36, "Current identity", ACCENT);
    let mut digits = [0u8; 20];
    // SAFETY: argument-free identity query.
    draw_number(canvas, PANEL_X + 10, PANEL_Y + 54, "UID ", unsafe {
        libwoven::syscall::getuid()
    }, &mut digits);
    // SAFETY: argument-free identity query.
    draw_number(canvas, PANEL_X + 150, PANEL_Y + 54, "GID ", unsafe {
        libwoven::syscall::getgid()
    }, &mut digits);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 88, "Audit trail", ACCENT);
    draw_number(canvas, PANEL_X + 10, PANEL_Y + 106, "Entries ", audit_count(), &mut digits);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 140, "Bounded view: 16 events", TEXT);
    let mut refresh_digits = [0u8; 20];
    draw_number(
        canvas,
        PANEL_X + 10,
        PANEL_Y + 156,
        "Refreshes ",
        u64::from(refresh_count),
        &mut refresh_digits,
    );
    canvas.fill_rect(
        BUTTON_X,
        BUTTON_Y,
        BUTTON_WIDTH,
        BUTTON_HEIGHT,
        if pressed { ACCENT } else { BUTTON },
    );
    canvas.draw_text(BUTTON_X + 14, BUTTON_Y + 6, "Refresh", TEXT);
    if let Some((x, y)) = pointer {
        canvas.fill_rect(i32::from(x).saturating_sub(2), i32::from(y).saturating_sub(2), 5, 5, if pressed { TEXT } else { POINTER_MARKER });
    }
}

fn draw_admin(canvas: &mut Canvas<'_>, admin: &AdminUi) {
    if admin.mode == 4 {
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 48, "Administrator access denied", TEXT);
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 82, "Press A to try again", TEXT);
        return;
    }
    if admin.mode < 3 {
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 36, "Administrator access", ACCENT);
        let prompt = if admin.mode == 1 { "Account name" } else { "Administrator PIN" };
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 60, prompt, TEXT);
        let mut field = [b' '; libwoven::accounts::MAX_PIN];
        let source = if admin.mode == 1 {
            &admin.name[..admin.name_length]
        } else {
            for byte in field.iter_mut().take(admin.pin_length) {
                *byte = b'*';
            }
            &field[..admin.pin_length]
        };
        let text = core::str::from_utf8(source).unwrap_or("");
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 82, text, TEXT);
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 116, "Enter: continue", TEXT);
        return;
    }
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 36, "Administrator directory", ACCENT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 52, "Users", ACCENT);
    for (row, item) in admin.users.iter().take(5).enumerate() {
        let name = core::str::from_utf8(&item.name[..item.length]).unwrap_or("?");
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 68 + row as i32 * 14, name, TEXT);
        if item.administrator {
            canvas.draw_text(PANEL_X + 125, PANEL_Y + 68 + row as i32 * 14, "admin", ACCENT);
        }
    }
    canvas.draw_text(PANEL_X + 190, PANEL_Y + 52, "Groups", ACCENT);
    for (row, item) in admin.groups.iter().take(5).enumerate() {
        let name = core::str::from_utf8(&item.name[..item.length]).unwrap_or("?");
        canvas.draw_text(PANEL_X + 190, PANEL_Y + 68 + row as i32 * 14, name, TEXT);
    }
    let mut digits = [0u8; 20];
    draw_number(canvas, PANEL_X + 10, PANEL_Y + 150, "Users ", admin.user_count as u64, &mut digits);
    draw_number(canvas, PANEL_X + 110, PANEL_Y + 150, "Groups ", admin.group_count as u64, &mut digits);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 168, "A: re-authenticate", TEXT);
}

fn refresh_admin(ui: &mut AdminUi) -> bool {
    let mut bytes = [0u8; libwoven::identity::ADMIN_QUERY_MAX_BYTES];
    let Ok(users) = libwoven::identity::admin_query(
        libwoven::identity::ADMIN_QUERY_USERS,
        &mut bytes,
    ) else {
        return false;
    };
    ui.users = [AdminRow::EMPTY; MAX_ADMIN_ROWS];
    ui.user_count = users.len().min(MAX_ADMIN_ROWS);
    for index in 0..ui.user_count {
        let Some(user) = users.user(index) else { return false };
        let row = &mut ui.users[index];
        row.length = user.name().len();
        row.name[..row.length].copy_from_slice(user.name());
        row.administrator = user.administrator;
    }
    let Ok(groups) = libwoven::identity::admin_query(
        libwoven::identity::ADMIN_QUERY_GROUPS,
        &mut bytes,
    ) else {
        return false;
    };
    ui.groups = [AdminRow::EMPTY; MAX_ADMIN_ROWS];
    ui.group_count = groups.len().min(MAX_ADMIN_ROWS);
    for index in 0..ui.group_count {
        let Some(group) = groups.group(index) else { return false };
        let row = &mut ui.groups[index];
        row.length = group.name().len();
        row.name[..row.length].copy_from_slice(group.name());
    }
    true
}

fn begin_admin(ui: &mut AdminUi) {
    let _ = libwoven::print("WovenSecurity: administrator authentication\n");
    if ui.name_length == 0 || ui.pin_length == 0 {
        ui.mode = 4;
        ui.clear_secret();
        return;
    }
    let Ok(session) = libwoven::identity::session_open(
        &ui.name[..ui.name_length],
        &ui.pin[..ui.pin_length],
    ) else {
        let _ = libwoven::print("WovenSecurity: administrator open denied\n");
        ui.mode = 4;
        ui.clear_secret();
        return;
    };
    if libwoven::identity::session_elevate(session, &ui.pin[..ui.pin_length]).is_err() {
        let _ = libwoven::print("WovenSecurity: administrator elevation denied\n");
        let _ = libwoven::identity::session_close(session);
        ui.clear_secret();
        ui.mode = 4;
        return;
    }
    ui.clear_secret();
    ui.session = Some(session);
    if refresh_admin(ui) {
        ui.mode = 3;
        let _ = libwoven::print("WovenSecurity: administrator directory ready\n");
    } else {
        let _ = libwoven::print("WovenSecurity: administrator query denied\n");
        ui.close();
        ui.mode = 4;
    }
}

fn render(
    window: &GraphicsSurface,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    refresh_count: u32,
    admin: &AdminUi,
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
    draw_security(&mut canvas, pointer, pressed, refresh_count, admin);
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

/// Open a read-only Security window through WovenShell's WOPN protocol.
pub fn run(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };
    let mut window = GraphicsSurface::create(WIDTH, HEIGHT).unwrap_or_else(|_| fail(1));
    let mut admin = AdminUi::EMPTY;
    if !render(&window, None, false, 0, &admin) {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Security";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err() {
        fail(5)
    }
    let _ = libwoven::print("WovenSecurity: window opened\n");
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
                admin.close();
                break;
            }
            if message.get(..4) == Some(&b"WSIZ"[..]) && length == 8 {
                let width = u32::from(u16::from_le_bytes([message[4], message[5]])).min(MAX_SURFACE_WIDTH as u32);
                let height = u32::from(u16::from_le_bytes([message[6], message[7]])).min(MAX_SURFACE_HEIGHT as u32);
                if let Ok(resized) = GraphicsSurface::create(width, height) {
                    if resized.share_with(shell).is_ok() && send_resize(&resized, shell) {
                        if render(&resized, pointer, pointer_pressed, refresh_count, &admin) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenSecurity: window resized\n");
                        } else {
                            drop(resized);
                        }
                    } else {
                        drop(resized);
                    }
                }
            } else if message.get(..4) == Some(&b"WPMV"[..]) && length == 8 {
                pointer = Some((u16::from_le_bytes([message[4], message[5]]), u16::from_le_bytes([message[6], message[7]])));
                if !render(&window, pointer, pointer_pressed, refresh_count, &admin) { fail(8) }
                let _ = libwoven::print("WovenSecurity: pointer updated\n");
            } else if message.get(..4) == Some(&b"WPBT"[..]) && length == 10 && message[4] == 0 {
                pointer_pressed = message[5] != 0;
                let local_x = u16::from_le_bytes([message[6], message[7]]);
                let local_y = u16::from_le_bytes([message[8], message[9]]);
                pointer = Some((local_x, local_y));
                if !pointer_pressed
                    && i32::from(local_x) >= BUTTON_X
                    && i32::from(local_x) < BUTTON_X + BUTTON_WIDTH as i32
                    && i32::from(local_y) >= BUTTON_Y
                    && i32::from(local_y) < BUTTON_Y + BUTTON_HEIGHT as i32
                {
                    refresh_count = refresh_count.saturating_add(1);
                    let _ = libwoven::print("WovenSecurity: pointer refreshed\n");
                }
                if !render(&window, pointer, pointer_pressed, refresh_count, &admin) { fail(8) }
                let _ = libwoven::print("WovenSecurity: pointer updated\n");
            } else if message.get(..4) == Some(&b"WKEY"[..]) && length >= 5 {
                match message[4] {
                    1 if admin.mode == 1 => admin.mode = 2,
                    1 if admin.mode == 2 => begin_admin(&mut admin),
                    2 if admin.mode == 1 => admin.name_length = admin.name_length.saturating_sub(1),
                    2 if admin.mode == 2 => admin.pin_length = admin.pin_length.saturating_sub(1),
                    _ => {}
                }
                if !render(&window, pointer, pointer_pressed, refresh_count, &admin) { fail(8) }
            } else if length == 1
                && message[0] == b'a'
                && matches!(admin.mode, 0 | 3 | 4)
            {
                if admin.mode == 0 {
                    admin.mode = 1;
                } else if admin.mode == 3 || admin.mode == 4 {
                    admin.close();
                    admin.mode = 1;
                }
                if !render(&window, pointer, pointer_pressed, refresh_count, &admin) { fail(8) }
            } else if length == 1 && message[0].is_ascii_graphic() {
                if admin.mode == 1 && admin.name_length < admin.name.len() {
                    admin.name[admin.name_length] = message[0];
                    admin.name_length += 1;
                } else if admin.mode == 2 && admin.pin_length < admin.pin.len() && message[0].is_ascii_digit() {
                    admin.pin[admin.pin_length] = message[0];
                    admin.pin_length += 1;
                }
                if !render(&window, pointer, pointer_pressed, refresh_count, &admin) { fail(8) }
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
        value = value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))?;
    }
    Some(value)
}
