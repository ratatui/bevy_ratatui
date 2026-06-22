//! Terminal-derived playfield size and availability.
//!
//! [`Game`](crate::game::Game) stores the logical [`Board`] on which the current round was created.
//! [`Playfield`] separately records whether the terminal can currently display that board. Sharing
//! this environmental state as a Bevy resource lets update and draw systems follow the same policy
//! without performing terminal I/O in the rule or rendering paths.

use bevy::prelude::*;
use ratatui::layout::Size;

use crate::{geometry::Board, terminal_cells::TERMINAL_COLUMNS_PER_CELL};

/// Rows reserved above the playfield for score and round state.
pub const HUD_HEIGHT: u16 = 3;

/// Rows reserved below the playfield for controls.
pub const CONTROLS_HEIGHT: u16 = 3;

/// Combined horizontal or vertical space consumed by both sides of the board border.
const BOARD_BORDER: u16 = 2;

/// Smallest logical width that leaves steering room and enough columns for the fixed UI.
///
/// Thirty double-width cells plus the border produce 62 terminal columns; the controls need 61.
const MIN_BOARD_WIDTH: u16 = 30;

/// Smallest logical height that leaves useful room to steer the starting snake.
const MIN_BOARD_HEIGHT: u16 = 20;

/// Smallest terminal width that fits both the logical board and fixed surrounding UI.
pub const MIN_TERMINAL_WIDTH: u16 = MIN_BOARD_WIDTH * TERMINAL_COLUMNS_PER_CELL + BOARD_BORDER;

/// Smallest terminal height after accounting for fixed UI rows, borders, and the board.
pub const MIN_TERMINAL_HEIGHT: u16 = HUD_HEIGHT + CONTROLS_HEIGHT + BOARD_BORDER + MIN_BOARD_HEIGHT;

/// Playable logical grid currently available from the terminal.
///
/// A temporary resize below the minimum records `None` instead of changing the player's pause
/// state or killing the snake behind the fallback message. The current round can therefore resume
/// when a usable terminal area returns.
#[derive(Debug, Default, Resource)]
pub struct Playfield {
    /// Board dimensions available from the terminal, or `None` while it is too small or
    /// unavailable.
    current: Option<Board>,
}

impl Playfield {
    /// Updates availability from the terminal size observed this frame.
    ///
    /// `None` also represents a failed terminal size query. Returning the resulting board lets the
    /// coordinating system restart the round only when playable dimensions actually changed.
    pub fn update_for_size(&mut self, size: Option<Size>) -> Option<Board> {
        self.current = size.and_then(|size| board_for_terminal_size(size.width, size.height));
        self.current
    }

    /// Returns whether the terminal can currently display and advance the round.
    pub fn is_available(&self) -> bool {
        self.current.is_some()
    }
}

/// Returns the logical board that fits inside the terminal.
///
/// Each logical board cell is rendered as two terminal columns and one terminal row. This keeps the
/// board smaller than half-block mode while making horizontal and vertical movement closer in
/// physical size.
fn board_for_terminal_size(width: u16, height: u16) -> Option<Board> {
    if width < MIN_TERMINAL_WIDTH || height < MIN_TERMINAL_HEIGHT {
        return None;
    }

    let board_width = width.checked_sub(BOARD_BORDER)? / TERMINAL_COLUMNS_PER_CELL;
    let board_rows = height.checked_sub(HUD_HEIGHT + CONTROLS_HEIGHT + BOARD_BORDER)?;

    let board_width = i16::try_from(board_width).ok()?;
    let board_rows = i16::try_from(board_rows).ok()?;
    Board::try_new(board_width, board_rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_size_maps_to_double_width_board() {
        assert_eq!(board_for_terminal_size(100, 40), Some(Board::new(49, 32)));
    }

    #[test]
    fn minimum_size_fits_the_board_and_fixed_ui() {
        assert_eq!(
            board_for_terminal_size(MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT),
            Some(Board::new(30, 20))
        );
        assert_eq!(
            board_for_terminal_size(MIN_TERMINAL_WIDTH - 1, MIN_TERMINAL_HEIGHT),
            None
        );
    }

    #[test]
    fn small_terminals_do_not_create_a_board() {
        assert_eq!(board_for_terminal_size(20, 15), None);
    }

    #[test]
    fn dimensions_larger_than_the_board_coordinate_type_are_rejected() {
        assert_eq!(board_for_terminal_size(100, u16::MAX), None);
    }

    #[test]
    fn unavailable_size_clears_playfield_availability() {
        let mut playfield = Playfield::default();
        playfield.update_for_size(Some(Size::new(100, 40)));
        assert!(playfield.is_available());

        playfield.update_for_size(None);
        assert!(!playfield.is_available());
    }
}
