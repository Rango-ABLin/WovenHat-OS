#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_images::{kind, parse, Command};

mod gui;

libwoven::entry!(main);

const MAX_ENTRIES: usize = 128;
const MAX_HEADER: usize = 32;

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        gui::run(args.get(2));
    }
    if args.get(0) == Some(b"/bin/wovenhat-images-self-test") || args.get(1) == Some(b"--self-test")
    {
        codec_self_test();
    }
    let mut raw = [&[][..]; 2];
    let mut count = 0usize;
    for index in 1..args.len() {
        if count == raw.len() {
            break;
        }
        if let Some(argument) = args.get(index) {
            raw[count] = argument;
            count += 1;
        }
    }
    let status = match parse(&raw[..count]) {
        Command::Help => {
            let _ = libwoven::print("WovenImages: --list PATH | --info PATH\n");
            0
        }
        Command::List(path) => list(path),
        Command::Info(path) => info(path),
        Command::Invalid => {
            let _ = libwoven::print("WovenImages: invalid command\n");
            2
        }
    };
    libwoven::exit(status)
}

fn codec_self_test() -> ! {
    let webp = [
        0x52, 0x49, 0x46, 0x46, 0x1e, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x4c, 0x11, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00, 0x00, 0x07, 0x50, 0x94, 0x52, 0x94,
        0xa7, 0xff, 0x81, 0x88, 0xe8, 0x7f, 0x00, 0x00,
    ];
    let mut pixels = [0u32; 4];
    if !matches!(
        libwoven::codecs::decode_webp_rgba(&webp, &mut pixels),
        Ok(metadata) if (metadata.width, metadata.height) == (2, 2)
    ) {
        let _ = libwoven::print("[WEBP] freestanding decode: FAILED\n");
        libwoven::exit(1);
    }
    let _ = libwoven::print("[WEBP] freestanding decode: PASSED 2x2\n");

    let mut bmp = [0u8; 62];
    bmp[..2].copy_from_slice(b"BM");
    bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&2i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&(-1i32).to_le_bytes());
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
    bmp[34..38].copy_from_slice(&8u32.to_le_bytes());
    bmp[54..62].copy_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
    let bmp_metadata = libwoven::media::parse_image_header(&bmp);
    let mut bmp_stream = SelfTestStream { bytes: &bmp };
    let mut bmp_pixels = [0u32; 2];
    if !matches!(
        bmp_metadata,
        Ok(metadata)
            if libwoven::codecs::decode_bmp_rgba_stream(
                &mut bmp_stream,
                metadata,
                &mut bmp_pixels
            )
            .is_ok()
            && bmp_pixels == [0xFF0000FF, 0x00FF00FF]
    ) {
        let _ = libwoven::print("[BMP] stream decode: FAILED\n");
        libwoven::exit(1);
    }
    let _ = libwoven::print("[BMP] stream decode: PASSED 2x1\n");

    let gif = [
        b'G', b'I', b'F', b'8', b'9', b'a', 1, 0, 1, 0, 0x80, 0, 0, 0, 0, 0, 255, 255, 255, 0x2c,
        0, 0, 0, 0, 1, 0, 1, 0, 0x40, 2, 2, 0x44, 1, 0, 0x3b,
    ];
    let gif_metadata = libwoven::media::parse_image_header(&gif);
    let mut gif_stream = SelfTestStream { bytes: &gif };
    let mut gif_pixels = [0u32; 1];
    if !matches!(
        gif_metadata,
        Ok(metadata)
            if libwoven::codecs::decode_gif_rgba_stream(
                &mut gif_stream,
                metadata,
                &mut gif_pixels
            )
            .is_ok()
            && gif_pixels[0] == 0x000000FF
    ) {
        let _ = libwoven::print("[GIF] stream decode: FAILED\n");
        libwoven::exit(1);
    }
    let _ = libwoven::print("[GIF] interlaced stream decode: PASSED 1x1\n");

    let animation = [
        b'G', b'I', b'F', b'8', b'9', b'a', 1, 0, 1, 0, 0x80, 0, 0, 0, 0, 0, 255, 255, 255, 0x2c,
        0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 0x44, 1, 0, 0x21, 0xf9, 4, 8, 5, 0, 0, 0, 0x2c, 0, 0, 0,
        0, 1, 0, 1, 0, 0, 2, 2, 0x4c, 1, 0, 0, 0x3b,
    ];
    let animation_metadata = libwoven::media::parse_image_header(&animation);
    let mut animation_stream = SelfTestStream { bytes: &animation };
    let mut animation_canvas = [0u32; 1];
    let mut animation_previous = [0u32; 1];
    let mut animation_sink = SelfTestGifSink::default();
    let animation_result = match animation_metadata {
        Ok(metadata) => libwoven::codecs::decode_gif_animation_stream(
            &mut animation_stream,
            metadata,
            &mut animation_canvas,
            &mut animation_previous,
            &mut animation_sink,
        ),
        Err(error) => Err(error),
    };
    if animation_result.is_err() {
        let _ = libwoven::print("[GIF] animation parser: FAILED\n");
        libwoven::exit(1);
    }
    let animation_ok = animation_sink.frames == 2
        && animation_sink.delays == [0, 5]
        && animation_sink.pixels == [0x000000FF, 0xFFFFFFFF];
    if !animation_ok {
        let _ = libwoven::print("[GIF] animation playback: FAILED\n");
        libwoven::exit(1);
    }
    let _ = libwoven::print("[GIF] animation playback: PASSED 2 frames\n");
    libwoven::exit(0)
}

#[derive(Default)]
struct SelfTestGifSink {
    frames: usize,
    delays: [u16; 2],
    pixels: [u32; 2],
}

impl libwoven::codecs::GifFrameSink for SelfTestGifSink {
    fn present(
        &mut self,
        info: libwoven::codecs::GifFrameInfo,
        pixels: &[u32],
    ) -> Result<(), libwoven::media::ImageError> {
        if self.frames >= self.pixels.len() || pixels.len() != 1 {
            return Err(libwoven::media::ImageError::Malformed);
        }
        self.delays[self.frames] = info.delay_centiseconds;
        self.pixels[self.frames] = pixels[0];
        self.frames += 1;
        Ok(())
    }
}

struct SelfTestStream<'a> {
    bytes: &'a [u8],
}

impl libwoven::codecs::ImageStream for SelfTestStream<'_> {
    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn read_at(
        &mut self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, libwoven::codecs::StreamError> {
        let start =
            usize::try_from(offset).map_err(|_| libwoven::codecs::StreamError::OutOfRange)?;
        if start > self.bytes.len() {
            return Err(libwoven::codecs::StreamError::OutOfRange);
        }
        let count = destination.len().min(self.bytes.len() - start);
        destination[..count].copy_from_slice(&self.bytes[start..start + count]);
        Ok(count)
    }
}

fn list(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let mut name = [0u8; libwoven::MAX_NAME];
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir(path, index, &mut name) {
            Ok((length, directory)) => {
                if directory {
                    continue;
                }
                let _ = libwoven::write(libwoven::syscall::STDOUT, &name[..length]);
                let _ = libwoven::print("\n");
            }
            Err(_) => break,
        }
    }
    0
}

fn info(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let Ok(descriptor) = libwoven::open(path) else {
        let _ = libwoven::print("WovenImages: cannot open file\n");
        return 2;
    };
    let mut header = [0u8; MAX_HEADER];
    let result = match libwoven::read(descriptor, &mut header) {
        Ok(length) => {
            let _ = libwoven::print("image: ");
            let _ = libwoven::print(kind(&header[..length]));
            let _ = libwoven::print(" header-bytes=");
            let mut digits = [0u8; 20];
            let text = libwoven::format_u64(length as u64, &mut digits);
            let _ = libwoven::write(libwoven::syscall::STDOUT, text);
            let _ = libwoven::print(" (bounded at 32)\n");
            0
        }
        Err(_) => 2,
    };
    let _ = libwoven::close(descriptor);
    result
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
