use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../assets/wallpapers/manifest.toml");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR missing"));
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
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let t = (x * 255 / WIDTH) as u8;
            let row = HEIGHT - 1 - y;
            let offset = 54 + row * ROW_BYTES + x * 3;
            bytes[offset] = 80u8.saturating_add(t / 3);
            bytes[offset + 1] = 40u8.saturating_add(t / 2);
            bytes[offset + 2] = 18u8.saturating_add(t);
        }
    }
    let path = out_dir.join("wovenhat-wallpaper.bmp");
    fs::write(&path, bytes).expect("failed to write generated wallpaper");
    println!("cargo:rustc-env=WOVENHAT_WALLPAPER_IMAGE={}", path.display());
}
