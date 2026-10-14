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
//! With `--damage-compositor` and `--damage-client <pid> <peer>` the pair
//! check damage-only compositing: the compositor sleeps on damage and must
//! copy exactly the region the client declared, and nothing it changed
//! without declaring.
//!
//! With `--wm` it is a small window manager: it stacks two clients' shared
//! windows, raises and focuses the next one on Tab or on a left click inside
//! it (it tracks the pointer from relative motion), and forwards each typed
//! character to the focused client only. `--wm-client <wm> <x> <y> <a|b>
//! <key>` is one such client: it places a window and expects exactly `<key>`.
//!
//! With `--alpha-compositor` it draws an opaque backdrop, a translucent
//! shadow from its own surface, and a `--client`'s shared window
//! translucently over both, then checks every region against the blend
//! formula.
//!
//! With `--scaled-compositor` it switches the display to scale 2, checks the
//! geometry the kernel reports, and animates a square paced by a
//! `FrameClock`, checking that no frame came early.
//!
//! With `--desktop-client <shell> <a|b|c>` it is an application on the
//! WovenShell desktop: it opens a window through the shell's protocol (and,
//! as `a`, a tray icon and a notification, then redraws part of its window),
//! and exits once the shell forwards it its key, or asks it to close. `c`
//! opens a window larger than the old 320×240 surface cap.
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
        Some(b"--damage-compositor") => damage_compositor(),
        Some(b"--damage-client") => damage_client(args.get(2), args.get(3)),
        Some(b"--wm") => window_manager(),
        Some(b"--alpha-compositor") => alpha_compositor(),
        Some(b"--scaled-compositor") => scaled_compositor(),
        Some(b"--desktop-client") => desktop_client(&args),
        Some(b"--wm-client") => wm_client(&args),
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

const DAMAGED_COLOR: u32 = rgb(20, 200, 60);
const UNDECLARED_COLOR: u32 = rgb(250, 250, 0);

/// Composites a client from its declared damage only.
fn damage_compositor() -> ! {
    use libwoven::{wait, Region};
    if libwoven::compositor_claim().is_err() {
        fail(100);
    }
    let _ = libwoven::print("hello-gui damage compositor: lease held\n");
    let mut message = [0u8; 8];
    let (length, client) = receive_bounded(&mut message, 101);
    if length != 8 {
        fail(102);
    }
    let token = u64::from_le_bytes(message);

    // The client declared the whole window before sending its token.
    match libwoven::compositor_blit_damage(token, CLIENT_X, CLIENT_Y) {
        Ok(Some(Region { x: 0, y: 0, width: 40, height: 30 })) => {}
        _ => fail(103),
    }
    if libwoven::screen_pixel(CLIENT_X + 10, CLIENT_Y + 5) != Ok(CLIENT_COLOR) {
        fail(104);
    }
    // Nothing more is pending.
    if libwoven::compositor_blit_damage(token, CLIENT_X, CLIENT_Y) != Ok(None) {
        fail(105);
    }
    let _ = libwoven::print("hello-gui damage compositor: window composited\n");

    // Sleep until the client damages its window. The gate lets the client
    // act only once this process is blocked here. The wait is level-checked,
    // so at its deadline it would still report damage that arrived without a
    // wakeup; timing it is what proves the damage itself woke this process.
    const WAKE_DEADLINE_TICKS: u64 = 500;
    let slept_at = libwoven::ticks();
    match libwoven::wait_any(wait::DAMAGE, Some(slept_at + WAKE_DEADLINE_TICKS)) {
        Ok(ready) if ready == wait::DAMAGE => {}
        Ok(_) => fail(106),
        Err(_) => fail(107),
    }
    if libwoven::ticks().wrapping_sub(slept_at) >= WAKE_DEADLINE_TICKS {
        fail(111);
    }
    match libwoven::compositor_blit_damage(token, CLIENT_X, CLIENT_Y) {
        Ok(Some(Region { x: 10, y: 5, width: 4, height: 3 })) => {}
        _ => fail(108),
    }
    // The declared region shows the new colour; the region the client
    // changed without declaring still shows the old one.
    if libwoven::screen_pixel(CLIENT_X + 10, CLIENT_Y + 5) != Ok(DAMAGED_COLOR)
        || libwoven::screen_pixel(CLIENT_X + 13, CLIENT_Y + 7) != Ok(DAMAGED_COLOR)
        || libwoven::screen_pixel(CLIENT_X + 30, CLIENT_Y + 20) != Ok(CLIENT_COLOR)
    {
        fail(109);
    }
    let _ = libwoven::print("hello-gui damage compositor: only the damage was copied\n");
    if libwoven::message_send(client, b"done").is_err() {
        fail(110);
    }
    libwoven::exit(0)
}

/// Declares the damage it makes, except for one deliberate omission.
fn damage_client(compositor: Option<&[u8]>, peer: Option<&[u8]>) -> ! {
    let (Some(compositor), Some(peer)) = (
        compositor.and_then(parse_u64).map(libwoven::process::Id),
        peer.and_then(parse_u64).map(libwoven::process::Id),
    ) else {
        fail(120)
    };
    let window = match GraphicsSurface::create(40, 30) {
        Ok(surface) => surface,
        Err(_) => fail(121),
    };
    if window.fill_rect(0, 0, 40, 30, CLIENT_COLOR).is_err()
        || window.share_with(compositor).is_err()
        || window.damage(0, 0, 40, 30).is_err()
    {
        fail(122);
    }
    if libwoven::message_send(compositor, &window.handle().to_le_bytes()).is_err() {
        fail(123);
    }
    // Wait for the gate's go-ahead, sent once the compositor is asleep.
    let mut go = [0u8; 4];
    let (length, sender) = receive_bounded(&mut go, 124);
    if sender != peer || &go[..length] != b"go" {
        fail(125);
    }
    if window.fill_rect(10, 5, 4, 3, DAMAGED_COLOR).is_err()
        || window.fill_rect(30, 20, 2, 2, UNDECLARED_COLOR).is_err()
        || window.damage(10, 5, 4, 3).is_err()
    {
        fail(126);
    }
    let _ = libwoven::print("hello-gui damage client: damage declared\n");
    let mut reply = [0u8; 8];
    let (length, sender) = receive_bounded(&mut reply, 127);
    if sender != compositor || &reply[..length] != b"done" {
        fail(128);
    }
    libwoven::exit(0)
}

const WM_WIDTH: u16 = 120;
const WM_HEIGHT: u16 = 80;

#[derive(Clone, Copy)]
struct Window {
    client: libwoven::process::Id,
    token: u64,
    x: i32,
    y: i32,
    color: u32,
}

impl Window {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x + i32::from(WM_WIDTH)
            && y < self.y + i32::from(WM_HEIGHT)
    }
}

/// Draw the windows bottom to top, present, and check that every window's
/// visible pixels show its own colour: the stacking order is what the screen
/// shows. Topmost is last.
fn composite(stack: &[Window; 2]) {
    for window in stack {
        if libwoven::compositor_blit_shared(window.token, window.x, window.y).is_err() {
            fail(140);
        }
    }
    if libwoven::compositor_present().is_err() {
        fail(141);
    }
    // A pixel inside both windows must belong to the top one; a pixel inside
    // only the bottom one must still show the bottom one.
    let (bottom, top) = (stack[0], stack[1]);
    let (shared_x, shared_y) = (top.x.max(bottom.x) + 2, top.y.max(bottom.y) + 2);
    if !(top.contains(shared_x, shared_y) && bottom.contains(shared_x, shared_y)) {
        fail(142);
    }
    if libwoven::screen_pixel(shared_x, shared_y) != Ok(top.color) {
        fail(143);
    }
    // Any corner of the bottom window that the top one does not cover.
    let (right, lower) = (
        bottom.x + i32::from(WM_WIDTH) - 2,
        bottom.y + i32::from(WM_HEIGHT) - 2,
    );
    let corners = [
        (bottom.x + 1, bottom.y + 1),
        (right, bottom.y + 1),
        (bottom.x + 1, lower),
        (right, lower),
    ];
    let Some(&(only_x, only_y)) = corners.iter().find(|&&(x, y)| !top.contains(x, y)) else {
        fail(145)
    };
    if libwoven::screen_pixel(only_x, only_y) != Ok(bottom.color) {
        fail(144);
    }
}

/// A window manager over two clients.
fn window_manager() -> ! {
    use libwoven::{wait, InputEvent, Key};
    if libwoven::compositor_claim().is_err() {
        fail(130);
    }
    // Taking the display also takes input focus.
    if libwoven::compositor_present().is_err() {
        fail(131);
    }
    let _ = libwoven::print("hello-gui wm: display and focus held\n");

    let mut windows = [None::<Window>; 2];
    for slot in windows.iter_mut() {
        let mut message = [0u8; 24];
        let (length, client) = receive_bounded(&mut message, 132);
        if length != 24 {
            fail(133);
        }
        let field = |index: usize| {
            u64::from_le_bytes(message[index * 8..index * 8 + 8].try_into().unwrap_or([0; 8]))
        };
        let origin = field(1);
        *slot = Some(Window {
            client,
            token: field(0),
            x: origin as u32 as i32,
            y: (origin >> 32) as u32 as i32,
            color: field(2) as u32,
        });
    }
    let (Some(first), Some(second)) = (windows[0], windows[1]) else {
        fail(134)
    };
    // Deterministic initial stack: the older client (lower pid) at the bottom.
    let mut stack = if first.client.0 < second.client.0 {
        [first, second]
    } else {
        [second, first]
    };
    composite(&stack);
    let _ = libwoven::print("hello-gui wm: windows stacked\n");

    // Tab, or a left click on the bottom window, raises and focuses it;
    // characters go to the focused (topmost) window's client and to no one
    // else. The pointer starts at the screen origin and follows relative
    // motion, clamped at zero.
    let (mut pointer_x, mut pointer_y) = (0i32, 0i32);
    let mut forwarded = 0;
    while forwarded < 2 {
        match libwoven::wait_any(wait::INPUT, Some(libwoven::ticks() + IPC_DEADLINE_TICKS)) {
            Ok(ready) if ready == wait::INPUT => {}
            _ => fail(135),
        }
        match libwoven::input_poll_event() {
            Ok(Some(InputEvent::Key(Key::Tab))) => {
                stack.swap(0, 1);
                composite(&stack);
                let _ = libwoven::print("hello-gui wm: raised and focused the other window\n");
            }
            Ok(Some(InputEvent::Key(Key::Char(character)))) => {
                let focused = stack[1].client;
                if libwoven::message_send(focused, &[character as u8]).is_err() {
                    fail(136);
                }
                forwarded += 1;
            }
            Ok(Some(InputEvent::PointerMove { dx, dy })) => {
                pointer_x = (pointer_x + i32::from(dx)).max(0);
                pointer_y = (pointer_y + i32::from(dy)).max(0);
            }
            Ok(Some(InputEvent::PointerButton { button: 0, pressed: true })) => {
                // Raise the topmost window under the pointer, if it is not
                // already on top. A click on empty desktop changes nothing.
                if !stack[1].contains(pointer_x, pointer_y) && stack[0].contains(pointer_x, pointer_y) {
                    stack.swap(0, 1);
                }
                composite(&stack);
                let _ = libwoven::print("hello-gui wm: click focused the window under the pointer\n");
            }
            Ok(Some(InputEvent::PointerButton { .. })) => {}
            _ => fail(137),
        }
    }
    let _ = libwoven::print("hello-gui wm: keys routed to the focused windows\n");
    libwoven::exit(0)
}

/// One window under the window manager; expects exactly one key.
fn wm_client(args: &Args) -> ! {
    let number = |index| args.get(index).and_then(parse_u64);
    let (Some(wm), Some(x), Some(y), Some(tag), Some(expected)) = (
        number(2).map(libwoven::process::Id),
        number(3),
        number(4),
        args.get(5),
        args.get(6),
    ) else {
        fail(150)
    };
    let color = match tag {
        b"a" => rgb(200, 60, 60),
        b"b" => rgb(60, 60, 200),
        _ => fail(151),
    };
    let window = match GraphicsSurface::create(u32::from(WM_WIDTH), u32::from(WM_HEIGHT)) {
        Ok(surface) => surface,
        Err(_) => fail(152),
    };
    if window.fill_rect(0, 0, WM_WIDTH, WM_HEIGHT, color).is_err()
        || window.share_with(wm).is_err()
    {
        fail(153);
    }
    let mut message = [0u8; 24];
    message[..8].copy_from_slice(&window.handle().to_le_bytes());
    message[8..16].copy_from_slice(&(x | (y << 32)).to_le_bytes());
    message[16..].copy_from_slice(&u64::from(color).to_le_bytes());
    if libwoven::message_send(wm, &message).is_err() {
        fail(154);
    }

    // Exactly one message from the window manager, carrying exactly our key.
    let mut key = [0u8; 4];
    let (length, sender) = receive_bounded(&mut key, 155);
    if sender != wm || length != 1 || key[0] != expected.first().copied().unwrap_or(0) {
        fail(156);
    }
    // And nothing more for a short quiet period. A misrouted key that comes
    // later still fails the gate: the window manager's send to this exited
    // client fails, and the client the key was meant for never receives it.
    let start = libwoven::ticks();
    while libwoven::ticks().wrapping_sub(start) < 50 {
        if libwoven::message_receive(&mut key).is_ok() {
            fail(157);
        }
        libwoven::yield_now();
    }
    let _ = libwoven::print("hello-gui wm client: received exactly its key\n");
    drop(window);
    libwoven::exit(0)
}

/// The kernel's blend formula, restated independently: per channel,
/// `(over * a + under * (255 - a) + 127) / 255`.
const fn blend(over: u32, under: u32, opacity: u8) -> u32 {
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

const BACKDROP: u32 = rgb(200, 200, 200);
const SHADOW_OPACITY: u8 = 96;
const WINDOW_OPACITY: u8 = 160;

/// Translucent window with a shadow, over an opaque backdrop.
fn alpha_compositor() -> ! {
    if libwoven::compositor_claim().is_err() {
        fail(160);
    }
    let backdrop = match GraphicsSurface::create(200, 150) {
        Ok(surface) => surface,
        Err(_) => fail(161),
    };
    let shadow = match GraphicsSurface::create(40, 30) {
        Ok(surface) => surface,
        Err(_) => fail(162),
    };
    if backdrop.fill_rect(0, 0, 200, 150, BACKDROP).is_err()
        || shadow.fill_rect(0, 0, 40, 30, rgb(0, 0, 0)).is_err()
        || backdrop.present(100, 100).is_err()
    {
        fail(163);
    }
    let _ = libwoven::print("hello-gui alpha: backdrop drawn\n");

    let mut message = [0u8; 16];
    let (length, client) = receive_bounded(&mut message, 164);
    if length != 16 {
        fail(165);
    }
    let window = u64::from_le_bytes(message[..8].try_into().unwrap_or([0; 8]));
    let private = u64::from_le_bytes(message[8..].try_into().unwrap_or([0; 8]));

    // Shadow offset down-right of the window, then the window over it.
    if shadow.present_blended(136, 136, SHADOW_OPACITY).is_err()
        || libwoven::compositor_blend_shared(window, 130, 130, WINDOW_OPACITY).is_err()
    {
        fail(166);
    }
    let shaded = blend(rgb(0, 0, 0), BACKDROP, SHADOW_OPACITY);
    let expected = [
        ((105, 105), BACKDROP),                                   // backdrop only
        ((172, 163), shaded),                                     // shadow only
        ((131, 131), blend(CLIENT_COLOR, BACKDROP, WINDOW_OPACITY)), // window on backdrop
        ((150, 150), blend(CLIENT_COLOR, shaded, WINDOW_OPACITY)),   // window on shadow
    ];
    for ((x, y), color) in expected {
        if libwoven::screen_pixel(x, y) != Ok(color) {
            fail(167);
        }
    }
    let _ = libwoven::print("hello-gui alpha: shadow and translucent window blended\n");

    // Opacity 0 changes nothing; the private surface is refused even
    // translucently; an opacity above 255 is refused rather than truncated.
    if libwoven::compositor_blend_shared(window, 400, 300, 0).is_err()
        || libwoven::screen_pixel(400, 300) != Ok(0)
        || libwoven::compositor_blend_shared(private, 0, 0, 128).is_ok()
    {
        fail(168);
    }
    // SAFETY: integer-only call; the kernel validates the opacity.
    if unsafe { syscall::graphics_compositor_blend_shared(window, 0, 0, 256) }
        != libwoven::syscall::ERROR
    {
        fail(169);
    }
    if libwoven::compositor_present().is_err() || libwoven::message_send(client, b"done").is_err() {
        fail(170);
    }
    drop(backdrop);
    drop(shadow);
    libwoven::exit(0)
}

/// An application on the WovenShell desktop.
fn desktop_client(args: &Args) -> ! {
    let Some(shell) = args.get(2).and_then(parse_u64).map(libwoven::process::Id) else {
        fail(180)
    };
    let (title, width, height, color, expected, extras): (&[u8], u16, u16, u32, u8, bool) =
        match args.get(3) {
            Some(b"a") => (b"Alpha", 120, 80, rgb(200, 60, 60), b'x', true),
            Some(b"b") => (b"Beta", 100, 60, rgb(60, 60, 200), b'y', false),
            Some(b"c") => (b"Wide", 400, 260, rgb(60, 160, 90), b'z', false),
            Some(b"n") => (b"Notes", 100, 60, rgb(150, 120, 40), b'n', false),
            Some(b"q") => (b"Quiet", 100, 60, rgb(90, 170, 170), b'q', false),
            _ => fail(181),
        };
    let mut window = match GraphicsSurface::create(u32::from(width), u32::from(height)) {
        Ok(surface) => surface,
        Err(_) => fail(182),
    };
    if window.fill_rect(0, 0, width, height, color).is_err() || window.share_with(shell).is_err() {
        fail(183);
    }
    let mut open = [0u8; 40];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&width.to_le_bytes());
    open[14..16].copy_from_slice(&height.to_le_bytes());
    open[16..16 + title.len()].copy_from_slice(title);
    if libwoven::message_send(shell, &open[..16 + title.len()]).is_err() {
        fail(184);
    }

    // Client A also shows a tray icon and a notification, then redraws part
    // of its window, which reaches the screen only through damage.
    let icon = if extras {
        let icon = match GraphicsSurface::create(16, 16) {
            Ok(surface) => surface,
            Err(_) => fail(185),
        };
        let mut tray = [0u8; 12];
        tray[..4].copy_from_slice(b"TRAY");
        tray[4..].copy_from_slice(&icon.handle().to_le_bytes());
        if icon.fill_rect(0, 0, 16, 16, rgb(40, 200, 90)).is_err()
            || icon.share_with(shell).is_err()
            || libwoven::message_send(shell, &tray).is_err()
            || libwoven::message_send(shell, b"NOTEAlpha ready").is_err()
        {
            fail(185);
        }
        let start = libwoven::ticks();
        while libwoven::ticks().wrapping_sub(start) < 100 {
            libwoven::yield_now();
        }
        if window.fill_rect(10, 10, 20, 10, rgb(255, 255, 255)).is_err()
            || window.damage(10, 10, 20, 10).is_err()
        {
            fail(186);
        }
        Some(icon)
    } else {
        None
    };

    // Client N posts three notifications, a little apart so the shell sees
    // them in order.
    if args.get(3) == Some(b"n") {
        for note in [&b"NOTEBuild done"[..], b"NOTEMail: 3 new", b"NOTEBattery 80%"] {
            if libwoven::message_send(shell, note).is_err() {
                fail(189);
            }
            let start = libwoven::ticks();
            while libwoven::ticks().wrapping_sub(start) < 10 {
                libwoven::yield_now();
            }
        }
    }

    // Exactly our key, from the shell.
    let deadline = libwoven::ticks() + 6000;
    let mut key = [0u8; 8];
    loop {
        match libwoven::wait_any(libwoven::wait::MESSAGE, Some(deadline)) {
            Ok(0) | Err(_) => fail(188),
            Ok(_) => {}
        }
        if let Ok((length, sender)) = libwoven::message_receive(&mut key) {
            // A resize request: answer with a new surface of that size,
            // then release the old one.
            if sender == shell && length == 8 && &key[..4] == b"WSIZ" {
                let w = u16::from_le_bytes([key[4], key[5]]);
                let h = u16::from_le_bytes([key[6], key[7]]);
                if let Ok(resized) = GraphicsSurface::create(u32::from(w), u32::from(h)) {
                    let mut reply = [0u8; 16];
                    reply[..4].copy_from_slice(b"WRSZ");
                    reply[4..12].copy_from_slice(&resized.handle().to_le_bytes());
                    reply[12..14].copy_from_slice(&w.to_le_bytes());
                    reply[14..].copy_from_slice(&h.to_le_bytes());
                    if resized.fill_rect(0, 0, w, h, color).is_ok()
                        && resized.share_with(shell).is_ok()
                        && libwoven::message_send(shell, &reply).is_ok()
                    {
                        window = resized;
                        let _ = libwoven::print("hello-gui desktop client: resized\n");
                    }
                }
                continue;
            }
            if sender == shell && &key[..length] == b"WCLS" {
                let _ = libwoven::print("hello-gui desktop client: closed by the shell\n");
                drop(icon);
                drop(window);
                libwoven::exit(0);
            }
            // Client Q posts a notification each time it is sent 'p'.
            if sender == shell && length == 1 && key[0] == b'p' && args.get(3) == Some(b"q") {
                if libwoven::message_send(shell, b"NOTEPing").is_err() {
                    fail(189);
                }
                continue;
            }
            if sender != shell || length != 1 || key[0] != expected {
                fail(187);
            }
            break;
        }
    }
    let _ = libwoven::print("hello-gui desktop client: received its key\n");
    drop(icon);
    drop(window);
    libwoven::exit(0)
}

const SQUARE: u32 = rgb(250, 210, 40);
const ANIMATION_FRAMES: u64 = 12;
const FRAME_PERIOD: u64 = 2;
const FRAME_STEP: i32 = 8;
const SQUARE_Y: i32 = 60;
const SQUARE_START: i32 = 10;

/// Display scale 2 and a paced animation.
fn scaled_compositor() -> ! {
    // The geometry belongs to the lease holder.
    if libwoven::compositor_info().is_ok() || libwoven::compositor_claim().is_err() {
        fail(171);
    }
    let full = match libwoven::compositor_info() {
        Ok(info) if info.scale == 1 => info,
        _ => fail(172),
    };
    // Out-of-range scales are refused and leave the geometry alone.
    if libwoven::compositor_set_scale(0).is_ok()
        || libwoven::compositor_set_scale(libwoven::MAX_DISPLAY_SCALE + 1).is_ok()
        || libwoven::compositor_info() != Ok(full)
    {
        fail(173);
    }
    let half = match libwoven::compositor_set_scale(2) {
        Ok(info) => info,
        Err(_) => fail(174),
    };
    // Unclamped, the surface is exactly half the framebuffer each way; the
    // kernel checks the exact geometry against the framebuffer itself.
    let exact = full.width < 1920 && full.height < 1200;
    if half.scale != 2
        || libwoven::compositor_info() != Ok(half)
        || (exact && (half.width != full.width / 2 || half.height != full.height / 2))
        || libwoven::screen_pixel(half.width as i32 - 1, half.height as i32 - 1) != Ok(0)
        || libwoven::screen_pixel(half.width as i32, 0).is_ok()
    {
        fail(174);
    }
    let _ = libwoven::print("hello-gui scaled: display at scale 2\n");

    let backdrop = match GraphicsSurface::create(200, 150) {
        Ok(surface) => surface,
        Err(_) => fail(175),
    };
    let square = match GraphicsSurface::create(16, 16) {
        Ok(surface) => surface,
        Err(_) => fail(175),
    };
    if backdrop.fill_rect(0, 0, 200, 150, BACKDROP).is_err()
        || square.fill_rect(0, 0, 16, 16, SQUARE).is_err()
        || backdrop.present(0, 0).is_err()
        || square.present(SQUARE_START, SQUARE_Y).is_err()
    {
        fail(175);
    }
    // The first present repaints the whole surface, each logical pixel as a
    // 2x2 block.
    let whole = u64::from(half.width * 2) * u64::from(half.height * 2);
    if libwoven::compositor_present() != Ok(whole) {
        fail(175);
    }

    // Animate: redraw the backdrop over the old square, draw the square at
    // its new position, present. Frames are paced by the clock; frames the
    // app is late for are skipped rather than delaying the rest.
    let mut clock = libwoven::FrameClock::new(FRAME_PERIOD);
    let start = clock.deadline() - FRAME_PERIOD;
    let mut frame = 0u64;
    let mut presented = 0u64;
    while frame < ANIMATION_FRAMES {
        let skipped = match clock.wait(0) {
            Ok(libwoven::FrameEvent::Frame { skipped }) => skipped,
            _ => fail(176),
        };
        frame = (frame + skipped + 1).min(ANIMATION_FRAMES);
        // No frame may be early: frame n is due at start + n * period.
        if libwoven::ticks() < start + frame * FRAME_PERIOD {
            fail(176);
        }
        let x = SQUARE_START + FRAME_STEP * frame as i32;
        if backdrop.present(0, 0).is_err() || square.present(x, SQUARE_Y).is_err() {
            fail(177);
        }
        // Only the backdrop's region is damaged: 200x150 at scale 2.
        if libwoven::compositor_present() != Ok(200 * 150 * 4) {
            fail(177);
        }
        presented += 1;
    }
    let elapsed = libwoven::ticks() - start;
    if elapsed < ANIMATION_FRAMES * FRAME_PERIOD || presented == 0 {
        fail(178);
    }
    let end = SQUARE_START + FRAME_STEP * ANIMATION_FRAMES as i32;
    if libwoven::screen_pixel(end + 8, SQUARE_Y + 8) != Ok(SQUARE)
        || libwoven::screen_pixel(SQUARE_START + 8, SQUARE_Y + 8) != Ok(BACKDROP)
        || libwoven::screen_pixel(300, 200) != Ok(0)
    {
        fail(179);
    }
    let mut line = [0u8; 80];
    let mut length = 0;
    let mut presented_digits = [0u8; 20];
    let mut elapsed_digits = [0u8; 20];
    for part in [
        &b"hello-gui scaled: "[..],
        format_u64(presented, &mut presented_digits),
        b" frames presented over ",
        format_u64(elapsed, &mut elapsed_digits),
        b" ticks\n",
    ] {
        line[length..length + part.len()].copy_from_slice(part);
        length += part.len();
    }
    let _ = libwoven::write(libwoven::syscall::STDOUT, &line[..length]);
    drop(backdrop);
    drop(square);
    libwoven::exit(0)
}

/// Decimal digits of `value`, written into the end of `buffer`.
fn format_u64(mut value: u64, buffer: &mut [u8; 20]) -> &[u8] {
    let mut index = buffer.len();
    loop {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    &buffer[index..]
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
    // Damage waits need DisplayIo; damage on a foreign surface is refused, as
    // is damage-only compositing without the lease.
    if libwoven::wait_any(libwoven::wait::DAMAGE, soon).is_ok() {
        fail(219);
    }
    // SAFETY: integer-only calls; the kernel validates every token.
    unsafe {
        if syscall::graphics_surface_damage(victim, 0, 0, 1, 1) != libwoven::syscall::ERROR {
            fail(220);
        }
    }
    if libwoven::compositor_blit_damage(victim, 0, 0).is_ok() {
        fail(221);
    }
    // Blending carries the same authority as blitting.
    if own.present_blended(0, 0, 128).is_ok()
        || libwoven::compositor_blend_shared(victim, 0, 0, 128).is_ok()
    {
        fail(222);
    }
    // So does the display geometry, read or changed.
    if libwoven::compositor_info().is_ok() || libwoven::compositor_set_scale(2).is_ok() {
        fail(223);
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
