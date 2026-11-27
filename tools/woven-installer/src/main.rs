#![cfg_attr(target_os = "uefi", no_std)]
#![cfg_attr(target_os = "uefi", no_main)]

#[cfg(target_os = "uefi")]
mod firmware;

#[cfg(not(target_os = "uefi"))]
fn main() {
    eprintln!("Build for x86_64-unknown-uefi; see docs/installer.md");
}
