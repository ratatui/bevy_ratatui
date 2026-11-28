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
///
/// [`ContextPlugin`] creates the context in `PreStartup`, in [`ContextSystems`], so after
/// successful setup and `PreStartup`'s deferred flush it is ready for ordinary `Startup` systems
/// (see [`ContextSystems`] for the full contract). A [`RatatuiContext`] inserted after adding the
/// plugins and before [`ContextSystems`] is kept. In terminal (non-`windowed`) builds, Kitty and
/// mouse modes apply only to a context [`ContextPlugin`] creates, while input forwarding (if
/// enabled) still requests focus reporting in `PostStartup` and the context is still removed on
/// [`AppExit`].
///
/// When the `windowed` feature is enabled anywhere in the build, this plugin group adds only
/// `WindowedPlugin` and [`ContextPlugin`], and its three terminal flags
/// ([`enable_kitty_protocol`](Self::enable_kitty_protocol),
/// [`enable_mouse_capture`](Self::enable_mouse_capture), and
/// [`enable_input_forwarding`](Self::enable_input_forwarding)) have no effect. In terminal
/// builds, `enable_input_forwarding` requires the `keyboard` feature and `enable_mouse_capture`
/// requires the `mouse` feature.
pub struct RatatuiPlugins {
    /// Use kitty protocol if available and enabled. Has no effect under `windowed`.
    pub enable_kitty_protocol: bool,
    /// Capture mouse if enabled (requires the `mouse` feature). Has no effect under `windowed`.
    pub enable_mouse_capture: bool,
    /// Forwards terminal input events to the bevy input system if enabled (requires the
    /// `keyboard` feature). Has no effect under `windowed`.
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

/// System set for [`ContextPlugin`]'s terminal context setup, running in `PreStartup`.
///
/// [`ContextPlugin`] checks for an existing [`RatatuiContext`] in the world and inserts a new one
/// via `Commands`. By default `PreStartup` flushes deferred commands at the end of the schedule
/// (`Schedule::set_apply_final_deferred(true)`), so once `PreStartup` completes after successful
/// setup, `Res<RatatuiContext>` is available to ordinary `Startup` systems without extra ordering.
/// In windowed mode, `TerminalRenderConfig` is available whenever `WindowedPlugin` was added
/// before insertion, and `TerminalTexture` is available whenever `Assets<Image>` (e.g. via
/// `DefaultPlugins`, or `AssetPlugin` with `ImagePlugin` / `App::init_asset::<Image>`) also
/// existed at insertion time.
///
/// To supply a pre-created [`RatatuiContext`], ensure it is visible in the world before
/// `ContextSystems` runs: insert it after adding the plugins with `App::insert_resource`, or from
/// a `PreStartup` system ordered `.before(ContextSystems)`. Unordered `Commands` insertions in
/// `PreStartup` are not guaranteed to precede setup. With `auto_insert_apply_deferred = false`, a
/// `Commands` producer `.before(ContextSystems)` requires an explicit [`ApplyDeferred`] before
/// setup (or direct `&mut World` insertion), and a reader `.after(ContextSystems)` inside
/// `PreStartup` requires an explicit [`ApplyDeferred`] after setup to observe a context created
/// by [`ContextPlugin`].
///
/// Initial `bevy_state` `OnEnter` transitions run in `StateTransition` before `PreStartup`;
/// insert a context before `App::run` if an initial `OnEnter` system needs it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContextSystems;

/// Initializes the selected terminal backend in `PreStartup` (in [`ContextSystems`]), and removes
/// the context on [`AppExit`] in terminal (crossterm) mode.
///
/// If a [`RatatuiContext`] already exists when [`ContextSystems`] runs, the plugin keeps it; in
/// terminal (non-`windowed`) builds, `enable_kitty_protocol` and `enable_mouse_capture` apply
/// only to a context it creates.
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

        app.add_systems(
            PreStartup,
            (move |mut commands: Commands, existing: Option<Res<RatatuiContext>>| -> Result {
                if existing.is_some() {
                    return Ok(());
                }
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
            })
            .in_set(ContextSystems),
        );

        #[cfg(not(feature = "windowed"))]
        app.add_systems(
            Last,
            (|mut commands: Commands| commands.remove_resource::<RatatuiContext>())
                .run_if(on_message::<AppExit>),
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings};

    use super::*;

    #[derive(Default, Resource)]
    struct Order(Vec<&'static str>);

    fn record(label: &'static str) -> impl FnMut(ResMut<Order>) {
        move |mut order| order.0.push(label)
    }

    #[test]
    fn context_plugin_schedules_in_pre_startup_without_ambiguities() {
        let mut app = App::new();
        app.add_plugins(ContextPlugin::default())
            // Keep the test headless: never open a real terminal.
            .configure_sets(PreStartup, ContextSystems.run_if(|| false));

        let settings = ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..default()
        };
        app.edit_schedule(PreStartup, |s| {
            s.set_build_settings(settings.clone());
        });
        app.edit_schedule(Startup, |s| {
            s.set_build_settings(settings.clone());
        });

        // Register reverse-ordered PreStartup hooks, unrelated systems in both schedules,
        // and a plain Startup consumer.
        app.init_resource::<Order>()
            .add_systems(
                PreStartup,
                (
                    record("after").after(ContextSystems),
                    record("before").before(ContextSystems),
                    || {},
                ),
            )
            .add_systems(Startup, (record("startup"), || {}));

        app.update();

        assert!(app.world().get_resource::<RatatuiContext>().is_none());
        assert_eq!(
            app.world().resource::<Order>().0,
            ["before", "after", "startup"]
        );
    }
}
