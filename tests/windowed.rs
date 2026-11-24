//! The windowed backend, driven in a headless app: no window is opened and nothing reaches a GPU.
#![cfg(feature = "windowed")]

use bevy::{
    camera::RenderTarget,
    prelude::*,
    text::{DEFAULT_FONT_DATA, TextPlugin},
    window::{PrimaryWindow, WindowRef},
};
use bevy_ratatui::{RatatuiContext, RatatuiPlugins, windowed::*};

/// An app with a 640×400 primary window.
fn app() -> App {
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

fn frames(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}

fn status(app: &mut App) -> TerminalStatus {
    app.world_mut()
        .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
        .single(app.world())
        .expect("the context has a texture")
        .status()
        .clone()
}

/// The geometry of a terminal that is drawn.
fn ready(app: &mut App) -> TerminalGeometry {
    match status(app) {
        TerminalStatus::Ready(geometry) => geometry,
        other => panic!("expected Ready, got {other:?}"),
    }
}

fn configure(app: &mut App, change: impl FnOnce(&mut TerminalRenderConfig)) {
    let mut config = app
        .world_mut()
        .query_filtered::<&mut TerminalRenderConfig, With<RatatuiContext>>()
        .single_mut(app.world_mut())
        .expect("the context has a configuration");
    change(&mut config);
}

fn window(app: &mut App, change: impl FnOnce(&mut Window)) {
    let mut window = app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("primary window");
    change(&mut window);
}

/// Draws `text` and returns the first row of the grid.
fn draw(app: &mut App, text: &str) -> String {
    let mut context = app.world_mut().resource_mut::<RatatuiContext>();
    context
        .draw(|frame| frame.render_widget(text, frame.area()))
        .expect("draw");
    first_row(app)
}

fn first_row(app: &App) -> String {
    let context = app.world().resource::<RatatuiContext>();
    context.backend().grid().row_text(0)
}

fn cameras(app: &mut App) -> Vec<RenderTarget> {
    let mut cameras = app
        .world_mut()
        .query_filtered::<&RenderTarget, With<Camera>>();
    cameras.iter(app.world()).cloned().collect()
}

#[test]
fn the_default_font_draws_without_system_fonts() {
    let mut app = app();
    frames(&mut app, 2);
    ready(&mut app);
}

#[test]
fn a_font_set_at_startup_is_kept_and_can_be_swapped_while_running() {
    let mut app = app();
    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let [initial, replacement] =
        [(); 2].map(|()| fonts.add(Font::from_bytes(DEFAULT_FONT_DATA.to_vec())));
    let at_startup = FontFaces::regular(initial);
    let font = at_startup.clone();
    app.add_systems(
        PostStartup,
        move |mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>| {
            config.font = font.clone();
        },
    );
    frames(&mut app, 2);
    configure(&mut app, |config| assert_eq!(config.font, at_startup));

    configure(&mut app, |config| {
        config.font = FontFaces::regular(replacement);
        config.sizing = TerminalSizing::font(24.0);
    });
    frames(&mut app, 2);
    ready(&mut app);
}

#[test]
fn a_grid_too_large_to_draw_is_fitted_on_the_next_frame() {
    // On the very first frame: 80 columns of a 200px font exceed any texture.
    let mut app = app();
    app.add_systems(
        PostStartup,
        |mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>| {
            config.sizing = TerminalSizing::font(200.0);
        },
    );
    let too_large = TerminalStatus::Failed(TerminalFailure::TextureTooLarge);
    frames(&mut app, 1);
    assert_eq!(status(&mut app), too_large);
    frames(&mut app, 1);
    ready(&mut app);

    // And when a grid of small cells meets a jump in font size.
    configure(&mut app, |config| config.sizing = TerminalSizing::font(8.0));
    frames(&mut app, 2);
    configure(&mut app, |config| {
        config.sizing = TerminalSizing::font(250.0)
    });
    frames(&mut app, 1);
    assert_eq!(status(&mut app), too_large);
    frames(&mut app, 1);
    ready(&mut app);
}

#[test]
fn a_window_larger_than_a_texture_settles_on_the_largest_grid_that_can_be_drawn() {
    let mut app = app();
    window(&mut app, |window| {
        window.resolution.set_physical_resolution(20_000, 400);
    });
    frames(&mut app, 6);
    let settled = ready(&mut app);
    let width = settled.size().x as f32;
    assert!(width <= 8192.0 && width + settled.physical_cell_size().x > 8192.0);
    for _ in 0..4 {
        frames(&mut app, 1);
        assert_eq!(ready(&mut app), settled, "the grid keeps changing");
    }
}

#[test]
fn a_fractional_scale_fills_every_column_that_fits() {
    let mut app = app();
    window(&mut app, |window| {
        window.resolution.set_scale_factor_override(Some(1.25));
        window.resolution.set_physical_resolution(1400, 400);
    });
    frames(&mut app, 6);
    let geometry = ready(&mut app);
    assert_eq!(geometry.raster_scale(), 1.25);
    let columns = (1400.0 / geometry.physical_cell_size().x).floor();
    assert_eq!(f32::from(geometry.grid().width), columns);
}

#[test]
fn a_frame_drawn_once_survives_fitting_the_window() {
    let mut app = app();
    app.add_systems(
        Update,
        |mut context: ResMut<RatatuiContext>, mut drawn: Local<bool>| -> Result {
            if !std::mem::replace(&mut *drawn, true) {
                context.draw(|frame| frame.render_widget("once", frame.area()))?;
            }
            Ok(())
        },
    );
    frames(&mut app, 6);
    assert_ne!(
        ready(&mut app).grid(),
        GridSize::new(80, 24),
        "the grid was fitted"
    );
    assert_eq!(first_row(&app).trim_end(), "once");
}

#[test]
fn a_window_dragged_narrower_and_back_keeps_its_frame() {
    let mut app = app();
    frames(&mut app, 4);
    let line = "0123456789".repeat(20);
    let whole = draw(&mut app, &line);
    assert!(!whole.ends_with(' '), "the line fills the row: {whole:?}");
    // Two resizes with no draw between them, as an application that draws on change sees.
    for width in [200, 640] {
        window(&mut app, |window| {
            window.resolution.set_physical_resolution(width, 400);
        });
        frames(&mut app, 1);
    }
    assert_eq!(first_row(&app), whole, "the frame is whole before any draw");
    // Ratatui takes the same frame to be on screen already and writes nothing.
    assert_eq!(draw(&mut app, &line), whole);
}

#[test]
fn a_camera_is_spawned_only_when_the_primary_window_has_none() {
    let primary =
        |target: &RenderTarget| matches!(target, RenderTarget::Window(WindowRef::Primary));

    // One spawned before startup, or in the last startup schedule, is the only camera.
    let mut before = app();
    before.world_mut().spawn(Camera2d);
    let mut in_post_startup = app();
    in_post_startup.add_systems(PostStartup, |mut commands: Commands| {
        commands.spawn(Camera2d);
    });
    for mut app in [before, in_post_startup] {
        frames(&mut app, 2);
        assert_eq!(cameras(&mut app).len(), 1);
    }

    // One on another window does not show the terminal: the primary window gets its own.
    let mut app = app();
    let other = app.world_mut().spawn(Window::default()).id();
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(other))));
    frames(&mut app, 2);
    let cameras = cameras(&mut app);
    assert_eq!(cameras.len(), 2);
    assert_eq!(cameras.iter().filter(|target| primary(target)).count(), 1);
}

#[test]
fn removing_the_context_stops_the_terminal_without_a_panic() {
    let mut app = app();
    frames(&mut app, 1);
    app.world_mut().remove_resource::<RatatuiContext>();
    frames(&mut app, 2);
}
