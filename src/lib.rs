//! A collection of plugins for building terminal-based applications with [Bevy] and [Ratatui].
//!
//! # Example
//!
//! ```rust,no_run
//! use std::time::Duration;
//!
//! use bevy::{
//!     app::{AppExit, ScheduleRunnerPlugin},
//!     prelude::*,
//! };
//! use bevy_ratatui::{event::KeyMessage, RatatuiContext, RatatuiPlugins};
//! use ratatui::crossterm::event::KeyCode;
//! use ratatui::text::Text;
//!
//! fn main() {
//!     let frame_time = Duration::from_secs_f32(1. / 60.);
//!
//!     App::new()
//!         .add_plugins((
//!             MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(frame_time)),
//!             RatatuiPlugins::default(),
//!         ))
//!         .add_systems(PreUpdate, input_system)
//!         .add_systems(Update, draw_system)
//!         .run();
//! }
//!
//! fn draw_system(mut context: ResMut<RatatuiContext>) -> Result {
//!     context.draw(|frame| {
//!         let text = Text::raw("hello world\npress 'q' to quit");
//!         frame.render_widget(text, frame.area());
//!     })?;
//!
//!     Ok(())
//! }
//!
//! fn input_system(mut messages: MessageReader<KeyMessage>, mut exit: MessageWriter<AppExit>) {
//!     for message in messages.read() {
//!         if let KeyCode::Char('q') = message.code {
//!             exit.write_default();
//!         }
//!     }
//! }
//! ```
//!
//! See the [examples] directory for more examples.
//!
//! # Input Forwarding
//!
//! The terminal input can be forwarded to the bevy input system. See the
//! [translation] module documentation for details.
//!
//! [Bevy]: https://bevyengine.org
//! [Ratatui]: https://ratatui.rs
//! [examples]: https://github.com/ratatui/bevy_ratatui/tree/main/examples

mod context_trait;
#[cfg(feature = "crossterm")]
mod crossterm_context;
mod ratatui_context;
mod ratatui_plugin;
#[cfg(feature = "windowed")]
mod windowed_context;

pub use ratatui_context::RatatuiContext;
pub use ratatui_plugin::RatatuiPlugins;

#[cfg(feature = "crossterm")]
pub use ratatui::crossterm;

pub mod context {
    pub use super::context_trait::TerminalContext;
    #[cfg(feature = "crossterm")]
    pub use super::crossterm_context::context::CrosstermContext;
    pub use super::ratatui_context::DefaultContext;
    pub use super::ratatui_plugin::ContextPlugin;
    #[cfg(feature = "windowed")]
    pub use super::windowed_context::context::WindowedContext;
}

#[cfg(feature = "crossterm")]
pub mod cleanup {
    pub use super::crossterm_context::cleanup::CleanupPlugin;
}

#[cfg(feature = "crossterm")]
pub mod error {
    pub use super::crossterm_context::error::ErrorPlugin;
}

#[cfg(feature = "crossterm")]
pub mod event {
    pub use super::crossterm_context::event::{
        CrosstermMessage, EventPlugin, FocusMessage, InputSet, KeyMessage, MouseMessage,
        PasteMessage, ResizeMessage,
    };
}

#[cfg(feature = "crossterm")]
pub mod kitty {
    pub use super::crossterm_context::kitty::{KittyEnabled, KittyPlugin};
}

#[cfg(all(feature = "crossterm", feature = "mouse"))]
pub mod mouse {
    pub use super::crossterm_context::mouse::{MouseEnabled, MousePlugin};
}

#[cfg(feature = "crossterm")]
pub mod translation {
    #[cfg(feature = "keyboard")]
    pub use super::crossterm_context::translation::*;
}

#[cfg(feature = "windowed")]
pub mod windowed {
    pub use super::windowed_context::plugin::WindowedPlugin;
}
