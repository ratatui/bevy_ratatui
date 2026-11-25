use bevy::{app::PluginGroupBuilder, prelude::*};

use crate::RatatuiContext;

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
        let builder = PluginGroupBuilder::start::<Self>();
        #[cfg(feature = "windowed")]
        let builder = builder.add(crate::windowed::WindowedPlugin);
        #[cfg(not(feature = "windowed"))]
        let builder = builder.add(crate::event::EventPlugin::default());
        #[cfg(all(feature = "keyboard", not(feature = "windowed")))]
        let builder = {
            let builder = builder.add(crate::translation::TranslationPlugin);
            if self.enable_input_forwarding {
                builder
            } else {
                builder.disable::<crate::translation::TranslationPlugin>()
            }
        };
        builder.add(ContextPlugin {
            enable_kitty_protocol: self.enable_kitty_protocol,
            enable_mouse_capture: self.enable_mouse_capture,
        })
    }
}

/// Initializes the selected terminal backend during startup and removes the
/// context on [`AppExit`].
pub struct ContextPlugin {
    /// Enable the Kitty keyboard protocol when supported by the terminal.
    pub enable_kitty_protocol: bool,
    /// Enable mouse capture when built with the `mouse` feature.
    pub enable_mouse_capture: bool,
}

impl Default for ContextPlugin {
    fn default() -> Self {
        Self {
            enable_kitty_protocol: true,
            enable_mouse_capture: false,
        }
    }
}

impl Plugin for ContextPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(feature = "windowed"))]
        let kitty = self.enable_kitty_protocol;
        #[cfg(all(not(feature = "windowed"), feature = "mouse"))]
        let mouse = self.enable_mouse_capture;

        app.add_systems(Startup, move |mut commands: Commands| -> Result {
            let context = RatatuiContext::init()?;
            #[cfg(not(feature = "windowed"))]
            if kitty {
                let _ = context.enable_kitty_protocol();
            }
            #[cfg(all(not(feature = "windowed"), feature = "mouse"))]
            if mouse {
                context.enable_mouse_capture()?;
            }
            commands.insert_resource(context);
            Ok(())
        });

        #[cfg(not(feature = "windowed"))]
        app.add_systems(
            Last,
            (|mut commands: Commands| commands.remove_resource::<RatatuiContext>())
                .run_if(on_message::<AppExit>),
        );
    }
}
