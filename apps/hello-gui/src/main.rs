//! A graphical WovenHat application.
//!
//! In its normal mode it allocates a client surface, draws a small window,
//! verifies it by reading pixels back, takes the compositor lease, places the
//! window on the compositor, verifies the screen and presents it to the
//! framebuffer. That needs the `DisplayIo` capability.
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
        Some(_) => fail(250),
        None => run(),
    }
}

fn run() -> ! {
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

    // 7. Dropping the surface releases the client slot. The compositor keeps
    //    the pixels it was handed, which is the correct semantics for a
    //    once-presented window; the lease is released when the process exits.
    drop(surface);
    let _ = libwoven::print("hello-gui: surface released\n");

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
