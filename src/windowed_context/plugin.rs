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
///
/// The plugin owns two things an application would otherwise set: the grid's size, which follows
/// the window, and `TerminalRenderConfig::raster.scale`, which follows the window's scale factor
/// and [`UiScale`] and is overwritten every frame.
///
/// Add the plugin before the [`RatatuiContext`] resource is inserted, as
/// [`RatatuiPlugins`](crate::RatatuiPlugins) does: a context that exists first is never drawn.
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

/// Spawns the UI node showing the terminal, and a 2D camera unless the application spawned an
/// active one that Bevy UI draws to by default: one on the primary window, or marked
/// `IsDefaultUiCamera`. A camera on another window or an image, or an inactive one, would show
/// nothing; the node is then shown by the camera spawned here, whichever Bevy UI would pick.
fn setup(mut commands: Commands, ui_camera: DefaultUiCamera, cameras: Query<&Camera>) {
    let mut view = commands.spawn((
        TerminalView,
        ImageNode::default(),
        Node {
            justify_self: JustifySelf::Center,
            align_self: AlignSelf::Center,
            ..default()
        },
    ));
    let usable = ui_camera.get().and_then(|camera| cameras.get(camera).ok());
    if !usable.is_some_and(|camera| camera.is_active) {
        let camera = view.commands().spawn(Camera2d).id();
        view.insert(UiTargetCamera(camera));
    }
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
            // Not drawn yet, or failed: keep what is shown.
            None => return,
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
