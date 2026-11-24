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

/// The UI node the plugin shows the terminal in: the only image node in the app.
fn view(app: &mut App) -> (Node, Handle<Image>, Option<Entity>) {
    let (node, image, camera) = app
        .world_mut()
        .query::<(&Node, &ImageNode, Option<&UiTargetCamera>)>()
        .single(app.world())
        .expect("one view");
    (
        node.clone(),
        image.image.clone(),
        camera.map(|camera| camera.0),
    )
}

fn image(app: &mut App) -> Handle<Image> {
    app.world_mut()
        .query_filtered::<&TerminalTexture, With<RatatuiContext>>()
        .single(app.world())
        .expect("the context has a texture")
        .image()
        .clone()
}

#[test]
fn the_default_font_draws_text_in_a_grid_fitted_to_the_window() {
    let mut app = app();
    frames(&mut app, 3);
    assert_eq!(draw(&mut app, "hello").trim_end(), "hello");
    frames(&mut app, 1);
    let geometry = ready(&mut app);
    // Not the 80×24 the context starts at: 640×400 pixels of the cell that was measured.
    let cell = geometry.physical_cell_size();
    let fitted = GridSize::new((640.0 / cell.x) as u16, (400.0 / cell.y) as u16);
    assert_eq!(geometry.grid(), fitted);
    assert_ne!(fitted, GridSize::new(80, 24));
}

#[test]
fn the_texture_is_shown_at_its_size_in_ui_pixels() {
    let mut app = app();
    app.insert_resource(UiScale(2.0));
    frames(&mut app, 3);
    let geometry = ready(&mut app);
    assert_eq!(geometry.raster_scale(), 2.0, "rasterized at the UI's scale");
    let (node, shown, _) = view(&mut app);
    // Half the texture's pixels, which the UI doubles again.
    let size = geometry.size().as_vec2() / 2.0;
    assert_eq!((node.width, node.height), (px(size.x), px(size.y)));
    assert_eq!(shown, image(&mut app));
}

#[test]
fn configuration_set_at_startup_is_kept_and_a_change_while_running_is_drawn() {
    let mut app = app();
    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let at_startup = FontFaces::regular(fonts.add(Font::from_bytes(DEFAULT_FONT_DATA.to_vec())));
    let font = at_startup.clone();
    app.add_systems(
        PostStartup,
        move |mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>| {
            config.font = font.clone();
        },
    );
    frames(&mut app, 3);
    configure(&mut app, |config| assert_eq!(config.font, at_startup));
    let before = ready(&mut app);

    configure(&mut app, |config| {
        config.sizing = TerminalSizing::font(before.font_size() * 2.0)
    });
    frames(&mut app, 3);
    let after = ready(&mut app);
    assert_eq!(after.font_size(), before.font_size() * 2.0);
    assert!(
        after.grid().width < before.grid().width,
        "larger cells, fewer columns"
    );
}

#[test]
fn a_change_of_scale_factor_keeps_the_window_filled() {
    let mut app = app();
    frames(&mut app, 3);
    let before = ready(&mut app);
    window(&mut app, |window| {
        window.resolution.set_scale_factor_override(Some(2.0));
    });
    frames(&mut app, 4);
    let after = ready(&mut app);
    assert_eq!(after.raster_scale(), 2.0);
    // The same physical pixels hold half as many cells of twice the size.
    let columns = (640.0 / after.physical_cell_size().x) as u16;
    assert_eq!(after.grid().width, columns);
    assert!(after.grid().width < before.grid().width);
}

#[test]
fn a_minimized_window_keeps_its_grid() {
    let mut app = app();
    frames(&mut app, 3);
    let before = ready(&mut app).grid();
    window(&mut app, |window| {
        window.resolution.set_physical_resolution(0, 0);
    });
    frames(&mut app, 3);
    assert_eq!(ready(&mut app).grid(), before);
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
    // 8192 is the limit `bevy_terminal` takes a texture to have when, as here, there is no GPU.
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
fn an_inactive_camera_does_not_stand_in_for_one_that_draws() {
    let mut app = app();
    let inactive = app
        .world_mut()
        .spawn((
            Camera2d,
            Camera {
                is_active: false,
                ..default()
            },
        ))
        .id();
    frames(&mut app, 2);
    assert_eq!(cameras(&mut app).len(), 2);
    // The terminal is shown by the camera spawned for it, whichever Bevy UI would pick.
    let (.., camera) = view(&mut app);
    assert!(camera.is_some_and(|camera| camera != inactive));
}

#[test]
fn a_removed_context_is_no_longer_shown_and_a_new_one_is() {
    let mut app = app();
    frames(&mut app, 3);
    let first = image(&mut app);
    assert_eq!(view(&mut app).1, first);

    app.world_mut().remove_resource::<RatatuiContext>();
    frames(&mut app, 2);
    let (node, shown, _) = view(&mut app);
    assert_eq!((node.width, shown), (Val::Auto, ImageNode::default().image));

    let context = RatatuiContext::init().expect("a context");
    app.world_mut().insert_resource(context);
    frames(&mut app, 3);
    ready(&mut app);
    assert_eq!(view(&mut app).1, image(&mut app));
}
