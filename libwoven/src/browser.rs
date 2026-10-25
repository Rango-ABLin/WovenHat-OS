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
    pub class_name: Option<&'a str>,
    pub id: Option<&'a str>,
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

fn attribute_value<'a>(attributes: &'a str, wanted: &str) -> Option<&'a str> {
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
        if name.eq_ignore_ascii_case(wanted) {
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
                let attributes = &tag[name_end..];
                let index = document.push(Node {
                    tag: name,
                    text: None,
                    parent,
                    class_name: attribute_value(attributes, "class"),
                    id: attribute_value(attributes, "id"),
                    style: attribute_value(attributes, "style"),
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
pub enum Selector<'a> {
    Tag(&'a str),
    Class(&'a str),
    Id(&'a str),
}

impl Selector<'_> {
    const fn specificity(self) -> u8 {
        match self {
            Self::Tag(_) => 0,
            Self::Class(_) => 1,
            Self::Id(_) => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rule<'a> {
    selector: Selector<'a>,
    declarations: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleSheetError {
    TooManyRules,
    MalformedRule,
    EmptySelector,
}

pub struct StyleSheet<'a, const R: usize> {
    rules: [Option<Rule<'a>>; R],
    len: usize,
}

impl<'a, const R: usize> StyleSheet<'a, R> {
    pub const fn new() -> Self {
        Self {
            rules: [None; R],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<Selector<'a>> {
        self.rules
            .get(index)
            .copied()
            .flatten()
            .map(|rule| rule.selector)
    }

    pub fn parse(source: &'a str) -> Result<Self, StyleSheetError> {
        Self::parse_internal(source, None)
    }

    /// Parse the bounded stylesheet for one viewport width. Only
    /// `@media (min-width: Npx)` and `@media (max-width: Npx)` conditions are
    /// accepted; inactive blocks are skipped without entering the rule
    /// capacity.
    pub fn parse_for_viewport(
        source: &'a str,
        viewport_width: u32,
    ) -> Result<Self, StyleSheetError> {
        Self::parse_internal(source, Some(viewport_width))
    }

    fn parse_internal(
        source: &'a str,
        viewport_width: Option<u32>,
    ) -> Result<Self, StyleSheetError> {
        let mut sheet = Self::new();
        sheet.parse_rules(source, viewport_width, 0)?;
        Ok(sheet)
    }

    fn parse_rules(
        &mut self,
        source: &'a str,
        viewport_width: Option<u32>,
        nesting: usize,
    ) -> Result<(), StyleSheetError> {
        if nesting > 8 {
            return Err(StyleSheetError::MalformedRule);
        }
        let mut cursor = 0usize;
        while !source[cursor..].trim_ascii().is_empty() {
            let open = cursor
                .checked_add(
                    source[cursor..]
                        .find('{')
                        .ok_or(StyleSheetError::MalformedRule)?,
                )
                .ok_or(StyleSheetError::MalformedRule)?;
            let header = source[cursor..open].trim_ascii();
            let mut depth = 1usize;
            let mut close = None;
            for (offset, byte) in source[open + 1..].bytes().enumerate() {
                match byte {
                    b'{' => {
                        depth += 1;
                        if depth > 8 {
                            return Err(StyleSheetError::MalformedRule);
                        }
                    }
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(open + 1 + offset);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let close = close.ok_or(StyleSheetError::MalformedRule)?;
            let body = &source[open + 1..close];
            if header.starts_with("@media") {
                let active = media_matches(header, viewport_width)?;
                if active {
                    self.parse_rules(body, viewport_width, nesting + 1)?;
                }
            } else {
                if body.contains('{') {
                    return Err(StyleSheetError::MalformedRule);
                }
                let selector = header;
                if selector.contains(',') {
                    return Err(StyleSheetError::MalformedRule);
                }
                let selector = selector
                    .strip_prefix('.')
                    .map(Selector::Class)
                    .or_else(|| selector.strip_prefix('#').map(Selector::Id))
                    .or_else(|| (!selector.is_empty()).then_some(Selector::Tag(selector)))
                    .ok_or(StyleSheetError::EmptySelector)?;
                if self.len == R {
                    return Err(StyleSheetError::TooManyRules);
                }
                self.rules[self.len] = Some(Rule {
                    selector,
                    declarations: body,
                });
                self.len += 1;
            }
            cursor = close + 1;
        }
        Ok(())
    }
}

fn media_matches(header: &str, viewport_width: Option<u32>) -> Result<bool, StyleSheetError> {
    let condition = header
        .strip_prefix("@media")
        .map(str::trim_ascii)
        .ok_or(StyleSheetError::MalformedRule)?;
    let prefix = if condition.starts_with("(min-width:") {
        "min"
    } else if condition.starts_with("(max-width:") {
        "max"
    } else {
        return Err(StyleSheetError::MalformedRule);
    };
    let pixels = condition
        .strip_prefix(if prefix == "min" {
            "(min-width:"
        } else {
            "(max-width:"
        })
        .and_then(|value| value.strip_suffix(')'))
        .map(str::trim_ascii)
        .and_then(|value| value.strip_suffix("px"))
        .and_then(|value| value.trim_ascii().parse::<u32>().ok())
        .ok_or(StyleSheetError::MalformedRule)?;
    Ok(viewport_width.is_some_and(|width| {
        if prefix == "min" {
            width >= pixels
        } else {
            width <= pixels
        }
    }))
}

impl<const R: usize> Default for StyleSheet<'_, R> {
    fn default() -> Self {
        Self::new()
    }
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
    width: Option<Length>,
    height: Option<Length>,
    margin: u32,
    display: DisplayMode,
    flex_direction: FlexDirection,
    gap: u32,
    grid_columns: usize,
    background: Option<u32>,
    color: Option<u32>,
}

#[derive(Clone, Copy)]
enum Length {
    Pixels(u32),
    Percent(u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DisplayMode {
    None,
    Block,
    Flex,
    Grid,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlexDirection {
    Row,
    Column,
}

fn css_color(value: &str) -> Option<u32> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

fn css_length(value: &str) -> Option<Length> {
    if let Some(px) = value.strip_suffix("px") {
        return px.parse::<u32>().ok().map(Length::Pixels);
    }
    let percent = value.strip_suffix('%')?.parse::<u8>().ok()?;
    (percent <= 100).then_some(Length::Percent(percent))
}

fn apply_declarations(result: &mut Style, source: &str) {
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
            result.width = css_length(value);
        } else if name.eq_ignore_ascii_case("height") {
            result.height = css_length(value);
        } else if name.eq_ignore_ascii_case("margin") {
            result.margin = parse_px().unwrap_or(0);
        } else if name.eq_ignore_ascii_case("display") {
            result.display = match value {
                "none" => DisplayMode::None,
                "flex" => DisplayMode::Flex,
                "grid" => DisplayMode::Grid,
                _ => DisplayMode::Block,
            };
        } else if name.eq_ignore_ascii_case("flex-direction") {
            if value == "column" {
                result.flex_direction = FlexDirection::Column;
            } else if value == "row" {
                result.flex_direction = FlexDirection::Row;
            }
        } else if name.eq_ignore_ascii_case("gap") {
            result.gap = parse_px().unwrap_or(0);
        } else if name.eq_ignore_ascii_case("grid-template-columns") {
            let columns = value
                .split_ascii_whitespace()
                .filter(|column| !column.is_empty())
                .count();
            if columns > 0 {
                result.grid_columns = columns;
            }
        } else if name.eq_ignore_ascii_case("background-color") {
            result.background = css_color(value);
        } else if name.eq_ignore_ascii_case("color") {
            result.color = css_color(value);
        }
    }
}

fn resolve_length(length: Length, base: u32) -> Option<u32> {
    match length {
        Length::Pixels(value) => Some(value),
        Length::Percent(percent) => base
            .checked_mul(u32::from(percent))
            .map(|value| value / 100),
    }
}

fn matches_selector(node: Node<'_>, selector: Selector<'_>) -> bool {
    match selector {
        Selector::Tag(tag) => node.tag.eq_ignore_ascii_case(tag),
        Selector::Class(class) => node
            .class_name
            .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class)),
        Selector::Id(id) => node.id == Some(id),
    }
}

fn style<const R: usize>(node: Node<'_>, sheet: &StyleSheet<'_, R>) -> Style {
    let mut result = Style {
        width: None,
        height: None,
        margin: 0,
        display: if matches!(node.tag, "script" | "style" | "head") {
            DisplayMode::None
        } else {
            DisplayMode::Block
        },
        flex_direction: FlexDirection::Row,
        gap: 0,
        grid_columns: 1,
        background: None,
        color: None,
    };
    for specificity in 0..=2 {
        for index in 0..sheet.len {
            if let Some(rule) = sheet.rules[index]
                && rule.selector.specificity() == specificity
                && matches_selector(node, rule.selector)
            {
                apply_declarations(&mut result, rule.declarations);
            }
        }
    }
    if let Some(source) = node.style {
        apply_declarations(&mut result, source);
    }
    result
}

fn inherited_color<const N: usize, const R: usize>(
    document: &Document<'_, N>,
    node_index: usize,
    sheet: &StyleSheet<'_, R>,
) -> Option<u32> {
    let mut current = Some(node_index);
    for _ in 0..16 {
        let index = current?;
        let node = document.node(index)?;
        if let Some(color) = style(node, sheet).color {
            return Some(color);
        }
        current = node.parent;
    }
    None
}

struct LayoutContext<'document, 'output, 'sheet, const N: usize, const R: usize> {
    document: &'document Document<'document, N>,
    output: &'output mut LayoutDocument<N>,
    sheet: &'sheet StyleSheet<'document, R>,
    viewport_height: u32,
}

fn layout_subtree<const N: usize, const R: usize>(
    context: &mut LayoutContext<'_, '_, '_, N, R>,
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
    let node_style = style(node, context.sheet);
    if node_style.display == DisplayMode::None {
        return Ok(0);
    }
    let margin = node_style.margin;
    let width = node_style
        .width
        .and_then(|length| resolve_length(length, available_width))
        .unwrap_or(available_width.saturating_sub(margin.saturating_mul(2)));
    let content_x = x.checked_add(margin).ok_or(LayoutError::Overflow)?;
    let content_y = y.checked_add(margin).ok_or(LayoutError::Overflow)?;
    let mut children = [0usize; 16];
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
        if child_count == children.len() {
            return Err(LayoutError::Overflow);
        }
        children[child_count] = child_index;
        child_count += 1;
    }
    let mut cursor_y = content_y;
    let mut content_height = 0u32;
    match node_style.display {
        DisplayMode::Block => {
            for &child_index in &children[..child_count] {
                let child_height =
                    layout_subtree(context, child_index, content_x, cursor_y, width, depth + 1)?;
                cursor_y = cursor_y
                    .checked_add(child_height)
                    .and_then(|value| value.checked_add(node_style.gap))
                    .ok_or(LayoutError::Overflow)?;
                content_height = cursor_y.saturating_sub(content_y);
            }
            content_height = content_height.saturating_sub(node_style.gap);
        }
        DisplayMode::Flex => {
            if node_style.flex_direction == FlexDirection::Column {
                for &child_index in &children[..child_count] {
                    let child_height = layout_subtree(
                        context,
                        child_index,
                        content_x,
                        cursor_y,
                        width,
                        depth + 1,
                    )?;
                    cursor_y = cursor_y
                        .checked_add(child_height)
                        .and_then(|value| value.checked_add(node_style.gap))
                        .ok_or(LayoutError::Overflow)?;
                }
                content_height = cursor_y
                    .saturating_sub(content_y)
                    .saturating_sub(node_style.gap);
            } else {
                let gap_total = node_style
                    .gap
                    .checked_mul(child_count.saturating_sub(1) as u32)
                    .ok_or(LayoutError::Overflow)?;
                let child_width = width.saturating_sub(gap_total) / child_count.max(1) as u32;
                let mut cursor_x = content_x;
                let mut row_height = 0u32;
                for &child_index in &children[..child_count] {
                    let child_height = layout_subtree(
                        context,
                        child_index,
                        cursor_x,
                        content_y,
                        child_width,
                        depth + 1,
                    )?;
                    cursor_x = cursor_x
                        .checked_add(child_width)
                        .and_then(|value| value.checked_add(node_style.gap))
                        .ok_or(LayoutError::Overflow)?;
                    row_height = row_height.max(child_height);
                }
                content_height = row_height;
            }
        }
        DisplayMode::Grid => {
            let columns = node_style.grid_columns.max(1).min(child_count.max(1));
            let gap_total = node_style
                .gap
                .checked_mul(columns.saturating_sub(1) as u32)
                .ok_or(LayoutError::Overflow)?;
            let child_width = width.saturating_sub(gap_total) / columns as u32;
            let mut row_heights = [0u32; 16];
            let rows = child_count.div_ceil(columns);
            if rows > row_heights.len() {
                return Err(LayoutError::Overflow);
            }
            for (position, &child_index) in children[..child_count].iter().enumerate() {
                let row = position / columns;
                let column = position % columns;
                let x = content_x
                    .checked_add((child_width + node_style.gap).saturating_mul(column as u32))
                    .ok_or(LayoutError::Overflow)?;
                let y = content_y
                    .checked_add(
                        row_heights[..row]
                            .iter()
                            .try_fold(0u32, |sum, height| {
                                sum.checked_add(*height)?.checked_add(node_style.gap)
                            })
                            .ok_or(LayoutError::Overflow)?,
                    )
                    .ok_or(LayoutError::Overflow)?;
                let child_height =
                    layout_subtree(context, child_index, x, y, child_width, depth + 1)?;
                row_heights[row] = row_heights[row].max(child_height);
            }
            content_height = row_heights[..rows]
                .iter()
                .try_fold(0u32, |sum, height| {
                    sum.checked_add(*height)?.checked_add(node_style.gap)
                })
                .ok_or(LayoutError::Overflow)?
                .saturating_sub(node_style.gap);
        }
        DisplayMode::None => unreachable!(),
    }
    let content_height = node_style
        .height
        .and_then(|length| resolve_length(length, context.viewport_height))
        .unwrap_or(if child_count > 0 { content_height } else { 24 });
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
    let sheet = StyleSheet::<0>::new();
    layout_with_stylesheet(document, viewport, &sheet)
}

pub fn layout_with_stylesheet<const N: usize, const R: usize>(
    document: &Document<'_, N>,
    viewport: Viewport,
    sheet: &StyleSheet<'_, R>,
) -> Result<LayoutDocument<N>, LayoutError> {
    if viewport.width == 0 || viewport.height == 0 {
        return Err(LayoutError::InvalidViewport);
    }
    let mut output = LayoutDocument::new();
    let mut context = LayoutContext {
        document,
        output: &mut output,
        sheet,
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
    let sheet = StyleSheet::<0>::new();
    paint_with_stylesheet(document, layout, &sheet)
}

pub fn paint_with_stylesheet<'a, const N: usize, const P: usize, const R: usize>(
    document: &'a Document<'a, N>,
    layout: &LayoutDocument<N>,
    sheet: &StyleSheet<'a, R>,
) -> Result<PaintList<'a, P>, PaintError> {
    let mut output = PaintList::new();
    for index in 0..layout.len() {
        let layout_box = layout.get(index).ok_or(PaintError::MissingNode)?;
        let node = document
            .node(layout_box.node)
            .ok_or(PaintError::MissingNode)?;
        let node_style = style(node, sheet);
        if let Some(color) = node_style.background {
            output.push(PaintCommand::Fill {
                rect: layout_box.rect,
                color,
            })?;
        }
        if let (Some(text), Some(color)) =
            (node.text, inherited_color(document, layout_box.node, sheet))
        {
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
        LayoutError, PaintCommand, ParseError, ScriptError, StyleSheet, Viewport, execute_script,
        layout, layout_with_stylesheet, paint, paint_with_stylesheet, parse_html, render,
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
    fn lays_out_bounded_flex_rows_and_grid_columns() {
        let flex = parse_html::<4>(
            br#"<main style="display:flex;gap:4px"><p style="height:10px"/><p style="height:20px"/></main>"#,
        )
        .unwrap();
        let flex_layout = layout(
            &flex,
            Viewport {
                width: 100,
                height: 100,
            },
        )
        .unwrap();
        let first = flex_layout.get(0).unwrap();
        let second = flex_layout.get(1).unwrap();
        assert_eq!((first.rect.x, first.rect.width), (0, 48));
        assert_eq!((second.rect.x, second.rect.width), (52, 48));

        let column = parse_html::<4>(
            br#"<main style="display:flex;flex-direction:column;gap:3px"><p style="height:10px"/><p style="height:20px"/></main>"#,
        )
        .unwrap();
        let column_layout = layout(
            &column,
            Viewport {
                width: 100,
                height: 100,
            },
        )
        .unwrap();
        assert_eq!(column_layout.get(0).unwrap().rect.y, 0);
        assert_eq!(column_layout.get(1).unwrap().rect.y, 13);

        let grid = parse_html::<5>(
            br#"<main style="display:grid;gap:2px;grid-template-columns:1fr 1fr"><p style="height:10px"/><p style="height:20px"/><p style="height:12px"/></main>"#,
        )
        .unwrap();
        let grid_layout = layout(
            &grid,
            Viewport {
                width: 100,
                height: 100,
            },
        )
        .unwrap();
        let third = grid_layout.get(2).unwrap();
        assert_eq!(third.rect.x, 0);
        assert_eq!(third.rect.y, 22);
        assert_eq!(third.rect.width, 49);
    }

    #[test]
    fn applies_bounded_class_and_id_rules_before_inline_style() {
        let document = parse_html::<4>(
            br#"<main id="root" class="frame"><p class="card" style="height:12px"/></main>"#,
        )
        .unwrap();
        let sheet = StyleSheet::<3>::parse(
            "#root { width: 100px; height: 40px; } .frame { width: 80px; display: flex; gap: 4px; } .card { height: 20px; }",
        )
        .unwrap();
        assert_eq!(sheet.len(), 3);
        let layout = layout_with_stylesheet(
            &document,
            Viewport {
                width: 200,
                height: 100,
            },
            &sheet,
        )
        .unwrap();
        assert_eq!(layout.get(0).unwrap().rect.height, 12);
        assert_eq!(layout.get(1).unwrap().rect.width, 100);
        assert_eq!(layout.get(1).unwrap().rect.height, 40);
    }

    #[test]
    fn resolves_bounded_percentage_lengths_against_container_and_viewport() {
        let document = parse_html::<3>(
            br#"<main style="width:50%;height:25%"><p style="width:50%;height:10px"/></main>"#,
        )
        .unwrap();
        let layout = layout(
            &document,
            Viewport {
                width: 200,
                height: 120,
            },
        )
        .unwrap();
        assert_eq!(layout.get(0).unwrap().rect.width, 50);
        assert_eq!(layout.get(1).unwrap().rect.width, 100);
        assert_eq!(layout.get(0).unwrap().rect.height, 10);
        assert_eq!(layout.get(1).unwrap().rect.height, 30);
    }

    #[test]
    fn selects_bounded_media_rules_for_the_requested_viewport() {
        let source = "@media (min-width: 200px) { main { width: 80%; } } @media (max-width: 199px) { main { width: 40%; } }";
        let document = parse_html::<2>(b"<main/>").unwrap();
        let wide = StyleSheet::<2>::parse_for_viewport(source, 240).unwrap();
        let narrow = StyleSheet::<2>::parse_for_viewport(source, 160).unwrap();
        assert_eq!(wide.len(), 1);
        assert_eq!(narrow.len(), 1);
        let wide_layout = layout_with_stylesheet(
            &document,
            Viewport {
                width: 100,
                height: 50,
            },
            &wide,
        )
        .unwrap();
        let narrow_layout = layout_with_stylesheet(
            &document,
            Viewport {
                width: 100,
                height: 50,
            },
            &narrow,
        )
        .unwrap();
        assert_eq!(wide_layout.get(0).unwrap().rect.width, 80);
        assert_eq!(narrow_layout.get(0).unwrap().rect.width, 40);
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
    fn inherits_stylesheet_text_color_through_bounded_parent_chain() {
        let document = parse_html::<4>(b"<main><p>Hello</p></main>").unwrap();
        let sheet = StyleSheet::<1>::parse("main { color: #123456; }").unwrap();
        let layout = layout_with_stylesheet(
            &document,
            Viewport {
                width: 100,
                height: 100,
            },
            &sheet,
        )
        .unwrap();
        let commands = paint_with_stylesheet::<4, 4, 1>(&document, &layout, &sheet).unwrap();
        assert!(commands.get(0).is_some_and(|command| matches!(
            command,
            PaintCommand::Text {
                color: 0x0012_3456,
                text: "Hello",
                ..
            }
        )));
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
