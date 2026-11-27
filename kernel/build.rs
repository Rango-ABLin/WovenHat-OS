use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../assets/wallpapers/manifest.toml");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR missing"));
    println!("cargo:rerun-if-env-changed=WOVENHAT_RECOVERY_IMAGE");
    let recovery = env::var_os("WOVENHAT_RECOVERY_IMAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("../target/system-apps")
                .join(env!("CARGO_PKG_VERSION"))
                .join("packages/woven-recovery.wapp")
        });
    println!("cargo:rerun-if-changed={}", recovery.display());
    let recovery_expression = if recovery.is_file() {
        format!(
            "include_bytes!({:?})",
            recovery
                .canonicalize()
                .expect("recovery image path")
                .to_string_lossy()
        )
    } else {
        // Existing lower-stage/system-apps builds remain compilable. A boot
        // explicitly selecting maintenance rejects this absent image at runtime.
        "&[]".into()
    };
    fs::write(out_dir.join("recovery-image.rs"), recovery_expression)
        .expect("recovery image expression");
    const WIDTH: usize = 160;
    const HEIGHT: usize = 90;
    const ROW_BYTES: usize = (WIDTH * 3 + 3) & !3;
    let mut bytes = vec![0u8; 54 + ROW_BYTES * HEIGHT];
    bytes[..2].copy_from_slice(b"BM");
    let file_length = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&file_length.to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&(WIDTH as i32).to_le_bytes());
    bytes[22..26].copy_from_slice(&(HEIGHT as i32).to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    bytes[34..38].copy_from_slice(&((ROW_BYTES * HEIGHT) as u32).to_le_bytes());
    // WovenHat palette: a vertical navy-to-teal gradient (matching
    // apps/woven-shell's WALLPAPER_TOP/WALLPAPER_BOTTOM) with a faint
    // basket-weave diagonal tint (matching the shell's own `wallpaper()`
    // weave motif), so this sample asset is visually consistent with the
    // live desktop background instead of an unrelated palette.
    const TOP: (u16, u16, u16) = (11, 29, 58);
    const BOTTOM: (u16, u16, u16) = (16, 78, 82);
    const WEAVE_PITCH: i32 = 14;
    const WEAVE_TINT: i16 = 10;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let t = (y * 255 / HEIGHT.max(1)) as u16;
            let lerp = |top: u16, bottom: u16| -> u8 {
                (top as u32 + (bottom as i32 - top as i32).max(0) as u32 * t as u32 / 255) as u8
            };
            let mut r = lerp(TOP.0, BOTTOM.0) as i16;
            let mut g = lerp(TOP.1, BOTTOM.1) as i16;
            let mut b = lerp(TOP.2, BOTTOM.2) as i16;
            let band = ((x as i32 + y as i32) / WEAVE_PITCH) % 2 == 0;
            let tint = if band { WEAVE_TINT } else { -WEAVE_TINT };
            r = (r + tint).clamp(0, 255);
            g = (g + tint).clamp(0, 255);
            b = (b + tint).clamp(0, 255);
            let row = HEIGHT - 1 - y;
            let offset = 54 + row * ROW_BYTES + x * 3;
            bytes[offset] = b as u8;
            bytes[offset + 1] = g as u8;
            bytes[offset + 2] = r as u8;
        }
    }
    let path = out_dir.join("wovenhat-wallpaper.bmp");
    fs::write(&path, bytes).expect("failed to write generated wallpaper");
    println!(
        "cargo:rustc-env=WOVENHAT_WALLPAPER_IMAGE={}",
        path.display()
    );
}
