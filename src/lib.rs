#![cfg_attr(docsrs, feature(doc_cfg))]
//! A collection of plugins for building terminal-based applications with [Bevy] and [Ratatui].
//!
//! # Example
//!
//! This example uses the default `crossterm` backend.
#![cfg_attr(
    feature = "windowed",
    doc = "For the windowed backend, see the [`windowed`] module."
)]
#![cfg_attr(
    not(feature = "windowed"),
    doc = "For the windowed backend, enable the `windowed` feature and see the `windowed` module."
)]
//!
#![cfg_attr(
    all(feature = "crossterm", not(feature = "windowed")),
    doc = "```rust,no_run"
)]
#![cfg_attr(
    not(all(feature = "crossterm", not(feature = "windowed"))),
    doc = "```rust,ignore"
)]
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
//! The terminal input can be forwarded to the bevy input system.
#![cfg_attr(
    all(feature = "crossterm", feature = "keyboard"),
    doc = "See the [`translation`] module documentation for details."
)]
#![cfg_attr(
    not(all(feature = "crossterm", feature = "keyboard")),
    doc = "See the `translation` module documentation (requires the `crossterm` and `keyboard` features) for details."
)]
//!
//! [Bevy]: https://bevyengine.org
//! [Ratatui]: https://ratatui.rs
//! [examples]: https://github.com/ratatui/bevy_ratatui/tree/main/examples

#[cfg(not(any(feature = "crossterm", feature = "windowed")))]
compile_error!(
    "bevy_ratatui requires at least one backend feature: enable `crossterm` or `windowed`."
);

#[cfg(feature = "crossterm")]
mod crossterm_context;
#[cfg(any(feature = "crossterm", feature = "windowed"))]
#[cfg_attr(docsrs, doc(auto_cfg = false))]
mod ratatui_plugin;
#[cfg(feature = "windowed")]
mod windowed_context;

/// The active Ratatui terminal context resource for the selected backend.
///
/// This is a compile-time alias (see [`context`]): when the `windowed` feature is enabled
/// anywhere in the build, it resolves to `WindowedContext`; otherwise it resolves to
/// `CrosstermContext`.
#[cfg(any(feature = "crossterm", feature = "windowed"))]
#[cfg_attr(docsrs, doc(auto_cfg = false))]
#[doc(inline)]
pub use context::DefaultContext as RatatuiContext;
#[cfg(any(feature = "crossterm", feature = "windowed"))]
#[cfg_attr(docsrs, doc(auto_cfg = false))]
pub use ratatui_plugin::{ContextSystems, RatatuiPlugins};

#[cfg(feature = "crossterm")]
pub use ratatui::crossterm;

#[cfg(any(feature = "crossterm", feature = "windowed"))]
#[cfg_attr(docsrs, doc(auto_cfg = false))]
pub mod context {
    #[cfg(feature = "crossterm")]
    pub use super::crossterm_context::context::CrosstermContext;
    pub use super::ratatui_plugin::{ContextPlugin, ContextSystems};
    #[cfg(feature = "windowed")]
    pub use super::windowed_context::context::WindowedContext;
    #[cfg(all(feature = "crossterm", not(feature = "windowed")))]
    pub use CrosstermContext as DefaultContext;
    #[cfg(feature = "windowed")]
    pub use WindowedContext as DefaultContext;
}

#[cfg(feature = "crossterm")]
pub mod event {
    pub use super::crossterm_context::event::{
        CrosstermMessage, EventPlugin, FocusMessage, InputSet, KeyMessage, MouseMessage,
        PasteMessage, ResizeMessage,
    };
}

#[cfg(all(feature = "crossterm", feature = "keyboard"))]
pub use crossterm_context::translation;

#[cfg(all(feature = "crossterm", not(feature = "keyboard")))]
pub mod translation {}

/// The windowed backend: the plugin, the backend the context draws into, and the renderer's
/// configuration and inspection types.
///
/// Draw to the context in `Update`, or in `PostUpdate` before
/// [`TerminalSystems::Sync`](windowed::TerminalSystems::Sync) (when `bevy_terminal` reads the
/// grid); drawing in `Last` shows one frame late.
///
/// The context is a resource that also carries the renderer's components. Set fonts, sizing,
/// theme and cursor on its [`TerminalRenderConfig`](windowed::TerminalRenderConfig) in
/// `Startup` (the context is created in `PreStartup`) or at any time after, and read what was
/// drawn from its [`TerminalTexture`](windowed::TerminalTexture):
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_ratatui::{RatatuiContext, RatatuiPlugins, windowed::*};
///
/// fn larger_text(mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>) {
///     config.sizing = TerminalSizing::font(24.0);
/// }
///
/// App::new()
///     .add_plugins((DefaultPlugins, RatatuiPlugins::default()))
///     .add_systems(Startup, larger_text);
/// ```
#[cfg(feature = "windowed")]
pub mod windowed {
    pub use super::windowed_context::plugin::{WindowedCursor, WindowedPlugin, WindowedSystems};
    /// The backend the windowed context draws into.
    pub use bevy_terminal_ratatui::RatatuiBackend;
    /// Renderer configuration and inspection types for the windowed terminal.
    pub use bevy_terminal_ratatui::prelude::{
        BlinkConfig, CellGeometry, CursorConfig, CursorStyle, FontFaces, FontHinting, FontSource,
        GridSize, RasterConfig, TerminalFailure, TerminalGeometry, TerminalRenderConfig,
        TerminalSizing, TerminalStats, TerminalStatus, TerminalSystems, TerminalTexture,
        TerminalTheme,
    };
    /// The grid the backend draws into, as [`RatatuiBackend::grid`] returns it, and its cells.
    pub use bevy_terminal_ratatui::prelude::{
        CellOccupancy, CellPosition, StyleFlags, TerminalCell, TerminalColor, TerminalGrid,
        TerminalStyle,
    };
}
