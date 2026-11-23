use bevy::{prelude::*, ui::UiSystems, window::PrimaryWindow};
use bevy_terminal_ratatui::prelude::*;

use crate::RatatuiContext;

/// A plugin that, rather than drawing to a terminal buffer, renders the ratatui buffer to a 2D
/// texture with `bevy_terminal`, and displays the result in a window. The grid is resized to fill
/// the primary window and rasterized at its scale factor.
///
/// By default, the terminal uses Bevy's built-in monospace font (`Handle::<Font>::default()`,
/// covering ASCII, with `bevy_terminal` rendering box-drawing, block, shade, and braille symbols
/// procedurally and requiring no system font discovery). Applications can supply embedded or
/// dynamically loaded font assets on `TerminalRenderConfig` in `PostStartup` or swap them in any
/// `Update` system while running.
pub struct WindowedPlugin;

impl Plugin for WindowedPlugin {
    fn build(&self, app: &mut App) {
        // Default to Bevy's built-in monospace font asset (no system font discovery).
        // Registered before `RatatuiSourcePlugin` so user configs set at creation or runtime
        // are never overwritten.
        let _ = app
            .world_mut()
            .try_register_required_components_with::<RatatuiContext, TerminalRenderConfig>(|| {
                TerminalRenderConfig {
                    font: FontFaces::regular(Handle::<Font>::default()),
                    ..default()
                }
            });

        app.add_plugins(RatatuiSourcePlugin::<RatatuiContext>::default())
            .add_systems(PostStartup, setup)
            // Before any `Update` system draws, so a resize never clears a frame already drawn.
            .add_systems(PreUpdate, (follow_window_scale, fit_to_window))
            .add_systems(
                PostUpdate,
                present
                    .after(TerminalSystems::Sync)
                    .before(UiSystems::Layout),
            );
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

/// The UI node that displays the terminal texture.
#[derive(Component)]
struct TerminalView;

/// Spawns a default 2D camera (if no camera was spawned during `Startup`) and the UI node showing
/// the terminal.
fn setup(mut commands: Commands, cameras: Query<(), With<Camera>>) {
    if cameras.is_empty() {
        commands.spawn(Camera2d);
    }
    commands.spawn((
        TerminalView,
        ImageNode::default(),
        Node {
            justify_self: JustifySelf::Center,
            align_self: AlignSelf::Center,
            ..default()
        },
    ));
}

/// The scale Bevy UI applies to this window: one logical UI pixel is this many physical pixels.
fn ui_scale_factor(window: &Window, ui_scale: Option<&UiScale>) -> f32 {
    window.scale_factor() * ui_scale.map_or(1.0, |scale| scale.0)
}

/// Rasterizes the terminal at the scale the UI displays it, so text stays sharp on HiDPI displays.
fn follow_window_scale(
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Option<Res<UiScale>>,
    mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
) {
    let scale = ui_scale_factor(&window, ui_scale.as_deref());
    if config.raster.scale != scale {
        config.raster.scale = scale;
    }
}

/// Resizes the grid to fill the window, at the cell size measured for the last frame.
fn fit_to_window(
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Option<Res<UiScale>>,
    texture: Single<&TerminalTexture, With<RatatuiContext>>,
    mut context: ResMut<RatatuiContext>,
) -> Result {
    let current = context.backend().grid().size();
    let Some(geometry) = texture.measured() else {
        // Recover from an oversized grid (whether on frame 1 or after a font-size jump) by
        // shrinking to 1×1 so the next sync measures the new cell size and refills the window.
        if matches!(
            texture.status(),
            TerminalStatus::Failed(TerminalFailure::TextureTooLarge)
        ) && current != GridSize::new(1, 1)
        {
            context.backend_mut().resize(1, 1);
            context.autoresize()?;
        }
        return Ok(());
    };
    // Geometry for another grid size was measured before the last resize.
    if geometry.grid() != current {
        return Ok(());
    }
    // The space the window offers, in the UI pixels the terminal's node is sized in.
    let available =
        window.resolution.physical_size().as_vec2() / ui_scale_factor(&window, ui_scale.as_deref());
    // A minimized window has no size; keep the grid, and what is drawn in it, until it returns.
    if available.min_element() <= 0.0 {
        return Ok(());
    }
    let grid = grid_for(available, geometry.cell_size());
    if grid != current {
        context.backend_mut().resize(grid.width, grid.height);
        context.autoresize()?;
    }
    Ok(())
}

/// Shows the rendered texture at its measured size.
fn present(
    texture: Single<&TerminalTexture, (With<RatatuiContext>, Changed<TerminalTexture>)>,
    view: Single<(&mut Node, &mut ImageNode), With<TerminalView>>,
) {
    let Some(geometry) = texture.measured() else {
        return;
    };
    let (mut node, mut image) = view.into_inner();
    let size = geometry.logical_size();
    if node.width != px(size.x) {
        node.width = px(size.x);
    }
    if node.height != px(size.y) {
        node.height = px(size.y);
    }
    if image.image != *texture.image() {
        image.image = texture.image().clone();
    }
}

#[cfg(test)]
mod tests {
    use bevy::text::{DEFAULT_FONT_DATA, TextPlugin};

    use super::*;
    use crate::RatatuiPlugins;

    fn headless_windowed_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TextPlugin,
            RatatuiPlugins::default(),
        ))
        .init_asset::<Image>();
        app.world_mut().spawn((
            Window {
                resolution: (640, 400).into(),
                ..default()
            },
            PrimaryWindow,
        ));
        app
    }

    #[test]
    fn default_font_renders_ready_without_system_fonts() {
        let mut app = headless_windowed_app();
        app.update();
        app.update();

        let texture = app
            .world_mut()
            .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
            .single(app.world())
            .expect("terminal texture attached");
        assert!(
            texture.measured().is_some(),
            "expected Ready status, got {:?}",
            texture.status()
        );
    }

    #[test]
    fn custom_font_at_creation_is_preserved_and_swappable_at_runtime() {
        #[derive(Resource)]
        struct InitialFont(Handle<Font>);

        let mut app = headless_windowed_app();
        let initial = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(DEFAULT_FONT_DATA.to_vec()));
        let replacement = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(DEFAULT_FONT_DATA.to_vec()));
        app.insert_resource(InitialFont(initial.clone()));
        app.add_systems(
            PostStartup,
            |font: Res<InitialFont>,
             mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>| {
                config.font = FontFaces::regular(font.0.clone());
            },
        );

        // Startup + PostStartup + PostUpdate sync.
        app.update();
        app.update();

        let config = app
            .world_mut()
            .query_filtered::<&TerminalRenderConfig, With<RatatuiContext>>()
            .single(app.world())
            .expect("render config attached");
        assert_eq!(
            config.font,
            FontFaces::regular(initial),
            "PostStartup setup must not overwrite a font configured at creation"
        );

        // Swap font live while the terminal exists.
        {
            let mut config = app
                .world_mut()
                .query_filtered::<&mut TerminalRenderConfig, With<RatatuiContext>>()
                .single_mut(app.world_mut())
                .expect("render config attached");
            config.font = FontFaces::regular(replacement.clone());
            config.sizing = TerminalSizing::font(24.0);
        }
        app.update();
        app.update();

        let (config, texture) = app
            .world_mut()
            .query_filtered::<(&TerminalRenderConfig, &TerminalTexture), With<RatatuiContext>>()
            .single(app.world())
            .expect("terminal components present");
        assert_eq!(config.font, FontFaces::regular(replacement));
        assert!(
            texture.measured().is_some(),
            "swapped font should reach Ready status, got {:?}",
            texture.status()
        );
    }

    #[test]
    fn existing_camera_is_not_duplicated() {
        let mut app = headless_windowed_app();
        app.world_mut().spawn(Camera2d);
        app.update();

        let camera_count = app
            .world_mut()
            .query_filtered::<Entity, With<Camera>>()
            .iter(app.world())
            .count();
        assert_eq!(camera_count, 1);
    }

    #[test]
    fn texture_too_large_on_first_frame_and_after_font_growth_recovers() {
        // Trigger A: oversized on the very first frame (80 columns * ~120px > 8192px).
        let mut app = headless_windowed_app();
        app.add_systems(
            PostStartup,
            |mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>| {
                config.sizing = TerminalSizing::font(200.0);
            },
        );
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
                .single(app.world())
                .expect("terminal texture attached")
                .status(),
            &TerminalStatus::Failed(TerminalFailure::TextureTooLarge)
        );

        // Next frames shrink to 1x1, measure the 200px cell, and fit the 640x400 window.
        app.update();
        app.update();
        let texture = app
            .world_mut()
            .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
            .single(app.world())
            .expect("terminal texture attached");
        assert!(
            texture.measured().is_some(),
            "expected recovery from frame-1 TextureTooLarge, got {:?}",
            texture.status()
        );

        // Trigger B: shrink font so the grid grows large, then jump font size so it exceeds
        // the texture limit again without the window size changing.
        app.world_mut()
            .query_filtered::<&mut TerminalRenderConfig, With<RatatuiContext>>()
            .single_mut(app.world_mut())
            .expect("render config attached")
            .sizing = TerminalSizing::font(8.0);
        app.update();
        app.update();
        app.world_mut()
            .query_filtered::<&mut TerminalRenderConfig, With<RatatuiContext>>()
            .single_mut(app.world_mut())
            .expect("render config attached")
            .sizing = TerminalSizing::font(250.0);
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
                .single(app.world())
                .expect("terminal texture attached")
                .status(),
            &TerminalStatus::Failed(TerminalFailure::TextureTooLarge)
        );

        app.update();
        app.update();
        let texture = app
            .world_mut()
            .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
            .single(app.world())
            .expect("terminal texture attached");
        assert!(
            texture.measured().is_some(),
            "expected recovery after font-growth TextureTooLarge, got {:?}",
            texture.status()
        );
    }
}
