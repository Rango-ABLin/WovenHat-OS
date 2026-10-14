#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::{rgb, Args, Canvas, GraphicsSurface};
use wovenhat_calculator::evaluate;

libwoven::entry!(main);

const WIDTH: u32 = 240;
const HEIGHT: u32 = 150;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenCalculator: failure\n");
    libwoven::exit(code)
}

fn main(args: Args) -> ! {
    let gui = args.get(1) == Some(b"--gui");
    let expression = if gui {
        args.get(2).unwrap_or(b"2 + 2")
    } else {
        args.get(1).unwrap_or(b"2 + 2")
    };
    let result = match evaluate(expression) {
        Ok(value) => value,
        Err(_) => fail(2),
    };
    if !gui {
        run_console(expression, result);
    }
    run_gui(expression, result)
}

fn run_console(expression: &[u8], result: i64) -> ! {
    let _ = libwoven::print("WovenCalculator: ");
    let _ = libwoven::write(libwoven::syscall::STDOUT, expression);
    let _ = libwoven::print(" = ");
    let mut digits = [0u8; 24];
    let result_text = format_signed(result, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, result_text.as_bytes());
    let _ = libwoven::print("\n");
    libwoven::exit(0)
}

fn run_gui(expression: &[u8], result: i64) -> ! {
    let surface = match GraphicsSurface::create(WIDTH, HEIGHT) {
        Ok(surface) => surface,
        Err(_) => fail(3),
    };
    if surface.fill_rect(0, 0, WIDTH as u16, HEIGHT as u16, BACKGROUND).is_err()
        || surface.fill_rect(12, 12, 216, 126, PANEL).is_err()
        || surface.stroke_rect(12, 12, 216, 126, ACCENT).is_err()
    {
        fail(4);
    }
    let mut pixels = [0u32; 216 * 126];
    let mut canvas = match Canvas::new(&mut pixels, 12, 12, 216, 126) {
        Some(canvas) => canvas,
        None => fail(5),
    };
    canvas.draw_text(22, 24, "WovenCalculator", TEXT);
    canvas.draw_text(22, 48, "Expression:", TEXT);
    let expression_text = core::str::from_utf8(expression).unwrap_or("?");
    canvas.draw_text(22, 64, expression_text, TEXT);
    canvas.draw_text(22, 88, "Result:", ACCENT);
    let mut digits = [0u8; 24];
    let result_text = format_signed(result, &mut digits);
    canvas.draw_text(22, 104, result_text, ACCENT);
    if surface.write_canvas(&canvas).is_err() || surface.present(60, 40).is_err() {
        fail(6);
    }
    let _ = libwoven::print("WovenCalculator: ready\n");
    libwoven::exit(0)
}

fn format_signed(value: i64, output: &mut [u8; 24]) -> &str {
    let negative = value < 0;
    let mut number = value.unsigned_abs();
    let mut position = output.len();
    if number == 0 {
        position -= 1;
        output[position] = b'0';
    } else {
        while number != 0 {
            position -= 1;
            output[position] = b'0' + (number % 10) as u8;
            number /= 10;
        }
    }
    if negative {
        position -= 1;
        output[position] = b'-';
    }
    core::str::from_utf8(&output[position..]).unwrap_or("?")
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
