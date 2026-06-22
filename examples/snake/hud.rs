//! Heads-up display and overlay widgets.
//!
//! The board renderer owns game cells. This module owns the text around the board: score/status,
//! controls, pause and game-over overlays, and the small-terminal fallback.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

use crate::game::{Game, GameStatus};

/// Draws score, level, length, and state.
pub fn render_hud(game: &Game, crash_grace_active: bool, area: Rect, buf: &mut Buffer) {
    let line = Line::from_iter([
        " Snake ".black().on_green(),
        "  ".into(),
        format!("Score: {}", game.score()).yellow(),
        "  ".into(),
        format!("High: {}", game.high_score()).into(),
        "  ".into(),
        format!("Level: {}", game.level()).into(),
        "  ".into(),
        format!("Length: {}", game.snake().segment_count()).into(),
        "  ".into(),
        status_label(game, crash_grace_active).fg(status_color(game.status())),
    ]);

    Paragraph::new(line)
        .block(Block::new().borders(Borders::BOTTOM))
        .centered()
        .render(area, buf);
}

/// Draws the control strip below the board.
pub fn render_controls(area: Rect, buf: &mut Buffer) {
    let controls = Line::from_iter([
        "Move: ".into(),
        "Arrows/WASD".cyan(),
        "   Pause: ".into(),
        "Space/P".cyan(),
        "   Restart: ".into(),
        "R".cyan(),
        "   Quit: ".into(),
        "Q/Esc".cyan(),
    ]);

    Paragraph::new(controls)
        .centered()
        .block(Block::new().borders(Borders::TOP))
        .render(area, buf);
}

/// Draws the pause or game-over overlay.
pub fn render_overlay(title: &str, message: &str, area: Rect, buf: &mut Buffer) {
    let overlay = centered_rect(34, 5, area);
    Clear.render(overlay, buf);

    let text = Text::from_iter([
        Line::from(title).bold().centered(),
        Line::default(),
        Line::from(message).centered(),
    ]);

    Paragraph::new(text)
        .block(Block::bordered().border_style(Color::Yellow))
        .white()
        .on_black()
        .centered()
        .wrap(Wrap { trim: true })
        .render(overlay, buf);
}

/// Draws the fallback message when the terminal cannot fit the board.
pub fn render_too_small(area: Rect, buf: &mut Buffer, min_width: u16, min_height: u16) {
    let text = Text::from_iter([
        Line::from("Terminal too small").bold().centered(),
        Line::default(),
        Line::from(format!("Snake needs at least {min_width}x{min_height}.")).centered(),
    ]);

    Paragraph::new(text)
        .centered()
        .wrap(Wrap { trim: true })
        .render(area, buf);
}

/// Centers a requested overlay while clamping it to the available terminal area.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    area.centered(Constraint::Length(width), Constraint::Length(height))
}

/// Maps lifecycle state to a compact HUD signal without affecting board colors.
fn status_color(status: GameStatus) -> Color {
    match status {
        GameStatus::Playing => Color::Green,
        GameStatus::Paused => Color::Yellow,
        GameStatus::GameOver => Color::Red,
    }
}

/// Gives crash grace precedence while playing because immediate input matters.
fn status_label(game: &Game, crash_grace_active: bool) -> &'static str {
    if game.status() == GameStatus::Playing && crash_grace_active {
        "Turn now"
    } else {
        match game.status() {
            GameStatus::Playing => "Playing",
            GameStatus::Paused => "Paused",
            GameStatus::GameOver => "Game over",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_area_is_centered_and_clamped_to_the_terminal() {
        let area = Rect::new(10, 20, 40, 9);

        assert_eq!(centered_rect(34, 5, area), Rect::new(13, 22, 34, 5));
        assert_eq!(centered_rect(80, 24, area), area);
    }

    #[test]
    fn grace_only_overrides_the_playing_status() {
        let mut game = Game::new();
        assert_eq!(status_label(&game, true), "Turn now");

        game.end_after_collision();
        assert_eq!(status_label(&game, true), "Game over");
    }
}
