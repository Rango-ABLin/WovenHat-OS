//! Bounded pixel decoders for the image asset boundary.

use crate::media::{ImageError, ImageFormat, ImageMetadata, MAX_PIXELS, parse_image_header};
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_png::PngDecoder;
use zune_jpeg::JpegDecoder;

fn capacity(metadata: ImageMetadata, output: &[u32]) -> Result<usize, ImageError> {
    let pixels = (metadata.width as usize)
        .checked_mul(metadata.height as usize)
        .ok_or(ImageError::InvalidDimensions)?;
    if pixels > MAX_PIXELS || output.len() < pixels {
        return Err(ImageError::OutputTooSmall);
    }
    Ok(pixels)
}

#[cfg(not(target_os = "none"))]
fn map_rgba(bytes: &[u8], output: &mut [u32]) -> Result<(), ImageError> {
    if bytes.len() != output.len().saturating_mul(4) {
        return Err(ImageError::Malformed);
    }
    let (chunks, remainder) = bytes.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err(ImageError::Malformed);
    }
    for (pixel, chunk) in output.iter_mut().zip(chunks) {
        *pixel = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    Ok(())
}

pub fn decode_png_rgba(bytes: &[u8], output: &mut [u32]) -> Result<ImageMetadata, ImageError> {
    let metadata = parse_image_header(bytes)?;
    if metadata.format != ImageFormat::Png {
        return Err(ImageError::UnsupportedEncoding);
    }
    capacity(metadata, output)?;
    let options = DecoderOptions::default()
        .png_set_add_alpha_channel(true)
        .png_set_strip_to_8bit(true);
    let mut decoder = PngDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_into(bytemuck_slice(output)).map_err(|_| ImageError::Malformed)?;
    Ok(metadata)
}

pub fn decode_jpeg_rgba(bytes: &[u8], output: &mut [u32]) -> Result<ImageMetadata, ImageError> {
    let metadata = parse_image_header(bytes)?;
    if metadata.format != ImageFormat::Jpeg {
        return Err(ImageError::UnsupportedEncoding);
    }
    let pixels = capacity(metadata, output)?;
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGB)
        .set_use_unsafe(false);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().map_err(|_| ImageError::Malformed)?;
    let mut rgb = alloc::vec![0u8; pixels.saturating_mul(3)];
    decoder.decode_into(&mut rgb).map_err(|_| ImageError::Malformed)?;
    let (chunks, remainder) = rgb.as_chunks::<3>();
    if !remainder.is_empty() {
        return Err(ImageError::Malformed);
    }
    for (pixel, chunk) in output.iter_mut().zip(chunks) {
        *pixel = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], 255]);
    }
    Ok(metadata)
}

pub fn decode_webp_rgba(bytes: &[u8], output: &mut [u32]) -> Result<ImageMetadata, ImageError> {
    let metadata = parse_image_header(bytes)?;
    if metadata.format != ImageFormat::WebP {
        return Err(ImageError::UnsupportedEncoding);
    }
    capacity(metadata, output)?;
    decode_webp_pixels(bytes, output, metadata)?;
    Ok(metadata)
}

#[cfg(target_os = "none")]
fn decode_webp_pixels(
    bytes: &[u8],
    output: &mut [u32],
    metadata: ImageMetadata,
) -> Result<(), ImageError> {
    zenwebp::decoder::decode_rgba_into(bytes, bytemuck_slice(output), metadata.width)
        .map_err(|_| ImageError::Malformed)?;
    Ok(())
}

#[cfg(not(target_os = "none"))]
fn decode_webp_pixels(
    bytes: &[u8],
    output: &mut [u32],
    metadata: ImageMetadata,
) -> Result<(), ImageError> {
    let image = oxideav_webp::decode_rgba8(bytes).map_err(|_| ImageError::Malformed)?;
    if image.width != metadata.width || image.height != metadata.height {
        return Err(ImageError::Malformed);
    }
    map_rgba(image.as_bytes(), output)
}

// The decoders accept a byte slice and the output is caller-owned u32 storage.
// This cast is valid because u32 has alignment 4 and the output slice is a
// contiguous initialized pixel buffer; the decoder only writes bytes.
fn bytemuck_slice(output: &mut [u32]) -> &mut [u8] {
    unsafe {
        core::slice::from_raw_parts_mut(output.as_mut_ptr().cast::<u8>(), output.len() * 4)
    }
}

#[cfg(test)]
mod tests {
    use super::decode_webp_rgba;

    #[test]
    fn decodes_lossless_webp_into_caller_pixels() {
        let webp = [
            0x52, 0x49, 0x46, 0x46, 0x1e, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
            0x56, 0x50, 0x38, 0x4c, 0x11, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00,
            0x00, 0x07, 0x50, 0x94, 0x52, 0x94, 0xa7, 0xff, 0x81, 0x88, 0xe8, 0x7f,
            0x00, 0x00,
        ];
        let mut pixels = [0u32; 4];
        let metadata = decode_webp_rgba(&webp, &mut pixels).unwrap();
        assert_eq!((metadata.width, metadata.height), (2, 2));
        assert!(pixels.iter().all(|pixel| *pixel != 0));
    }

    #[test]
    fn decodes_extended_lossy_webp_with_alpha() {
        let webp = [
            0x52, 0x49, 0x46, 0x46, 0x54, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
            0x56, 0x50, 0x38, 0x58, 0x0a, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x41, 0x4c, 0x50, 0x48, 0x05, 0x00,
            0x00, 0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x56, 0x50, 0x38, 0x20,
            0x28, 0x00, 0x00, 0x00, 0x90, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x02, 0x00,
            0x02, 0x00, 0x00, 0xc0, 0x12, 0x25, 0x00, 0x4e, 0x97, 0x40, 0x00, 0x8c,
            0xd0, 0x00, 0xfe, 0xfe, 0x22, 0x72, 0xfd, 0x5d, 0x89, 0xf0, 0x9f, 0xa0,
            0xfe, 0xe6, 0xc7, 0x38, 0xb6, 0x43, 0x00, 0x00,
        ];
        let mut pixels = [0u32; 4];
        let metadata = decode_webp_rgba(&webp, &mut pixels).unwrap();
        assert_eq!((metadata.width, metadata.height), (2, 2));
        assert!(pixels.iter().all(|pixel| (pixel & 0xff) < 255));
    }
}
