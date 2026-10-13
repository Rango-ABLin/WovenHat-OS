//! Relink when the linker script changes: Cargo does not track files named
//! only in link arguments, so an edit to link.x would otherwise leave a stale
//! image for the kernel to embed.
fn main() {
    println!("cargo:rerun-if-changed=link.x");
}
