//! Minimal deterministic PDF text-layout bridge.
//! Only basic Latin single-byte literal text supported. This is an initial
//! drawable text command pipeline, not PDF font or encoding conformance.
use crate::content::{interpret, ContentError, Operation};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphRun<'a> {
    pub bytes: &'a [u8],
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    Content(ContentError),
    InvalidCoordinate,
    TooManyRuns,
}

fn valid_coordinate(value: f32) -> bool {
    value.is_finite() && value.abs() <= 32768.0
}

/// Convert a supported subset of PDF page text operators into positioned runs.
/// PDF coordinates originate at bottom left, so the Y coordinate is inverted.
pub fn layout<'a>(
    content: &'a [u8],
    page_height: f32,
    mut emit: impl FnMut(GlyphRun<'a>) -> Result<(), LayoutError>,
) -> Result<(), LayoutError> {
    if !valid_coordinate(page_height) || page_height <= 0.0 {
        return Err(LayoutError::InvalidCoordinate);
    }
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    let mut font_size = 12.0f32;
    let mut count = 0usize;
    interpret(content, |operation| {
        match operation {
            Operation::BeginText => {
                x = 0.0;
                y = 0.0;
            }
            Operation::SetFont { size, .. } => {
                if !size.is_finite() || size <= 0.0 || size > 1024.0 {
                    return Err(ContentError::Malformed);
                }
                font_size = size;
            }
            Operation::MoveText { x: dx, y: dy } => {
                x += dx;
                y += dy;
                if !valid_coordinate(x) || !valid_coordinate(y) {
                    return Err(ContentError::Malformed);
                }
            }
            Operation::ShowText(bytes) => {
                count += 1;
                if count > 1024 || bytes.len() > 4096 {
                    return Err(ContentError::Unsupported);
                }
                emit(GlyphRun {
                    bytes,
                    x,
                    y: page_height - y,
                    font_size,
                })
                .map_err(|_| ContentError::Malformed)?;
            }
            _ => {}
        }
        Ok(())
    })
    .map_err(LayoutError::Content)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translates_simple_text_run() {
        let mut count = 0;
        assert_eq!(
            layout(b"BT /F1 12 Tf 10 20 Td (Hello) Tj ET", 600.0, |run| {
                count += 1;
                assert_eq!(run.bytes, b"Hello");
                assert_eq!((run.x, run.y, run.font_size), (10.0, 580.0, 12.0));
                Ok(())
            }),
            Ok(())
        );
        assert_eq!(count, 1);
    }
    #[test]
    fn refuses_unsupported_and_invalid_input() {
        assert_eq!(
            layout(b"0 0 m", 600.0, |_| Ok(())),
            Err(LayoutError::Content(ContentError::Unsupported))
        );
        assert_eq!(
            layout(b"", 0.0, |_| Ok(())),
            Err(LayoutError::InvalidCoordinate)
        );
    }
}
