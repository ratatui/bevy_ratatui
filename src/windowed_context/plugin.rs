use bevy::{
    prelude::*,
    ui::{DefaultUiCamera, UiSystems},
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
        app.add_plugins(RatatuiSourcePlugin::<RatatuiContext>::default())
            // After every startup schedule, so a camera spawned in any of them is seen.
            .add_systems(PreUpdate, setup.run_if(run_once))
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

/// Spawns the UI node showing the terminal, and a 2D camera unless the application spawned one
/// that Bevy UI draws to by default: one on the primary window, or marked `IsDefaultUiCamera`.
/// A camera on another window or an image would show nothing.
fn setup(mut commands: Commands, ui_camera: DefaultUiCamera) {
    if ui_camera.get().is_none() {
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

/// Resizes the grid to fill the window, at the cell the terminal measured.
///
/// The cell is measured whatever the grid's size, so a grid too large for a texture (on the
/// first frame, or after a jump in font size) is fitted like any other. The backend keeps what
/// is drawn through any number of resizes: Ratatui adopts the new size, and clears, on its next
/// draw, and an application that draws only when its state changes keeps its last frame until
/// then.
fn fit_to_window(
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Option<Res<UiScale>>,
    texture: Single<&TerminalTexture, With<RatatuiContext>>,
    mut context: ResMut<RatatuiContext>,
) {
    let window_size = window.resolution.physical_size().as_vec2();
    // A minimized window has no size; keep the grid, and what is drawn in it, until it returns.
    if window_size.min_element() <= 0.0 {
        return;
    }
    let Some(cell) = texture.cell() else {
        return;
    };
    // The window in the pixels the terminal is rasterized in: its own physical pixels, unless
    // `bevy_terminal` clamped the scale. Whole pixels divide into whole cells exactly, which
    // sizes in UI pixels do not at fractional scales.
    let raster_per_window = cell.raster_scale() / ui_scale_factor(&window, ui_scale.as_deref());
    // A window larger than a texture can be is filled as far as one reaches.
    let grid = cell.grid_for(window_size * raster_per_window);
    // Compared first, so that an unchanged context is not marked changed.
    if grid != context.backend().grid().size() {
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
    fn a_window_dragged_narrower_and_back_keeps_its_frame() {
        let mut app = headless_windowed_app();
        for _ in 0..4 {
            app.update();
        }
        let line = "0123456789".repeat(20);
        let draw = |app: &mut App| {
            let mut context = app.world_mut().resource_mut::<RatatuiContext>();
            context
                .draw(|frame| frame.render_widget(line.as_str(), frame.area()))
                .expect("draw");
            context.backend().grid().row_text(0)
        };
        let whole = draw(&mut app);
        assert!(!whole.ends_with(' '), "the line fills the row: {whole:?}");
        // Two resizes with no draw between them, as an application that draws on change sees.
        for width in [200, 640] {
            app.world_mut()
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(app.world_mut())
                .expect("primary window")
                .resolution
                .set_physical_resolution(width, 400);
            app.update();
        }
        let shown = |app: &App| {
            let context = app.world().resource::<RatatuiContext>();
            context.backend().grid().row_text(0)
        };
        assert_eq!(shown(&app), whole, "the frame is whole before any draw");
        // Ratatui takes the same frame to be on screen already and writes nothing.
        assert_eq!(draw(&mut app), whole);
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
    fn a_camera_on_another_window_does_not_stand_in_for_one_on_the_primary() {
        use bevy::{camera::RenderTarget, window::WindowRef};
        let mut app = headless_windowed_app();
        let other = app.world_mut().spawn(Window::default()).id();
        app.world_mut()
            .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(other))));
        app.update();
        app.update();
        let primary = app
            .world_mut()
            .query::<&RenderTarget>()
            .iter(app.world())
            .filter(|target| matches!(target, RenderTarget::Window(WindowRef::Primary)))
            .count();
        assert_eq!(primary, 1, "a camera for the primary window is spawned");
    }

    #[test]
    fn a_camera_spawned_in_the_last_startup_schedule_is_not_duplicated() {
        let mut app = headless_windowed_app();
        app.add_systems(PostStartup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        });
        app.update();
        app.update();
        let cameras = app
            .world_mut()
            .query_filtered::<Entity, With<Camera>>()
            .iter(app.world())
            .count();
        assert_eq!(cameras, 1);
    }

    #[test]
    fn a_removed_context_stops_the_terminal_without_a_panic() {
        let mut app = headless_windowed_app();
        app.update();
        app.world_mut().remove_resource::<RatatuiContext>();
        app.update();
        app.update();
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

        // The 200px cell was measured all the same, so the next frame fits the 640x400 window.
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
