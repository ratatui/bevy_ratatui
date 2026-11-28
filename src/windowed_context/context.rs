use std::io;

use bevy::prelude::*;
use bevy_terminal_ratatui::RatatuiBackend;
use ratatui::Terminal;

/// Ratatui context that draws into a cell grid which
/// [`WindowedPlugin`](crate::windowed::WindowedPlugin) renders to a texture and displays in a
/// window, instead of drawing to a terminal buffer.
///
/// It dereferences to the Ratatui [`Terminal`]. Call [`draw`](Terminal::draw) on it once per
/// frame, from one system in `Update` (or in `PostUpdate` before
/// [`TerminalSystems::Sync`](crate::windowed::TerminalSystems::Sync)): a second `draw` in the
/// same frame starts from a cleared buffer and erases what the first one drew. To let several
/// systems draw parts of one frame, see [Drawing a frame](crate#drawing-a-frame).
///
/// The grid follows the window, and keeps showing the last frame until the next draw. Resizing
/// Ratatui's own buffers ([`autoresize`](Terminal::autoresize), which `draw` calls) clears the
/// grid, so call it only on a frame that goes on to draw.
#[derive(Resource, Deref, DerefMut)]
pub struct WindowedContext(Terminal<RatatuiBackend>);

/// Names the context without printing the screen it holds.
impl std::fmt::Debug for WindowedContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowedContext").finish_non_exhaustive()
    }
}

impl WindowedContext {
    /// Create a terminal backed by the windowed renderer.
    pub fn init() -> Result<Self> {
        Ok(Self(Terminal::new(RatatuiBackend::new(80, 24))?))
    }

    /// No-op: the windowed renderer acquires no terminal modes.
    pub fn restore() -> Result {
        Ok(())
    }

    /// Always `Ok(false)`: the windowed renderer uses Bevy keyboard input.
    pub fn enable_kitty_protocol(&self) -> io::Result<bool> {
        Ok(false)
    }

    /// Always `false`: the windowed renderer uses Bevy keyboard input.
    pub fn kitty_enabled(&self) -> bool {
        false
    }

    /// No-op: the windowed renderer uses Bevy mouse input.
    #[cfg(feature = "mouse")]
    pub fn enable_mouse_capture(&self) -> io::Result<()> {
        Ok(())
    }

    /// Always `false`: the windowed renderer uses Bevy mouse input.
    #[cfg(feature = "mouse")]
    pub fn mouse_enabled(&self) -> bool {
        false
    }
}
