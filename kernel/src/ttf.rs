//! Bounded freestanding TrueType loader used by the kernel typography gate.
//!
//! This intentionally implements only the static-font path needed by the
//! bundled OFL fixture: sfnt table validation, cmap format 4/12 lookup,
//! horizontal metrics, and simple `glyf` outlines. Composite glyphs and
//! malformed/oversized tables fail closed to the existing bitmap font.

use alloc::vec;
use alloc::vec::Vec;

const MAX_TABLES: usize = 64;
const MAX_POINTS: usize = 256;
const MAX_CONTOURS: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    BadFont,
    MissingTable,
    Unsupported,
    TooLarge,
}

#[derive(Clone, Copy)]
struct Table {
    tag: [u8; 4],
    offset: usize,
    length: usize,
}

#[derive(Clone, Copy)]
struct Point {
    x: i16,
    y: i16,
}

type GlyphBounds = (usize, usize, i16, i16, i16, i16);

pub struct RasterTarget<'a> {
    pub pixels: &'a mut [u32],
    pub width: usize,
    pub height: usize,
    pub origin_x: i32,
    pub baseline: i32,
    pub scale: i32,
    pub color: u32,
}

pub struct Face<'a> {
    bytes: &'a [u8],
    cmap4: Option<Table>,
    cmap12: Option<Table>,
    head: Table,
    maxp: Table,
    hhea: Table,
    hmtx: Table,
    loca: Table,
    glyf: Table,
    units_per_em: u16,
    glyph_count: u16,
    horizontal_metrics: u16,
    long_loca: bool,
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

fn i16_at(bytes: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?,
        *bytes.get(offset + 3)?,
    ]))
}

fn table_span(bytes: &[u8], table: Table) -> Option<&[u8]> {
    bytes.get(table.offset..table.offset.checked_add(table.length)?)
}

fn find_table(tables: &[Table], tag: [u8; 4]) -> Option<Table> {
    tables.iter().copied().find(|table| table.tag == tag)
}

impl<'a> Face<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < 12 || u32_at(bytes, 0) != Some(0x0001_0000) {
            return Err(Error::BadFont);
        }
        let count = usize::from(u16_at(bytes, 4).ok_or(Error::BadFont)?);
        if count == 0 || count > MAX_TABLES {
            return Err(Error::TooLarge);
        }
        let directory_end = 12usize
            .checked_add(count.checked_mul(16).ok_or(Error::TooLarge)?)
            .ok_or(Error::TooLarge)?;
        if directory_end > bytes.len() {
            return Err(Error::BadFont);
        }
        let mut tables = [Table {
            tag: [0; 4],
            offset: 0,
            length: 0,
        }; MAX_TABLES];
        for (index, slot) in tables.iter_mut().enumerate().take(count) {
            let record = 12 + index * 16;
            let offset = usize::try_from(u32_at(bytes, record + 8).ok_or(Error::BadFont)?)
                .map_err(|_| Error::TooLarge)?;
            let length = usize::try_from(u32_at(bytes, record + 12).ok_or(Error::BadFont)?)
                .map_err(|_| Error::TooLarge)?;
            let table = Table {
                tag: bytes[record..record + 4]
                    .try_into()
                    .map_err(|_| Error::BadFont)?,
                offset,
                length,
            };
            if table_span(bytes, table).is_none() {
                return Err(Error::BadFont);
            }
            *slot = table;
        }
        let tables = &tables[..count];
        let head = find_table(tables, *b"head").ok_or(Error::MissingTable)?;
        let maxp = find_table(tables, *b"maxp").ok_or(Error::MissingTable)?;
        let cmap = find_table(tables, *b"cmap").ok_or(Error::MissingTable)?;
        let hhea = find_table(tables, *b"hhea").ok_or(Error::MissingTable)?;
        let hmtx = find_table(tables, *b"hmtx").ok_or(Error::MissingTable)?;
        let loca = find_table(tables, *b"loca").ok_or(Error::MissingTable)?;
        let glyf = find_table(tables, *b"glyf").ok_or(Error::MissingTable)?;
        let units_per_em = u16_at(bytes, head.offset + 18).ok_or(Error::BadFont)?;
        let glyph_count = u16_at(bytes, maxp.offset + 4).ok_or(Error::BadFont)?;
        let horizontal_metrics = u16_at(bytes, hhea.offset + 34).ok_or(Error::BadFont)?;
        let index_to_loc_format = i16_at(bytes, head.offset + 50).ok_or(Error::BadFont)?;
        if units_per_em == 0
            || glyph_count == 0
            || horizontal_metrics == 0
            || usize::from(horizontal_metrics) > usize::from(glyph_count)
            || !(index_to_loc_format == 0 || index_to_loc_format == 1)
        {
            return Err(Error::BadFont);
        }
        let cmap_bytes = table_span(bytes, cmap).ok_or(Error::BadFont)?;
        let records = usize::from(u16_at(cmap_bytes, 2).ok_or(Error::BadFont)?);
        if records > 64
            || 4usize
                .checked_add(records * 8)
                .is_none_or(|end| end > cmap_bytes.len())
        {
            return Err(Error::BadFont);
        }
        let mut cmap4 = None;
        let mut cmap12 = None;
        for index in 0..records {
            let record = 4 + index * 8;
            let platform = u16_at(cmap_bytes, record).ok_or(Error::BadFont)?;
            let encoding = u16_at(cmap_bytes, record + 2).ok_or(Error::BadFont)?;
            let offset = usize::try_from(u32_at(cmap_bytes, record + 4).ok_or(Error::BadFont)?)
                .map_err(|_| Error::TooLarge)?;
            let subtable = cmap.offset.checked_add(offset).ok_or(Error::TooLarge)?;
            let Some(format) = u16_at(bytes, subtable) else {
                continue;
            };
            if (platform == 0 || platform == 3)
                && (platform == 0 || encoding == 1 || encoding == 10)
            {
                match format {
                    4 if cmap4.is_none() => {
                        cmap4 = Some(Table {
                            tag: *b"cm4 ",
                            offset: subtable,
                            length: cmap.length.saturating_sub(offset),
                        })
                    }
                    12 if cmap12.is_none() => {
                        cmap12 = Some(Table {
                            tag: *b"cm12",
                            offset: subtable,
                            length: cmap.length.saturating_sub(offset),
                        })
                    }
                    _ => {}
                }
            }
        }
        if cmap4.is_none() && cmap12.is_none() {
            return Err(Error::Unsupported);
        }
        Ok(Self {
            bytes,
            cmap4,
            cmap12,
            head,
            maxp,
            hhea,
            hmtx,
            loca,
            glyf,
            units_per_em,
            glyph_count,
            horizontal_metrics,
            long_loca: index_to_loc_format == 1,
        })
    }

    pub const fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    pub fn glyph_index(&self, character: char) -> Option<u16> {
        let code = u32::from(character);
        if let Some(cmap) = self.cmap12 {
            let table = table_span(self.bytes, cmap)?;
            let groups = usize::try_from(u32_at(table, 12)?).ok()?;
            if groups > 4096 {
                return None;
            }
            for index in 0..groups {
                let entry = 16usize.checked_add(index.checked_mul(12)?)?;
                let start = u32_at(table, entry)?;
                let end = u32_at(table, entry + 4)?;
                if code < start {
                    break;
                }
                if code <= end {
                    let glyph = u32_at(table, entry + 8)?.checked_add(code - start)?;
                    return u16::try_from(glyph)
                        .ok()
                        .filter(|id| *id < self.glyph_count);
                }
            }
        }
        let cmap = self.cmap4?;
        let table = table_span(self.bytes, cmap)?;
        if code > u32::from(u16::MAX) || u16_at(table, 2)? as usize > table.len() {
            return None;
        }
        let code = code as u16;
        let segments = usize::from(u16_at(table, 6)?) / 2;
        if segments == 0 || segments > 128 {
            return None;
        }
        let end_codes = 14usize;
        let reserved = end_codes.checked_add(segments.checked_mul(2)?)?;
        let starts = reserved.checked_add(2)?;
        let deltas = starts.checked_add(segments.checked_mul(2)?)?;
        let ranges = deltas.checked_add(segments.checked_mul(2)?)?;
        for index in 0..segments {
            let end = u16_at(table, end_codes + index * 2)?;
            if code > end {
                continue;
            }
            let start = u16_at(table, starts + index * 2)?;
            if code < start {
                return None;
            }
            let delta = i32::from(i16_at(table, deltas + index * 2)?);
            let range = u16_at(table, ranges + index * 2)?;
            let glyph = if range == 0 {
                u16::try_from((i32::from(code) + delta) & i32::from(u16::MAX)).ok()?
            } else {
                let location = ranges
                    .checked_add(index * 2)?
                    .checked_add(usize::from(range))?
                    .checked_add(usize::from(code - start) * 2)?;
                let raw = u16_at(table, location)?;
                if raw == 0 {
                    0
                } else {
                    u16::try_from((i32::from(raw) + delta) & i32::from(u16::MAX)).ok()?
                }
            };
            return (glyph < self.glyph_count).then_some(glyph);
        }
        None
    }

    pub fn advance(&self, glyph: u16) -> Option<u16> {
        if glyph >= self.glyph_count {
            return None;
        }
        let metric = usize::from(glyph.min(self.horizontal_metrics - 1));
        u16_at(self.bytes, self.hmtx.offset + metric.checked_mul(4)?)
    }

    fn glyph_bounds(&self, glyph: u16) -> Result<Option<GlyphBounds>, Error> {
        let loca_span = table_span(self.bytes, self.loca).ok_or(Error::BadFont)?;
        let entry = usize::from(glyph)
            .checked_mul(if self.long_loca { 4 } else { 2 })
            .ok_or(Error::TooLarge)?;
        let start = if self.long_loca {
            usize::try_from(u32_at(loca_span, entry).ok_or(Error::BadFont)?)
                .map_err(|_| Error::TooLarge)?
        } else {
            usize::from(u16_at(loca_span, entry).ok_or(Error::BadFont)?) * 2
        };
        let end_entry = entry
            .checked_add(if self.long_loca { 4 } else { 2 })
            .ok_or(Error::TooLarge)?;
        let end = if self.long_loca {
            usize::try_from(u32_at(loca_span, end_entry).ok_or(Error::BadFont)?)
                .map_err(|_| Error::TooLarge)?
        } else {
            usize::from(u16_at(loca_span, end_entry).ok_or(Error::BadFont)?) * 2
        };
        if start > end || end > self.glyf.length {
            return Err(Error::BadFont);
        }
        if start == end {
            return Ok(None);
        }
        let glyph_start = self.glyf.offset.checked_add(start).ok_or(Error::TooLarge)?;
        if glyph_start
            .checked_add(10)
            .is_none_or(|limit| limit > self.bytes.len())
        {
            return Err(Error::BadFont);
        }
        let contours = i16_at(self.bytes, glyph_start).ok_or(Error::BadFont)?;
        if contours < 0 {
            return Err(Error::Unsupported);
        }
        let contours = usize::try_from(contours).map_err(|_| Error::TooLarge)?;
        if contours > MAX_CONTOURS {
            return Err(Error::TooLarge);
        }
        Ok(Some((
            glyph_start,
            contours,
            i16_at(self.bytes, glyph_start + 2).ok_or(Error::BadFont)?,
            i16_at(self.bytes, glyph_start + 4).ok_or(Error::BadFont)?,
            i16_at(self.bytes, glyph_start + 6).ok_or(Error::BadFont)?,
            i16_at(self.bytes, glyph_start + 8).ok_or(Error::BadFont)?,
        )))
    }

    fn points(
        &self,
        glyph: u16,
        output: &mut [Point; MAX_POINTS],
        ends: &mut [usize; MAX_CONTOURS],
    ) -> Result<(usize, usize), Error> {
        let Some((start, contours, _, _, _, _)) = self.glyph_bounds(glyph)? else {
            return Ok((0, 0));
        };
        let mut offset = start + 10;
        let mut contour_ends = [0usize; MAX_CONTOURS];
        for end in contour_ends.iter_mut().take(contours) {
            *end = usize::from(u16_at(self.bytes, offset).ok_or(Error::BadFont)?);
            offset += 2;
        }
        let count = contour_ends[contours - 1]
            .checked_add(1)
            .ok_or(Error::TooLarge)?;
        if count > MAX_POINTS {
            return Err(Error::TooLarge);
        }
        let instruction_bytes = usize::from(u16_at(self.bytes, offset).ok_or(Error::BadFont)?);
        offset = offset
            .checked_add(2 + instruction_bytes)
            .ok_or(Error::TooLarge)?;
        let mut flags = [0u8; MAX_POINTS];
        let mut index = 0;
        while index < count {
            let flag = *self.bytes.get(offset).ok_or(Error::BadFont)?;
            offset += 1;
            flags[index] = flag;
            index += 1;
            if flag & 0x08 != 0 {
                let repeat = usize::from(*self.bytes.get(offset).ok_or(Error::BadFont)?);
                offset += 1;
                if index.checked_add(repeat).is_none_or(|end| end > count) {
                    return Err(Error::BadFont);
                }
                for flag_slot in flags[index..index + repeat].iter_mut() {
                    *flag_slot = flag;
                }
                index += repeat;
            }
        }
        let mut x = 0i16;
        for (index, point) in output.iter_mut().take(count).enumerate() {
            if flags[index] & 0x02 != 0 {
                let delta = i16::from(*self.bytes.get(offset).ok_or(Error::BadFont)?);
                offset += 1;
                x = if flags[index] & 0x10 != 0 {
                    x.saturating_add(delta)
                } else {
                    x.saturating_sub(delta)
                };
            } else if flags[index] & 0x10 == 0 {
                x = x.saturating_add(i16_at(self.bytes, offset).ok_or(Error::BadFont)?);
                offset += 2;
            }
            point.x = x;
        }
        let mut y = 0i16;
        for (index, point) in output.iter_mut().take(count).enumerate() {
            if flags[index] & 0x04 != 0 {
                let delta = i16::from(*self.bytes.get(offset).ok_or(Error::BadFont)?);
                offset += 1;
                y = if flags[index] & 0x20 != 0 {
                    y.saturating_add(delta)
                } else {
                    y.saturating_sub(delta)
                };
            } else if flags[index] & 0x20 == 0 {
                y = y.saturating_add(i16_at(self.bytes, offset).ok_or(Error::BadFont)?);
                offset += 2;
            }
            point.y = y;
        }
        ends[..contours].copy_from_slice(&contour_ends[..contours]);
        Ok((count, contours))
    }

    pub fn render_glyph(&self, glyph: u16, target: &mut RasterTarget<'_>) -> Result<(), Error> {
        if target.width == 0
            || target.height == 0
            || target.pixels.len()
                < target
                    .width
                    .checked_mul(target.height)
                    .ok_or(Error::TooLarge)?
            || target.scale <= 0
        {
            return Err(Error::BadFont);
        }
        let mut points = [Point { x: 0, y: 0 }; MAX_POINTS];
        let mut ends = [0usize; MAX_CONTOURS];
        let (count, contours) = self.points(glyph, &mut points, &mut ends)?;
        if count == 0 {
            return Ok(());
        }
        let units = i32::from(self.units_per_em);
        for y in 0..target.height {
            for x in 0..target.width {
                let px = i32::try_from(x).map_err(|_| Error::TooLarge)?;
                let py = i32::try_from(y).map_err(|_| Error::TooLarge)?;
                let mut inside = false;
                let mut begin = 0;
                for end in ends.iter().copied().take(contours) {
                    let mut previous = end;
                    for index in begin..=end {
                        let a = points[previous];
                        let b = points[index];
                        let ax = target.origin_x + i32::from(a.x) * target.scale / units;
                        let ay = target.baseline - i32::from(a.y) * target.scale / units;
                        let bx = target.origin_x + i32::from(b.x) * target.scale / units;
                        let by = target.baseline - i32::from(b.y) * target.scale / units;
                        if ((ay > py) != (by > py))
                            && (px < (bx - ax) * (py - ay) / (by - ay).max(1) + ax)
                        {
                            inside = !inside;
                        }
                        previous = index;
                    }
                    begin = end + 1;
                }
                if inside {
                    target.pixels[y * target.width + x] = target.color;
                }
            }
        }
        Ok(())
    }

    pub fn render_text(&self, text: &str, target: &mut RasterTarget<'_>) -> Result<usize, Error> {
        let units = i32::from(self.units_per_em);
        let mut cursor = target.origin_x;
        let mut count = 0;
        for character in text.chars() {
            if character == '\n' {
                return Err(Error::Unsupported);
            }
            let glyph = self.glyph_index(character).ok_or(Error::Unsupported)?;
            let original_origin = target.origin_x;
            target.origin_x = cursor;
            self.render_glyph(glyph, target)?;
            target.origin_x = original_origin;
            cursor = cursor.saturating_add(
                i32::from(self.advance(glyph).ok_or(Error::BadFont)?) * target.scale / units,
            );
            count += 1;
        }
        Ok(count)
    }
}

pub fn bundled_face() -> Result<Face<'static>, Error> {
    Face::from_bytes(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"))
}

pub fn structural_self_test() -> bool {
    let Ok(face) = bundled_face() else {
        return false;
    };
    let mut pixels = vec![0u32; 256 * 64];
    let mut target = RasterTarget {
        pixels: &mut pixels,
        width: 256,
        height: 64,
        origin_x: 4,
        baseline: 54,
        scale: 40,
        color: 0xffff_ffff,
    };
    if face.render_text("WΩЖ", &mut target).is_err() {
        return false;
    }
    pixels.iter().any(|pixel| *pixel != 0)
}
