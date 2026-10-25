//! Bounded pixel decoders for the image asset boundary.

use crate::media::{ImageError, ImageFormat, ImageMetadata, MAX_PIXELS, parse_image_header};
use alloc::vec::Vec;
use zune_core::bytestream::{ZByteIoError, ZByteReaderTrait, ZCursor, ZSeekFrom};
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;
use zune_png::PngDecoder;

/// Source-side error for a chunk-backed image stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamError {
    Read,
    OutOfRange,
}

/// Seekable encoded-image source. Implementations may fetch from VFS, a file,
/// or a network cache; the decoder requests only the bytes it currently needs.
pub trait ImageStream {
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn read_at(&mut self, offset: u64, destination: &mut [u8]) -> Result<usize, StreamError>;
}

struct ChunkReader<'a, S: ImageStream> {
    source: &'a mut S,
    position: u64,
}

impl<'a, S: ImageStream> ChunkReader<'a, S> {
    fn new(source: &'a mut S) -> Self {
        Self {
            source,
            position: 0,
        }
    }

    fn read_into(&mut self, destination: &mut [u8]) -> Result<usize, ZByteIoError> {
        let mut total = 0usize;
        while total < destination.len() && self.position < self.source.len() {
            let request = (destination.len() - total).min(1024);
            let count = self
                .source
                .read_at(self.position, &mut destination[total..total + request])
                .map_err(|_| ZByteIoError::Generic("image stream read failed"))?;
            if count == 0 || count > request {
                return Err(ZByteIoError::Generic("image stream read made no progress"));
            }
            self.position = self
                .position
                .checked_add(count as u64)
                .ok_or(ZByteIoError::Generic("image stream position overflow"))?;
            total += count;
        }
        Ok(total)
    }
}

impl<S: ImageStream> ZByteReaderTrait for ChunkReader<'_, S> {
    fn read_byte_no_error(&mut self) -> u8 {
        let mut byte = [0u8; 1];
        if self.read_into(&mut byte).unwrap_or(0) == 1 {
            byte[0]
        } else {
            0
        }
    }

    fn read_exact_bytes(&mut self, buffer: &mut [u8]) -> Result<(), ZByteIoError> {
        let start = self.position;
        let count = self.read_into(buffer)?;
        if count == buffer.len() {
            Ok(())
        } else {
            self.position = start;
            Err(ZByteIoError::NotEnoughBytes(count, buffer.len()))
        }
    }

    fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, ZByteIoError> {
        self.read_into(buffer)
    }

    fn peek_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, ZByteIoError> {
        let position = self.position;
        let count = self.read_into(buffer);
        self.position = position;
        count
    }

    fn peek_exact_bytes(&mut self, buffer: &mut [u8]) -> Result<(), ZByteIoError> {
        let position = self.position;
        let result = self.read_exact_bytes(buffer);
        self.position = position;
        result
    }

    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError> {
        let target = match from {
            ZSeekFrom::Start(offset) => offset,
            ZSeekFrom::Current(offset) => self
                .position
                .checked_add_signed(offset)
                .ok_or(ZByteIoError::SeekError("image stream seek underflow"))?,
            ZSeekFrom::End(offset) => self
                .source
                .len()
                .checked_add_signed(offset)
                .ok_or(ZByteIoError::SeekError("image stream seek underflow"))?,
        };
        if target > self.source.len() {
            return Err(ZByteIoError::SeekError("image stream seek out of range"));
        }
        self.position = target;
        Ok(target)
    }

    fn is_eof(&mut self) -> Result<bool, ZByteIoError> {
        Ok(self.position >= self.source.len())
    }

    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        Ok(self.position)
    }

    fn read_remaining(&mut self, sink: &mut Vec<u8>) -> Result<usize, ZByteIoError> {
        let mut total = 0usize;
        let mut chunk = [0u8; 1024];
        while !self.is_eof()? {
            let count = self.read_into(&mut chunk)?;
            if count == 0 {
                break;
            }
            sink.extend_from_slice(&chunk[..count]);
            total = total
                .checked_add(count)
                .ok_or(ZByteIoError::Generic("image stream length overflow"))?;
        }
        Ok(total)
    }
}

fn read_stream_exact<S: ImageStream>(
    stream: &mut S,
    offset: u64,
    destination: &mut [u8],
) -> Result<(), ImageError> {
    let mut filled = 0usize;
    while filled < destination.len() {
        let request = (destination.len() - filled).min(1024);
        let request_offset = offset
            .checked_add(filled as u64)
            .ok_or(ImageError::Malformed)?;
        let count = stream
            .read_at(request_offset, &mut destination[filled..filled + request])
            .map_err(|_| ImageError::Truncated)?;
        if count == 0 || count > request {
            return Err(ImageError::Truncated);
        }
        filled += count;
    }
    Ok(())
}

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
    decoder
        .decode_into(bytemuck_slice(output))
        .map_err(|_| ImageError::Malformed)?;
    Ok(metadata)
}

/// Decode PNG pixels from a seekable chunk source without presenting the
/// complete encoded asset as one caller allocation.
pub fn decode_png_rgba_stream<S: ImageStream>(
    stream: &mut S,
    metadata: ImageMetadata,
    output: &mut [u32],
) -> Result<ImageMetadata, ImageError> {
    if metadata.format != ImageFormat::Png {
        return Err(ImageError::UnsupportedEncoding);
    }
    capacity(metadata, output)?;
    let options = DecoderOptions::default()
        .png_set_add_alpha_channel(true)
        .png_set_strip_to_8bit(true);
    let mut decoder = PngDecoder::new_with_options(ChunkReader::new(stream), options);
    decoder
        .decode_into(bytemuck_slice(output))
        .map_err(|_| ImageError::Malformed)?;
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
    decoder
        .decode_headers()
        .map_err(|_| ImageError::Malformed)?;
    let mut rgb = alloc::vec![0u8; pixels.saturating_mul(3)];
    decoder
        .decode_into(&mut rgb)
        .map_err(|_| ImageError::Malformed)?;
    let (chunks, remainder) = rgb.as_chunks::<3>();
    if !remainder.is_empty() {
        return Err(ImageError::Malformed);
    }
    for (pixel, chunk) in output.iter_mut().zip(chunks) {
        *pixel = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], 255]);
    }
    Ok(metadata)
}

/// Decode JPEG pixels from a seekable chunk source without presenting the
/// complete encoded asset as one caller allocation.
pub fn decode_jpeg_rgba_stream<S: ImageStream>(
    stream: &mut S,
    metadata: ImageMetadata,
    output: &mut [u32],
) -> Result<ImageMetadata, ImageError> {
    if metadata.format != ImageFormat::Jpeg {
        return Err(ImageError::UnsupportedEncoding);
    }
    let pixels = capacity(metadata, output)?;
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGB)
        .set_use_unsafe(false);
    let mut decoder = JpegDecoder::new_with_options(ChunkReader::new(stream), options);
    decoder
        .decode_headers()
        .map_err(|_| ImageError::Malformed)?;
    let mut rgb = alloc::vec![0u8; pixels.saturating_mul(3)];
    decoder
        .decode_into(&mut rgb)
        .map_err(|_| ImageError::Malformed)?;
    let (chunks, remainder) = rgb.as_chunks::<3>();
    if !remainder.is_empty() {
        return Err(ImageError::Malformed);
    }
    for (pixel, chunk) in output.iter_mut().zip(chunks) {
        *pixel = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], 255]);
    }
    Ok(metadata)
}

/// Decode an uncompressed 24/32-bit BMP from a chunk source one bounded row
/// at a time. Indexed, compressed, and malformed BMPs fail closed.
pub fn decode_bmp_rgba_stream<S: ImageStream>(
    stream: &mut S,
    metadata: ImageMetadata,
    output: &mut [u32],
) -> Result<ImageMetadata, ImageError> {
    if metadata.format != ImageFormat::Bmp {
        return Err(ImageError::UnsupportedEncoding);
    }
    let pixels = capacity(metadata, output)?;
    let mut header = [0u8; 54];
    read_stream_exact(stream, 0, &mut header)?;
    if !header.starts_with(b"BM") {
        return Err(ImageError::UnsupportedEncoding);
    }
    let dib_size = u32::from_le_bytes(header[14..18].try_into().unwrap());
    let planes = u16::from_le_bytes([header[26], header[27]]);
    let bits = u16::from_le_bytes([header[28], header[29]]);
    let compression = u32::from_le_bytes(header[30..34].try_into().unwrap());
    if dib_size < 40 || planes != 1 || (bits != 24 && bits != 32) || compression != 0 {
        return Err(ImageError::UnsupportedEncoding);
    }
    let width = metadata.width as usize;
    let height = metadata.height as usize;
    let pixel_offset = u32::from_le_bytes(header[10..14].try_into().unwrap()) as u64;
    let row_bytes = width
        .checked_mul(usize::from(bits / 8))
        .and_then(|row| row.checked_add(3).map(|padded| padded & !3))
        .ok_or(ImageError::Malformed)?;
    let pixel_bytes = row_bytes.checked_mul(height).ok_or(ImageError::Malformed)?;
    let end = pixel_offset
        .checked_add(pixel_bytes as u64)
        .ok_or(ImageError::Malformed)?;
    if pixel_offset < 14 + u64::from(dib_size) || end > stream.len() {
        return Err(ImageError::Truncated);
    }
    let source_height = i32::from_le_bytes(header[22..26].try_into().unwrap());
    let mut row = [0u8; 32 * 1024];
    if row_bytes > row.len() {
        return Err(ImageError::InvalidDimensions);
    }
    for source_y in 0..height {
        let row_offset = pixel_offset
            .checked_add((source_y * row_bytes) as u64)
            .ok_or(ImageError::Malformed)?;
        read_stream_exact(stream, row_offset, &mut row[..row_bytes])?;
        let destination_y = if source_height < 0 {
            source_y
        } else {
            height - 1 - source_y
        };
        for x in 0..width {
            let offset = x * usize::from(bits / 8);
            let alpha = if bits == 32 { row[offset + 3] } else { 255 };
            output[destination_y * width + x] =
                u32::from_be_bytes([row[offset + 2], row[offset + 1], row[offset], alpha]);
        }
    }
    debug_assert_eq!(pixels, width * height);
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

/// Feed WebP data to the decoder in bounded source requests and rasterize into
/// caller-owned pixels. `zenwebp::StreamingDecoder` incrementally accepts the
/// encoded bytes, but retains the complete RIFF payload internally until the
/// final decode. This is therefore an incremental-delivery API, not a
/// memory-bounded WebP decoder.
pub fn decode_webp_rgba_stream<S: ImageStream>(
    stream: &mut S,
    metadata: ImageMetadata,
    output: &mut [u32],
) -> Result<ImageMetadata, ImageError> {
    if metadata.format != ImageFormat::WebP {
        return Err(ImageError::UnsupportedEncoding);
    }
    capacity(metadata, output)?;

    let mut decoder = zenwebp::decoder::StreamingDecoder::new();
    let mut offset = 0u64;
    let mut chunk = [0u8; 1024];
    loop {
        let count = stream
            .read_at(offset, &mut chunk)
            .map_err(|_| ImageError::Truncated)?;
        if count == 0 || count > chunk.len() {
            return Err(ImageError::Truncated);
        }
        offset = offset
            .checked_add(count as u64)
            .ok_or(ImageError::Malformed)?;
        let status = decoder
            .append(&chunk[..count])
            .map_err(|_| ImageError::Malformed)?;
        if matches!(status, zenwebp::decoder::StreamStatus::Complete) {
            break;
        }
    }

    let info = decoder.info().map_err(|_| ImageError::Malformed)?;
    if info.width != metadata.width || info.height != metadata.height {
        return Err(ImageError::Malformed);
    }
    finish_webp_stream(decoder, output, metadata)?;
    Ok(metadata)
}

#[cfg(target_os = "none")]
fn finish_webp_stream(
    decoder: zenwebp::decoder::StreamingDecoder,
    output: &mut [u32],
    metadata: ImageMetadata,
) -> Result<(), ImageError> {
    let (width, height) = decoder
        .finish_rgba_into(bytemuck_slice(output))
        .map_err(|_| ImageError::Malformed)?;
    if width != metadata.width || height != metadata.height {
        return Err(ImageError::Malformed);
    }
    Ok(())
}

#[cfg(not(target_os = "none"))]
fn finish_webp_stream(
    decoder: zenwebp::decoder::StreamingDecoder,
    output: &mut [u32],
    metadata: ImageMetadata,
) -> Result<(), ImageError> {
    let bytes = decoder.into_inner();
    decode_webp_pixels(&bytes, output, metadata)
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
    unsafe { core::slice::from_raw_parts_mut(output.as_mut_ptr().cast::<u8>(), output.len() * 4) }
}

#[cfg(test)]
mod tests {
    use super::{
        ImageStream, StreamError, decode_bmp_rgba_stream, decode_png_rgba_stream, decode_webp_rgba,
        decode_webp_rgba_stream,
    };
    use crate::media::parse_image_header;

    struct MemoryStream<'a> {
        bytes: &'a [u8],
        largest_request: usize,
    }

    impl ImageStream for MemoryStream<'_> {
        fn len(&self) -> u64 {
            self.bytes.len() as u64
        }

        fn read_at(&mut self, offset: u64, destination: &mut [u8]) -> Result<usize, StreamError> {
            self.largest_request = self.largest_request.max(destination.len());
            let start = usize::try_from(offset).map_err(|_| StreamError::OutOfRange)?;
            if start > self.bytes.len() {
                return Err(StreamError::OutOfRange);
            }
            let count = destination.len().min(self.bytes.len() - start);
            destination[..count].copy_from_slice(&self.bytes[start..start + count]);
            Ok(count)
        }
    }

    #[test]
    fn decodes_png_through_bounded_chunk_reader() {
        let png = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9c, 0x63, 0xe0, 0x12, 0x91, 0xfb, 0x0f, 0x00, 0x01, 0xa4, 0x01, 0x3c, 0x93, 0x8b,
            0x0e, 0xb7, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let metadata = parse_image_header(&png).unwrap();
        let mut stream = MemoryStream {
            bytes: &png,
            largest_request: 0,
        };
        let mut pixels = [0u32; 1];
        decode_png_rgba_stream(&mut stream, metadata, &mut pixels).unwrap();
        assert_eq!(pixels[0], 0xff1e140a);
        assert!(stream.largest_request <= 1024);
    }

    #[test]
    fn decodes_bmp_rows_through_bounded_chunk_reader() {
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
        let metadata = parse_image_header(&bmp).unwrap();
        let mut stream = MemoryStream {
            bytes: &bmp,
            largest_request: 0,
        };
        let mut pixels = [0u32; 2];
        decode_bmp_rgba_stream(&mut stream, metadata, &mut pixels).unwrap();
        assert_eq!(pixels, [0xFF0000FF, 0x00FF00FF]);
        assert!(stream.largest_request <= 1024);
    }

    #[test]
    fn decodes_lossless_webp_into_caller_pixels() {
        let webp = [
            0x52, 0x49, 0x46, 0x46, 0x1e, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50,
            0x38, 0x4c, 0x11, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00, 0x00, 0x07, 0x50, 0x94,
            0x52, 0x94, 0xa7, 0xff, 0x81, 0x88, 0xe8, 0x7f, 0x00, 0x00,
        ];
        let mut pixels = [0u32; 4];
        let metadata = decode_webp_rgba(&webp, &mut pixels).unwrap();
        assert_eq!((metadata.width, metadata.height), (2, 2));
        assert!(pixels.iter().all(|pixel| *pixel != 0));
    }

    #[test]
    fn decodes_webp_through_incremental_stream_input() {
        let webp = [
            0x52, 0x49, 0x46, 0x46, 0x1e, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50,
            0x38, 0x4c, 0x11, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00, 0x00, 0x07, 0x50, 0x94,
            0x52, 0x94, 0xa7, 0xff, 0x81, 0x88, 0xe8, 0x7f, 0x00, 0x00,
        ];
        let metadata = parse_image_header(&webp).unwrap();
        let mut stream = MemoryStream {
            bytes: &webp,
            largest_request: 0,
        };
        let mut pixels = [0u32; 4];
        let decoded = decode_webp_rgba_stream(&mut stream, metadata, &mut pixels).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert!(pixels.iter().all(|pixel| *pixel != 0));
        assert!(stream.largest_request <= 1024);
    }

    #[test]
    fn decodes_extended_lossy_webp_with_alpha() {
        let webp = [
            0x52, 0x49, 0x46, 0x46, 0x54, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50,
            0x38, 0x58, 0x0a, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x41, 0x4c, 0x50, 0x48, 0x05, 0x00, 0x00, 0x00, 0x00, 0x80, 0x80, 0x80,
            0x80, 0x00, 0x56, 0x50, 0x38, 0x20, 0x28, 0x00, 0x00, 0x00, 0x90, 0x01, 0x00, 0x9d,
            0x01, 0x2a, 0x02, 0x00, 0x02, 0x00, 0x00, 0xc0, 0x12, 0x25, 0x00, 0x4e, 0x97, 0x40,
            0x00, 0x8c, 0xd0, 0x00, 0xfe, 0xfe, 0x22, 0x72, 0xfd, 0x5d, 0x89, 0xf0, 0x9f, 0xa0,
            0xfe, 0xe6, 0xc7, 0x38, 0xb6, 0x43, 0x00, 0x00,
        ];
        let mut pixels = [0u32; 4];
        let metadata = decode_webp_rgba(&webp, &mut pixels).unwrap();
        assert_eq!((metadata.width, metadata.height), (2, 2));
        assert!(pixels.iter().all(|pixel| (pixel & 0xff) < 255));
    }
}
