//! The windowed context, driven in a headless app: no window is opened and nothing reaches a GPU.
//! `bevy_terminal_ratatui` tests the fitting and display themselves; these cover what is this
//! crate's: the context as a resource, and configuring it through the re-exports.
#![cfg(feature = "windowed")]

use bevy::{
    prelude::*,
    text::{DEFAULT_FONT_DATA, TextPlugin},
    window::PrimaryWindow,
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

/// Whether the context was changed since the last frame.
#[derive(Resource, Default)]
struct Changed(bool);

#[test]
fn a_frame_drawn_once_survives_fitting_the_window() {
    let mut app = app();
    app.add_systems(
        Update,
        |mut context: ResMut<RatatuiContext>,
         mut drawn: Local<bool>,
         mut changed: ResMut<Changed>|
         -> Result {
            changed.0 = context.is_changed();
            if !std::mem::replace(&mut *drawn, true) {
                context.draw(|frame| frame.render_widget("once", frame.area()))?;
            }
            Ok(())
        },
    )
    .init_resource::<Changed>();
    frames(&mut app, 6);
    let idle = !app.world().resource::<Changed>().0;
    assert!(idle, "a fitted context is left alone, not drawn again");
    assert_ne!(
        ready(&mut app).grid(),
        GridSize::new(80, 24),
        "the grid was fitted"
    );
    assert_eq!(first_row(&app).trim_end(), "once");
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
