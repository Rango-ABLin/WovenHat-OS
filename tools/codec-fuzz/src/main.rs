use libwoven::codecs::{
    GifFrameInfo, GifFrameSink, ImageStream, StreamError, decode_bmp_rgba_stream,
    decode_gif_animation_stream, decode_gif_rgba_stream, decode_jpeg_rgba_stream,
    decode_png_rgba_stream, decode_webp_rgba_stream,
};
use libwoven::media::{ImageFormat, ImageMetadata, parse_image_header};

struct RandomStream<'a> {
    bytes: &'a [u8],
}

impl ImageStream for RandomStream<'_> {
    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn read_at(&mut self, offset: u64, destination: &mut [u8]) -> Result<usize, StreamError> {
        let start = usize::try_from(offset).map_err(|_| StreamError::OutOfRange)?;
        if start > self.bytes.len() {
            return Err(StreamError::OutOfRange);
        }
        let count = destination.len().min(self.bytes.len() - start);
        destination[..count].copy_from_slice(&self.bytes[start..start + count]);
        Ok(count)
    }
}

struct Sink;

impl GifFrameSink for Sink {
    fn present(
        &mut self,
        _info: GifFrameInfo,
        _pixels: &[u32],
    ) -> Result<(), libwoven::media::ImageError> {
        Ok(())
    }
}

fn main() {
    let mut state = 0x6d2b_79f5_u32;
    let mut exercised = 0usize;
    for iteration in 0..50_000 {
        let length = (next(&mut state) as usize) % 2048;
        let mut bytes = vec![0u8; length];
        for byte in &mut bytes {
            *byte = next(&mut state) as u8;
        }
        let _ = parse_image_header(&bytes);
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Bmp,
            ImageFormat::Gif,
            ImageFormat::WebP,
        ] {
            let metadata = ImageMetadata {
                format,
                width: 1,
                height: 1,
            };
            let mut stream = RandomStream { bytes: &bytes };
            let mut pixels = [0u32; 1];
            match format {
                ImageFormat::Png => {
                    let _ = decode_png_rgba_stream(&mut stream, metadata, &mut pixels);
                }
                ImageFormat::Jpeg => {
                    let _ = decode_jpeg_rgba_stream(&mut stream, metadata, &mut pixels);
                }
                ImageFormat::Bmp => {
                    let _ = decode_bmp_rgba_stream(&mut stream, metadata, &mut pixels);
                }
                ImageFormat::Gif => {
                    let _ = decode_gif_rgba_stream(&mut stream, metadata, &mut pixels);
                    let mut previous = [0u32; 1];
                    let mut sink = Sink;
                    let _ = decode_gif_animation_stream(
                        &mut stream,
                        metadata,
                        &mut pixels,
                        &mut previous,
                        &mut sink,
                    );
                }
                ImageFormat::WebP => {
                    let _ = decode_webp_rgba_stream(&mut stream, metadata, &mut pixels);
                }
            }
            exercised += 1;
        }
        if iteration % 5000 == 0 {
            println!("codec fuzz iterations={iteration} cases={exercised}");
        }
    }
    println!("codec fuzz complete iterations=50000 cases={exercised}");
}

fn next(state: &mut u32) -> u32 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    *state = value;
    value
}
