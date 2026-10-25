//! Bounded browser-document substrate.
//!
//! The parser intentionally owns no network, script, or pixel resources. It
//! produces a fixed-capacity borrowed DOM that later layout and policy layers
//! can consume without heap allocation.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    InvalidUtf8,
    TooManyNodes,
    TooDeep,
    MalformedTag,
    MismatchedClose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptError {
    MissingNode,
    NotScript,
    MissingSource,
    Runtime(crate::javascript::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node<'a> {
    pub tag: &'a str,
    pub text: Option<&'a str>,
    pub parent: Option<usize>,
    pub style: Option<&'a str>,
}

pub struct Document<'a, const N: usize> {
    nodes: [Option<Node<'a>>; N],
    len: usize,
}

impl<'a, const N: usize> Document<'a, N> {
    pub const fn new() -> Self {
        Self {
            nodes: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn node(&self, index: usize) -> Option<Node<'a>> {
        self.nodes.get(index).copied().flatten()
    }

    fn push(&mut self, node: Node<'a>) -> Result<usize, ParseError> {
        if self.len == N {
            return Err(ParseError::TooManyNodes);
        }
        let index = self.len;
        self.nodes[index] = Some(node);
        self.len += 1;
        Ok(index)
    }
}

fn style_attribute(attributes: &str) -> Option<&str> {
    let mut rest = attributes;
    while !rest.trim_ascii().is_empty() {
        rest = rest.trim_ascii_start();
        let name_end = rest
            .find(|character: char| !character.is_ascii_alphanumeric() && character != '-')
            .unwrap_or(rest.len());
        if name_end == 0 {
            break;
        }
        let name = &rest[..name_end];
        rest = rest[name_end..].trim_ascii_start();
        if !rest.starts_with('=') {
            rest = rest
                .find(|character: char| character.is_ascii_whitespace())
                .map_or("", |index| &rest[index..]);
            continue;
        }
        rest = rest[1..].trim_ascii_start();
        let (value, consumed) = if let Some(quote @ ('"' | '\'')) = rest.chars().next() {
            let value_start = quote.len_utf8();
            let value_end = rest[value_start..].find(quote)? + value_start;
            (&rest[value_start..value_end], value_end + quote.len_utf8())
        } else {
            let value_end = rest
                .find(|character: char| character.is_ascii_whitespace())
                .unwrap_or(rest.len());
            (&rest[..value_end], value_end)
        };
        rest = &rest[consumed..];
        if name.eq_ignore_ascii_case("style") {
            return Some(value);
        }
    }
    None
}

impl<'a, const N: usize> Default for Document<'a, N> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_html<'a, const N: usize>(input: &'a [u8]) -> Result<Document<'a, N>, ParseError> {
    let source = core::str::from_utf8(input).map_err(|_| ParseError::InvalidUtf8)?;
    let mut document = Document::new();
    let mut stack = [0usize; 16];
    let mut depth = 0usize;
    let mut cursor = 0usize;
    while cursor < source.len() {
        let rest = &source[cursor..];
        if let Some(open) = rest.find('<') {
            if open > 0 {
                let text = &rest[..open];
                if !text.trim_ascii().is_empty()
                    && let Some(parent) = depth
                        .checked_sub(1)
                        .and_then(|index| stack.get(index).copied())
                    && let Some(node) = &mut document.nodes[parent]
                {
                    node.text = Some(text);
                }
            }
            let tag_start = cursor + open + 1;
            let tag_end = source[tag_start..]
                .find('>')
                .map(|offset| tag_start + offset)
                .ok_or(ParseError::MalformedTag)?;
            let mut tag = &source[tag_start..tag_end];
            let closing = tag.starts_with('/');
            if closing {
                tag = &tag[1..];
            }
            let self_closing = tag.ends_with('/');
            if self_closing {
                tag = tag[..tag.len() - 1].trim_ascii();
            }
            let name_end = tag
                .find(|character: char| character.is_ascii_whitespace())
                .unwrap_or(tag.len());
            let name = &tag[..name_end];
            if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                return Err(ParseError::MalformedTag);
            }
            if closing {
                if depth == 0 || document.node(stack[depth - 1]).map(|node| node.tag) != Some(name)
                {
                    return Err(ParseError::MismatchedClose);
                }
                depth -= 1;
            } else {
                let parent = depth
                    .checked_sub(1)
                    .and_then(|index| stack.get(index).copied());
                let style = style_attribute(&tag[name_end..]);
                let index = document.push(Node {
                    tag: name,
                    text: None,
                    parent,
                    style,
                })?;
                if !self_closing {
                    if depth == stack.len() {
                        return Err(ParseError::TooDeep);
                    }
                    stack[depth] = index;
                    depth += 1;
                }
            }
            cursor = tag_end + 1;
        } else {
            let text = rest;
            if !text.trim_ascii().is_empty()
                && let Some(parent) = depth
                    .checked_sub(1)
                    .and_then(|index| stack.get(index).copied())
                && let Some(node) = &mut document.nodes[parent]
            {
                node.text = Some(text);
            }
            cursor = source.len();
        }
    }
    if depth != 0 {
        return Err(ParseError::MismatchedClose);
    }
    Ok(document)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutBox {
    pub node: usize,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintCommand<'a> {
    Fill {
        rect: Rect,
        color: u32,
    },
    Text {
        x: u32,
        y: u32,
        text: &'a str,
        color: u32,
    },
}

pub struct PaintList<'a, const N: usize> {
    commands: [Option<PaintCommand<'a>>; N],
    len: usize,
}

impl<'a, const N: usize> PaintList<'a, N> {
    pub const fn new() -> Self {
        Self {
            commands: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<PaintCommand<'a>> {
        self.commands.get(index).copied().flatten()
    }

    fn push(&mut self, command: PaintCommand<'a>) -> Result<(), PaintError> {
        if self.len == N {
            return Err(PaintError::TooManyCommands);
        }
        self.commands[self.len] = Some(command);
        self.len += 1;
        Ok(())
    }
}

impl<const N: usize> Default for PaintList<'_, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintError {
    TooManyCommands,
    MissingNode,
    CoordinateOutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    InvalidViewport,
    TooManyBoxes,
    Overflow,
}

pub struct LayoutDocument<const N: usize> {
    boxes: [Option<LayoutBox>; N],
    len: usize,
}

impl<const N: usize> LayoutDocument<N> {
    pub const fn new() -> Self {
        Self {
            boxes: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<LayoutBox> {
        self.boxes.get(index).copied().flatten()
    }

    fn push(&mut self, layout: LayoutBox) -> Result<usize, LayoutError> {
        if self.len == N {
            return Err(LayoutError::TooManyBoxes);
        }
        let index = self.len;
        self.boxes[index] = Some(layout);
        self.len += 1;
        Ok(index)
    }
}

impl<const N: usize> Default for LayoutDocument<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct Style {
    width: Option<u32>,
    height: Option<u32>,
    margin: u32,
    display: bool,
    background: Option<u32>,
    color: Option<u32>,
}

fn css_color(value: &str) -> Option<u32> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

fn style(node: Node<'_>) -> Style {
    let mut result = Style {
        width: None,
        height: None,
        margin: 0,
        display: !matches!(node.tag, "script" | "style" | "head"),
        background: None,
        color: None,
    };
    let Some(source) = node.style else {
        return result;
    };
    for declaration in source.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let value = value.trim_ascii();
        let parse_px = || {
            value
                .strip_suffix("px")
                .and_then(|px| px.parse::<u32>().ok())
        };
        let name = name.trim_ascii();
        if name.eq_ignore_ascii_case("width") {
            result.width = parse_px();
        } else if name.eq_ignore_ascii_case("height") {
            result.height = parse_px();
        } else if name.eq_ignore_ascii_case("margin") {
            result.margin = parse_px().unwrap_or(0);
        } else if name.eq_ignore_ascii_case("display") {
            result.display = value != "none";
        } else if name.eq_ignore_ascii_case("background-color") {
            result.background = css_color(value);
        } else if name.eq_ignore_ascii_case("color") {
            result.color = css_color(value);
        }
    }
    result
}

struct LayoutContext<'document, 'output, const N: usize> {
    document: &'document Document<'document, N>,
    output: &'output mut LayoutDocument<N>,
    viewport_height: u32,
}

fn layout_subtree<const N: usize>(
    context: &mut LayoutContext<'_, '_, N>,
    node_index: usize,
    x: u32,
    y: u32,
    available_width: u32,
    depth: usize,
) -> Result<u32, LayoutError> {
    if depth >= 16 {
        return Err(LayoutError::Overflow);
    }
    let node = context
        .document
        .node(node_index)
        .ok_or(LayoutError::Overflow)?;
    let node_style = style(node);
    if !node_style.display {
        return Ok(0);
    }
    let margin = node_style.margin;
    let width = node_style
        .width
        .unwrap_or(available_width.saturating_sub(margin.saturating_mul(2)));
    let content_x = x.checked_add(margin).ok_or(LayoutError::Overflow)?;
    let content_y = y.checked_add(margin).ok_or(LayoutError::Overflow)?;
    let mut cursor_y = content_y;
    let mut child_count = 0usize;
    for child_index in 0..context.document.len() {
        if context
            .document
            .node(child_index)
            .and_then(|child| child.parent)
            != Some(node_index)
        {
            continue;
        }
        let child_height =
            layout_subtree(context, child_index, content_x, cursor_y, width, depth + 1)?;
        cursor_y = cursor_y
            .checked_add(child_height)
            .ok_or(LayoutError::Overflow)?;
        child_count += 1;
    }
    let content_height = node_style
        .height
        .or_else(|| (child_count > 0).then_some(cursor_y.saturating_sub(content_y)))
        .unwrap_or(24);
    let height = content_height
        .checked_add(margin.saturating_mul(2))
        .ok_or(LayoutError::Overflow)?;
    let bottom = y.checked_add(height).ok_or(LayoutError::Overflow)?;
    if y > context.viewport_height || bottom < y || bottom > context.viewport_height {
        return Err(LayoutError::Overflow);
    }
    context.output.push(LayoutBox {
        node: node_index,
        rect: Rect {
            x,
            y,
            width,
            height,
        },
    })?;
    Ok(height)
}

pub fn layout<const N: usize>(
    document: &Document<'_, N>,
    viewport: Viewport,
) -> Result<LayoutDocument<N>, LayoutError> {
    if viewport.width == 0 || viewport.height == 0 {
        return Err(LayoutError::InvalidViewport);
    }
    let mut output = LayoutDocument::new();
    let mut context = LayoutContext {
        document,
        output: &mut output,
        viewport_height: viewport.height,
    };
    let mut cursor_y = 0;
    for node_index in 0..context.document.len() {
        if context
            .document
            .node(node_index)
            .and_then(|node| node.parent)
            .is_some()
        {
            continue;
        }
        let height = layout_subtree(&mut context, node_index, 0, cursor_y, viewport.width, 0)?;
        cursor_y = cursor_y.checked_add(height).ok_or(LayoutError::Overflow)?;
    }
    Ok(output)
}

/// Convert layout boxes into renderer-neutral fill and text commands.
///
/// This intentionally covers only the inline style properties supported by
/// [`layout`]. A compositor may translate these commands to a [`crate::Canvas`], a
/// GPU command buffer, or another renderer without coupling the browser
/// parser to a display backend.
pub fn paint<'a, const N: usize, const P: usize>(
    document: &'a Document<'a, N>,
    layout: &LayoutDocument<N>,
) -> Result<PaintList<'a, P>, PaintError> {
    let mut output = PaintList::new();
    for index in 0..layout.len() {
        let layout_box = layout.get(index).ok_or(PaintError::MissingNode)?;
        let node = document
            .node(layout_box.node)
            .ok_or(PaintError::MissingNode)?;
        let node_style = style(node);
        if let Some(color) = node_style.background {
            output.push(PaintCommand::Fill {
                rect: layout_box.rect,
                color,
            })?;
        }
        if let (Some(text), Some(color)) = (node.text, node_style.color) {
            output.push(PaintCommand::Text {
                x: layout_box.rect.x,
                y: layout_box.rect.y,
                text,
                color,
            })?;
        }
    }
    Ok(output)
}

/// Render a bounded paint list through the existing software canvas.
pub fn render<const N: usize>(
    canvas: &mut crate::Canvas<'_>,
    commands: &PaintList<'_, N>,
) -> Result<(), PaintError> {
    for index in 0..commands.len() {
        match commands.get(index).ok_or(PaintError::MissingNode)? {
            PaintCommand::Fill { rect, color } => {
                let x = i32::try_from(rect.x).map_err(|_| PaintError::CoordinateOutOfRange)?;
                let y = i32::try_from(rect.y).map_err(|_| PaintError::CoordinateOutOfRange)?;
                let width =
                    usize::try_from(rect.width).map_err(|_| PaintError::CoordinateOutOfRange)?;
                let height =
                    usize::try_from(rect.height).map_err(|_| PaintError::CoordinateOutOfRange)?;
                canvas.fill_rect(x, y, width, height, color);
            }
            PaintCommand::Text { x, y, text, color } => {
                let x = i32::try_from(x).map_err(|_| PaintError::CoordinateOutOfRange)?;
                let y = i32::try_from(y).map_err(|_| PaintError::CoordinateOutOfRange)?;
                canvas.draw_text(x, y, text, color);
            }
        }
    }
    Ok(())
}

/// Execute one inline `<script>` node with the bounded scalar runtime.
///
/// The runtime receives only the node's borrowed source and has no DOM,
/// filesystem, network, or compositor capabilities. This is an explicit
/// integration seam, not a claim of general ECMAScript support.
pub fn execute_script<'a, const N: usize, const V: usize>(
    document: &'a Document<'a, N>,
    node_index: usize,
    runtime: &mut crate::javascript::Runtime<'a, V>,
) -> Result<crate::javascript::Value, ScriptError> {
    let node = document.node(node_index).ok_or(ScriptError::MissingNode)?;
    if node.tag != "script" {
        return Err(ScriptError::NotScript);
    }
    let source = node.text.ok_or(ScriptError::MissingSource)?;
    crate::javascript::eval(source, runtime).map_err(ScriptError::Runtime)
}

#[cfg(test)]
mod tests {
    use super::{
        LayoutError, PaintCommand, ParseError, ScriptError, Viewport, execute_script, layout,
        paint, parse_html, render,
    };

    #[test]
    fn parses_borrowed_nested_document_with_text() {
        let document = parse_html::<4>(b"<main><p>Hello</p><br/></main>").unwrap();
        assert_eq!(document.len(), 3);
        assert_eq!(document.node(0).unwrap().tag, "main");
        assert_eq!(document.node(1).unwrap().text, Some("Hello"));
        assert_eq!(document.node(2).unwrap().parent, Some(0));
        assert_eq!(document.node(0).unwrap().style, None);
    }

    #[test]
    fn rejects_malformed_markup_and_bounds_nodes() {
        assert!(matches!(
            parse_html::<4>(b"<main><p></main>"),
            Err(ParseError::MismatchedClose)
        ));
        assert!(matches!(
            parse_html::<1>(b"<main><p></p></main>"),
            Err(ParseError::TooManyNodes)
        ));
        assert!(matches!(
            parse_html::<4>(b"<main"),
            Err(ParseError::MalformedTag)
        ));
    }

    #[test]
    fn parses_inline_style_and_lays_out_bounded_blocks() {
        let document =
            parse_html::<4>(br#"<main style="margin:4px"><p style="height:20px">Hello</p></main>"#)
                .unwrap();
        assert_eq!(document.node(0).unwrap().style, Some("margin:4px"));
        let layout = layout(
            &document,
            Viewport {
                width: 320,
                height: 200,
            },
        )
        .unwrap();
        assert_eq!(layout.len(), 2);
        assert_eq!(layout.get(0).unwrap().node, 1);
        assert_eq!(layout.get(1).unwrap().node, 0);
        assert_eq!(layout.get(0).unwrap().rect.height, 20);
    }

    #[test]
    fn rejects_invalid_viewport_and_layout_overflow() {
        let document = parse_html::<2>(b"<main><p/></main>").unwrap();
        assert!(matches!(
            layout(
                &document,
                Viewport {
                    width: 0,
                    height: 10
                }
            ),
            Err(LayoutError::InvalidViewport)
        ));
        let document = parse_html::<2>(b"<main style=\"height:4294967295px\"/>").unwrap();
        assert!(matches!(
            layout(
                &document,
                Viewport {
                    width: 10,
                    height: 10,
                }
            ),
            Err(LayoutError::Overflow)
        ));
    }

    #[test]
    fn emits_bounded_background_and_text_paint_commands() {
        let document = parse_html::<4>(
            br#"<main style="background-color:#102030"><p style="color:#ffffff">Hello</p></main>"#,
        )
        .unwrap();
        let layout = layout(
            &document,
            Viewport {
                width: 320,
                height: 200,
            },
        )
        .unwrap();
        let commands = paint::<4, 4>(&document, &layout).unwrap();
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            commands.get(0),
            Some(PaintCommand::Text {
                text: "Hello",
                color: 0x00ff_ffff,
                ..
            })
        ));
        assert!(matches!(
            commands.get(1),
            Some(PaintCommand::Fill {
                color: 0x0010_2030,
                ..
            })
        ));
    }

    #[test]
    fn rejects_paint_command_overflow() {
        let document = parse_html::<2>(
            br#"<main style="background-color:#000000"><p style="color:#ffffff">x</p></main>"#,
        )
        .unwrap();
        let layout = layout(
            &document,
            Viewport {
                width: 100,
                height: 100,
            },
        )
        .unwrap();
        assert!(matches!(
            paint::<2, 1>(&document, &layout),
            Err(super::PaintError::TooManyCommands)
        ));
    }

    #[test]
    fn renders_fill_commands_into_the_software_canvas() {
        let document =
            parse_html::<2>(br#"<main style="width:4px;height:4px;background-color:#102030"/>"#)
                .unwrap();
        let layout = layout(
            &document,
            Viewport {
                width: 4,
                height: 4,
            },
        )
        .unwrap();
        let commands = paint::<2, 2>(&document, &layout).unwrap();
        let mut pixels = [0u32; 16];
        let mut canvas = crate::Canvas::new(&mut pixels, 0, 0, 4, 4).unwrap();
        render(&mut canvas, &commands).unwrap();
        assert_eq!(canvas.get(0, 0), Some(0x0010_2030));
        assert_eq!(canvas.get(3, 3), Some(0x0010_2030));
    }

    #[test]
    fn executes_only_inline_script_nodes_through_bounded_runtime() {
        let document =
            parse_html::<4>(b"<main><script>let answer = 6 * 7; answer + 1</script></main>")
                .unwrap();
        let mut runtime = crate::javascript::Runtime::<2>::new();
        assert_eq!(
            execute_script(&document, 1, &mut runtime),
            Ok(crate::javascript::Value::Integer(43))
        );
        assert_eq!(
            runtime.get("answer"),
            Some(crate::javascript::Value::Integer(42))
        );
        assert_eq!(
            execute_script(&document, 0, &mut runtime),
            Err(ScriptError::NotScript)
        );
    }
}
