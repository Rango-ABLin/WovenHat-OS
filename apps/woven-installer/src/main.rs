#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_installer::{Screen, State};

libwoven::entry!(main);

const EXIT_OK: u64 = 0;
const EXIT_USAGE: u64 = 2;

fn main(args: Args) -> ! {
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
