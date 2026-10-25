//! Bounded image-media metadata parsing.
//!
//! Decoders must validate dimensions before allocating or rasterizing. This
//! module exposes only the shared header contract; format-specific pixel
//! decoding remains a separate implementation boundary.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Gif,
    Bmp,
    Jpeg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageMetadata {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    Truncated,
    Unsupported,
    InvalidDimensions,
}

const MAX_DIMENSION: u32 = 8192;

fn dimensions(format: ImageFormat, width: u32, height: u32) -> Result<ImageMetadata, ImageError> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(ImageError::InvalidDimensions);
    }
    Ok(ImageMetadata {
        format,
        width,
        height,
    })
}

pub fn parse_image_header(bytes: &[u8]) -> Result<ImageMetadata, ImageError> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
            return Err(ImageError::Truncated);
        }
        let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        return dimensions(ImageFormat::Png, width, height);
    }
    if bytes.len() >= 10 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        let width = u32::from(u16::from_le_bytes([bytes[6], bytes[7]]));
        let height = u32::from(u16::from_le_bytes([bytes[8], bytes[9]]));
        return dimensions(ImageFormat::Gif, width, height);
    }
    if bytes.starts_with(b"BM") {
        if bytes.len() < 26 {
            return Err(ImageError::Truncated);
        }
        let dib_size = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
        if dib_size < 40 || bytes.len() < 14 + dib_size as usize {
            return Err(ImageError::Truncated);
        }
        let width = i32::from_le_bytes(bytes[18..22].try_into().unwrap());
        let height = i32::from_le_bytes(bytes[22..26].try_into().unwrap());
        if width <= 0 || height == 0 {
            return Err(ImageError::InvalidDimensions);
        }
        return dimensions(ImageFormat::Bmp, width as u32, height.unsigned_abs());
    }
    if bytes.starts_with(b"\xff\xd8") {
        let mut cursor = 2usize;
        while cursor + 3 < bytes.len() {
            if bytes[cursor] != 0xff {
                cursor += 1;
                continue;
            }
            while cursor < bytes.len() && bytes[cursor] == 0xff {
                cursor += 1;
            }
            if cursor >= bytes.len() {
                break;
            }
            let marker = bytes[cursor];
            cursor += 1;
            if marker == 0xd9 || marker == 0xda {
                break;
            }
            if cursor + 2 > bytes.len() {
                return Err(ImageError::Truncated);
            }
            let length = usize::from(u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]));
            if length < 2 || cursor + length > bytes.len() {
                return Err(ImageError::Truncated);
            }
            if (0xc0..=0xc3).contains(&marker)
                || (0xc5..=0xc7).contains(&marker)
                || (0xc9..=0xcb).contains(&marker)
                || (0xcd..=0xcf).contains(&marker)
            {
                if length < 7 {
                    return Err(ImageError::Truncated);
                }
                let height = u32::from(u16::from_be_bytes([bytes[cursor + 3], bytes[cursor + 4]]));
                let width = u32::from(u16::from_be_bytes([bytes[cursor + 5], bytes[cursor + 6]]));
                return dimensions(ImageFormat::Jpeg, width, height);
            }
            cursor += length;
        }
        return Err(ImageError::Truncated);
    }
    Err(ImageError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::{ImageError, ImageFormat, parse_image_header};

    #[test]
    fn parses_bounded_png_gif_and_bmp_headers() {
        let mut png = [0u8; 24];
        png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        png[12..16].copy_from_slice(b"IHDR");
        png[16..20].copy_from_slice(&640u32.to_be_bytes());
        png[20..24].copy_from_slice(&480u32.to_be_bytes());
        assert_eq!(parse_image_header(&png).unwrap().format, ImageFormat::Png);
        let gif = *b"GIF89a\x10\0\x08\0";
        assert_eq!(parse_image_header(&gif).unwrap().width, 16);
        let mut bmp = [0u8; 54];
        bmp[..2].copy_from_slice(b"BM");
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&32i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&(-24i32).to_le_bytes());
        assert_eq!(parse_image_header(&bmp).unwrap().height, 24);
    }

    #[test]
    fn rejects_oversized_or_unknown_media_headers() {
        let mut png = [0u8; 24];
        png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        png[12..16].copy_from_slice(b"IHDR");
        png[16..20].copy_from_slice(&9000u32.to_be_bytes());
        png[20..24].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(parse_image_header(&png), Err(ImageError::InvalidDimensions));
        assert_eq!(parse_image_header(b"nope"), Err(ImageError::Unsupported));
    }
}
