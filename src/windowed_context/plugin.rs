use bevy::{
    camera::RenderTarget,
    prelude::*,
    ui::{DefaultUiCamera, UiSystems},
    window::{PrimaryWindow, WindowRef},
};
use bevy_terminal_ratatui::prelude::*;
use ratatui::layout::Position;

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
/// supply embedded or dynamically loaded font assets on `TerminalRenderConfig` in `Startup` (or
/// any time after), and swap them in any `Update` system while running.
///
/// The plugin owns two things an application would otherwise set: the grid's size, which follows
/// the window, and `TerminalRenderConfig::raster.scale`, which follows the window's scale factor
/// and [`UiScale`] and is overwritten every frame.
///
/// Add the plugin before the [`RatatuiContext`] resource is inserted, as
/// [`RatatuiPlugins`](crate::RatatuiPlugins) does: a context that exists first is never drawn.
/// To supply a pre-created context, insert it after adding the plugins — with
/// `App::insert_resource`, or in `PreStartup` ordered before
/// [`ContextSystems`](crate::ContextSystems) (see [`ContextSystems`](crate::ContextSystems) for
/// deferred command flushing requirements).
///
/// For mouse input, [`WindowedCursor`] is the cell under the cursor.
pub struct WindowedPlugin;

/// The terminal cell under the mouse cursor, updated in `PreUpdate`: `None` while the cursor is
/// outside the primary window or the terminal shown in it, or the terminal is not drawn.
///
/// Windowed mode has no mouse messages of its own. Read this with Bevy's
/// `ButtonInput<MouseButton>` and `MouseWheel` to know which cell a click or a scroll is over.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowedCursor(pub Option<Position>);

impl Plugin for WindowedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RatatuiSourcePlugin::<RatatuiContext>::default())
            .init_resource::<WindowedCursor>()
            // After every startup schedule, so a camera spawned in any of them is seen.
            .add_systems(PreUpdate, setup.run_if(run_once))
            // Before any `Update` system draws, so a resize never clears a frame already drawn.
            .add_systems(
                PreUpdate,
                (follow_window_scale, fit_to_window, track_cursor),
            )
            .add_systems(
                PostUpdate,
                // Before Bevy UI measures and lays out the node.
                present
                    .after(TerminalSystems::Sync)
                    .before(UiSystems::Content),
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
        ratatui::Terminal::backend_mut(self)
    }
}

/// The UI node that displays the terminal texture.
#[derive(Component)]
struct TerminalView;

/// Spawns the UI node showing the terminal, and a 2D camera unless the primary window has an
/// active one.
///
/// Bevy UI draws a node to its default camera: the one marked `IsDefaultUiCamera`, or the
/// highest-order one on the primary window. Where that is an active camera on the primary window
/// the node is left to it. Where it is not (it is inactive, or marked on another window or an
/// image) the node is bound to another active camera on the primary window, or to the one
/// spawned here.
fn setup(
    mut commands: Commands,
    ui_camera: DefaultUiCamera,
    cameras: Query<(Entity, &Camera, &RenderTarget)>,
    primary_window: Query<(), With<PrimaryWindow>>,
) {
    let mut view = commands.spawn((
        TerminalView,
        ImageNode::default(),
        Node {
            justify_self: JustifySelf::Center,
            align_self: AlignSelf::Center,
            ..default()
        },
    ));
    let usable = cameras.iter().filter(|(_, camera, target)| {
        camera.is_active
            && match target {
                RenderTarget::Window(WindowRef::Primary) => true,
                RenderTarget::Window(WindowRef::Entity(window)) => primary_window.contains(*window),
                _ => false,
            }
    });
    let default = ui_camera.get();
    if usable.clone().any(|(camera, ..)| Some(camera) == default) {
        return;
    }
    let camera = match usable.max_by_key(|(camera, settings, _)| (settings.order, *camera)) {
        Some((camera, ..)) => camera,
        None => view.commands().spawn(Camera2d).id(),
    };
    view.insert(UiTargetCamera(camera));
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

/// Finds the cell under the cursor in the terminal as it was last shown: centered in the window,
/// one texture pixel to `shown` physical pixels.
fn track_cursor(
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Option<Res<UiScale>>,
    texture: Option<Single<&TerminalTexture, With<RatatuiContext>>>,
    mut cursor: ResMut<WindowedCursor>,
) {
    let cell = || {
        let geometry = texture.as_ref()?.measured()?;
        let position = window.physical_cursor_position()?;
        // One, unless `bevy_terminal` clamped the scale it rasterizes at.
        let shown = ui_scale_factor(&window, ui_scale.as_deref()) / geometry.raster_scale();
        let terminal = geometry.size().as_vec2() * shown;
        let origin = (window.resolution.physical_size().as_vec2() - terminal) / 2.0;
        let cell = (position - origin) / (geometry.physical_cell_size() * shown);
        let grid = geometry.grid();
        // Converted only once known to be inside the grid, where the casts are exact.
        let inside = cell.cmpge(Vec2::ZERO).all()
            && cell.x < f32::from(grid.width)
            && cell.y < f32::from(grid.height);
        inside.then(|| Position::new(cell.x as u16, cell.y as u16))
    };
    let cell = WindowedCursor(cell());
    // Compared first, so that systems run on its change only when the cell does.
    if *cursor != cell {
        *cursor = cell;
    }
}

/// Shows the rendered texture at its measured size, and nothing once the context is removed
/// (which also lets go of its image).
fn present(
    texture: Option<Single<&TerminalTexture, With<RatatuiContext>>>,
    view: Single<(&mut Node, &mut ImageNode), With<TerminalView>>,
) {
    let (width, height, shown) = match &texture {
        Some(texture) => match texture.measured() {
            Some(geometry) => {
                let size = geometry.logical_size();
                (px(size.x), px(size.y), texture.image().clone())
            }
            // Not drawn yet, or failed: keep what is shown of this context, and nothing of one
            // it replaced.
            None if view.1.image == *texture.image() => return,
            None => (Val::Auto, Val::Auto, ImageNode::default().image),
        },
        None => (Val::Auto, Val::Auto, ImageNode::default().image),
    };
    let (mut node, mut image) = view.into_inner();
    // Compared first, so that an unchanged node is not laid out again.
    if node.width != width {
        node.width = width;
    }
    if node.height != height {
        node.height = height;
    }
    if image.image != shown {
        image.image = shown;
    }
}
