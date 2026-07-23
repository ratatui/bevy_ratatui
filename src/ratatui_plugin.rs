use std::marker::PhantomData;

use bevy::{
    app::{Plugin, PluginGroup, PluginGroupBuilder, Startup},
    prelude::{Commands, Result},
};

use crate::RatatuiContext;

use crate::context::TerminalContext;

/// A plugin group that includes all the plugins in the Ratatui crate.
///
/// # Example
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_ratatui::RatatuiPlugins;
///
/// App::new().add_plugins(RatatuiPlugins::default());
/// ```
pub struct RatatuiPlugins {
    /// Use kitty protocol if available and enabled.
    pub enable_kitty_protocol: bool,
    /// Capture mouse if enabled.
    pub enable_mouse_capture: bool,
    /// Forwards terminal input events to the bevy input system if enabled.
    pub enable_input_forwarding: bool,
}

impl RatatuiPlugins {
    pub fn in_context<C: TerminalContext>(self) -> RatatuiPluginsFor<C> {
        RatatuiPluginsFor(self, PhantomData)
    }
}

impl Default for RatatuiPlugins {
    fn default() -> Self {
        Self {
            enable_kitty_protocol: true,
            enable_mouse_capture: false,
            enable_input_forwarding: false,
        }
    }
}

impl PluginGroup for RatatuiPlugins {
    fn build(self) -> PluginGroupBuilder {
        self.in_context::<crate::context::DefaultContext>().build()
    }
}

pub struct RatatuiPluginsFor<C: TerminalContext>(RatatuiPlugins, PhantomData<fn() -> C>);

impl<C: TerminalContext> Default for RatatuiPluginsFor<C> {
    fn default() -> Self {
        RatatuiPlugins::default().in_context::<C>()
    }
}

impl<C: TerminalContext> PluginGroup for RatatuiPluginsFor<C> {
    fn build(self) -> PluginGroupBuilder {
        let mut builder = PluginGroupBuilder::start::<Self>();

        builder = builder.add(ContextPlugin::<C>::default());

        builder = C::configure_plugin_group(&self.0, builder);

        builder
    }
}

/// The plugin responsible for adding the `RatatuiContext` resource to your bevy application.
pub struct ContextPlugin<C: TerminalContext = crate::context::DefaultContext>(
    PhantomData<fn() -> C>,
);

impl<C: TerminalContext> Default for ContextPlugin<C> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<C: TerminalContext> Plugin for ContextPlugin<C> {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(Startup, context_setup::<C>);
    }
}

/// A startup system that sets up the terminal context.
pub fn context_setup<C: TerminalContext>(mut commands: Commands) -> Result {
    let terminal = RatatuiContext::<C>::init()?;
    commands.insert_resource(terminal);

    Ok(())
}
