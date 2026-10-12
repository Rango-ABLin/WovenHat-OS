//! A graphical WovenHat application.
//!
//! In its normal mode it allocates a client surface, draws a small window,
//! verifies it by reading pixels back, takes the compositor lease, places the
//! window on the compositor, verifies the screen and presents it to the
//! framebuffer. That needs the `DisplayIo` capability.
//!
//! With `--input` it then waits for the keys `o`, `k`, Enter, which the gate
//! publishes once this process holds input focus, and checks it received
//! exactly those, in order.
//!
//! With `--wait` it does the same through the blocking `input_wait`, sleeping
//! between keys, then checks that a timed wait with no input returns on
//! schedule.
//!
//! With `--compositor` it acts as a separate compositor process: it takes the
//! lease, receives two surface tokens from a client over IPC, draws the one
//! the client shared, and must be refused the one it did not. With
//! `--client <pid>` it is that client: it shares one surface with `<pid>`,
//! sends both tokens, and waits for the compositor's reply.
//!
//! With `--multiplex <peer>` it holds focus and sleeps in one `wait_any` over
//! input and IPC: it must be woken by a message from `<peer>`, then by a key,
//! then time out.
//!
//! With `--probe <handle>` it runs as an ordinary process and checks that the
//! kernel refuses it everything it has no authority for: every compositor
//! operation, and every operation on the surface `<handle>`, which belongs to
//! another process, including through a token forged to name this process.
//! Each failed check exits with its own status so the gate can tell which one.
#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::{rgb, syscall, Args, GraphicsSurface};

libwoven::entry!(main);

const BACKGROUND: u32 = rgb(4, 12, 28);
const TITLE_BAR: u32 = rgb(40, 220, 230);
const CONTENT: u32 = rgb(230, 230, 240);
const BORDER: u32 = rgb(0, 0, 0);

const WIDTH: u32 = 200;
const HEIGHT: u32 = 120;

/// Fail the process with a numeric reason so the gate can tell which step
/// failed from the exit status.
fn fail(code: u64) -> ! {
    let _ = libwoven::print("hello-gui: failure at step ");
    let mut digits = [0u8; 20];
    let text = libwoven::format_u64(code, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, text);
    let _ = libwoven::print("\n");
    libwoven::exit(code)
}

fn main(args: Args) -> ! {
    match args.get(1) {
        Some(b"--probe") => probe(args.get(2)),
        Some(b"--input") => run(Input::Poll),
        Some(b"--wait") => run(Input::Wait),
        Some(b"--compositor") => compositor(),
        Some(b"--client") => client(args.get(2)),
        Some(b"--multiplex") => multiplex(args.get(2)),
        Some(_) => fail(250),
        None => run(Input::None),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    None,
    Poll,
    Wait,
}

fn run(input: Input) -> ! {
    // 1. Allocate the surface.
    let surface = match GraphicsSurface::create(WIDTH, HEIGHT) {
        Ok(surface) => surface,
        Err(_) => fail(1),
    };
    let _ = libwoven::print("hello-gui: surface created\n");

    // 2. Paint the background.
    if surface
        .fill_rect(0, 0, WIDTH as u16, HEIGHT as u16, BACKGROUND)
        .is_err()
    {
        fail(2);
    }

    // 3. A window: title bar, content area, border.
    if surface.fill_rect(10, 10, 180, 16, TITLE_BAR).is_err() {
        fail(3);
    }
    if surface.fill_rect(10, 26, 180, 80, CONTENT).is_err() {
        fail(4);
    }
    // One stroke rather than four fills. Besides being clearer, this
    // exercises the stroke syscall path.
    if surface.stroke_rect(10, 10, 180, 96, BORDER).is_err() {
        fail(5);
    }

    // 4. Verify by reading back representative pixels. These coordinates fall
    //    in each distinct region, so a wrong fill or an off-by-one would show.
    //    Reading from userspace is what makes this a kernel contract test
    //    rather than a userspace self-check.
    let samples = [
        (5u32, 5u32, BACKGROUND),      // outside window
        (100, 15, TITLE_BAR),          // inside title bar
        (100, 60, CONTENT),            // content area
        (10, 10, BORDER),              // top-left corner
        (189, 105, BORDER),            // bottom-right corner
        (10, 50, BORDER),              // left edge
        (189, 50, BORDER),             // right edge
    ];
    for (x, y, expected) in samples.iter().copied() {
        match surface.pixel(x, y) {
            Ok(actual) if actual == expected => {}
            Ok(_) => fail(10 + y as u64),
            Err(_) => fail(100 + y as u64),
        }
    }
    let _ = libwoven::print("hello-gui: window rendered and verified\n");

    // 5. Take the compositor lease, place the window on the screen, and
    //    verify by reading the compositor surface back. If the screen pixel
    //    at a known offset is the content colour, every step from syscall
    //    dispatch through client surface through compositor blit worked.
    if libwoven::compositor_claim().is_err() {
        fail(11);
    }
    const SCREEN_X: i32 = 60;
    const SCREEN_Y: i32 = 40;
    if surface.present(SCREEN_X, SCREEN_Y).is_err() {
        fail(6);
    }
    let _ = libwoven::print("hello-gui: window presented\n");

    // Pick a point inside the content area in screen coordinates.
    let content_x = SCREEN_X + 100;
    let content_y = SCREEN_Y + 60;
    match libwoven::screen_pixel(content_x, content_y) {
        Ok(color) if color == CONTENT => {}
        Ok(_) => fail(7),
        Err(_) => fail(8),
    }
    // A point outside the window should still carry the compositor background
    // (0), proving nothing wrote where it was not asked to.
    match libwoven::screen_pixel(SCREEN_X - 5, SCREEN_Y - 5) {
        Ok(0) => {}
        Ok(_) => fail(9),
        Err(_) => fail(10),
    }
    let _ = libwoven::print("hello-gui: screen verified\n");

    // 6. Show it: copy the compositor's damage to the framebuffer.
    match libwoven::compositor_present() {
        Ok(written) if written > 0 => {}
        Ok(_) => fail(12),
        Err(_) => fail(13),
    }
    let _ = libwoven::print("hello-gui: framebuffer presented\n");

    match input {
        Input::None => {}
        Input::Poll => await_keys(),
        Input::Wait => await_keys_blocking(),
    }

    // 7. Dropping the surface releases the client slot. The compositor keeps
    //    the pixels it was handed, which is the correct semantics for a
    //    once-presented window; the lease is released when the process exits.
    drop(surface);
    let _ = libwoven::print("hello-gui: surface released\n");

    libwoven::exit(0)
}

/// Wait for exactly `o`, `k`, Enter. The kernel publishes them once this
/// process holds focus, so a wrong, missing or extra key is a routing defect.
fn await_keys() {
    use libwoven::Key;
    const EXPECTED: [Key; 3] = [Key::Char('o'), Key::Char('k'), Key::Enter];
    const DEADLINE_TICKS: u64 = 2000;

    let start = libwoven::ticks();
    let mut received = 0;
    while received < EXPECTED.len() {
        match libwoven::input_poll() {
            Ok(Some(key)) if key == EXPECTED[received] => received += 1,
            Ok(Some(_)) => fail(30 + received as u64),
            Ok(None) => {
                if libwoven::ticks().wrapping_sub(start) > DEADLINE_TICKS {
                    fail(34);
                }
                libwoven::yield_now();
            }
            Err(_) => fail(29),
        }
    }
    if !matches!(libwoven::input_poll(), Ok(None)) {
        fail(33);
    }
    let _ = libwoven::print("hello-gui: input received\n");
}

/// The `await_keys` contract through the blocking call: no polling loop, the
/// process sleeps in the kernel between keys. Then a timed wait with nothing
/// pending must time out, and not before its deadline.
fn await_keys_blocking() {
    use libwoven::Key;
    const EXPECTED: [Key; 3] = [Key::Char('o'), Key::Char('k'), Key::Enter];
    const TIMEOUT_TICKS: u64 = 20;

    for (index, expected) in EXPECTED.iter().enumerate() {
        match libwoven::input_wait(None) {
            Ok(Some(key)) if key == *expected => {}
            Ok(Some(_)) => fail(40 + index as u64),
            Ok(None) => fail(44),
            Err(_) => fail(45),
        }
    }
    let _ = libwoven::print("hello-gui: blocking input received\n");

    let start = libwoven::ticks();
    match libwoven::input_wait(Some(start + TIMEOUT_TICKS)) {
        Ok(None) => {}
        Ok(Some(_)) => fail(46),
        Err(_) => fail(47),
    }
    if libwoven::ticks().wrapping_sub(start) < TIMEOUT_TICKS {
        fail(48);
    }
    let _ = libwoven::print("hello-gui: wait timed out on schedule\n");
}

const CLIENT_COLOR: u32 = rgb(180, 40, 90);
const PRIVATE_COLOR: u32 = rgb(9, 9, 9);
const CLIENT_X: i32 = 300;
const CLIENT_Y: i32 = 200;
const IPC_DEADLINE_TICKS: u64 = 2000;

/// Wait for a message, bounded so a missing peer fails the gate instead of
/// hanging it.
fn receive_bounded(buffer: &mut [u8], code: u64) -> (usize, libwoven::process::Id) {
    let start = libwoven::ticks();
    loop {
        if let Ok(received) = libwoven::message_receive(buffer) {
            return received;
        }
        if libwoven::ticks().wrapping_sub(start) > IPC_DEADLINE_TICKS {
            fail(code);
        }
        libwoven::yield_now();
    }
}

/// A compositor process: draws a client's window from a shared surface.
fn compositor() -> ! {
    if libwoven::compositor_claim().is_err() {
        fail(60);
    }
    let _ = libwoven::print("hello-gui compositor: lease held\n");

    let mut message = [0u8; 16];
    let (length, client) = receive_bounded(&mut message, 61);
    if length != 16 {
        fail(62);
    }
    let shared = u64::from_le_bytes(message[..8].try_into().unwrap_or([0; 8]));
    let private = u64::from_le_bytes(message[8..].try_into().unwrap_or([0; 8]));

    // The shared surface is drawable, and lands with the client's pixels.
    if libwoven::compositor_blit_shared(shared, CLIENT_X, CLIENT_Y).is_err() {
        fail(63);
    }
    match libwoven::screen_pixel(CLIENT_X, CLIENT_Y) {
        Ok(color) if color == CLIENT_COLOR => {}
        _ => fail(64),
    }
    // The surface the client kept private is refused, even with its genuine
    // token, and so is the shared one through the owner-only blit path.
    if libwoven::compositor_blit_shared(private, 0, 0).is_ok() {
        fail(65);
    }
    // SAFETY: integer-only call; the kernel validates the token.
    if unsafe { syscall::graphics_compositor_blit(shared, 0, 0) } != libwoven::syscall::ERROR {
        fail(66);
    }
    let _ = libwoven::print("hello-gui compositor: client window composited\n");

    match libwoven::compositor_present() {
        Ok(written) if written > 0 => {}
        _ => fail(67),
    }
    if libwoven::message_send(client, b"done").is_err() {
        fail(68);
    }
    libwoven::exit(0)
}

/// A client of a compositor process.
fn client(compositor: Option<&[u8]>) -> ! {
    let Some(compositor) = compositor.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(70)
    };
    let window = match GraphicsSurface::create(40, 30) {
        Ok(surface) => surface,
        Err(_) => fail(71),
    };
    let private = match GraphicsSurface::create(8, 8) {
        Ok(surface) => surface,
        Err(_) => fail(72),
    };
    if window.fill_rect(0, 0, 40, 30, CLIENT_COLOR).is_err()
        || private.fill_rect(0, 0, 8, 8, PRIVATE_COLOR).is_err()
    {
        fail(73);
    }
    if window.share_with(compositor).is_err() {
        fail(74);
    }
    // A client has no display authority of its own.
    if libwoven::compositor_blit_shared(window.handle(), 0, 0).is_ok() {
        fail(75);
    }
    let mut message = [0u8; 16];
    message[..8].copy_from_slice(&window.handle().to_le_bytes());
    message[8..].copy_from_slice(&private.handle().to_le_bytes());
    if libwoven::message_send(compositor, &message).is_err() {
        fail(76);
    }
    let _ = libwoven::print("hello-gui client: window shared\n");

    // Keep the surfaces alive until the compositor has drawn them.
    let mut reply = [0u8; 8];
    let (length, sender) = receive_bounded(&mut reply, 77);
    if sender != compositor || &reply[..length] != b"done" {
        fail(78);
    }
    let _ = libwoven::print("hello-gui client: compositor confirmed\n");
    libwoven::exit(0)
}

/// One sleep over input and messages.
fn multiplex(peer: Option<&[u8]>) -> ! {
    use libwoven::{wait, Key};
    let Some(peer) = peer.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(80)
    };
    // Take the display, and with it focus.
    let surface = match GraphicsSurface::create(16, 16) {
        Ok(surface) => surface,
        Err(_) => fail(81),
    };
    if surface.fill_rect(0, 0, 16, 16, CONTENT).is_err()
        || libwoven::compositor_claim().is_err()
        || surface.present(0, 0).is_err()
        || libwoven::compositor_present().is_err()
    {
        fail(82);
    }
    let _ = libwoven::print("hello-gui multiplex: focus held\n");

    let both = wait::INPUT | wait::MESSAGE;
    // The gate sends a message first: only the message source is ready.
    match libwoven::wait_any(both, None) {
        Ok(ready) if ready == wait::MESSAGE => {}
        Ok(_) => fail(83),
        Err(_) => fail(84),
    }
    let mut message = [0u8; 8];
    match libwoven::message_receive(&mut message) {
        Ok((4, sender)) if sender == peer && &message[..4] == b"ping" => {}
        _ => fail(85),
    }
    // Then a key: only input is ready.
    match libwoven::wait_any(both, None) {
        Ok(ready) if ready == wait::INPUT => {}
        Ok(_) => fail(86),
        Err(_) => fail(87),
    }
    if libwoven::input_poll() != Ok(Some(Key::Char('z'))) {
        fail(88);
    }
    let _ = libwoven::print("hello-gui multiplex: woken by message, then by key\n");

    // Nothing else is coming: a timed wait returns 0, not before its deadline.
    const TIMEOUT_TICKS: u64 = 20;
    let start = libwoven::ticks();
    if libwoven::wait_any(both, Some(start + TIMEOUT_TICKS)) != Ok(0) {
        fail(89);
    }
    if libwoven::ticks().wrapping_sub(start) < TIMEOUT_TICKS {
        fail(90);
    }
    let _ = libwoven::print("hello-gui multiplex: timed out on schedule\n");
    drop(surface);
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

/// Every call here must be refused. Exit 0 means the kernel refused them all.
fn probe(victim: Option<&[u8]>) -> ! {
    let Some(victim) = victim.and_then(parse_u64) else {
        fail(200)
    };

    // Client surfaces need no authority: the probe can still draw for itself.
    let own = match GraphicsSurface::create(16, 16) {
        Ok(surface) => surface,
        Err(_) => fail(201),
    };
    if own.fill_rect(0, 0, 16, 16, CONTENT).is_err() {
        fail(202);
    }

    // Without DisplayIo, the compositor is off limits.
    if libwoven::compositor_claim().is_ok() {
        fail(203);
    }
    if own.present(0, 0).is_ok() {
        fail(204);
    }
    if libwoven::screen_pixel(0, 0).is_ok() {
        fail(205);
    }
    if libwoven::compositor_present().is_ok() {
        fail(206);
    }
    // Without InputIo and focus, input is off limits too.
    if libwoven::input_poll().is_ok() {
        fail(215);
    }
    // Bounded deadline, so a broken check cannot hang the gate.
    if libwoven::input_wait(Some(libwoven::ticks() + 1)).is_ok() {
        fail(216);
    }
    // The combined wait: input needs InputIo, and malformed masks are refused.
    let soon = Some(libwoven::ticks() + 1);
    if libwoven::wait_any(libwoven::wait::INPUT, soon).is_ok() {
        fail(217);
    }
    if libwoven::wait_any(0, soon).is_ok() || libwoven::wait_any(1 << 7, soon).is_ok() {
        fail(218);
    }
    let _ = libwoven::print("hello-gui probe: compositor refused\n");

    // Another process's surface, through its genuine token...
    // SAFETY: integer-only calls; the kernel validates every token.
    unsafe {
        if syscall::graphics_surface_pixel(victim, 0, 0) != libwoven::syscall::ERROR {
            fail(207);
        }
        if syscall::graphics_surface_fill(victim, 0, 0, 8, 8, BORDER) != libwoven::syscall::ERROR {
            fail(208);
        }
        if syscall::graphics_surface_stroke(victim, 0, 0, 8, 8, BORDER) != libwoven::syscall::ERROR
        {
            fail(209);
        }
        if syscall::graphics_compositor_blit(victim, 0, 0) != libwoven::syscall::ERROR {
            fail(210);
        }
        if syscall::graphics_surface_destroy(victim) != libwoven::syscall::ERROR {
            fail(211);
        }
    }

    // ...and through a token forged to name this process as the owner, with
    // the victim's slot and generation. The registry must still refuse it.
    let forged = (victim & 0xffff_ffff) | (libwoven::current_pid().0 << 32);
    // SAFETY: integer-only calls; the kernel validates every token.
    unsafe {
        if syscall::graphics_surface_pixel(forged, 0, 0) != libwoven::syscall::ERROR {
            fail(212);
        }
        if syscall::graphics_surface_fill(forged, 0, 0, 8, 8, BORDER) != libwoven::syscall::ERROR {
            fail(213);
        }
        if syscall::graphics_surface_destroy(forged) != libwoven::syscall::ERROR {
            fail(214);
        }
    }
    let _ = libwoven::print("hello-gui probe: foreign surface refused\n");

    drop(own);
    libwoven::exit(0)
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
