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
    WebP,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageMetadata {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

pub const MAX_ASSETS: usize = 32;
pub const MAX_ASSET_NAME: usize = 64;
pub const THUMBNAIL_SIDE: usize = 32;
pub const THUMBNAIL_PIXELS: usize = THUMBNAIL_SIDE * THUMBNAIL_SIDE;
pub const MAX_WALLPAPERS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetRecord {
    pub name: [u8; MAX_ASSET_NAME],
    pub name_len: u8,
    pub metadata: ImageMetadata,
    pub byte_len: u32,
}

impl AssetRecord {
    pub fn name(&self) -> &[u8] {
        &self.name[..usize::from(self.name_len)]
    }
}

pub struct AssetManifest {
    records: [Option<AssetRecord>; MAX_ASSETS],
    len: usize,
}

impl AssetManifest {
    pub const fn new() -> Self {
        Self {
            records: [None; MAX_ASSETS],
            len: 0,
        }
    }

    pub fn register(&mut self, name: &[u8], bytes: &[u8]) -> Result<usize, ImageError> {
        if name.is_empty() || name.len() > MAX_ASSET_NAME || self.len == MAX_ASSETS {
            return Err(ImageError::Malformed);
        }
        let metadata = parse_image_header(bytes)?;
        let mut record = AssetRecord {
            name: [0; MAX_ASSET_NAME],
            name_len: name.len() as u8,
            metadata,
            byte_len: bytes.len().min(u32::MAX as usize) as u32,
        };
        record.name[..name.len()].copy_from_slice(name);
        let index = self.len;
        self.records[index] = Some(record);
        self.len += 1;
        Ok(index)
    }

    pub fn get(&self, index: usize) -> Option<AssetRecord> {
        self.records.get(index).copied().flatten()
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for AssetManifest {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Thumbnail {
    pub key: u64,
    pub width: u8,
    pub height: u8,
    pub pixels: [u32; THUMBNAIL_PIXELS],
}

pub struct ThumbnailCache<const CAPACITY: usize = 4> {
    entries: [Option<Thumbnail>; CAPACITY],
    next: usize,
}

impl<const CAPACITY: usize> ThumbnailCache<CAPACITY> {
    pub const fn new() -> Self {
        Self {
            entries: [const { None }; CAPACITY],
            next: 0,
        }
    }

    pub fn get(&self, key: u64) -> Option<&Thumbnail> {
        self.entries
            .iter()
            .flatten()
            .find(|thumbnail| thumbnail.key == key)
    }

    pub fn insert(&mut self, key: u64, width: usize, height: usize, pixels: &[u32]) -> bool {
        if CAPACITY == 0
            || width == 0
            || height == 0
            || width > THUMBNAIL_SIDE
            || height > THUMBNAIL_SIDE
            || pixels.len() < width.saturating_mul(height)
        {
            return false;
        }
        let mut thumbnail = Thumbnail {
            key,
            width: width as u8,
            height: height as u8,
            pixels: [0; THUMBNAIL_PIXELS],
        };
        for row in 0..height {
            let source = &pixels[row * width..(row + 1) * width];
            thumbnail.pixels[row * THUMBNAIL_SIDE..row * THUMBNAIL_SIDE + width]
                .copy_from_slice(source);
        }
        self.entries[self.next] = Some(thumbnail);
        self.next = (self.next + 1) % CAPACITY;
        true
    }
}

impl<const CAPACITY: usize> Default for ThumbnailCache<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WallpaperLicense {
    pub id: [u8; MAX_ASSET_NAME],
    pub id_len: u8,
    pub spdx: [u8; 32],
    pub spdx_len: u8,
    pub source_hash: [u8; 32],
}

pub struct WallpaperCatalog {
    records: [Option<WallpaperLicense>; MAX_WALLPAPERS],
    len: usize,
}

impl WallpaperCatalog {
    pub const fn new() -> Self {
        Self {
            records: [None; MAX_WALLPAPERS],
            len: 0,
        }
    }

    pub fn register(
        &mut self,
        id: &[u8],
        spdx: &[u8],
        source_hash: [u8; 32],
    ) -> bool {
        if id.is_empty()
            || id.len() > MAX_ASSET_NAME
            || spdx.is_empty()
            || spdx.len() > 32
            || source_hash == [0; 32]
            || self.len == MAX_WALLPAPERS
        {
            return false;
        }
        let mut record = WallpaperLicense {
            id: [0; MAX_ASSET_NAME],
            id_len: id.len() as u8,
            spdx: [0; 32],
            spdx_len: spdx.len() as u8,
            source_hash,
        };
        record.id[..id.len()].copy_from_slice(id);
        record.spdx[..spdx.len()].copy_from_slice(spdx);
        self.records[self.len] = Some(record);
        self.len += 1;
        true
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for WallpaperCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfsAssetError {
    Open,
    Read,
    InvalidImage(ImageError),
    TooLarge,
}

/// Read one immutable image asset from the mounted VFS into caller storage.
/// The descriptor is always closed before returning.
pub fn load_vfs_asset(path: &str, buffer: &mut [u8]) -> Result<(usize, ImageMetadata), VfsAssetError> {
    let descriptor = crate::open(path).map_err(|_| VfsAssetError::Open)?;
    let result = crate::read_all(descriptor, buffer).map_err(|_| VfsAssetError::Read);
    let _ = crate::close(descriptor);
    let length = result?;
    if length == buffer.len() {
        return Err(VfsAssetError::TooLarge);
    }
    let metadata = parse_image_header(&buffer[..length]).map_err(VfsAssetError::InvalidImage)?;
    Ok((length, metadata))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    Truncated,
    Unsupported,
    InvalidDimensions,
    UnsupportedEncoding,
    OutputTooSmall,
    Malformed,
}

const MAX_DIMENSION: u32 = 8192;
pub const MAX_PIXELS: usize = 16 * 1024 * 1024;

fn dimensions(format: ImageFormat, width: u32, height: u32) -> Result<ImageMetadata, ImageError> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(ImageError::InvalidDimensions);
    }
    if (width as u64).saturating_mul(height as u64) > MAX_PIXELS as u64 {
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
    if bytes.len() >= 20 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return match &bytes[12..16] {
            b"VP8X" if bytes.len() >= 30 => {
                let width = 1
                    + (u32::from(bytes[24])
                        | (u32::from(bytes[25]) << 8)
                        | (u32::from(bytes[26]) << 16));
                let height = 1
                    + (u32::from(bytes[27])
                        | (u32::from(bytes[28]) << 8)
                        | (u32::from(bytes[29]) << 16));
                dimensions(ImageFormat::WebP, width, height)
            }
            b"VP8L" if bytes.len() >= 25 && bytes[20] == 0x2f => {
                let bits = u32::from(bytes[21])
                    | (u32::from(bytes[22]) << 8)
                    | (u32::from(bytes[23]) << 16)
                    | (u32::from(bytes[24]) << 24);
                dimensions(
                    ImageFormat::WebP,
                    1 + (bits & 0x3fff),
                    1 + ((bits >> 14) & 0x3fff),
                )
            }
            b"VP8 " if bytes.len() >= 30 && bytes[23..26] == [0x9d, 0x01, 0x2a] => {
                let width = u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3fff;
                let height = u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3fff;
                dimensions(ImageFormat::WebP, u32::from(width), u32::from(height))
            }
            _ => Err(ImageError::UnsupportedEncoding),
        };
    }
    Err(ImageError::Unsupported)
}

/// Decode an uncompressed 24/32-bit BMP into caller-owned RGBA pixels.
///
/// The decoder validates every offset, stride, dimension and output bound
/// before writing. Compressed BMPs and indexed formats fail closed; callers
/// can route those formats to a separately budgeted decoder.
pub fn decode_bmp_rgba(bytes: &[u8], output: &mut [u32]) -> Result<ImageMetadata, ImageError> {
    let metadata = parse_image_header(bytes)?;
    if metadata.format != ImageFormat::Bmp || bytes.len() < 54 {
        return Err(ImageError::UnsupportedEncoding);
    }
    let dib_size = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
    let width = metadata.width as usize;
    let height = metadata.height as usize;
    if dib_size < 40 || bytes.len() < 14 + dib_size as usize || output.len() < width * height {
        return Err(ImageError::OutputTooSmall);
    }
    let planes = u16::from_le_bytes([bytes[26], bytes[27]]);
    let bits = u16::from_le_bytes([bytes[28], bytes[29]]);
    let compression = u32::from_le_bytes(bytes[30..34].try_into().unwrap());
    if planes != 1 || (bits != 24 && bits != 32) || compression != 0 {
        return Err(ImageError::UnsupportedEncoding);
    }
    let pixel_offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
    let row_bytes = width
        .checked_mul(usize::from(bits / 8))
        .and_then(|row| row.checked_add(3).map(|padded| padded & !3))
        .ok_or(ImageError::Malformed)?;
    let pixel_bytes = row_bytes.checked_mul(height).ok_or(ImageError::Malformed)?;
    let end = pixel_offset.checked_add(pixel_bytes).ok_or(ImageError::Malformed)?;
    if pixel_offset < 14 + dib_size as usize || end > bytes.len() {
        return Err(ImageError::Truncated);
    }
    let source_height = i32::from_le_bytes(bytes[22..26].try_into().unwrap());
    for source_y in 0..height {
        let destination_y = if source_height < 0 { source_y } else { height - 1 - source_y };
        let row = &bytes[pixel_offset + source_y * row_bytes..];
        for x in 0..width {
            let offset = x * usize::from(bits / 8);
            let b = row[offset];
            let g = row[offset + 1];
            let r = row[offset + 2];
            let a = if bits == 32 { row[offset + 3] } else { 255 };
            output[destination_y * width + x] = u32::from_be_bytes([r, g, b, a]);
        }
    }
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::{
        AssetManifest, ImageError, ImageFormat, ThumbnailCache, WallpaperCatalog, decode_bmp_rgba,
        parse_image_header,
    };

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

    #[test]
    fn decodes_bounded_24_bit_bmp_to_rgba() {
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
        let mut pixels = [0u32; 2];
        assert_eq!(decode_bmp_rgba(&bmp, &mut pixels).unwrap().width, 2);
        assert_eq!(pixels, [0xFF0000FF, 0x00FF00FF]);
    }

    #[test]
    fn validates_webp_vp8x_dimensions_without_allocating() {
        let mut webp = [0u8; 30];
        webp[..4].copy_from_slice(b"RIFF");
        webp[8..12].copy_from_slice(b"WEBP");
        webp[12..16].copy_from_slice(b"VP8X");
        webp[24] = 9;
        webp[27] = 4;
        let metadata = parse_image_header(&webp).unwrap();
        assert_eq!(metadata.format, ImageFormat::WebP);
        assert_eq!((metadata.width, metadata.height), (10, 5));
    }

    #[test]
    fn parses_webp_lossless_dimensions() {
        let webp = [
            0x52, 0x49, 0x46, 0x46, 0x1e, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
            0x56, 0x50, 0x38, 0x4c, 0x11, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00,
            0x00, 0x07, 0x50, 0x94, 0x52, 0x94, 0xa7, 0xff, 0x81, 0x88, 0xe8, 0x7f,
            0x00, 0x00,
        ];
        let metadata = parse_image_header(&webp).unwrap();
        assert_eq!((metadata.width, metadata.height), (2, 2));
    }

    #[test]
    fn manifest_registers_bounded_metadata_without_copying_pixels() {
        let mut png = [0u8; 24];
        png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        png[12..16].copy_from_slice(b"IHDR");
        png[16..20].copy_from_slice(&64u32.to_be_bytes());
        png[20..24].copy_from_slice(&32u32.to_be_bytes());
        let mut manifest = AssetManifest::new();
        let index = manifest.register(b"wallpaper.png", &png).unwrap();
        let record = manifest.get(index).unwrap();
        assert_eq!(manifest.len(), 1);
        assert_eq!(record.name(), b"wallpaper.png");
        assert_eq!(record.metadata.width, 64);
        assert_eq!(record.byte_len, 24);
    }

    #[test]
    fn thumbnail_cache_replaces_in_fixed_round_robin_storage() {
        let mut cache = ThumbnailCache::<2>::new();
        let pixels = [0x1122_3344; 4];
        assert!(cache.insert(1, 2, 2, &pixels));
        assert_eq!(cache.get(1).unwrap().pixels[0], 0x1122_3344);
        assert!(cache.insert(2, 1, 1, &pixels));
        assert!(cache.insert(3, 1, 1, &pixels));
        assert!(cache.get(3).is_some());
        assert!(cache.get(1).is_none());
    }

    #[test]
    fn wallpaper_catalog_requires_license_and_source_digest() {
        let mut catalog = WallpaperCatalog::new();
        assert!(!catalog.register(b"wall", b"CC0-1.0", [0; 32]));
        assert!(catalog.register(b"wall", b"CC0-1.0", [7; 32]));
        assert_eq!(catalog.len(), 1);
    }
}
