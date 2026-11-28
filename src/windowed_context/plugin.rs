use bevy::{
    camera::RenderTarget,
    ecs::query::QueryData,
    input::InputSystems,
    math::DVec2,
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
/// Camera selection runs once on the first `PreUpdate`: if Bevy UI's default camera is already
/// an active [`Camera2d`] or [`Camera3d`] on the primary window, the view is left to it (so later
/// changes to [`IsDefaultUiCamera`] can still redirect the view); otherwise the view is bound
/// once via [`UiTargetCamera`] to another active primary-window 2D/3D camera or to a [`Camera2d`]
/// spawned for it.
///
/// `bevy_terminal` reads the grid in `PostUpdate` ([`TerminalSystems::Sync`]), so draw in
/// `Update` (or in `PostUpdate` ordered `.before(TerminalSystems::Sync)`). Drawing in `Last`
/// shows one frame late.
///
/// For mouse input, [`WindowedCursor`] is the cell under the cursor.
pub struct WindowedPlugin;

/// System set for [`WindowedPlugin`]'s per-frame `PreUpdate` systems (window scaling, grid
/// fitting, and [`WindowedCursor`] tracking; one-shot view/camera setup runs outside the set).
///
/// Ordered after Bevy's [`InputSystems`](bevy::input::InputSystems) and before
/// [`UiSystems::Focus`](bevy::ui::UiSystems::Focus) in `PreUpdate`, so a `PreUpdate` system
/// ordered `.after(WindowedSystems)` observes both fresh Bevy mouse/keyboard input and this
/// frame's [`WindowedCursor`] (and resized [`RatatuiContext`]).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowedSystems;

/// The terminal cell under the mouse cursor, updated in `PreUpdate` in [`WindowedSystems`]
/// (after Bevy's [`InputSystems`](bevy::input::InputSystems)) from the current pointer position
/// against the last completed UI layout and measured grid.
///
/// It is `None` before UI layout has run, while there is no unique primary window, while the
/// terminal view is hidden, transparent, not drawn, or showing another image, when its camera is
/// missing, inactive, not a 2D/3D UI camera, or not targeting the primary window, or while the
/// cursor is outside the visible terminal. It does not check occlusion by other UI nodes and
/// assumes the plugin-managed view keeps its default image box and orientation.
///
/// Order `PreUpdate` consumers `.after(WindowedSystems)` to observe both fresh Bevy input and
/// this frame's cursor position.
///
/// Windowed mode has no mouse messages of its own. Read this with Bevy's
/// `ButtonInput<MouseButton>` and `MouseWheel` to know which cell a click or a scroll is over.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowedCursor(pub Option<Position>);

impl Plugin for WindowedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RatatuiSourcePlugin::<RatatuiContext>::default())
            .init_resource::<WindowedCursor>()
            .configure_sets(
                PreUpdate,
                WindowedSystems.after(InputSystems).before(UiSystems::Focus),
            )
            // After every startup schedule, so a camera spawned in any of them is seen.
            .add_systems(PreUpdate, setup.run_if(run_once))
            // Before any `Update` system draws, so a resize never clears a frame already drawn.
            .add_systems(
                PreUpdate,
                (follow_window_scale, fit_to_window, track_cursor).in_set(WindowedSystems),
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

/// Bevy UI renders only to `Camera2d` and `Camera3d` cameras.
type UiCameraFilter = Or<(With<Camera2d>, With<Camera3d>)>;

/// Spawns the UI node showing the terminal, and a 2D camera unless the primary window has an
/// active 2D/3D camera.
///
/// Bevy UI draws a node to its default camera: the one marked `IsDefaultUiCamera`, or the
/// highest-order one on the primary window. Where that is an active 2D/3D camera on the primary
/// window the node is left to it. Where it is not (it is inactive, not a `Camera2d`/`Camera3d`,
/// or marked on another window or an image) the node is bound to another active 2D/3D camera on
/// the primary window, or to the one spawned here.
fn setup(
    mut commands: Commands,
    ui_camera: DefaultUiCamera,
    cameras: Query<(Entity, &Camera, &RenderTarget), UiCameraFilter>,
    primary_window: Query<(), With<PrimaryWindow>>,
) {
    let mut view = commands.spawn((
        TerminalView,
        ImageNode {
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
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

#[derive(QueryData)]
struct TerminalViewLayout {
    computed: &'static ComputedNode,
    transform: &'static UiGlobalTransform,
    visibility: &'static InheritedVisibility,
    target_camera: &'static ComputedUiTargetCamera,
    image: &'static ImageNode,
    clip: Option<&'static CalculatedClip>,
}

/// Finds the cell under the cursor in the terminal as it was last laid out by Bevy UI.
///
/// Clears [`WindowedCursor`] whenever there is no unique primary window, no measured terminal
/// texture, or no visible, laid-out view displaying it.
fn track_cursor(
    window: Option<Single<(Entity, &Window), With<PrimaryWindow>>>,
    texture: Option<Single<&TerminalTexture, With<RatatuiContext>>>,
    view: Option<Single<TerminalViewLayout, With<TerminalView>>>,
    cameras: Query<(&Camera, &RenderTarget), UiCameraFilter>,
    mut cursor: ResMut<WindowedCursor>,
) {
    let cell = || {
        let &(window_entity, window) = window.as_deref()?;
        let texture = texture.as_deref()?;
        let grid = texture.measured()?.grid();
        let view = view.as_deref()?;

        // Reject non-finite outer sizes before calling `content_box()`, whose internal
        // `Rect::from_center_size` asserts non-negative dimensions (which fails on NaN).
        if !view.visibility.get()
            || !view.computed.size().is_finite()
            || view.computed.is_empty()
            || view.image.image != *texture.image()
            || view.image.color.is_fully_transparent()
        {
            return None;
        }

        let (camera, target) = cameras.get(view.target_camera.get()?).ok()?;
        let RenderTarget::Window(target) = target else {
            return None;
        };
        if !camera.is_active
            || matches!(target, WindowRef::Entity(entity) if *entity != window_entity)
        {
            return None;
        }

        // Require target info to exist (matching Bevy's UI renderer) and read origin/size
        // separately instead of `physical_viewport_rect()`, whose `min + size` can overflow `u32`.
        let target_size = camera.physical_target_size()?;
        let viewport_size = camera.physical_viewport_size()?;
        if target_size.min_element() == 0 || viewport_size.min_element() == 0 {
            return None;
        }
        let viewport_origin = camera
            .viewport
            .as_ref()
            .map_or(UVec2::ZERO, |v| v.physical_position);
        let viewport_pos = window.physical_cursor_position()? - viewport_origin.as_vec2();
        if !viewport_pos.cmpge(Vec2::ZERO).all()
            || !viewport_pos.cmplt(viewport_size.as_vec2()).all()
            || view
                .clip
                .is_some_and(|clip| !clip.contains_point(viewport_pos))
        {
            return None;
        }

        // Combine the 2×2 `UiGlobalTransform` inverse and grid mapping into a single `f64`
        // division: avoids Bevy 0.20's eager `UiGlobalTransform::try_inverse` (which can panic
        // under `glam_assert` and accepts non-finite determinants) and prevents intermediate
        // rounding drift on scaled cell boundaries (e.g. `scale.x = 1.5`, 60 cols in 400 px).
        let m = view.transform.matrix2;
        let (a, b, c, d) = (
            f64::from(m.x_axis.x),
            f64::from(m.y_axis.x),
            f64::from(m.x_axis.y),
            f64::from(m.y_axis.y),
        );
        let det = a * d - b * c;
        if !det.is_finite() || det == 0.0 {
            return None;
        }
        let delta = viewport_pos.as_dvec2() - view.transform.translation.as_dvec2();
        let adj = DVec2::new(d * delta.x - b * delta.y, a * delta.y - c * delta.x);

        let content = view.computed.content_box();
        let min = content.min.as_dvec2();
        let max = content.max.as_dvec2();
        let extent = max - min;
        if !extent.is_finite() || !extent.cmpgt(DVec2::ZERO).all() {
            return None;
        }
        let offset = (adj - min * det) * det.signum();
        let span = extent * det.abs();
        if !offset.is_finite()
            || !span.is_finite()
            || !span.cmpgt(DVec2::ZERO).all()
            || !offset.cmpge(DVec2::ZERO).all()
            || !offset.cmplt(span).all()
        {
            return None;
        }

        // Multiply before dividing so exact internal cell boundaries stay exact, and clamp
        // sub-ULP rounding at the far edge (`offset < span`) to the last cell.
        if grid.width == 0 || grid.height == 0 {
            return None;
        }
        let dimensions = DVec2::new(f64::from(grid.width), f64::from(grid.height));
        let cell = ((offset * dimensions) / span).min(dimensions - DVec2::ONE);
        (cell.cmpge(DVec2::ZERO).all() && cell.cmplt(dimensions).all())
            .then(|| Position::new(cell.x as u16, cell.y as u16))
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
