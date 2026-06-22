//! Terminal controls for the snake example.
//!
//! [`bevy_ratatui::RatatuiPlugins`] publishes terminal input as messages such as [`KeyMessage`]. A
//! useful application boundary is to translate those transport-specific events once into a small
//! application message such as [`GameCommand`]. Downstream systems can then express intent without
//! depending on Crossterm key codes, and another input source can emit the same commands later.
//!
//! The repeat policy remains application-specific. Snake accepts repeats for directions so held
//! keys stay responsive, but accepts pause and restart only on the initial press because repeating
//! those commands would produce several state changes for one physical key hold.

use bevy::prelude::*;
use bevy_ratatui::event::KeyMessage;
use ratatui::crossterm::event::{KeyCode, KeyEventKind};

use crate::geometry::Direction;

/// Player intent after terminal-specific input has been translated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Message)]
pub enum GameCommand {
    /// Queue a turn for the next snake step.
    Turn(Direction),
    /// Pause or resume the current round.
    TogglePause,
    /// Start a fresh round on the current board.
    Restart,
    /// Exit the example.
    Quit,
}

/// Translates `bevy_ratatui` key messages into application-level commands.
///
/// `MessageReader` and `MessageWriter` keep this system independent from the
/// [`Game`](crate::game::Game) resource. That separation is useful when input mapping and
/// application behavior need different tests or schedule placement.
pub fn translate_keyboard_input(
    mut key_messages: MessageReader<KeyMessage>,
    mut game_commands: MessageWriter<GameCommand>,
) {
    for message in key_messages.read() {
        if let Some(command) = command_for_key(message.code, message.kind) {
            game_commands.write(command);
        }
    }
}

/// Maps one terminal key event to player intent.
///
/// Movement is the only repeatable intent. One-shot commands reject both repeat and release events
/// so terminal repeat settings cannot make pause state or round initialization nondeterministic.
fn command_for_key(code: KeyCode, kind: KeyEventKind) -> Option<GameCommand> {
    if kind == KeyEventKind::Release {
        return None;
    }

    let command = match code {
        KeyCode::Up => GameCommand::Turn(Direction::Up),
        KeyCode::Down => GameCommand::Turn(Direction::Down),
        KeyCode::Left => GameCommand::Turn(Direction::Left),
        KeyCode::Right => GameCommand::Turn(Direction::Right),
        KeyCode::Esc => GameCommand::Quit,
        KeyCode::Char(' ') => GameCommand::TogglePause,
        KeyCode::Char(character) => match character.to_ascii_lowercase() {
            'w' => GameCommand::Turn(Direction::Up),
            's' => GameCommand::Turn(Direction::Down),
            'a' => GameCommand::Turn(Direction::Left),
            'd' => GameCommand::Turn(Direction::Right),
            'p' => GameCommand::TogglePause,
            'r' => GameCommand::Restart,
            'q' => GameCommand::Quit,
            _ => return None,
        },
        _ => return None,
    };

    if kind == KeyEventKind::Repeat && !matches!(command, GameCommand::Turn(_)) {
        return None;
    }

    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_movement_keys_to_turns() {
        assert_eq!(
            command_for_key(KeyCode::Up, KeyEventKind::Press),
            Some(GameCommand::Turn(Direction::Up))
        );
        assert_eq!(
            command_for_key(KeyCode::Char('a'), KeyEventKind::Press),
            Some(GameCommand::Turn(Direction::Left))
        );
        assert_eq!(
            command_for_key(KeyCode::Char('D'), KeyEventKind::Press),
            Some(GameCommand::Turn(Direction::Right))
        );
    }

    #[test]
    fn maps_control_keys_to_commands() {
        assert_eq!(
            command_for_key(KeyCode::Char('p'), KeyEventKind::Press),
            Some(GameCommand::TogglePause)
        );
        assert_eq!(
            command_for_key(KeyCode::Char(' '), KeyEventKind::Press),
            Some(GameCommand::TogglePause)
        );
        assert_eq!(
            command_for_key(KeyCode::Char('r'), KeyEventKind::Press),
            Some(GameCommand::Restart)
        );
        assert_eq!(
            command_for_key(KeyCode::Esc, KeyEventKind::Press),
            Some(GameCommand::Quit)
        );
    }

    #[test]
    fn repeats_only_repeat_movement_intent() {
        assert_eq!(
            command_for_key(KeyCode::Up, KeyEventKind::Repeat),
            Some(GameCommand::Turn(Direction::Up))
        );
        assert_eq!(
            command_for_key(KeyCode::Char('p'), KeyEventKind::Repeat),
            None
        );
        assert_eq!(
            command_for_key(KeyCode::Char('r'), KeyEventKind::Repeat),
            None
        );
        assert_eq!(command_for_key(KeyCode::Up, KeyEventKind::Release), None);
    }
}
