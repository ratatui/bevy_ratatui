//! A complete `bevy_ratatui` application organized around Bevy schedules and resources.
//!
//! Run it with:
//!
//! ```sh
//! cargo run --example snake
//! ```
//!
//! The app follows a reusable terminal-application flow. [`RatatuiPlugins`] owns terminal setup and
//! emits input messages. [`PreUpdate`] translates those messages into application intent, chained
//! [`Update`] systems mutate resources in a known order, and [`PostUpdate`] draws the resulting
//! state through [`bevy_ratatui::RatatuiContext`]. Keeping drawing last means widgets only project
//! state; they do not become another owner of application behavior.
//!
//! Start with the four modules that show the reusable `bevy_ratatui` integration:
//!
//! - [`controls`] translates terminal key messages into game commands.
//! - [`game_loop`] connects Bevy time, commands, and terminal resize events to the rules.
//! - [`terminal`] projects the current game state into Ratatui widgets and cells.
//! - [`main`](crate) wires those systems into Bevy schedules.
//!
//! The remaining modules keep game-specific detail out of that path:
//!
//! - [`game`], [`snake`], and [`geometry`] own framework-independent rules and values.
//! - [`playfield`] shares terminal-derived board availability between update and rendering.
//! - [`hud`] renders text widgets around the board.
//! - [`terminal_cells`] renders stable logical game cells to terminal cells.
//! - [`snake_rendering`] renders stable segments and smoothly moving ends.

#![warn(rustdoc::broken_intra_doc_links)]

pub mod controls;
pub mod game;
pub mod game_loop;
pub mod geometry;
pub mod hud;
pub mod playfield;
pub mod snake;
pub mod snake_rendering;
pub mod terminal;
pub mod terminal_cells;

use std::time::Duration;

use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use bevy_ratatui::{RatatuiPlugins, event::InputSet};
use controls::{GameCommand, translate_keyboard_input};
use game::Game;
use game_loop::{GameTiming, advance_game, apply_game_commands, sync_playfield};
use playfield::Playfield;
use terminal::draw_terminal;

/// Builds the terminal app and assigns input, state updates, and drawing to separate schedules.
///
/// `ScheduleRunnerPlugin` supplies a regular frame cadence without Bevy's window stack. The 60 FPS
/// frame rate keeps terminal input and partial-cell rendering responsive, while
/// [`game_loop::GameTiming`] demonstrates that application updates can use their own slower
/// cadence.
fn main() -> Result<()> {
    color_eyre::install()?;

    let translate_input = translate_keyboard_input.in_set(InputSet::Post);

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1. / 60.,
            ))),
            RatatuiPlugins::default(),
        ))
        .init_resource::<Game>()
        .init_resource::<GameTiming>()
        .init_resource::<Playfield>()
        .add_message::<GameCommand>()
        .add_systems(PreUpdate, translate_input)
        .add_systems(
            Update,
            (sync_playfield, apply_game_commands, advance_game).chain(),
        )
        .add_systems(PostUpdate, draw_terminal)
        .run();

    Ok(())
}
