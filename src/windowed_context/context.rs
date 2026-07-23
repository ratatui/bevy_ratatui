use std::ops::DerefMut;

use std::fmt::Debug;

use bevy::prelude::*;

use ratatui::Terminal;

use crate::context::TerminalContext;
use soft_ratatui::embedded_graphics_unicodefonts::{
    mono_8x13_atlas, mono_8x13_bold_atlas, mono_8x13_italic_atlas,
};
use soft_ratatui::{EmbeddedGraphics, RasterBackend, SoftBackend};

use super::plugin::WindowedPlugin;

/// Ratatui context that will set up a window and render the ratatui buffer using a 2D texture,
/// instead of drawing to a terminal buffer.
#[derive(Deref, DerefMut)]
pub struct WindowedContext(Terminal<SoftBackend<EmbeddedGraphics>>);

/// Trait for windowed contexts backed by [`soft_ratatui::SoftBackend`].
pub trait SoftTerminalContext:
    TerminalContext<Backend = SoftBackend<Self::RasterBackend>>
    + DerefMut<Target = Terminal<SoftBackend<Self::RasterBackend>>>
{
    type RasterBackend: RasterBackend;

    fn soft_backend(&self) -> &SoftBackend<Self::RasterBackend> {
        self.backend()
    }

    fn soft_backend_mut(&mut self) -> &mut SoftBackend<Self::RasterBackend> {
        self.backend_mut()
    }

    fn configure_windowed_plugin_group(
        mut builder: bevy::app::PluginGroupBuilder,
    ) -> bevy::app::PluginGroupBuilder
    where
        Self: Sized,
    {
        builder = builder.add(WindowedPlugin::<Self>::default());
        builder
    }
}

impl<T, R> SoftTerminalContext for T
where
    T: TerminalContext<Backend = SoftBackend<R>> + DerefMut<Target = Terminal<SoftBackend<R>>>,
    R: RasterBackend,
{
    type RasterBackend = R;
}

impl Debug for WindowedContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WindowedContext()")
    }
}

impl TerminalContext for WindowedContext {
    type Backend = SoftBackend<EmbeddedGraphics>;

    fn init() -> Result<Self> {
        let font_regular = mono_8x13_atlas();
        let font_italic = mono_8x13_italic_atlas();
        let font_bold = mono_8x13_bold_atlas();
        let backend = SoftBackend::<EmbeddedGraphics>::new(
            100,
            50,
            font_regular,
            Some(font_bold),
            Some(font_italic),
        );
        let terminal = Terminal::new(backend)?;
        Ok(Self(terminal))
    }

    fn restore() -> Result<()> {
        Ok(())
    }

    fn configure_plugin_group(
        _group: &crate::RatatuiPlugins,
        builder: bevy::app::PluginGroupBuilder,
    ) -> bevy::app::PluginGroupBuilder {
        Self::configure_windowed_plugin_group(builder)
    }
}
