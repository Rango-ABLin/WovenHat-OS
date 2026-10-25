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
    #[cfg(not(target_os = "none"))]
    let image = oxideav_webp::decode_rgba8(bytes).map_err(|_| ImageError::Malformed)?;
    #[cfg(target_os = "none")]
    return {
        let _ = (bytes, output);
        Err(ImageError::UnsupportedEncoding)
    };
    #[cfg(not(target_os = "none"))]
    if image.width != metadata.width || image.height != metadata.height {
        return Err(ImageError::Malformed);
    }
    #[cfg(not(target_os = "none"))]
    map_rgba(image.as_bytes(), output)?;
    #[cfg(not(target_os = "none"))]
    Ok(metadata)
}

// The decoders accept a byte slice and the output is caller-owned u32 storage.
// This cast is valid because u32 has alignment 4 and the output slice is a
// contiguous initialized pixel buffer; the decoder only writes bytes.
fn bytemuck_slice(output: &mut [u32]) -> &mut [u8] {
    unsafe {
        core::slice::from_raw_parts_mut(output.as_mut_ptr().cast::<u8>(), output.len() * 4)
    }
}
