use bevy::{
    camera::RenderTarget, prelude::*, render::renderer::RenderDevice, ui::UiSystems,
    window::PrimaryWindow,
};
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

/// Spawns a default 2D camera (if no camera drawing to a window was spawned during `Startup`;
/// one drawing to an image shows nothing) and the UI node showing the terminal.
fn setup(mut commands: Commands, cameras: Query<&RenderTarget, With<Camera>>) {
    if !cameras
        .iter()
        .any(|target| matches!(target, RenderTarget::Window(_)))
    {
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
/// A scale that is not a positive number counts as one, as `bevy_terminal` rasterizes it.
fn ui_scale_factor(window: &Window, ui_scale: Option<&UiScale>) -> f32 {
    let scale = window.scale_factor() * ui_scale.map_or(1.0, |scale| scale.0);
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
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
///
/// Only the backend's grid is resized, which keeps what is drawn in the cells both sizes share:
/// Ratatui adopts the new size, and clears, on its next draw. An application that draws only
/// when its state changes keeps its last frame until then.
fn fit_to_window(
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Option<Res<UiScale>>,
    texture: Single<&TerminalTexture, With<RatatuiContext>>,
    device: Option<Res<RenderDevice>>,
    mut context: ResMut<RatatuiContext>,
) {
    let window_size = window.resolution.physical_size().as_vec2();
    // A minimized window has no size; keep the grid, and what is drawn in it, until it returns.
    if window_size.min_element() <= 0.0 {
        return;
    }
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
        }
        return;
    };
    // Geometry for another grid size was measured before the last resize.
    if geometry.grid() != current {
        return;
    }
    // The window in the pixels the terminal is rasterized in: its own physical pixels, unless
    // `bevy_terminal` clamped the scale. Whole pixels divide into whole cells exactly, which
    // sizes in UI pixels do not at fractional scales.
    let raster_per_window = geometry.raster_scale() / ui_scale_factor(&window, ui_scale.as_deref());
    // A window larger than a texture can be is filled as far as one reaches, rather than asking
    // for a grid that cannot be drawn. `bevy_terminal` assumes the same limit without a device.
    let limit = device.map_or(8192, |device| device.limits().max_texture_dimension_2d);
    let available = (window_size * raster_per_window).min(Vec2::splat(limit as f32));
    let grid = grid_for(available, geometry.physical_cell_size());
    if grid != current {
        context.backend_mut().resize(grid.width, grid.height);
    }
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
    fn a_window_larger_than_a_texture_settles_on_the_largest_drawable_grid() {
        let mut app = headless_windowed_app();
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .expect("primary window")
            .resolution
            .set_physical_resolution(20_000, 400);
        let status = |app: &mut App| {
            app.update();
            app.world_mut()
                .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
                .single(app.world())
                .expect("terminal texture attached")
                .status()
                .clone()
        };
        for _ in 0..6 {
            status(&mut app);
        }
        let settled = status(&mut app);
        let geometry = settled.clone();
        let TerminalStatus::Ready(geometry) = geometry else {
            panic!("expected Ready status, got {settled:?}");
        };
        assert!(geometry.size().x <= 8192 && geometry.size().x > 8192 - 64);
        for _ in 0..4 {
            assert_eq!(status(&mut app), settled, "the grid keeps changing");
        }
    }

    #[test]
    fn a_frame_drawn_once_survives_fitting_the_window() {
        let mut app = headless_windowed_app();
        app.add_systems(
            Update,
            |mut context: ResMut<RatatuiContext>, mut drawn: Local<bool>| -> Result {
                if !std::mem::replace(&mut *drawn, true) {
                    context.draw(|frame| frame.render_widget("once", frame.area()))?;
                }
                Ok(())
            },
        );
        for _ in 0..6 {
            app.update();
        }
        let context = app.world().resource::<RatatuiContext>();
        let grid = context.backend().grid();
        assert_ne!(grid.size(), GridSize::new(80, 24), "the grid was fitted");
        assert_eq!(grid.row_text(0).trim_end(), "once");
    }

    #[test]
    fn a_fractional_scale_fills_every_column_that_fits() {
        let mut app = headless_windowed_app();
        let mut window = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .expect("primary window");
        window.resolution.set_scale_factor_override(Some(1.25));
        window.resolution.set_physical_resolution(1400, 400);
        for _ in 0..6 {
            app.update();
        }
        let geometry = *app
            .world_mut()
            .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
            .single(app.world())
            .expect("terminal texture attached")
            .measured()
            .expect("ready");
        assert_eq!(geometry.raster_scale(), 1.25);
        let cell = geometry.physical_cell_size().x as u32;
        assert_eq!(1400 % cell, 0, "the window is a whole number of cells wide");
        assert_eq!(u32::from(geometry.grid().width), 1400 / cell);
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
