//! A graphical WovenHat application.
//!
//! Allocates a client surface through the kernel, draws a small window into
//! it, and reads individual pixels back so the kernel's acceptance gate can
//! verify the whole user → kernel → surface path worked.
//!
//! Composition onto the framebuffer is Phase 5's work; this program's purpose
//! is to prove that a userspace process can own a drawing surface, fill it
//! correctly, and that the kernel is honouring the ownership rules.
#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::{rgb, GraphicsSurface};

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

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
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

    // 5. Place the window on the screen through the compositor, then verify
    //    by reading pixels back from the compositor surface. If the screen
    //    pixel at a known offset is the content colour, every step from
    //    syscall dispatch through client surface through compositor blit
    //    worked.
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

    // 6. Dropping the surface releases the client slot. The compositor keeps
    //    the pixels it was handed, which is the correct semantics for a
    //    once-presented window.
    drop(surface);
    let _ = libwoven::print("hello-gui: surface released\n");

    libwoven::exit(0)
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
