//! Drawing primitives for logical snake cells.
//!
//! This module owns the conversion from one logical board cell to terminal cells. Higher-level
//! modules decide *which* cells to draw; this module decides how background, food, stable body
//! cells, moving-segment underlays, and colored partial glyphs are painted. Keeping color inversion
//! here beside stable-cell painting makes every terminal-cell foreground/background rule locally
//! visible. Unicode glyph lookup stays here because these glyphs are the terminal representation
//! selected by the fraction-painting functions below.

use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier, Style},
};

use crate::geometry::Point;

/// Terminal columns used by one square-looking logical game cell.
///
/// This module owns the logical-to-terminal projection, so sizing, stable rendering, and animated
/// rendering all use this single definition.
pub const TERMINAL_COLUMNS_PER_CELL: u16 = 2;
/// Primary low-contrast playfield background.
pub const BOARD_BACKGROUND: Color = Color::Rgb(3, 10, 8);
/// Alternate checker cell, kept close to the primary color to avoid visual noise during movement.
pub const BOARD_BACKGROUND_ALT: Color = Color::Rgb(4, 12, 9);
/// Brighter snake color that keeps the moving head distinguishable from its body underlay.
pub const HEAD_COLOR: Color = Color::Rgb(189, 255, 128);
/// Shared body and connection-underlay color.
pub const BODY_COLOR: Color = Color::Rgb(88, 214, 141);
/// Food foreground hint for terminals that honor emoji styling.
pub const FOOD_COLOR: Color = Color::Rgb(255, 92, 92);

/// Surface visible behind a partial block glyph.
///
/// Unicode gives left and lower partial block glyphs. Right and top fractions are represented by
/// swapping foreground and background colors, so the underlay must be explicit instead of inferred
/// from the current buffer cell.
#[derive(Clone, Copy, Debug)]
pub enum Underlay {
    /// Empty board surface for one logical board cell.
    Board(Color),
    /// Stable snake-body surface.
    Body,
}

impl Underlay {
    /// Returns an empty board underlay for `point`.
    pub fn board(point: Point) -> Self {
        Self::Board(background_for(point))
    }

    /// Returns the terminal color for this underlay.
    pub fn color(self) -> Color {
        match self {
            Self::Board(color) => color,
            Self::Body => BODY_COLOR,
        }
    }
}

/// Renders the background for one logical board cell.
pub fn render_background_cell(buf: &mut Buffer, x: u16, y: u16, point: Point) {
    fill_board_cell(buf, x, y, background_for(point));
}

/// Renders food as a single double-width emoji.
///
/// The second cell is left blank so terminals that treat the emoji as width two have room for it.
pub fn render_food_cell(buf: &mut Buffer, x: u16, y: u16, point: Point) {
    let background = background_for(point);
    buf[(x, y)].set_symbol("🍎").set_style(
        Style::new()
            .fg(FOOD_COLOR)
            .bg(background)
            .add_modifier(Modifier::BOLD),
    );
    for column in x + 1..x + TERMINAL_COLUMNS_PER_CELL {
        buf[(column, y)]
            .set_symbol(" ")
            .set_style(Style::new().bg(background));
    }
}

/// Renders a left-side horizontal fraction over an explicit underlay.
pub fn render_left_fraction(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    amount: u8,
    fill: Color,
    underlay: Underlay,
) {
    if amount == 0 {
        return;
    }

    if amount >= 8 {
        render_full_cell(buf, x, y, fill);
        return;
    }

    buf[(x, y)]
        .set_symbol(horizontal_block(amount))
        .set_style(partial_style(fill, underlay.color()));
}

/// Renders a right-side fraction by swapping glyph foreground and background.
///
/// Unicode provides left eighth blocks but no matching right eighth blocks. The complementary left
/// glyph therefore paints the underlay while the background color becomes the visible snake edge.
pub fn render_right_fraction(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    amount: u8,
    fill: Color,
    underlay: Underlay,
) {
    if amount == 0 {
        return;
    }

    if amount >= 8 {
        render_full_cell(buf, x, y, fill);
        return;
    }

    buf[(x, y)]
        .set_symbol(horizontal_block(8 - amount))
        .set_style(partial_style(underlay.color(), fill));
}

/// Renders a top-side fraction using the complement of a lower block glyph.
pub fn render_top_fraction(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    amount: u8,
    fill: Color,
    underlay: Underlay,
) {
    if amount == 0 {
        return;
    }

    if amount >= 8 {
        render_full_cell(buf, x, y, fill);
        return;
    }

    buf[(x, y)]
        .set_symbol(vertical_block(8 - amount))
        .set_style(partial_style(underlay.color(), fill));
}

/// Renders a bottom-side vertical fraction over an explicit underlay.
pub fn render_bottom_fraction(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    amount: u8,
    fill: Color,
    underlay: Underlay,
) {
    if amount == 0 {
        return;
    }

    if amount >= 8 {
        render_full_cell(buf, x, y, fill);
        return;
    }

    buf[(x, y)]
        .set_symbol(vertical_block(amount))
        .set_style(partial_style(fill, underlay.color()));
}

/// Renders one stable snake segment on its logical board cell.
pub fn render_snake_cell(buf: &mut Buffer, board_x: u16, board_y: u16, color: Color, point: Point) {
    let terminal_x = board_x + point.x.max(0) as u16 * TERMINAL_COLUMNS_PER_CELL;
    let terminal_y = board_y + point.y.max(0) as u16;
    fill_snake_cell(buf, terminal_x, terminal_y, color);
}

/// Renders the full two-column underlay for a moving head or tail.
pub fn render_underlay_cell(
    buf: &mut Buffer,
    board_x: u16,
    board_y: u16,
    point: Point,
    underlay: Underlay,
) {
    let terminal_x = board_x + point.x.max(0) as u16 * TERMINAL_COLUMNS_PER_CELL;
    let terminal_y = board_y + point.y.max(0) as u16;

    match underlay {
        Underlay::Body => fill_snake_cell(buf, terminal_x, terminal_y, BODY_COLOR),
        Underlay::Board(color) => fill_board_cell(buf, terminal_x, terminal_y, color),
    }
}

/// Renders one full terminal cell shared by stable snake cells and complete fractional coverage.
///
/// Keeping both paths on this primitive guarantees that interpolation reaching a cell boundary has
/// the same foreground and background colors as the stable frame that follows it.
pub fn render_full_cell(buf: &mut Buffer, terminal_x: u16, terminal_y: u16, color: Color) {
    let style = Style::new().fg(color).bg(BOARD_BACKGROUND);

    buf[(terminal_x, terminal_y)]
        .set_symbol("█")
        .set_style(style);
}

/// Builds a color-only style used for full and partial snake cells.
///
/// Right and top fractions swap foreground and background to synthesize missing Unicode glyphs.
/// Text modifiers would therefore apply to the snake on some edges and its underlay on others. The
/// explicit head and body colors carry visual identity consistently without modifiers.
pub fn partial_style(fg: Color, bg: Color) -> Style {
    Style::new().fg(fg).bg(bg)
}

/// Fills both terminal columns that represent one logical snake cell.
fn fill_snake_cell(buf: &mut Buffer, terminal_x: u16, terminal_y: u16, color: Color) {
    for column in terminal_x..terminal_x + TERMINAL_COLUMNS_PER_CELL {
        render_full_cell(buf, column, terminal_y, color);
    }
}

/// Clears both terminal columns to one board color, including any prior wide glyph.
fn fill_board_cell(buf: &mut Buffer, terminal_x: u16, terminal_y: u16, color: Color) {
    let style = Style::new().bg(color);
    for column in terminal_x..terminal_x + TERMINAL_COLUMNS_PER_CELL {
        buf[(column, terminal_y)].set_symbol(" ").set_style(style);
    }
}

/// Returns the subtle checker color for one logical point.
///
/// Food and partial moving cells must use this same function; otherwise their explicit terminal
/// backgrounds create rectangular patches against alternate cells.
fn background_for(point: Point) -> Color {
    if (point.x + point.y) % 2 == 0 {
        BOARD_BACKGROUND
    } else {
        BOARD_BACKGROUND_ALT
    }
}

/// Returns a left-side block glyph for `amount` eighths.
fn horizontal_block(amount: u8) -> &'static str {
    match amount {
        0 => " ",
        1 => "▏",
        2 => "▎",
        3 => "▍",
        4 => "▌",
        5 => "▋",
        6 => "▊",
        7 => "▉",
        _ => "█",
    }
}

/// Returns a lower-side block glyph for `amount` eighths.
fn vertical_block(amount: u8) -> &'static str {
    match amount {
        0 => " ",
        1 => "▁",
        2 => "▂",
        3 => "▃",
        4 => "▄",
        5 => "▅",
        6 => "▆",
        7 => "▇",
        _ => "█",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_body_uses_the_same_solid_surface_as_partial_underlay() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 1, 1));

        render_full_cell(&mut buffer, 0, 0, BODY_COLOR);

        assert_eq!(buffer[(0, 0)].symbol(), "█");
        assert_eq!(buffer[(0, 0)].style().fg, Some(BODY_COLOR));
        assert_eq!(buffer[(0, 0)].style().add_modifier, Modifier::empty());

        let partial = partial_style(BODY_COLOR, BOARD_BACKGROUND);
        assert_eq!(partial.add_modifier, Modifier::empty());
    }

    #[test]
    fn board_underlay_uses_the_logical_cell_background() {
        assert_eq!(Underlay::board(Point::new(0, 0)).color(), BOARD_BACKGROUND);
        assert_eq!(
            Underlay::board(Point::new(1, 0)).color(),
            BOARD_BACKGROUND_ALT
        );
    }

    #[test]
    fn food_preserves_the_background_of_its_logical_cell() {
        let width = TERMINAL_COLUMNS_PER_CELL;
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, width, 1));

        render_food_cell(&mut buffer, 0, 0, Point::new(1, 0));

        assert_eq!(buffer[(0, 0)].style().bg, Some(BOARD_BACKGROUND_ALT));
        assert_eq!(buffer[(1, 0)].style().bg, Some(BOARD_BACKGROUND_ALT));
    }

    #[test]
    fn right_fraction_can_leave_body_colored_underlay() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 1, 1));

        render_right_fraction(&mut buffer, 0, 0, 2, HEAD_COLOR, Underlay::Body);

        assert_eq!(buffer[(0, 0)].symbol(), "▊");
        assert_eq!(buffer[(0, 0)].style().fg, Some(BODY_COLOR));
        assert_eq!(buffer[(0, 0)].style().bg, Some(HEAD_COLOR));
    }

    #[test]
    fn left_fraction_uses_body_over_background() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 1, 1));

        render_left_fraction(
            &mut buffer,
            0,
            0,
            2,
            BODY_COLOR,
            Underlay::board(Point::new(0, 0)),
        );

        assert_eq!(buffer[(0, 0)].symbol(), "▎");
        assert_eq!(buffer[(0, 0)].style().fg, Some(BODY_COLOR));
        assert_eq!(buffer[(0, 0)].style().bg, Some(BOARD_BACKGROUND));
    }

    #[test]
    fn left_fraction_can_leave_body_colored_underlay() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 1, 1));

        render_left_fraction(&mut buffer, 0, 0, 2, BODY_COLOR, Underlay::Body);

        assert_eq!(buffer[(0, 0)].symbol(), "▎");
        assert_eq!(buffer[(0, 0)].style().fg, Some(BODY_COLOR));
        assert_eq!(buffer[(0, 0)].style().bg, Some(BODY_COLOR));
    }

    #[test]
    fn block_glyphs_cover_each_axis_in_eighths() {
        assert_eq!(horizontal_block(1), "▏");
        assert_eq!(horizontal_block(4), "▌");
        assert_eq!(horizontal_block(8), "█");
        assert_eq!(vertical_block(1), "▁");
        assert_eq!(vertical_block(4), "▄");
        assert_eq!(vertical_block(8), "█");
    }
}
