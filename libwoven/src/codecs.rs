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

struct GifStreamReader<'a, S: ImageStream> {
    source: &'a mut S,
    position: u64,
    sub_remaining: usize,
    sub_ended: bool,
}

impl<'a, S: ImageStream> GifStreamReader<'a, S> {
    fn new(source: &'a mut S) -> Self {
        Self {
            source,
            position: 0,
            sub_remaining: 0,
            sub_ended: false,
        }
    }

    fn read_exact(&mut self, destination: &mut [u8]) -> Result<(), ImageError> {
        read_stream_exact(self.source, self.position, destination)?;
        self.position = self
            .position
            .checked_add(destination.len() as u64)
            .ok_or(ImageError::Malformed)?;
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8, ImageError> {
        let mut byte = [0u8; 1];
        self.read_exact(&mut byte)?;
        Ok(byte[0])
    }

    fn skip_subblocks(&mut self) -> Result<(), ImageError> {
        let mut scratch = [0u8; 1024];
        loop {
            let size = usize::from(self.read_u8()?);
            if size == 0 {
                return Ok(());
            }
            let mut remaining = size;
            while remaining != 0 {
                let count = remaining.min(scratch.len());
                self.read_exact(&mut scratch[..count])?;
                remaining -= count;
            }
        }
    }

    fn next_data_byte(&mut self) -> Result<Option<u8>, ImageError> {
        if self.sub_ended {
            return Ok(None);
        }
        if self.sub_remaining == 0 {
            self.sub_remaining = usize::from(self.read_u8()?);
            if self.sub_remaining == 0 {
                self.sub_ended = true;
                return Ok(None);
            }
        }
        self.sub_remaining -= 1;
        Ok(Some(self.read_u8()?))
    }

    fn finish_data(&mut self) -> Result<(), ImageError> {
        if !self.sub_ended {
            self.skip_subblocks()?;
            self.sub_ended = true;
            self.sub_remaining = 0;
        }
        Ok(())
    }
}

struct GifStreamBits<'reader, 'source, S: ImageStream> {
    reader: &'reader mut GifStreamReader<'source, S>,
    bits: u32,
    available: u8,
}

impl<'reader, 'source, S: ImageStream> GifStreamBits<'reader, 'source, S> {
    fn new(reader: &'reader mut GifStreamReader<'source, S>) -> Self {
        Self {
            reader,
            bits: 0,
            available: 0,
        }
    }

    fn read(&mut self, width: u8) -> Result<Option<u16>, ImageError> {
        while self.available < width {
            let Some(byte) = self.reader.next_data_byte()? else {
                return Ok(None);
            };
            self.bits |= u32::from(byte) << self.available;
            self.available += 8;
        }
        let code = (self.bits & ((1u32 << width) - 1)) as u16;
        self.bits >>= width;
        self.available -= width;
        Ok(Some(code))
    }
}

struct GifPixelWriter<'a> {
    output: &'a mut [u32],
    output_index: usize,
    expected: usize,
    width: usize,
    screen_width: usize,
    left: usize,
    top: usize,
    palette: &'a [u8; 768],
    palette_entries: usize,
    transparent: Option<u8>,
}

impl GifPixelWriter<'_> {
    fn write(&mut self, palette_index: u8) -> Result<(), ImageError> {
        if self.output_index >= self.expected || usize::from(palette_index) >= self.palette_entries
        {
            return Err(ImageError::Malformed);
        }
        if self.transparent != Some(palette_index) {
            let offset = usize::from(palette_index) * 3;
            let destination = (self.top + self.output_index / self.width) * self.screen_width
                + self.left
                + self.output_index % self.width;
            self.output[destination] = u32::from_be_bytes([
                self.palette[offset],
                self.palette[offset + 1],
                self.palette[offset + 2],
                255,
            ]);
        }
        self.output_index += 1;
        Ok(())
    }
}

/// Decode a single non-interlaced GIF frame directly from an `ImageStream`.
/// Only fixed palette/LZW tables and a 1024-byte source request are retained;
/// the encoded GIF payload is never assembled in memory.
pub fn decode_gif_rgba_stream<S: ImageStream>(
    stream: &mut S,
    metadata: ImageMetadata,
    output: &mut [u32],
) -> Result<ImageMetadata, ImageError> {
    if metadata.format != ImageFormat::Gif {
        return Err(ImageError::UnsupportedEncoding);
    }
    let pixels = capacity(metadata, output)?;
    let mut reader = GifStreamReader::new(stream);
    let mut header = [0u8; 13];
    reader.read_exact(&mut header)?;
    if !(header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a")) {
        return Err(ImageError::UnsupportedEncoding);
    }
    let logical_width = usize::from(u16::from_le_bytes([header[6], header[7]]));
    let logical_height = usize::from(u16::from_le_bytes([header[8], header[9]]));
    if logical_width != metadata.width as usize || logical_height != metadata.height as usize {
        return Err(ImageError::Malformed);
    }
    let packed = header[10];
    let mut palette = [0u8; 768];
    let global_entries = if packed & 0x80 != 0 {
        2usize << usize::from(packed & 0x07)
    } else {
        0
    };
    if global_entries != 0 {
        reader.read_exact(&mut palette[..global_entries * 3])?;
    }
    output[..pixels].fill(0);
    let mut transparent = None;
    let mut frame_seen = false;
    loop {
        match reader.read_u8()? {
            0x3b if frame_seen => return Ok(metadata),
            0x3b => return Err(ImageError::Truncated),
            0x21 => {
                let label = reader.read_u8()?;
                if label == 0xf9 {
                    if reader.read_u8()? != 4 {
                        return Err(ImageError::Malformed);
                    }
                    let mut block = [0u8; 4];
                    reader.read_exact(&mut block)?;
                    if block[0] & 1 != 0 {
                        transparent = Some(block[3]);
                    }
                    if reader.read_u8()? != 0 {
                        return Err(ImageError::Malformed);
                    }
                } else {
                    reader.skip_subblocks()?;
                }
            }
            0x2c => {
                if frame_seen {
                    return Err(ImageError::UnsupportedEncoding);
                }
                frame_seen = true;
                let mut descriptor = [0u8; 9];
                reader.read_exact(&mut descriptor)?;
                let left = usize::from(u16::from_le_bytes([descriptor[0], descriptor[1]]));
                let top = usize::from(u16::from_le_bytes([descriptor[2], descriptor[3]]));
                let width = usize::from(u16::from_le_bytes([descriptor[4], descriptor[5]]));
                let height = usize::from(u16::from_le_bytes([descriptor[6], descriptor[7]]));
                let image_packed = descriptor[8];
                if width == 0 || height == 0 || image_packed & 0x40 != 0 {
                    return Err(ImageError::UnsupportedEncoding);
                }
                let right = left.checked_add(width).ok_or(ImageError::Malformed)?;
                let bottom = top.checked_add(height).ok_or(ImageError::Malformed)?;
                if right > logical_width || bottom > logical_height {
                    return Err(ImageError::Malformed);
                }
                let local_entries = if image_packed & 0x80 != 0 {
                    2usize << usize::from(image_packed & 0x07)
                } else {
                    0
                };
                let palette_entries = if local_entries != 0 {
                    reader.read_exact(&mut palette[..local_entries * 3])?;
                    local_entries
                } else {
                    global_entries
                };
                if palette_entries == 0 {
                    return Err(ImageError::UnsupportedEncoding);
                }
                let min_code_size = reader.read_u8()?;
                if !(2..=8).contains(&min_code_size) {
                    return Err(ImageError::Malformed);
                }
                reader.sub_remaining = 0;
                reader.sub_ended = false;
                let clear = 1u16 << min_code_size;
                let end = clear + 1;
                let mut code_size = min_code_size + 1;
                let mut next_code = end + 1;
                let mut prefix = [0u16; 4096];
                let mut suffix = [0u8; 4096];
                let mut stack = [0u8; 4096];
                let mut old = None;
                let mut first = 0u8;
                let expected = width.checked_mul(height).ok_or(ImageError::Malformed)?;
                let mut writer = GifPixelWriter {
                    output,
                    output_index: 0,
                    expected,
                    width,
                    screen_width: logical_width,
                    left,
                    top,
                    palette: &palette,
                    palette_entries,
                    transparent,
                };
                let mut saw_end = false;
                {
                    let mut bits = GifStreamBits::new(&mut reader);
                    while let Some(raw_code) = bits.read(code_size)? {
                        if raw_code == clear {
                            code_size = min_code_size + 1;
                            next_code = end + 1;
                            old = None;
                            continue;
                        }
                        if raw_code == end {
                            saw_end = true;
                            break;
                        }
                        if raw_code >= 4096 || raw_code > next_code {
                            return Err(ImageError::Malformed);
                        }
                        let mut code = raw_code;
                        let mut stack_len = 0usize;
                        if let Some(old_code) = old {
                            let in_code = raw_code;
                            if code == next_code {
                                stack[stack_len] = first;
                                stack_len += 1;
                                code = old_code;
                            }
                            while code >= clear {
                                let index = usize::from(code);
                                if index >= next_code as usize || stack_len == stack.len() {
                                    return Err(ImageError::Malformed);
                                }
                                stack[stack_len] = suffix[index];
                                stack_len += 1;
                                code = prefix[index];
                            }
                            first = code as u8;
                            stack[stack_len] = first;
                            stack_len += 1;
                            while stack_len != 0 {
                                writer.write(stack[stack_len - 1])?;
                                stack_len -= 1;
                            }
                            if next_code < 4096 {
                                prefix[next_code as usize] = old_code;
                                suffix[next_code as usize] = first;
                                next_code += 1;
                                if next_code == (1u16 << code_size) && code_size < 12 {
                                    code_size += 1;
                                }
                            }
                            old = Some(in_code);
                        } else {
                            if code >= clear {
                                return Err(ImageError::Malformed);
                            }
                            first = code as u8;
                            writer.write(first)?;
                            old = Some(code);
                        }
                    }
                }
                reader.finish_data()?;
                if !saw_end || writer.output_index != expected {
                    return Err(ImageError::Truncated);
                }
            }
            _ => return Err(ImageError::Malformed),
        }
    }
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
        ImageStream, StreamError, decode_bmp_rgba_stream, decode_gif_rgba_stream,
        decode_png_rgba_stream, decode_webp_rgba, decode_webp_rgba_stream,
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
    fn delivers_gif_through_bounded_chunk_reader() {
        let gif = [
            b'G', b'I', b'F', b'8', b'9', b'a', 1, 0, 1, 0, 0x80, 0, 0, 0, 0, 0, 255, 255, 255,
            0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 0x44, 1, 0, 0x3b,
        ];
        let metadata = parse_image_header(&gif).unwrap();
        let mut stream = MemoryStream {
            bytes: &gif,
            largest_request: 0,
        };
        let mut pixels = [0u32; 1];
        decode_gif_rgba_stream(&mut stream, metadata, &mut pixels).unwrap();
        assert_eq!(pixels[0], 0x000000ff);
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
