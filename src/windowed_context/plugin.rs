use bevy::prelude::*;
use bevy_terminal_ratatui::prelude::*;

use crate::RatatuiContext;

/// A plugin that, rather than drawing to a terminal buffer, renders the ratatui buffer to a 2D
/// texture with `bevy_terminal`, and displays the result in a window. The grid is resized to fill
/// the primary window and rasterized at its scale factor.
///
/// By default, the terminal uses Bevy's built-in monospace font (`Handle::<Font>::default()`,
/// covering ASCII, with `bevy_terminal` rendering box-drawing, block, shade, and braille symbols
/// procedurally and requiring no system font discovery). That font has one face, and Bevy does
/// not synthesize weight or slant, so bold and italic text look like regular text until the
/// configuration names bold and italic faces (see the `windowed_fonts` example). Applications can
/// supply embedded or dynamically loaded font assets on `TerminalRenderConfig` in `PostStartup` or
/// swap them in any `Update` system while running.
///
/// The fitting and display are `bevy_terminal_ratatui`'s `RatatuiWindowPlugin`, which owns two
/// things an application would otherwise set: the grid's size, which follows the window, and
/// `TerminalRenderConfig::raster.scale`, which follows the window's scale factor and [`UiScale`]
/// and is overwritten every frame. The grid is fitted in `PreUpdate`, so a draw in `Update` is at
/// this frame's size; order other systems against it with
/// [`RatatuiWindowSystems`](crate::windowed::RatatuiWindowSystems).
///
/// Add the plugin before the [`RatatuiContext`] resource is inserted, as
/// [`RatatuiPlugins`](crate::RatatuiPlugins) does: a context that exists first is never drawn.
pub struct WindowedPlugin;

impl Plugin for WindowedPlugin {
    fn build(&self, app: &mut App) {
        // Draws the context's grid, fits it to the primary window and shows it there.
        app.add_plugins(RatatuiWindowPlugin::<RatatuiContext>::default());
    }
}

/// The grid `bevy_terminal` renders is the one the context's backend draws into.
impl TerminalSource for RatatuiContext {
    fn grid(&self) -> &TerminalGrid {
        self.backend().grid()
    }
}

impl RatatuiSource for RatatuiContext {
    fn backend_mut(&mut self) -> &mut RatatuiBackend {
        self.0.backend_mut()
    }
}
