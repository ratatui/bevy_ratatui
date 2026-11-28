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
//! # Drawing a frame
//!
//! Call [`draw`](ratatui::Terminal::draw) on the context **once per frame**, in `Update`
#![cfg_attr(
    feature = "windowed",
    doc = "(or, in windowed builds, in `PostUpdate` before [`TerminalSystems::Sync`](windowed::TerminalSystems::Sync))."
)]
#![cfg_attr(
    not(feature = "windowed"),
    doc = "(or, in windowed builds, in `PostUpdate` before `TerminalSystems::Sync`)."
)]
//! Ratatui is double buffered: each `draw` presents a whole frame and then swaps buffers, clearing
//! the one the next `draw` renders into, so a second `draw` in the same frame replaces the first
//! rather than adding to it. Split the frame with Ratatui layouts inside that one draw system and
//! let other systems contribute the data it draws, as the [layout example] does.
//!
//! When parts of the screen belong to decoupled systems or plugins, they can instead render into
//! the context's current buffer and leave presenting it to one system that runs after them all,
//! with [`Terminal::apply_buffer_with_cursor`](ratatui::Terminal::apply_buffer_with_cursor)
//! (Ratatui 0.30.1 or later). The [multi_draw example] does this; the essentials are:
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_ratatui::RatatuiContext;
//! use ratatui::{layout::{Position, Rect}, text::Line, widgets::Widget};
//!
//! #[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
//! struct DrawSystems;
//!
//! /// What this frame's contributors have done.
//! #[derive(Resource, Default)]
//! struct FramePass {
//!     started: bool,
//!     cursor: Option<Position>,
//! }
//!
//! /// Adopts the terminal size once, before the first contributor of a frame draws.
//! fn begin_frame(context: &mut RatatuiContext, pass: &mut FramePass) -> Result<Rect> {
//!     if !pass.started {
//!         context.autoresize()?;
//!         pass.started = true;
//!     }
//!     Ok(context.current_buffer_mut().area)
//! }
//!
//! fn draw_header(mut context: ResMut<RatatuiContext>, mut pass: ResMut<FramePass>) -> Result {
//!     let area = begin_frame(&mut context, &mut pass)?;
//!     Line::from("header").render(area, context.current_buffer_mut());
//!     Ok(())
//! }
//!
//! fn draw_prompt(mut context: ResMut<RatatuiContext>, mut pass: ResMut<FramePass>) -> Result {
//!     let area = begin_frame(&mut context, &mut pass)?;
//!     let row = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
//!     Line::from("> ").render(row, context.current_buffer_mut());
//!     pass.cursor = Some(Position::new(2, row.y));
//!     Ok(())
//! }
//!
//! fn present_frame(mut context: ResMut<RatatuiContext>, mut pass: ResMut<FramePass>) -> Result {
//!     // Take the cursor first so an idle frame or error never leaves a stale position behind.
//!     let cursor = pass.cursor.take();
//!     // Nothing was drawn this frame: keep showing the last one.
//!     if std::mem::take(&mut pass.started) {
//!         context.apply_buffer_with_cursor(cursor)?;
//!     }
//!     Ok(())
//! }
//!
//! App::new().init_resource::<FramePass>().add_systems(
//!     Update,
//!     (
//!         (draw_header, draw_prompt).chain().in_set(DrawSystems),
//!         present_frame.after(DrawSystems),
//!     ),
//! );
//! ```
//!
//! Each part of this keeps a rule that `draw` otherwise keeps for you:
//!
//! 1. **Resize lazily, once.** The first contributor of a frame calls
//!    [`autoresize`](ratatui::Terminal::autoresize), which clears both buffers when the size
//!    changed, so it must run before anything is drawn, and only on frames that are drawn. A
//!    frame no contributor draws then leaves the screen alone; in windowed builds that keeps the
//!    last frame visible while the window is resized.
//! 2. **Skip idle frames.** Presenting a buffer nobody drew into diffs it against the last frame
//!    and blanks the screen, so `present_frame` does nothing unless a contributor started the
//!    frame.
//! 3. **Redraw the whole frame.** The buffer starts out empty every frame, so every contributor
//!    must draw on every frame that is presented; a region nobody drew comes out blank. Put any
//!    run condition on the whole `DrawSystems` set so contributors skip together, and order
//!    contributors whose areas overlap so later ones paint over earlier ones.
//! 4. **Pass the cursor through a resource.** Render with `Widget::render` or
//!    `StatefulWidget::render` into
//!    [`current_buffer_mut`](ratatui::Terminal::current_buffer_mut). Do not use
//!    `get_frame().set_cursor_position(..)`: only `draw` reads the position a `Frame` records,
//!    so it is silently dropped. Store it in the pass instead and present with
//!    `pass.cursor.take()`, which also hides the cursor on later frames that set none.
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
//! [layout example]: https://github.com/ratatui/bevy_ratatui/blob/main/examples/layout.rs
//! [multi_draw example]: https://github.com/ratatui/bevy_ratatui/blob/main/examples/multi_draw.rs

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
/// Draw to the context once per frame in `Update`, or in `PostUpdate` before
/// [`TerminalSystems::Sync`](windowed::TerminalSystems::Sync) (when `bevy_terminal` reads the
/// grid); drawing in `Last` shows one frame late. See [Drawing a frame](crate#drawing-a-frame)
/// for why one `draw` per frame, and for composing a frame from several systems.
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
