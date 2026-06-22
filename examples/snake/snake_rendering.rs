//! Complete terminal rendering for the snake.
//!
//! Game rules move the snake in whole [`Point`] cells. This module paints stable middle segments
//! and draws the head and tail partway between logical cells. Keeping those choices together makes
//! the growth exception and moving-end underlay policy visible without teaching terminal layout
//! about the representation of [`Snake`].

use ratatui::{buffer::Buffer, layout::Rect, style::Color};

use crate::{
    geometry::Point,
    snake::Snake,
    terminal_cells::{
        BODY_COLOR, HEAD_COLOR, TERMINAL_COLUMNS_PER_CELL, Underlay, render_bottom_fraction,
        render_full_cell, render_left_fraction, render_right_fraction, render_snake_cell,
        render_top_fraction, render_underlay_cell,
    },
};

/// Renders stable body cells and independently interpolated head and tail cells.
///
/// Growth leaves the previous and current paths at different lengths, so the new tail remains a
/// stable full cell for that interval instead of interpolating between segments that do not
/// correspond. Middle segments always remain stable because independently rounding every corner
/// makes turns shimmer between frames.
pub fn render_snake(snake: &Snake, movement_progress: f32, board: Rect, buf: &mut Buffer) {
    let movement = snake.movement();
    let segments = snake.segments();
    let previous_tail = movement.previous_tail();
    let smooth_tail = previous_tail.is_some();
    let tail_index = segments.len().saturating_sub(1);

    for (index, segment) in segments.iter().enumerate().rev() {
        if index == 0 || smooth_tail && index == tail_index {
            continue;
        }

        render_snake_cell(buf, board.x, board.y, BODY_COLOR, *segment);
    }

    if let Some(previous) = previous_tail {
        let current_tail = segments.back().copied();
        if let Some(current) = current_tail {
            render_moving_snake_tail(buf, board.x, board.y, previous, current, movement_progress);
        }
    }

    if let Some(head) = segments.front().copied() {
        let previous = movement.previous_head();
        render_moving_snake_head(buf, board.x, board.y, previous, head, movement_progress);
    }
}

/// Renders the snake head between two logical board cells.
///
/// The head is a two-column by one-row logical cell. During a movement tick this function computes
/// the fractional terminal cells covered by that rectangle and paints them with eighth-block
/// glyphs. The body is not drawn here because rounded body interpolation makes turns shimmer.
fn render_moving_snake_head(
    buf: &mut Buffer,
    board_x: u16,
    board_y: u16,
    previous: Point,
    current: Point,
    progress: f32,
) {
    let motion = EndMotion::head(previous, current, progress);
    render_moving_segment(buf, board_x, board_y, motion);
}

/// Renders the snake tail between two logical board cells.
///
/// The tail uses the same coverage projection as the head. Its old cell reveals the board
/// background as it contracts, while its current cell keeps a body underlay so the tail stays
/// visually connected to the stable body.
fn render_moving_snake_tail(
    buf: &mut Buffer,
    board_x: u16,
    board_y: u16,
    previous: Point,
    current: Point,
    progress: f32,
) {
    let motion = EndMotion::tail(previous, current, progress);
    render_moving_segment(buf, board_x, board_y, motion);
}

/// Logical movement being projected between two rendered snake positions.
///
/// Grouping geometry with its complete paint policy keeps the horizontal and vertical renderers
/// focused on axis-specific coverage. Head and tail constructors make their different underlays
/// visible once instead of routing each paint decision through another enum. Terminal origins and
/// buffer access remain function arguments because they are rendering effects, not properties of a
/// snake's movement.
#[derive(Clone, Copy, Debug)]
struct EndMotion {
    /// Snake cell occupied before the latest logical step.
    previous: Point,
    /// Snake cell occupied after the latest logical step.
    current: Point,
    /// Fraction of the logical step visible in this frame.
    progress: f32,
    /// Color painted by full and partial cells.
    color: Color,
    /// Surface revealed in the cell this end leaves.
    previous_underlay: Underlay,
    /// Surface already present in the cell this end enters.
    current_underlay: Underlay,
}

impl EndMotion {
    /// Creates head motion over body in the old cell and board in the new cell.
    fn head(previous: Point, current: Point, progress: f32) -> Self {
        Self {
            previous,
            current,
            progress: progress.clamp(0.0, 1.0),
            color: HEAD_COLOR,
            previous_underlay: Underlay::Body,
            current_underlay: Underlay::board(current),
        }
    }

    /// Creates tail motion that reveals board while staying connected to the body ahead.
    fn tail(previous: Point, current: Point, progress: f32) -> Self {
        Self {
            previous,
            current,
            progress: progress.clamp(0.0, 1.0),
            color: BODY_COLOR,
            previous_underlay: Underlay::board(previous),
            current_underlay: Underlay::Body,
        }
    }
}

/// Coverage of one terminal cell along either movement axis.
///
/// The axis-specific renderer decides whether leading and trailing mean left and right or top and
/// bottom. Keeping that distinction at the paint boundary lets the interpolation math stay shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Coverage {
    /// The moving segment covers the whole terminal cell.
    Full,
    /// The segment covers the start of the axis by this many eighths.
    Leading(u8),
    /// The segment covers the end of the axis by this many eighths.
    Trailing(u8),
}

impl Coverage {
    /// Computes the visible portion of one unit cell touched by a moving span.
    fn for_span(start: f32, end: f32, cell_start: f32) -> Self {
        let cell_end = cell_start + 1.0;
        let covered_start = start.max(cell_start);
        let covered_end = end.min(cell_end);

        let covers_whole_cell =
            covered_start <= cell_start + f32::EPSILON && covered_end >= cell_end - f32::EPSILON;
        if covers_whole_cell {
            return Self::Full;
        }

        let amount = eighths(covered_end - covered_start);
        if covered_start <= cell_start + f32::EPSILON {
            Self::Leading(amount)
        } else {
            Self::Trailing(amount)
        }
    }
}

/// Returns the unit terminal cells touched by a span along either axis.
fn covered_cells(start: f32, end: f32) -> std::ops::RangeInclusive<i16> {
    let start = start.floor() as i16;
    let end = (end.ceil() as i16 - 1).max(start);
    start..=end
}

/// Returns whether one unit terminal cell overlaps a covered span.
fn cell_overlaps(cell_start: f32, span_start: f32, span_end: f32) -> bool {
    let cell_end = cell_start + 1.0;
    cell_start < span_end && cell_end > span_start
}

/// Quantizes fractional terminal-cell coverage to the available eighth-block resolution.
fn eighths(amount: f32) -> u8 {
    (amount.clamp(0.0, 1.0) * 8.0).round() as u8
}

/// Draws one moving end after establishing both cells' complete underlay surfaces.
///
/// Non-adjacent or unchanged points fall back to one stable cell. This covers initial frames and
/// avoids applying fractional geometry to malformed interpolation pairs.
fn render_moving_segment(buf: &mut Buffer, board_x: u16, board_y: u16, motion: EndMotion) {
    render_moving_underlays(buf, board_x, board_y, motion);

    match (
        motion.current.x - motion.previous.x,
        motion.current.y - motion.previous.y,
    ) {
        (-1 | 1, 0) => render_horizontal_segment(buf, board_x, board_y, motion),
        (0, -1 | 1) => render_vertical_segment(buf, board_x, board_y, motion),
        _ => render_snake_cell(buf, board_x, board_y, motion.color, motion.current),
    }
}

/// Restores both logical cells before partial glyphs overwrite the moving rectangle.
///
/// Ratatui uses frame diffs, so repainting complete underlays prevents stale glyphs or colors from
/// a wider previous fraction from surviving into the next frame.
fn render_moving_underlays(buf: &mut Buffer, board_x: u16, board_y: u16, motion: EndMotion) {
    render_underlay_cell(
        buf,
        board_x,
        board_y,
        motion.previous,
        motion.previous_underlay,
    );
    render_underlay_cell(
        buf,
        board_x,
        board_y,
        motion.current,
        motion.current_underlay,
    );
}

/// Converts horizontal interpolation into terminal-column coverage and underlays.
fn render_horizontal_segment(buf: &mut Buffer, board_x: u16, board_y: u16, motion: EndMotion) {
    let cell_width = f32::from(TERMINAL_COLUMNS_PER_CELL);
    let previous_left = motion.previous.x as f32 * cell_width;
    let current_left = motion.current.x as f32 * cell_width;
    let left = previous_left + (current_left - previous_left) * motion.progress;
    let right = left + cell_width;
    let previous_right = previous_left + cell_width;
    let terminal_y = board_y + motion.previous.y.max(0) as u16;

    for column in covered_cells(left, right) {
        let coverage = Coverage::for_span(left, right, column as f32);
        let underlay = if cell_overlaps(column as f32, previous_left, previous_right) {
            motion.previous_underlay
        } else {
            motion.current_underlay
        };
        let terminal_x = board_x + column.max(0) as u16;
        render_horizontal_coverage(
            buf,
            terminal_x,
            terminal_y,
            coverage,
            motion.color,
            underlay,
        );
    }
}

/// Converts vertical interpolation into terminal-row coverage for both wide-cell columns.
fn render_vertical_segment(buf: &mut Buffer, board_x: u16, board_y: u16, motion: EndMotion) {
    let previous_top = motion.previous.y as f32;
    let current_top = motion.current.y as f32;
    let top = previous_top + (current_top - previous_top) * motion.progress;
    let bottom = top + 1.0;
    let previous_bottom = previous_top + 1.0;
    let terminal_x = board_x + motion.previous.x.max(0) as u16 * TERMINAL_COLUMNS_PER_CELL;

    for row in covered_cells(top, bottom) {
        let coverage = Coverage::for_span(top, bottom, row as f32);
        let underlay = if cell_overlaps(row as f32, previous_top, previous_bottom) {
            motion.previous_underlay
        } else {
            motion.current_underlay
        };
        let terminal_y = board_y + row.max(0) as u16;
        for column in terminal_x..terminal_x + TERMINAL_COLUMNS_PER_CELL {
            render_vertical_coverage(buf, column, terminal_y, coverage, motion.color, underlay);
        }
    }
}

/// Maps axis-neutral leading and trailing coverage to left and right block glyphs.
fn render_horizontal_coverage(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    coverage: Coverage,
    color: Color,
    underlay: Underlay,
) {
    match coverage {
        Coverage::Full => render_full_cell(buf, x, y, color),
        Coverage::Leading(amount) => render_left_fraction(buf, x, y, amount, color, underlay),
        Coverage::Trailing(amount) => render_right_fraction(buf, x, y, amount, color, underlay),
    }
}

/// Paints one vertical coverage case into one of the logical cell's two terminal columns.
fn render_vertical_coverage(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    coverage: Coverage,
    color: Color,
    underlay: Underlay,
) {
    match coverage {
        Coverage::Full => render_full_cell(buf, x, y, color),
        Coverage::Leading(amount) => render_top_fraction(buf, x, y, amount, color, underlay),
        Coverage::Trailing(amount) => {
            render_bottom_fraction(buf, x, y, amount, color, underlay);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;

    use super::{render_moving_snake_head, render_moving_snake_tail, render_snake};
    use crate::{
        geometry::{Board, Direction, Point},
        snake::Snake,
        terminal_cells::{BOARD_BACKGROUND, BOARD_BACKGROUND_ALT, BODY_COLOR, HEAD_COLOR},
    };

    #[test]
    fn coverage_tracks_leading_full_and_trailing_cells() {
        assert_eq!(
            super::Coverage::for_span(0.25, 2.25, 0.0),
            super::Coverage::Trailing(6)
        );
        assert_eq!(
            super::Coverage::for_span(0.25, 2.25, 1.0),
            super::Coverage::Full
        );
        assert_eq!(
            super::Coverage::for_span(0.25, 2.25, 2.0),
            super::Coverage::Leading(2)
        );
    }

    #[test]
    fn fractions_round_to_terminal_block_steps() {
        assert_eq!(super::eighths(0.0), 0);
        assert_eq!(super::eighths(0.125), 1);
        assert_eq!(super::eighths(0.5), 4);
        assert_eq!(super::eighths(1.0), 8);
    }

    #[test]
    fn moving_tail_paints_the_whole_current_tail_underlay() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 4, 1));

        render_moving_snake_tail(&mut buffer, 0, 0, Point::new(0, 0), Point::new(1, 0), 0.25);

        assert_eq!(buffer[(3, 0)].symbol(), "█");
        assert_eq!(buffer[(3, 0)].style().fg, Some(BODY_COLOR));
    }

    #[test]
    fn moving_head_reveals_body_without_a_different_colored_gap() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 4, 1));

        render_moving_snake_head(&mut buffer, 0, 0, Point::new(0, 0), Point::new(1, 0), 0.25);

        assert_eq!(buffer[(0, 0)].symbol(), "▌");
        assert_eq!(buffer[(0, 0)].style().fg, Some(BODY_COLOR));
        assert_eq!(buffer[(0, 0)].style().bg, Some(HEAD_COLOR));
        assert_eq!(buffer[(1, 0)].style().fg, Some(HEAD_COLOR));
    }

    #[test]
    fn moving_tail_reveals_the_previous_cell_board_background() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 4, 1));

        render_moving_snake_tail(&mut buffer, 0, 0, Point::new(1, 0), Point::new(0, 0), 0.25);

        assert_eq!(buffer[(3, 0)].style().bg, Some(BOARD_BACKGROUND_ALT));
    }

    #[test]
    fn vertical_tail_uses_partial_height_blocks() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 2, 2));

        render_moving_snake_tail(&mut buffer, 0, 0, Point::new(0, 0), Point::new(0, 1), 0.25);

        assert_eq!(buffer[(0, 0)].symbol(), "▆");
        assert_eq!(buffer[(1, 0)].symbol(), "▆");
        assert_eq!(buffer[(0, 0)].style().fg, Some(BODY_COLOR));
        assert_eq!(buffer[(0, 0)].style().bg, Some(BOARD_BACKGROUND));
    }

    #[test]
    fn growth_keeps_the_new_tail_as_a_stable_body_cell() {
        let mut snake = Snake::new(Board::new(24, 20));
        snake.replace_segments_for_test(
            std::collections::VecDeque::from([Point::new(1, 0), Point::new(0, 0)]),
            Direction::Right,
        );
        snake.advance(Point::new(2, 0), true);
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 6, 1));

        render_snake(
            &snake,
            0.5,
            ratatui::layout::Rect::new(0, 0, 6, 1),
            &mut buffer,
        );

        assert_eq!(buffer[(0, 0)].symbol(), "█");
        assert_eq!(buffer[(1, 0)].symbol(), "█");
        assert_eq!(buffer[(0, 0)].style().fg, Some(BODY_COLOR));
    }
}
