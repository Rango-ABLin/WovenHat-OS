//! Bounded adapter from PDF positioned text runs to a simple text painter.
//! It is intentionally a preview renderer: font encoding, embedded fonts,
//! matrices, complex scripts and graphical operators are unsupported.
use crate::layout::{layout, LayoutError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintError {
    Layout,
    InvalidScale,
    NonAscii,
    OutsideViewport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawText<'a> {
    pub text: &'a str,
    pub x: i32,
    pub y: i32,
}

/// Emit draw commands for a deliberately limited ASCII-only page.
/// Unsupported text bytes are rejected instead of silently mis-rendered.
pub fn paint_text_preview<'a>(
    decoded_content: &'a [u8],
    page_height: f32,
    scale: f32,
    viewport_width: i32,
    viewport_height: i32,
    mut draw: impl FnMut(DrawText<'a>) -> Result<(), PaintError>,
) -> Result<(), PaintError> {
    if !scale.is_finite() || scale <= 0.0 || scale > 8.0 {
        return Err(PaintError::InvalidScale);
    }
    let mut rejected = None;
    let result = layout(decoded_content, page_height, |run| {
        if rejected.is_some() {
            return Err(LayoutError::InvalidCoordinate);
        }
        if !run.bytes.iter().all(|b| matches!(b, 0x20..=0x7e)) {
            rejected = Some(PaintError::NonAscii);
            return Err(LayoutError::InvalidCoordinate);
        }
        let Ok(text) = core::str::from_utf8(run.bytes) else {
            rejected = Some(PaintError::NonAscii);
            return Err(LayoutError::InvalidCoordinate);
        };
        let px = run.x * scale;
        let py = run.y * scale;
        if !px.is_finite()
            || !py.is_finite()
            || px < 0.0
            || py < 0.0
            || px >= viewport_width as f32
            || py >= viewport_height as f32
        {
            rejected = Some(PaintError::OutsideViewport);
            return Err(LayoutError::InvalidCoordinate);
        }
        if let Err(e) = draw(DrawText {
            text,
            x: px as i32,
            y: py as i32,
        }) {
            rejected = Some(e);
            return Err(LayoutError::InvalidCoordinate);
        }
        Ok(())
    });
    if let Some(e) = rejected {
        return Err(e);
    }
    result.map_err(|_| PaintError::Layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn produces_canvas_coordinates() {
        let mut rendered = 0;
        assert_eq!(
            paint_text_preview(
                b"BT /F1 12 Tf 10 20 Td (Hello) Tj ET",
                100.0,
                2.0,
                300,
                300,
                |item| {
                    assert_eq!(item.text, "Hello");
                    assert_eq!((item.x, item.y), (20, 160));
                    rendered += 1;
                    Ok(())
                },
            ),
            Ok(())
        );
        assert_eq!(rendered, 1);
    }
    #[test]
    fn fails_closed_on_unsupported_bytes() {
        assert_eq!(
            paint_text_preview(b"BT (\xFF) Tj ET", 100.0, 1.0, 300, 300, |_| Ok(())),
            Err(PaintError::NonAscii)
        );
    }
}
