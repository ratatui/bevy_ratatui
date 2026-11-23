use bevy::prelude::*;
use bevy_terminal_ratatui::RatatuiBackend;
use ratatui::Terminal;

use crate::context::TerminalContext;

use super::plugin::WindowedPlugin;

/// Ratatui context that draws into a cell grid which [`WindowedPlugin`] renders to a texture and
/// displays in a window, instead of drawing to a terminal buffer.
#[derive(Debug, Deref, DerefMut)]
pub struct WindowedContext(Terminal<RatatuiBackend>);

impl TerminalContext<RatatuiBackend> for WindowedContext {
    fn init() -> Result<Self> {
        // A starting size only: the grid follows the window once the font has been measured.
        Ok(Self(Terminal::new(RatatuiBackend::new(80, 24))?))
    }

    fn restore() -> Result<()> {
        Ok(())
    }

    fn configure_plugin_group(
        _group: &crate::RatatuiPlugins,
        mut builder: bevy::app::PluginGroupBuilder,
    ) -> bevy::app::PluginGroupBuilder {
        builder = builder.add(WindowedPlugin);

        builder
    }
}
