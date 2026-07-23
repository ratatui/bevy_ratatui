use std::ops::Deref;

use bevy::{app::PluginGroupBuilder, prelude::Result};
use ratatui::{Terminal, prelude::Backend};

use crate::RatatuiPlugins;

/// Trait for types that implement lifecycle functions for initializing a terminal context and
/// restoring the terminal state after exiting. Implementors must also use their implementation of
/// the `configure_plugin_group()` function to add any systems, resources, events, etcetera
/// necessary for the functioning of its associated Ratatui backend or its particular
/// functionality.
pub trait TerminalContext:
    Sized + Send + Sync + Deref<Target = Terminal<Self::Backend>> + 'static
{
    type Backend: Backend + 'static;

    /// Initialize the terminal context.
    fn init() -> Result<Self>;

    /// Restore the terminal to its normal state after exiting.
    fn restore() -> Result<()>;

    /// Configure the plugin group to add the plugins necessary for this particular backend's
    /// functionality.
    fn configure_plugin_group(
        group: &RatatuiPlugins,
        builder: PluginGroupBuilder,
    ) -> PluginGroupBuilder;
}
