//! Terminal layout and rendering.
//!
//! [`crate::playfield`] owns terminal-size policy. This module performs the other half of the
//! `bevy_ratatui` integration: [`draw_terminal`] projects [`Game`] into Ratatui cells.
//!
//! [`draw_terminal`] borrows [`RatatuiContext`], calls `draw`, and renders directly into the
//! frame buffer. The module is otherwise a projection layer: it reads [`Game`] and writes terminal
//! cells, but it does not mutate game rules. [`crate::game_loop::sync_playfield`] updates the
//! shared playfield before drawing so the rule layer already knows the active board.
//! This separation keeps Ratatui's immediate-mode rendering from becoming an implicit second update
//! loop.
//!
//! The playfield uses [`crate::terminal_cells`] to render each logical [`Point`] as two
//! side-by-side terminal cells. That keeps the arena readable while compensating for terminal
//! cells that are usually taller than they are wide.
//!
//! Smooth movement is a rendering concern, not a rule concern. The head and tail are drawn between
//! logical movement ticks, using partial terminal-cell block glyphs. The middle of the body is
//! rendered on stable board cells so turns keep a consistent corner shape instead of flickering as
//! adjacent body segments round through different sub-cell positions.

use bevy::prelude::*;
use bevy_ratatui::RatatuiContext;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction as LayoutDirection, Layout, Margin, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Widget},
};

use crate::{
    game::{Game, GameStatus},
    game_loop::GameTiming,
    geometry::Point,
    hud::{render_controls, render_hud, render_overlay, render_too_small},
    playfield::{CONTROLS_HEIGHT, HUD_HEIGHT, MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH, Playfield},
    snake_rendering::render_snake,
    terminal_cells::{
        BOARD_BACKGROUND, TERMINAL_COLUMNS_PER_CELL, render_background_cell, render_food_cell,
    },
};

/// Border color separating collision bounds from the checker background.
const WALL_COLOR: Color = Color::Rgb(66, 120, 91);

/// Draws the current [`Game`] resource through [`RatatuiContext`].
///
/// Returning Bevy's `Result` allows terminal draw failures to propagate through the system runner.
/// The closure receives the current Ratatui frame and writes widgets into its buffer; because this
/// system runs in `PostUpdate`, it observes all chained game-loop changes from the same app frame.
/// [`Playfield`] supplies the same availability decision that gated simulation during update.
pub fn draw_terminal(
    mut context: ResMut<RatatuiContext>,
    game: Res<Game>,
    timing: Res<GameTiming>,
    playfield: Res<Playfield>,
) -> Result {
    let playfield_available = playfield.is_available();
    context.draw(|frame| {
        let area = frame.area();
        render_game(
            game.as_ref(),
            timing.movement_progress(),
            timing.crash_grace_active(),
            playfield_available,
            area,
            frame.buffer_mut(),
        );
    })?;

    Ok(())
}

/// Projects one complete game frame using the board availability synchronized during `Update`.
fn render_game(
    game: &Game,
    movement_progress: f32,
    crash_grace_active: bool,
    playfield_available: bool,
    area: Rect,
    buf: &mut Buffer,
) {
    if !playfield_available {
        render_too_small(area, buf, MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT);
        return;
    }

    let layout = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(HUD_HEIGHT),
            Constraint::Min(0),
            Constraint::Length(CONTROLS_HEIGHT),
        ])
        .split(area);

    render_hud(game, crash_grace_active, layout[0], buf);
    render_board(game, movement_progress, layout[1], buf);
    render_controls(layout[2], buf);

    match game.status() {
        GameStatus::Paused => render_overlay("Paused", "Press Space or P to resume", area, buf),
        GameStatus::GameOver => {
            render_overlay("Game over", "Press R to restart or Q to quit", area, buf)
        }
        GameStatus::Playing => {}
    }
}

/// Paints the bordered playfield before layering food and snake cells over its background.
fn render_board(game: &Game, movement_progress: f32, area: Rect, buf: &mut Buffer) {
    Block::default()
        .title(" Snake ")
        .borders(Borders::ALL)
        .border_style(Style::new().fg(WALL_COLOR))
        .style(Style::new().bg(BOARD_BACKGROUND))
        .render(area, buf);

    let board = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    // Each logical game cell renders as two terminal columns. This keeps the board readable on
    // ordinary terminals while avoiding the tall-cell feel of a single-column grid.
    let game_board = game.board();
    for y in 0..game_board.height() {
        for x in 0..game_board.width() {
            let column = board.x + x as u16 * TERMINAL_COLUMNS_PER_CELL;
            let point = Point::new(x, y);
            render_background_cell(buf, column, board.y + y as u16, point);
            if Some(point) == game.food() {
                render_food_cell(buf, column, board.y + y as u16, point);
            }
        }
    }

    render_snake(game.snake(), movement_progress, board, buf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_playfield_uses_fallback_even_when_frame_is_large() {
        let area = Rect::new(0, 0, 100, 40);
        let mut buffer = Buffer::empty(area);

        render_game(&Game::new(), 0.0, false, false, area, &mut buffer);

        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .concat();
        assert!(rendered.contains("Terminal too small"));
    }
}
