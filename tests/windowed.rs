//! The windowed backend, driven in a headless app: no window is opened and nothing reaches a GPU.
#![cfg(feature = "windowed")]

use bevy::{
    camera::{
        CameraUpdateSystems, RenderTarget, RenderTargetInfo, Viewport, visibility::VisibilityPlugin,
    },
    ecs::schedule::{LogLevel, ScheduleBuildSettings},
    input::{ButtonState, InputPlugin, mouse::MouseButtonInput},
    mesh::MeshPlugin,
    prelude::*,
    text::{DEFAULT_FONT_DATA, TextPlugin},
    ui::{UiPlugin, UiSystems},
    window::{PrimaryWindow, WindowRef},
};
use bevy_ratatui::{ContextSystems, RatatuiContext, RatatuiPlugins, windowed::*};

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

/// Enforces strict ambiguity detection on `PreStartup` and `Startup`, and configures automatic
/// deferred command flushing.
fn strict_startup(app: &mut App, auto_insert_apply_deferred: bool) {
    let settings = ScheduleBuildSettings {
        auto_insert_apply_deferred,
        ambiguity_detection: LogLevel::Error,
        ..default()
    };
    app.edit_schedule(PreStartup, |s| {
        s.set_build_settings(settings.clone());
    });
    app.edit_schedule(Startup, |s| {
        s.set_build_settings(settings.clone());
    });
}

#[derive(Resource, Default)]
struct Ran(bool);

#[test]
fn the_context_and_its_components_are_there_in_startup() {
    let mut app = app();
    strict_startup(&mut app, false);
    // Unrelated systems in both schedules must stay ambiguity-free alongside `ContextSystems`,
    // and an unannotated `Startup` consumer must see the context and renderer components.
    app.init_resource::<Ran>()
        .add_systems(PreStartup, || {})
        .add_systems(
            Startup,
            (
                || {},
                |_: Res<RatatuiContext>,
                 mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
                 _: Single<&TerminalTexture, With<RatatuiContext>>,
                 mut ran: ResMut<Ran>| {
                    config.sizing = TerminalSizing::font(24.0);
                    ran.0 = true;
                },
            ),
        );
    app.update();
    assert!(app.world().resource::<Ran>().0, "the system was skipped");
    frames(&mut app, 3);

    // Drawn as the same configuration set later, in `PostStartup`.
    let mut later = self::app();
    later.add_systems(
        PostStartup,
        |mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>| {
            config.sizing = TerminalSizing::font(24.0);
        },
    );
    frames(&mut later, 4);
    let unconfigured = {
        let mut app = self::app();
        frames(&mut app, 4);
        ready(&mut app).font_size()
    };
    let font_size = ready(&mut app).font_size();
    assert_eq!(font_size, ready(&mut later).font_size());
    assert_ne!(font_size, unconfigured);
}

#[test]
fn pre_startup_after_context_systems_sees_the_context_and_its_components() {
    let mut app = app();
    strict_startup(&mut app, true);
    app.init_resource::<Ran>().add_systems(
        PreStartup,
        (
            || {},
            (|_: Res<RatatuiContext>,
              mut config: Single<&mut TerminalRenderConfig, With<RatatuiContext>>,
              _: Single<&TerminalTexture, With<RatatuiContext>>,
              mut ran: ResMut<Ran>| {
                config.sizing = TerminalSizing::font(24.0);
                ran.0 = true;
            })
            .after(ContextSystems),
        ),
    );
    app.update();
    assert!(app.world().resource::<Ran>().0, "the system was skipped");
    frames(&mut app, 3);
    configure(&mut app, |config| {
        assert_eq!(config.sizing, TerminalSizing::font(24.0));
    });
    ready(&mut app);
}

#[test]
fn a_context_inserted_before_setup_is_kept() {
    let kept = || {
        let mut context = RatatuiContext::init().expect("a context");
        context
            .draw(|frame| frame.render_widget("kept", frame.area()))
            .expect("draw");
        context
    };
    let mut inserted = app();
    strict_startup(&mut inserted, true);
    inserted.insert_resource(kept());

    let mut pre_startup_commands = app();
    strict_startup(&mut pre_startup_commands, true);
    let mut context = Some(kept());
    pre_startup_commands.add_systems(
        PreStartup,
        (move |mut commands: Commands| {
            if let Some(context) = context.take() {
                commands.insert_resource(context);
            }
        })
        .before(ContextSystems),
    );

    let mut pre_startup_world = app();
    strict_startup(&mut pre_startup_world, false);
    let mut context = Some(kept());
    pre_startup_world.add_systems(
        PreStartup,
        (move |world: &mut World| {
            if let Some(context) = context.take() {
                world.insert_resource(context);
            }
        })
        .before(ContextSystems),
    );

    for mut app in [inserted, pre_startup_commands, pre_startup_world] {
        // Kept and carrying its renderer components in plain `Startup`.
        app.init_resource::<Ran>().add_systems(
            Startup,
            |context: Res<RatatuiContext>,
             _: Single<&TerminalRenderConfig, With<RatatuiContext>>,
             _: Single<&TerminalTexture, With<RatatuiContext>>,
             mut ran: ResMut<Ran>| {
                ran.0 = context.backend().grid().row_text(0).trim_end() == "kept";
            },
        );
        frames(&mut app, 3);
        assert!(app.world().resource::<Ran>().0, "not kept in Startup");
        ready(&mut app);
        assert_eq!(first_row(&app).trim_end(), "kept");
    }
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
fn a_window_dragged_narrower_and_back_keeps_its_frame() {
    let mut app = app();
    frames(&mut app, 4);
    let line = "0123456789".repeat(20);
    let whole = draw(&mut app, &line);
    assert!(!whole.ends_with(' '), "the line fills the row: {whole:?}");
    // Two resizes with no draw between them, as an application that draws on change sees.
    let wide = ready(&mut app).grid().width;
    for width in [200, 640] {
        window(&mut app, |window| {
            window.resolution.set_physical_resolution(width, 400);
        });
        frames(&mut app, 1);
        let narrowed = ready(&mut app).grid().width < wide;
        assert_eq!(narrowed, width == 200, "the grid follows each resize");
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

    // A bare `Camera` without `Camera2d` or `Camera3d` is not a UI camera, so `setup` spawns a
    // `Camera2d` and binds the view to it.
    let mut bare = self::app();
    let bare_cam = bare
        .world_mut()
        .spawn((Camera::default(), RenderTarget::default()))
        .id();
    frames(&mut bare, 2);
    assert_eq!(cameras(&mut bare).len(), 2);
    assert!(view(&mut bare).2.is_some_and(|camera| camera != bare_cam));
}

#[test]
fn a_default_ui_camera_on_another_window_does_not_take_the_terminal() {
    let mut app = app();
    let other = app.world_mut().spawn(Window::default()).id();
    app.world_mut().spawn((
        Camera2d,
        IsDefaultUiCamera,
        RenderTarget::Window(WindowRef::Entity(other)),
    ));
    let primary = app.world_mut().spawn(Camera2d).id();
    frames(&mut app, 2);
    assert_eq!(cameras(&mut app).len(), 2, "none is spawned");
    assert_eq!(view(&mut app).2, Some(primary));
}

#[test]
fn an_inactive_camera_does_not_hide_an_active_one_below_it() {
    let mut app = app();
    let inactive = Camera {
        order: 1,
        is_active: false,
        ..default()
    };
    app.world_mut().spawn((Camera2d, inactive));
    let active = app.world_mut().spawn(Camera2d).id();
    frames(&mut app, 2);
    assert_eq!(cameras(&mut app).len(), 2, "none is spawned");
    assert_eq!(view(&mut app).2, Some(active));
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

    // One replaced within a frame by a context that cannot be drawn is not left on screen.
    app.world_mut().remove_resource::<RatatuiContext>();
    let context = RatatuiContext::init().expect("a context");
    app.world_mut().insert_resource(context);
    configure(&mut app, |config| {
        config.sizing = TerminalSizing::font(f32::NAN)
    });
    frames(&mut app, 2);
    assert!(matches!(status(&mut app), TerminalStatus::Failed(_)));
    assert_eq!(view(&mut app).1, ImageNode::default().image);
}

/// Syncs each camera's `computed.target_info` from its window, standing in for Bevy's render
/// `camera_system` in headless tests.
fn sync_camera_targets(
    primary: Query<&Window, With<PrimaryWindow>>,
    windows: Query<&Window>,
    mut cameras: Query<(&mut Camera, &RenderTarget)>,
) {
    let primary_window = primary.single().ok();
    for (mut camera, target) in &mut cameras {
        camera.computed.target_info = match target {
            RenderTarget::Window(WindowRef::Primary) => primary_window,
            RenderTarget::Window(WindowRef::Entity(entity)) => windows.get(*entity).ok(),
            _ => None,
        }
        .map(|window| RenderTargetInfo {
            physical_size: window.resolution.physical_size(),
            scale_factor: window.scale_factor(),
        });
    }
}

/// An app with a 640×400 primary window and real Bevy UI layout, visibility propagation, and
/// input systems running headless.
fn ui_app() -> App {
    let mut app = app();
    app.add_plugins((InputPlugin, MeshPlugin, VisibilityPlugin, UiPlugin))
        .init_asset::<TextureAtlasLayout>()
        .add_systems(PostUpdate, sync_camera_targets.in_set(CameraUpdateSystems));
    app
}

/// The laid-out physical rectangle of `TerminalView` in camera viewport coordinates.
/// Assumes an axis-aligned transform (translation and positive scale only).
fn view_rect(app: &mut App) -> Rect {
    let (computed, transform) = app
        .world_mut()
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<ImageNode>>()
        .single(app.world())
        .expect("one laid-out view");
    let content = computed.content_box();
    Rect {
        min: transform.transform_point2(content.min),
        max: transform.transform_point2(content.max),
    }
}

/// Puts the mouse at a physical pixel of the window (or outside it) and returns the cell there.
fn cell_at(app: &mut App, pixel: Option<(f64, f64)>) -> Option<(u16, u16)> {
    window(app, |window| {
        window.set_physical_cursor_position(pixel.map(Into::into));
    });
    frames(app, 1);
    let cell = app.world().resource::<WindowedCursor>().0;
    cell.map(|cell| (cell.x, cell.y))
}

#[test]
fn the_cursor_is_mapped_to_the_cell_under_it() {
    let mut app = ui_app();
    // 645×410 leaves a gutter around the centered terminal, of half a cell at most.
    window(&mut app, |window| {
        window.resolution.set_physical_resolution(645, 410);
    });
    frames(&mut app, 4);
    let geometry = ready(&mut app);
    let (cell, grid) = (geometry.physical_cell_size(), geometry.grid());
    let rect = view_rect(&mut app);
    assert!(
        rect.min.min_element() >= 1.0,
        "a gutter on every side: {:?}",
        rect.min
    );
    // A pixel inside the cell at `column`, `row`.
    let inside = |column: u16, row: u16| {
        let pixel = rect.min + (Vec2::new(column.into(), row.into()) + 0.5) * cell;
        Some((f64::from(pixel.x), f64::from(pixel.y)))
    };

    assert_eq!(cell_at(&mut app, None), None, "outside the window");
    assert_eq!(cell_at(&mut app, Some((0.0, 0.0))), None, "in the gutter");
    assert_eq!(
        cell_at(&mut app, Some((644.0, 409.0))),
        None,
        "in the gutter"
    );
    assert_eq!(cell_at(&mut app, Some((644.0, 205.0))), None, "right of it");
    assert_eq!(cell_at(&mut app, Some((322.0, 409.0))), None, "below it");
    let last = (grid.width - 1, grid.height - 1);
    for at in [(0, 0), (3, 2), last] {
        assert_eq!(cell_at(&mut app, inside(at.0, at.1)), Some(at));
    }

    // At another scale the same cell is elsewhere, and still found.
    window(&mut app, |window| {
        window.resolution.set_scale_factor_override(Some(1.5));
    });
    frames(&mut app, 4);
    let scaled = ready(&mut app);
    assert_ne!(scaled.physical_cell_size(), cell);
    let scaled_rect = view_rect(&mut app);
    let cell_size =
        scaled_rect.size() / Vec2::new(scaled.grid().width.into(), scaled.grid().height.into());
    let pixel = scaled_rect.min + Vec2::new(3.5, 2.5) * cell_size;
    let pixel = Some((f64::from(pixel.x), f64::from(pixel.y)));
    assert_eq!(cell_at(&mut app, pixel), Some((3, 2)));

    // A terminal that cannot be drawn has no cells to point at.
    configure(&mut app, |config| {
        config.sizing = TerminalSizing::font(f32::NAN)
    });
    frames(&mut app, 2);
    assert_eq!(cell_at(&mut app, pixel), None);
}

#[test]
fn cursor_follows_laid_out_view_under_rounding_scaling_and_viewport() {
    let mut app = ui_app();
    // Fixed 16×16 cells in a 645×411 window guarantee an odd physical gutter on both axes at
    // every scale factor below, so unrounded centering lands at `.5` while Taffy rounds edges.
    let size = UVec2::new(645, 411);
    window(&mut app, |window| {
        window.resolution.set_physical_resolution(size.x, size.y);
    });
    frames(&mut app, 2);
    configure(&mut app, |config| {
        config.sizing = TerminalSizing::Fixed {
            font_size: 16.0,
            cell_size: Vec2::splat(16.0),
        };
    });

    for (dpi, ui_scale, viewport_offset) in [
        (1.0, 1.0, UVec2::ZERO),
        (2.0, 1.0, UVec2::ZERO),
        (1.5, 1.0, UVec2::ZERO),
        (1.0, 2.0, UVec2::ZERO),
        (1.0, 1.0, UVec2::new(4, 7)),
    ] {
        window(&mut app, |window| {
            window.resolution.set_scale_factor_override(Some(dpi));
        });
        app.world_mut().resource_mut::<UiScale>().0 = ui_scale;
        let viewport_size = size - viewport_offset;
        {
            let mut camera = app
                .world_mut()
                .query::<&mut Camera>()
                .single_mut(app.world_mut())
                .expect("one camera");
            camera.viewport = (viewport_offset != UVec2::ZERO).then_some(Viewport {
                physical_position: viewport_offset,
                physical_size: viewport_size,
                ..default()
            });
        }
        frames(&mut app, 4);

        let geometry = ready(&mut app);
        let grid = geometry.grid();
        let rect = view_rect(&mut app);
        let min = viewport_offset.as_vec2() + rect.min;
        let max = viewport_offset.as_vec2() + rect.max;

        // Verify odd-gutter rounding on the un-offset cases: unrounded origin ends in `.5` on
        // both axes, while the laid-out rect snaps to whole pixels.
        if viewport_offset == UVec2::ZERO {
            let unrounded = (size.as_vec2() - geometry.size().as_vec2()) / 2.0;
            assert_eq!(unrounded.fract(), Vec2::splat(0.5));
            assert_eq!(rect.min.fract(), Vec2::ZERO);
            // Probe midway between the unrounded origin and the rounded origin: old unrounded
            // centering would map this inside cell (0, 0) or outside depending on rounding side.
            let outside_x = if rect.min.x > unrounded.x {
                f64::from(min.x) - 0.25
            } else {
                f64::from(max.x) + 0.25
            };
            let outside_y = if rect.min.y > unrounded.y {
                f64::from(min.y) - 0.25
            } else {
                f64::from(max.y) + 0.25
            };
            let mid = (min + max) / 2.0;
            assert_eq!(cell_at(&mut app, Some((outside_x, f64::from(mid.y)))), None);
            assert_eq!(cell_at(&mut app, Some((f64::from(mid.x), outside_y))), None);
        }

        let last = (grid.width - 1, grid.height - 1);
        let mid = (min + max) / 2.0;
        for (pixel, expected) in [
            ((f64::from(min.x) - 0.01, f64::from(mid.y)), None),
            ((f64::from(min.x), f64::from(min.y)), Some((0, 0))),
            (
                (f64::from(max.x.next_down()), f64::from(max.y.next_down())),
                Some(last),
            ),
            ((f64::from(max.x), f64::from(mid.y)), None),
            ((f64::from(mid.x), f64::from(max.y)), None),
        ] {
            assert_eq!(
                cell_at(&mut app, Some(pixel)),
                expected,
                "pixel={pixel:?} dpi={dpi} ui_scale={ui_scale} viewport={viewport_offset}"
            );
        }
    }
}

#[test]
fn cursor_maps_exact_seams_and_ui_transforms() {
    // 1. Exact internal cell seam: multiply before dividing keeps `341 * 58 / 638 == 31` exact
    //    (whereas `341.0 / 638.0 * 58.0` rounds to `30.999999999999996`).
    {
        let mut app = ui_app();
        window(&mut app, |window| {
            window.resolution.set_physical_resolution(638, 400);
        });
        frames(&mut app, 2);
        configure(&mut app, |config| {
            config.sizing = TerminalSizing::Fixed {
                font_size: 16.0,
                cell_size: Vec2::new(11.0, 20.0),
            };
        });
        frames(&mut app, 4);
        let seam_geometry = ready(&mut app);
        assert_eq!(seam_geometry.grid().width, 58);
        assert_eq!(seam_geometry.size().x, 638);
        let seam_rect = view_rect(&mut app);
        assert_eq!(
            cell_at(
                &mut app,
                Some((
                    f64::from(seam_rect.min.x) + 341.0,
                    f64::from(seam_rect.center().y),
                ))
            )
            .map(|(col, _)| col),
            Some(31),
            "exact internal seam at column 31"
        );
    }

    // 2. Scaled exact internal cell seam: 560px content box (70 cols of 8×20) centered at x = 280
    //    with `scale.x = 91 / 64`. Column 11 starts at local x = -192, which transforms to
    //    viewport x = 7 exactly.
    {
        let mut app = ui_app();
        window(&mut app, |window| {
            window.resolution.set_physical_resolution(560, 400);
        });
        frames(&mut app, 2);
        configure(&mut app, |config| {
            config.sizing = TerminalSizing::Fixed {
                font_size: 16.0,
                cell_size: Vec2::new(8.0, 20.0),
            };
        });
        app.world_mut()
            .query_filtered::<&mut UiTransform, With<ImageNode>>()
            .single_mut(app.world_mut())
            .expect("view transform")
            .scale = Vec2::new(91.0 / 64.0, 1.0);
        frames(&mut app, 4);
        let scaled_seam = ready(&mut app);
        assert_eq!(scaled_seam.grid().width, 70);
        assert_eq!(scaled_seam.size().x, 560);
        assert_eq!(
            cell_at(&mut app, Some((7.0, 200.0))).map(|(col, _)| col),
            Some(11),
            "exact scaled internal seam at column 11"
        );
    }

    // 3. Non-representable local seam on an integer screen pixel: 60 columns in a 400 px content
    //    box centered at x = 300 with `scale.x = 1.5` spans 600 px so column 1 starts at x = 10.
    //    Dividing by `det` before subtracting `min` would drift to `0.9999999999999986` (col 0).
    {
        let mut app = ui_app();
        window(&mut app, |window| {
            window.resolution.set_physical_resolution(600, 400);
        });
        frames(&mut app, 2);
        configure(&mut app, |config| {
            config.sizing = TerminalSizing::Fixed {
                font_size: 16.0,
                cell_size: Vec2::new(10.0, 20.0),
            };
        });
        {
            let (mut node, mut transform) = app
                .world_mut()
                .query_filtered::<(&mut Node, &mut UiTransform), With<ImageNode>>()
                .single_mut(app.world_mut())
                .expect("view node and transform");
            node.max_width = px(400.0);
            transform.scale = Vec2::new(1.5, 1.0);
        }
        frames(&mut app, 4);
        assert_eq!(ready(&mut app).grid().width, 60);
        assert_eq!(
            cell_at(&mut app, Some((f64::from(10.0_f32.next_down()), 200.0))).map(|(col, _)| col),
            Some(0),
            "just left of scaled column 1 seam"
        );
        assert_eq!(
            cell_at(&mut app, Some((10.0, 200.0))).map(|(col, _)| col),
            Some(1),
            "exact scaled column 1 seam at x = 10"
        );
    }

    // 4. Translation shifts the hit-test target with layout.
    {
        let mut app = ui_app();
        frames(&mut app, 4);
        let before_shift = view_rect(&mut app);
        app.world_mut()
            .query_filtered::<&mut UiTransform, With<ImageNode>>()
            .single_mut(app.world_mut())
            .expect("view transform")
            .translation = Val2::px(20.0, 10.0);
        frames(&mut app, 2);
        let shifted = view_rect(&mut app);
        assert_eq!(shifted.min, before_shift.min + Vec2::new(20.0, 10.0));
        assert_eq!(
            cell_at(
                &mut app,
                Some((
                    f64::from(shifted.min.x) + 1.0,
                    f64::from(shifted.min.y) + 1.0
                ))
            ),
            Some((0, 0))
        );
        assert_eq!(
            cell_at(
                &mut app,
                Some((
                    f64::from(before_shift.min.x) + 1.0,
                    f64::from(before_shift.min.y) + 1.0
                ))
            ),
            None
        );
    }

    // 5. Reflection (`det < 0`, e.g. `scale.x = -1.0`) mirrors columns across the view.
    {
        let mut app = ui_app();
        frames(&mut app, 4);
        let grid = ready(&mut app).grid();
        let rect = view_rect(&mut app);
        app.world_mut()
            .query_filtered::<&mut UiTransform, With<ImageNode>>()
            .single_mut(app.world_mut())
            .expect("view transform")
            .scale = Vec2::new(-1.0, 1.0);
        frames(&mut app, 2);
        let mid_y = f64::from(rect.center().y);
        assert_eq!(
            cell_at(&mut app, Some((f64::from(rect.max.x) - 0.5, mid_y))).map(|(col, _)| col),
            Some(0),
            "reflected right edge maps to column 0"
        );
        assert_eq!(
            cell_at(&mut app, Some((f64::from(rect.min.x) + 0.5, mid_y))).map(|(col, _)| col),
            Some(grid.width - 1),
            "reflected left edge maps to last column"
        );
    }
}

#[test]
fn cursor_respects_camera_viewport_and_clipping_ancestor() {
    let mut app = ui_app();
    window(&mut app, |window| {
        window.resolution.set_physical_resolution(800, 600);
    });
    frames(&mut app, 2);
    configure(&mut app, |config| {
        config.sizing = TerminalSizing::Fixed {
            font_size: 16.0,
            cell_size: Vec2::new(11.0, 20.0),
        };
    });
    let vp_pos = UVec2::new(50, 40);
    let vp_size = UVec2::new(400, 300);
    app.world_mut()
        .query::<&mut Camera>()
        .single_mut(app.world_mut())
        .expect("one camera")
        .viewport = Some(Viewport {
        physical_position: vp_pos,
        physical_size: vp_size,
        ..default()
    });
    frames(&mut app, 4);
    let vp_center = vp_pos.as_vec2() + vp_size.as_vec2() / 2.0;
    assert!(
        cell_at(
            &mut app,
            Some((f64::from(vp_center.x), f64::from(vp_center.y)))
        )
        .is_some()
    );
    assert_eq!(
        cell_at(
            &mut app,
            Some((f64::from(vp_pos.x) - 1.0, f64::from(vp_center.y)))
        ),
        None,
        "left of viewport"
    );
    assert_eq!(
        cell_at(
            &mut app,
            Some((f64::from(vp_pos.x + vp_size.x), f64::from(vp_center.y)))
        ),
        None,
        "at viewport right edge"
    );

    // A clipping ancestor (`Overflow::clip()`) populates `CalculatedClip` via Bevy's UI layout:
    // keep the child unshrunk (`flex_shrink = 0.0`) so it genuinely overflows the 200px ancestor,
    // then verify the *same* pixel hits under `Overflow::visible()` and clears under `Overflow::clip()`.
    let view_entity = app
        .world_mut()
        .query_filtered::<Entity, With<ImageNode>>()
        .single(app.world())
        .expect("one view");
    app.world_mut()
        .entity_mut(view_entity)
        .get_mut::<Node>()
        .expect("view node")
        .flex_shrink = 0.0;
    let clipper = app
        .world_mut()
        .spawn(Node {
            width: px(200.0),
            height: px(vp_size.y as f32),
            overflow: Overflow::visible(),
            ..default()
        })
        .id();
    app.world_mut().entity_mut(clipper).add_child(view_entity);
    frames(&mut app, 2);
    let unclipped_rect = view_rect(&mut app);
    assert!(
        unclipped_rect.max.x > 250.0,
        "child view must overflow 200px ancestor: {unclipped_rect:?}"
    );
    let inside_clip = vp_pos.as_vec2() + Vec2::new(100.0, 100.0);
    let probe_past_200 = vp_pos.as_vec2() + Vec2::new(250.0, 100.0);
    assert!(
        cell_at(
            &mut app,
            Some((f64::from(probe_past_200.x), f64::from(probe_past_200.y)))
        )
        .is_some(),
        "same pixel hits when ancestor uses Overflow::visible()"
    );
    app.world_mut()
        .entity_mut(clipper)
        .get_mut::<Node>()
        .expect("clipper node")
        .overflow = Overflow::clip();
    frames(&mut app, 2);
    assert!(
        cell_at(
            &mut app,
            Some((f64::from(inside_clip.x), f64::from(inside_clip.y)))
        )
        .is_some(),
        "inside clip"
    );
    assert_eq!(
        cell_at(
            &mut app,
            Some((f64::from(probe_past_200.x), f64::from(probe_past_200.y)))
        ),
        None,
        "same pixel cleared once ancestor uses Overflow::clip()"
    );
}

/// Returns a `ui_app` with `WindowedCursor` hovering a valid cell.
fn hovered_ui_app() -> App {
    let mut app = ui_app();
    frames(&mut app, 4);
    ready(&mut app);
    let center = view_rect(&mut app).center();
    assert!(cell_at(&mut app, Some((f64::from(center.x), f64::from(center.y)))).is_some());
    app
}

#[test]
fn stale_cursor_is_cleared_and_change_detection_stays_quiet() {
    // 1. Without UI layout (`UiPlugin` absent), a ready texture produces `None`.
    {
        let mut lean = app();
        frames(&mut lean, 4);
        let center = ready(&mut lean).size().as_vec2() / 2.0;
        assert_eq!(
            cell_at(&mut lean, Some((f64::from(center.x), f64::from(center.y)))),
            None
        );
    }

    // 2. Stable `Some` does not trigger change detection on subsequent frames.
    {
        let mut app = hovered_ui_app();
        let last_changed = app.world().resource_ref::<WindowedCursor>().last_changed();
        frames(&mut app, 2);
        assert_eq!(
            app.world().resource_ref::<WindowedCursor>().last_changed(),
            last_changed,
            "unchanged Some cursor should not trigger change detection"
        );
    }

    // 3. Despawning the primary window clears the cursor and keeps change detection quiet
    //    (keep a non-primary window alive to verify it is not adopted as primary).
    {
        let mut app = hovered_ui_app();
        app.world_mut().spawn(Window::default());
        let primary = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .expect("primary window");
        app.world_mut().despawn(primary);
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
        let last_changed = app.world().resource_ref::<WindowedCursor>().last_changed();
        frames(&mut app, 2);
        assert_eq!(
            app.world().resource_ref::<WindowedCursor>().last_changed(),
            last_changed
        );
    }

    // 4. Two `PrimaryWindow`s clear the cursor quietly, and removing the duplicate recovers it.
    {
        let mut app = hovered_ui_app();
        let other = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
        let last_changed = app.world().resource_ref::<WindowedCursor>().last_changed();
        frames(&mut app, 2);
        assert_eq!(
            app.world().resource_ref::<WindowedCursor>().last_changed(),
            last_changed
        );
        app.world_mut().entity_mut(other).remove::<PrimaryWindow>();
        // Frame 1's `PostUpdate` restores camera target info; frame 2's `PreUpdate` recovers it.
        frames(&mut app, 2);
        assert!(app.world().resource::<WindowedCursor>().0.is_some());
    }

    // 5. Hiding `TerminalView` clears the cursor and restoring visibility recovers it.
    {
        let mut app = hovered_ui_app();
        let view_entity = app
            .world_mut()
            .query_filtered::<Entity, With<ImageNode>>()
            .single(app.world())
            .expect("one view");
        app.world_mut()
            .entity_mut(view_entity)
            .insert(Visibility::Hidden);
        frames(&mut app, 2);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
        app.world_mut()
            .entity_mut(view_entity)
            .insert(Visibility::Visible);
        frames(&mut app, 2);
        assert!(app.world().resource::<WindowedCursor>().0.is_some());
    }

    // 6. Removing `RatatuiContext` clears the cursor.
    {
        let mut app = hovered_ui_app();
        app.world_mut().remove_resource::<RatatuiContext>();
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
    }

    // 7. Mismatched `ImageNode` handle clears the cursor.
    {
        let mut app = hovered_ui_app();
        let other_image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.world_mut()
            .query::<&mut ImageNode>()
            .single_mut(app.world_mut())
            .expect("one view")
            .image = other_image;
        // Inspect during `Update` right after `track_cursor` runs, before `PostUpdate`'s
        // `present` restores the texture handle.
        #[derive(Resource, Default)]
        struct SeenInUpdate(WindowedCursor);
        app.init_resource::<SeenInUpdate>().add_systems(
            Update,
            |cursor: Res<WindowedCursor>, mut seen: ResMut<SeenInUpdate>| {
                seen.0 = *cursor;
            },
        );
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<SeenInUpdate>().0.0, None);
    }

    // 8. Fully transparent image tint clears the cursor (Bevy skips drawing transparent images).
    {
        let mut app = hovered_ui_app();
        app.world_mut()
            .query::<&mut ImageNode>()
            .single_mut(app.world_mut())
            .expect("one view")
            .color = Color::NONE;
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
    }

    // 9. Inactive camera, non-2D/3D bare camera, or camera targeting a non-primary window clears
    //    the cursor.
    {
        let mut app = hovered_ui_app();
        app.world_mut()
            .query::<&mut Camera>()
            .single_mut(app.world_mut())
            .expect("one camera")
            .is_active = false;
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
    }
    {
        let mut app = hovered_ui_app();
        let cam_entity = app
            .world_mut()
            .query_filtered::<Entity, With<Camera>>()
            .single(app.world())
            .expect("one camera");
        app.world_mut().entity_mut(cam_entity).remove::<Camera2d>();
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
    }
    {
        let mut app = hovered_ui_app();
        let other = app.world_mut().spawn(Window::default()).id();
        *app.world_mut()
            .query_filtered::<&mut RenderTarget, With<Camera>>()
            .single_mut(app.world_mut())
            .expect("one camera") = RenderTarget::Window(WindowRef::Entity(other));
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
    }

    // 10. A custom viewport whose `physical_position + physical_size` would overflow `u32` does
    //     not panic (prior layout is still in place on the immediate `PreUpdate` frame).
    {
        let mut app = hovered_ui_app();
        app.world_mut()
            .query::<&mut Camera>()
            .single_mut(app.world_mut())
            .expect("one camera")
            .viewport = Some(Viewport {
            physical_position: UVec2::new(1, 0),
            physical_size: UVec2::new(u32::MAX, 400),
            ..default()
        });
        // Disable `UiSystems::Focus` because Bevy's own focus system calls
        // `Camera::physical_viewport_rect()`, which overflows `u32`.
        app.configure_sets(PreUpdate, UiSystems::Focus.run_if(|| false));
        frames(&mut app, 1);
        assert!(app.world().resource::<WindowedCursor>().0.is_some());
    }

    // 11. A singular `UiGlobalTransform` (determinant 0) clears to `None` without panicking.
    //     `UiSystems::Focus` is disabled so Bevy's eager `UiGlobalTransform::try_inverse` does
    //     not panic first when `bevy/glam_assert` is enabled.
    {
        let mut app = hovered_ui_app();
        app.configure_sets(PreUpdate, UiSystems::Focus.run_if(|| false));
        app.world_mut()
            .query_filtered::<&mut UiTransform, With<ImageNode>>()
            .single_mut(app.world_mut())
            .expect("view transform")
            .scale = Vec2::ZERO;
        frames(&mut app, 2);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
    }

    // 12. A `NaN` `ComputedNode::size` clears to `None` without panicking in `content_box()`.
    {
        let mut app = hovered_ui_app();
        app.configure_sets(PreUpdate, UiSystems::Focus.run_if(|| false));
        app.world_mut()
            .query_filtered::<&mut ComputedNode, With<ImageNode>>()
            .single_mut(app.world_mut())
            .expect("computed node")
            .size = Vec2::NAN;
        frames(&mut app, 1);
        assert_eq!(app.world().resource::<WindowedCursor>().0, None);
    }
}

#[test]
fn windowed_systems_orders_after_input_without_ambiguities() {
    #[derive(Resource, Default)]
    struct Observed {
        cursor: WindowedCursor,
        left_just_pressed: bool,
    }

    let mut app = ui_app();
    app.edit_schedule(PreUpdate, |schedule| {
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..default()
        });
    });
    // Include a `&mut Window` system after `UiSystems::Focus` (matching `bevy_ui_widgets`'s
    // IME toggle system in `DefaultPlugins`) to verify `WindowedSystems.before(UiSystems::Focus)`
    // stays ambiguity-free.
    app.init_resource::<Observed>().add_systems(
        PreUpdate,
        (
            (|_windows: Query<&mut Window, With<PrimaryWindow>>| {}).after(UiSystems::Focus),
            (|cursor: Res<WindowedCursor>,
              buttons: Res<ButtonInput<MouseButton>>,
              mut observed: ResMut<Observed>| {
                observed.cursor = *cursor;
                observed.left_just_pressed = buttons.just_pressed(MouseButton::Left);
            })
            .after(WindowedSystems),
        ),
    );

    frames(&mut app, 4);
    let grid = ready(&mut app).grid();
    let rect = view_rect(&mut app);
    assert_eq!(
        cell_at(
            &mut app,
            Some((f64::from(rect.min.x), f64::from(rect.min.y)))
        ),
        Some((0, 0))
    );

    // Move cursor to the last cell AND send a left-button press in the same frame; a `PreUpdate`
    // consumer ordered `.after(WindowedSystems)` must see both updates immediately.
    let primary = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("primary window");
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window: primary,
    });
    let last = (grid.width - 1, grid.height - 1);
    assert_eq!(
        cell_at(
            &mut app,
            Some((f64::from(rect.max.x) - 0.5, f64::from(rect.max.y) - 0.5))
        ),
        Some(last)
    );

    let observed = app.world().resource::<Observed>();
    assert_eq!(observed.cursor.0.map(|pos| (pos.x, pos.y)), Some(last));
    assert!(observed.left_just_pressed);
}
