#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::{rgb, Args, Canvas, GraphicsSurface};
use wovenhat_installer::{Screen, State};

mod service;

libwoven::entry!(main);

const EXIT_OK: u64 = 0;
const EXIT_USAGE: u64 = 2;
const GUI_WIDTH: u32 = 320;
const GUI_HEIGHT: u32 = 240;
const MAX_GUI_WIDTH: usize = 320;
const MAX_GUI_HEIGHT: usize = 240;
const BACKGROUND: u32 = rgb(12, 16, 28);
const PANEL: u32 = rgb(29, 38, 60);
const PANEL_DARK: u32 = rgb(21, 28, 45);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const MUTED: u32 = rgb(170, 185, 205);
const BUTTON: u32 = rgb(45, 65, 90);
const BUTTON_X: i32 = 210;
const BUTTON_Y: i32 = 195;
const BUTTON_WIDTH: usize = 92;
const BUTTON_HEIGHT: usize = 28;

struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_GUI_WIDTH * MAX_GUI_HEIGHT]>);

// SAFETY: the application is single-threaded and run_gui holds the only
// mutable reference while it renders into the process-owned buffer.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; MAX_GUI_WIDTH * MAX_GUI_HEIGHT]));

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--service") {
        service::run();
    }
    if args.get(1) == Some(b"--gui") {
        run_gui(args.get(2), args.get(3));
    }
    let input = args
        .get(1)
        .unwrap_or(b"screen=welcome;plan=validated;validated=1");
    if input == b"help" || input == b"--help" {
        let _ = libwoven::print(
            "WovenInstaller snapshot renderer; no install operation is available\n",
        );
        libwoven::exit(EXIT_OK);
    }
    let Ok(state) = State::parse(input) else {
        let _ = libwoven::eprint("WovenInstaller: invalid or unvalidated state; no action taken\n");
        libwoven::exit(EXIT_USAGE);
    };
    render(state);
    libwoven::exit(EXIT_OK)
}

fn run_gui(shell_arg: Option<&[u8]>, snapshot: Option<&[u8]>) -> ! {
    let Some(shell) = shell_arg.and_then(parse_u64).map(libwoven::process::Id) else {
        libwoven::exit(EXIT_USAGE)
    };
    let input = snapshot.unwrap_or(b"screen=welcome;plan=validated;validated=1");
    let Ok(state) = State::parse(input) else {
        let _ = libwoven::eprint("WovenInstaller: invalid or unvalidated GUI state\n");
        libwoven::exit(EXIT_USAGE);
    };
    let mut window = match GraphicsSurface::create(GUI_WIDTH, GUI_HEIGHT) {
        Ok(window) => window,
        Err(_) => libwoven::exit(3),
    };
    if !render_gui(&window, state, None, false) || window.share_with(shell).is_err() {
        libwoven::exit(4)
    }
    const TITLE: &[u8] = b"WovenInstaller";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(GUI_WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(GUI_HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err() {
        libwoven::exit(5)
    }

    let mut message = [0u8; 64];
    let mut pointer = None;
    let mut pressed = false;
    loop {
        if libwoven::wait_any(libwoven::wait::MESSAGE, None).is_err() {
            libwoven::exit(6)
        }
        let Ok((length, sender)) = libwoven::message_receive(&mut message) else {
            continue;
        };
        if sender != shell {
            continue;
        }
        if message.get(..length.min(4)) == Some(&b"WCLS"[..]) {
            break;
        }
        if message.get(..4) == Some(&b"WSIZ"[..]) && length == 8 {
            let width = u32::from(u16::from_le_bytes([message[4], message[5]])).min(MAX_GUI_WIDTH as u32);
            let height = u32::from(u16::from_le_bytes([message[6], message[7]])).min(MAX_GUI_HEIGHT as u32);
            if let Ok(resized) = GraphicsSurface::create(width, height) {
                if resized.share_with(shell).is_ok() && send_resize(&resized, shell)
                    && render_gui(&resized, state, pointer, pressed)
                {
                    let previous = core::mem::replace(&mut window, resized);
                    drop(previous);
                }
            }
            continue;
        }
        if message.get(..4) == Some(&b"WPMV"[..]) && length == 8 {
            pointer = Some((
                u16::from_le_bytes([message[4], message[5]]),
                u16::from_le_bytes([message[6], message[7]]),
            ));
            let _ = render_gui(&window, state, pointer, pressed);
            continue;
        }
        if message.get(..4) == Some(&b"WPBT"[..]) && length == 10 && message[4] == 0 {
            pressed = message[5] != 0;
            pointer = Some((
                u16::from_le_bytes([message[6], message[7]]),
                u16::from_le_bytes([message[8], message[9]]),
            ));
            if !pressed && in_close_button(pointer) {
                let _ = libwoven::message_send(shell, b"WCLS");
                break;
            }
            let _ = render_gui(&window, state, pointer, pressed);
        }
    }
    drop(window);
    libwoven::exit(EXIT_OK)
}

fn render_gui(window: &GraphicsSurface, state: State<'_>, pointer: Option<(u16, u16)>, pressed: bool) -> bool {
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
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(24, 20, canvas.width().saturating_sub(48), canvas.height().saturating_sub(40), PANEL);
    canvas.fill_rect(24, 20, canvas.width().saturating_sub(48), 42, PANEL_DARK);
    canvas.draw_text(42, 34, "WovenInstaller", TEXT);
    let (title, detail) = screen_text(state.screen);
    canvas.draw_text(42, 82, title, ACCENT);
    canvas.draw_text(42, 103, detail, MUTED);
    draw_field(&mut canvas, 42, 140, "Target", state.target);
    draw_field(&mut canvas, 42, 166, "Plan", state.plan);
    draw_field(&mut canvas, 42, 192, "Settings", state.settings);
    if !state.message.is_empty() {
        draw_field(&mut canvas, 42, 218, "Message", state.message);
    }
    canvas.fill_rect(
        BUTTON_X,
        BUTTON_Y,
        BUTTON_WIDTH,
        BUTTON_HEIGHT,
        if pressed { ACCENT } else { BUTTON },
    );
    canvas.draw_text(BUTTON_X + 26, BUTTON_Y + 9, "Close", TEXT);
    if let Some((x, y)) = pointer {
        canvas.fill_rect(i32::from(x).saturating_sub(1), i32::from(y).saturating_sub(1), 3, 3, ACCENT);
    }
    window.write_canvas(&canvas).is_ok()
}

fn draw_field(canvas: &mut Canvas<'_>, x: i32, y: i32, label: &str, value: &[u8]) {
    if value.is_empty() {
        return;
    }
    canvas.draw_text(x, y, label, MUTED);
    if let Ok(text) = core::str::from_utf8(value) {
        canvas.draw_text(x + 76, y, text, TEXT);
    }
}

fn screen_text(screen: Screen) -> (&'static str, &'static str) {
    match screen {
        Screen::Welcome => ("Welcome", "Review the validated installation plan."),
        Screen::TargetSelection => ("Target selection", "The selected target is shown below."),
        Screen::PartitionReview => ("Partition review", "Review the proposed partition layout."),
        Screen::Settings => ("Settings", "Review user, timezone, keyboard, and network settings."),
        Screen::Progress => ("Progress", "Installation progress is reported by the executor."),
        Screen::Cancelling => ("Cancelling", "The operation is being cancelled."),
        Screen::Cancelled => ("Cancelled", "No further action was taken."),
        Screen::Success => ("Success", "The validated operation completed."),
        Screen::Failure => ("Failure", "The operation failed closed."),
    }
}

fn in_close_button(pointer: Option<(u16, u16)>) -> bool {
    let Some((x, y)) = pointer else { return false };
    i32::from(x) >= BUTTON_X && i32::from(x) < BUTTON_X + BUTTON_WIDTH as i32
        && i32::from(y) >= BUTTON_Y && i32::from(y) < BUTTON_Y + BUTTON_HEIGHT as i32
}

fn send_resize(window: &GraphicsSurface, shell: libwoven::process::Id) -> bool {
    let mut response = [0u8; 16];
    response[..4].copy_from_slice(b"WRSZ");
    response[4..12].copy_from_slice(&window.handle().to_le_bytes());
    response[12..14].copy_from_slice(&(window.width() as u16).to_le_bytes());
    response[14..].copy_from_slice(&(window.height() as u16).to_le_bytes());
    libwoven::message_send(shell, &response).is_ok()
}

fn parse_u64(text: &[u8]) -> Option<u64> {
    if text.is_empty() { return None; }
    let mut value = 0u64;
    for digit in text {
        if !digit.is_ascii_digit() { return None; }
        value = value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))?;
    }
    Some(value)
}

fn render(state: State<'_>) {
    let _ = libwoven::print("WovenInstaller\n============\n");
    match state.screen {
        Screen::Welcome => print_title("Welcome", "Review a validated installation plan."),
        Screen::TargetSelection => print_title("Target", "Target selection is display-only."),
        Screen::PartitionReview => {
            print_title("Partition plan", "The validated plan is shown below.")
        }
        Screen::Settings => print_title(
            "Settings",
            "Settings are shown from the validated snapshot.",
        ),
        Screen::Progress => print_title(
            "Progress",
            "Installation execution is unavailable in this build.",
        ),
        Screen::Cancelling => print_title(
            "Cancelling",
            "Cancellation is represented; no operation is running.",
        ),
        Screen::Cancelled => print_title("Cancelled", "No installation operation was started."),
        Screen::Success => print_title(
            "Success",
            "This is a state-model success screen only; no install ran.",
        ),
        Screen::Failure => print_title("Failure", "The validated state reports a failure."),
    }
    print_field("target", state.target);
    print_field("plan", state.plan);
    print_field("settings", state.settings);
    print_field("message", state.message);
    if !state.error.is_empty() {
        print_field("error", state.error);
    }
    if matches!(state.screen, Screen::Progress | Screen::Success) {
        let _ = libwoven::print("progress=");
        let mut digits = [0u8; 3];
        let _ = libwoven::write(
            libwoven::syscall::STDOUT,
            libwoven::format_u64(u64::from(state.progress), &mut digits),
        );
        let _ = libwoven::print(" confirmed=");
        let _ = libwoven::print(if state.confirmed { "1\n" } else { "0\n" });
        let _ = libwoven::print("install_action=unavailable (fail-closed)\n");
    }
}

fn print_title(title: &str, detail: &str) {
    let _ = libwoven::print(title);
    let _ = libwoven::print("\n");
    let _ = libwoven::print(detail);
    let _ = libwoven::print("\n");
}

fn print_field(name: &str, value: &[u8]) {
    if value.is_empty() {
        return;
    }
    let _ = libwoven::print(name);
    let _ = libwoven::print("=");
    let _ = libwoven::write(libwoven::syscall::STDOUT, value);
    let _ = libwoven::print("\n");
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
