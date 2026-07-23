use std::io::{Stdout, stdout};

use bevy::prelude::*;

use ratatui::Terminal;
use ratatui::crossterm::{
    ExecutableCommand, cursor,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::backend::CrosstermBackend;

use crate::{RatatuiPlugins, context::TerminalContext};

use super::{cleanup::CleanupPlugin, error::ErrorPlugin, event::EventPlugin, kitty::KittyPlugin};

#[cfg(feature = "mouse")]
use super::mouse::MousePlugin;
#[cfg(feature = "keyboard")]
use super::translation::TranslationPlugin;

/// Ratatui context that will draw to the terminal buffer using crossterm.
#[derive(Deref, DerefMut, Debug)]
pub struct CrosstermContext(Terminal<CrosstermBackend<Stdout>>);

impl TerminalContext for CrosstermContext {
    type Backend = CrosstermBackend<Stdout>;

    fn init() -> Result<Self> {
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self(terminal))
    }

    fn restore() -> Result<()> {
        let mut stdout = stdout();
        stdout
            .execute(LeaveAlternateScreen)?
            .execute(cursor::Show)?;
        disable_raw_mode()?;
        Ok(())
    }

    fn configure_plugin_group(
        group: &RatatuiPlugins,
        mut builder: bevy::app::PluginGroupBuilder,
    ) -> bevy::app::PluginGroupBuilder {
        builder = builder
            .add(CleanupPlugin::<Self>::default())
            .add(ErrorPlugin::<Self>::default())
            .add(EventPlugin::default())
            .add(KittyPlugin::<Self>::default());

        #[cfg(feature = "mouse")]
        let builder = builder.add(MousePlugin);
        #[cfg(feature = "keyboard")]
        let builder = builder.add(TranslationPlugin);

        let mut builder = builder;
        if !group.enable_kitty_protocol {
            builder = builder.disable::<KittyPlugin<Self>>();
        }

        #[cfg(feature = "mouse")]
        if !group.enable_mouse_capture {
            builder = builder.disable::<MousePlugin>();
        }

        #[cfg(feature = "keyboard")]
        if !group.enable_input_forwarding {
            builder = builder.disable::<TranslationPlugin>();
        }

        builder
    }
}
