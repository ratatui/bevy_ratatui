//! Several independent draw systems composing one frame, without an extra buffer.
//!
//! Ratatui is double buffered: `context.draw` renders into the current buffer, writes what
//! differs from the previous frame, then swaps to a cleared buffer. A second `draw` in the same
//! frame therefore starts from a blank buffer and erases what the first one drew. The default
//! answer is one draw system that reads everything from the ECS (see `examples/layout.rs`).
//!
//! When systems or plugins that do not know about each other must each draw a region, take
//! `draw` apart instead. Every contributor renders straight into the terminal's current buffer
//! and one system presents it afterwards:
//!
//! - [`begin_frame`] is called by every contributor. The first caller of a frame adopts the
//!   backend's size (`Terminal::autoresize`) and marks the [`FramePass`] started.
//! - The contributors run in the [`DrawSystems`] set, in a fixed order, and render with
//!   `Widget::render` / `StatefulWidget::render` into `context.current_buffer_mut()`.
//! - [`present_frame`] runs after the set and calls `Terminal::apply_buffer_with_cursor`
//!   (Ratatui 0.30.1 or newer), which does the rest of what `draw` does.
//!
//! Four rules keep this correct. Each is marked `Rule N` where the code follows it:
//!
//! 1. Resize lazily: call `autoresize` once, from the first contributor, never from a system
//!    that runs on idle frames. A resize clears the screen, so it must be followed by a redraw.
//! 2. Present only a started pass. Presenting an untouched buffer writes a blank frame.
//! 3. Redraw everything: on a frame that presents, every contributor must render. Whatever is
//!    left unpainted is blank in the new frame.
//! 4. Pass the cursor through the resource. `get_frame().set_cursor_position(..)` is silently
//!    dropped because only `draw` reads the position a `Frame` records. Presenting with `None`
//!    hides the cursor, so it disappears on a frame in which nobody asks for it.
//!
//! The same code runs in a terminal and, with the `windowed` feature, in a window.
//!
//! Keys:
//! - Up & Down: select a note
//! - Left & Right: move the cursor in the editor
//! - C: show or hide the cursor
//! - P: redraw every frame, or only when something changed (watch the frame counter)
//! - Q or Esc: quit

#[cfg(not(feature = "windowed"))]
use std::time::Duration;

#[cfg(not(feature = "windowed"))]
use bevy::app::ScheduleRunnerPlugin;
use bevy::{app::AppExit, prelude::*};
#[cfg(not(feature = "windowed"))]
use bevy_ratatui::event::KeyMessage;
use bevy_ratatui::{RatatuiContext, RatatuiPlugins};
use ratatui::{
    Terminal,
    backend::Backend,
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, List, ListState, Paragraph, StatefulWidget, Widget},
};

fn main() -> Result {
    color_eyre::install()?;

    let mut app = App::new();

    #[cfg(not(feature = "windowed"))]
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
            1. / 60.,
        ))),
        RatatuiPlugins::default(),
    ));

    #[cfg(feature = "windowed")]
    app.add_plugins((DefaultPlugins, RatatuiPlugins::default()));

    app.add_plugins(multi_draw).run();

    Ok(())
}

/// Everything the example adds on top of Bevy's and `bevy_ratatui`'s plugins.
fn multi_draw(app: &mut App) {
    app.init_resource::<Ui>()
        .init_resource::<FramePass>()
        // Skipping the whole set is what makes a frame idle: no contributor runs, the pass
        // never starts, and `present_frame` leaves the screen alone.
        .configure_sets(Update, DrawSystems.run_if(needs_redraw))
        .add_systems(
            Update,
            (
                // Chained, as every contributor writes to the same buffer. Later systems paint
                // over earlier ones wherever their areas overlap.
                (draw_chrome, draw_sidebar, draw_editor)
                    .chain()
                    .in_set(DrawSystems),
                present_frame.after(DrawSystems),
            ),
        );

    #[cfg(not(feature = "windowed"))]
    app.add_systems(Update, terminal_input.before(DrawSystems));

    #[cfg(feature = "windowed")]
    app.add_systems(Update, window_input.before(DrawSystems));
}

/// The systems that contribute to the frame. A plugin adds its own draw system to this set,
/// ordered against the others it overlaps.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DrawSystems;

/// What the contributors of one frame share.
#[derive(Resource, Default)]
struct FramePass {
    /// Whether a contributor has rendered into the current buffer this frame.
    started: bool,
    /// Where to show the cursor in this frame. Left unset, the cursor is hidden.
    cursor: Option<Position>,
}

/// Called by every contributor before it renders. Returns the area to lay out.
fn begin_frame<B: Backend>(
    terminal: &mut Terminal<B>,
    pass: &mut FramePass,
) -> Result<Rect, B::Error> {
    if !pass.started {
        // Rule 1: adopt the backend's size once per frame, and only on a frame that draws.
        // In windowed mode the grid follows the window while the app is idle and keeps showing
        // the last frame; resizing Ratatui's buffers clears it, which only a redraw repairs.
        terminal.autoresize()?;
        pass.started = true;
    }
    Ok(terminal.current_buffer_mut().area)
}

/// Ends the pass: writes the composed buffer to the backend, if anything was drawn into it.
fn present<B: Backend>(terminal: &mut Terminal<B>, pass: &mut FramePass) -> Result<(), B::Error> {
    // Rule 4: `take` the cursor first so neither an idle frame nor an error leaves a stale
    // position behind, and a frame in which no contributor asks for the cursor hides it.
    let cursor = pass.cursor.take();
    // Rule 2: nothing rendered, nothing to present. The current buffer is blank after the last
    // swap, so applying it now would erase the screen.
    if !std::mem::take(&mut pass.started) {
        return Ok(());
    }
    terminal.apply_buffer_with_cursor(cursor)?;
    Ok(())
}

fn present_frame(mut context: ResMut<RatatuiContext>, mut pass: ResMut<FramePass>) -> Result {
    present(&mut context, &mut pass)?;
    Ok(())
}

/// The run condition of [`DrawSystems`]. It covers the whole set rather than single systems
/// (rule 3): either every contributor renders or none does.
///
/// A real application redrawing on change would also redraw when the terminal is resized.
fn needs_redraw(ui: Res<Ui>) -> bool {
    !ui.redraw_on_change || ui.is_changed()
}

/// The header and footer.
fn draw_chrome(
    mut context: ResMut<RatatuiContext>,
    mut pass: ResMut<FramePass>,
    ui: Res<Ui>,
    mut frames: Local<u32>,
) -> Result {
    let panes = Panes::split(begin_frame(&mut context, &mut pass)?);
    *frames = frames.wrapping_add(1);
    render_chrome(&ui, *frames, &panes, context.current_buffer_mut());
    Ok(())
}

/// The list of notes, a stateful widget whose scroll offset this system owns.
fn draw_sidebar(
    mut context: ResMut<RatatuiContext>,
    mut pass: ResMut<FramePass>,
    ui: Res<Ui>,
    mut list: Local<ListState>,
) -> Result {
    let panes = Panes::split(begin_frame(&mut context, &mut pass)?);
    render_sidebar(&ui, &mut list, panes.sidebar, context.current_buffer_mut());
    Ok(())
}

/// The selected note, and the cursor in it.
fn draw_editor(
    mut context: ResMut<RatatuiContext>,
    mut pass: ResMut<FramePass>,
    ui: Res<Ui>,
) -> Result {
    let panes = Panes::split(begin_frame(&mut context, &mut pass)?);
    let cursor = render_editor(&ui, panes.editor, context.current_buffer_mut());
    // Rule 4: ask for the cursor through the pass. Only overwrite it when this system wants it,
    // so that another contributor's request survives.
    if cursor.is_some() {
        pass.cursor = cursor;
    }
    Ok(())
}

/// The regions the contributors agree on. Each one computes them from the frame's area, so
/// they need no shared state beyond [`FramePass`].
struct Panes {
    header: Rect,
    sidebar: Rect,
    editor: Rect,
    footer: Rect,
}

impl Panes {
    fn split(area: Rect) -> Self {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let [sidebar, editor] =
            Layout::horizontal([Constraint::Length(18), Constraint::Fill(1)]).areas(body);
        Self {
            header,
            sidebar,
            editor,
            footer,
        }
    }
}

fn render_chrome(ui: &Ui, frames: u32, panes: &Panes, buf: &mut Buffer) {
    let on_off = |on| if on { "on" } else { "off" };
    let redraw = if ui.redraw_on_change {
        "on change"
    } else {
        "every frame"
    };
    Line::from(format!(
        " multi_draw | frame {frames} | redraw: {redraw} | cursor: {}",
        on_off(ui.cursor),
    ))
    .style(Modifier::REVERSED)
    .render(panes.header, buf);
    Line::from(" Up/Down note | Left/Right cursor | C cursor | P redraw | Q quit")
        .style(Modifier::DIM)
        .render(panes.footer, buf);
}

fn render_sidebar(ui: &Ui, list: &mut ListState, area: Rect, buf: &mut Buffer) {
    list.select(Some(ui.selected));
    let notes = List::new(NOTES.iter().map(|note| note.title))
        .block(Block::bordered().title(" Notes "))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    StatefulWidget::render(notes, area, buf, list);
}

/// Renders the selected note and returns where its cursor belongs, if it is shown.
fn render_editor(ui: &Ui, area: Rect, buf: &mut Buffer) -> Option<Position> {
    let block = Block::bordered().title(" Editor ");
    let inner = block.inner(area);
    Paragraph::new(ui.note().body)
        .block(block)
        .render(area, buf);
    let last_column = inner.width.checked_sub(1)?;
    (ui.cursor && inner.height > 0)
        .then(|| Position::new(inner.x.saturating_add(ui.column.min(last_column)), inner.y))
}

struct Note {
    title: &'static str,
    body: &'static str,
}

const NOTES: [Note; 3] = [
    Note {
        title: "Groceries",
        body: "Tea, oat milk, two lemons.",
    },
    Note {
        title: "Ideas",
        body: "A terminal that is also a window.",
    },
    Note {
        title: "Todo",
        body: "Draw once per frame.",
    },
];

/// The state every contributor reads. Input updates it when a handled key is pressed, so that
/// `is_changed` drives redrawing on change.
#[derive(Resource)]
struct Ui {
    /// The index of the selected note in [`NOTES`].
    selected: usize,
    /// The cursor's column in the note's text.
    column: u16,
    /// Whether the editor shows the cursor.
    cursor: bool,
    /// Draw only on frames in which this resource changed, rather than on every frame.
    redraw_on_change: bool,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            selected: 0,
            column: 0,
            cursor: true,
            redraw_on_change: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Action {
    Previous,
    Next,
    Left,
    Right,
    ToggleCursor,
    ToggleRedraw,
}

impl Ui {
    fn note(&self) -> &'static Note {
        const EMPTY: Note = Note {
            title: "",
            body: "",
        };
        NOTES.get(self.selected).unwrap_or(&EMPTY)
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Previous => self.selected = self.selected.saturating_sub(1),
            Action::Next => self.selected = (self.selected + 1).min(NOTES.len().saturating_sub(1)),
            Action::Left => self.column = self.column.saturating_sub(1),
            Action::Right => self.column = self.column.saturating_add(1),
            Action::ToggleCursor => self.cursor = !self.cursor,
            Action::ToggleRedraw => self.redraw_on_change = !self.redraw_on_change,
        }
        // Keep the cursor on the note's text, or just past its end.
        let end = u16::try_from(self.note().body.chars().count()).unwrap_or(u16::MAX);
        self.column = self.column.min(end);
    }
}

#[cfg(not(feature = "windowed"))]
fn terminal_input(
    mut keys: MessageReader<KeyMessage>,
    mut ui: ResMut<Ui>,
    mut exit: MessageWriter<AppExit>,
) {
    use ratatui::crossterm::event::{KeyCode, KeyEventKind};
    for key in keys.read() {
        // Terminals with the Kitty keyboard protocol also report key releases.
        if key.kind == KeyEventKind::Release {
            continue;
        }
        let action = match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                exit.write_default();
                continue;
            }
            KeyCode::Up => Action::Previous,
            KeyCode::Down => Action::Next,
            KeyCode::Left => Action::Left,
            KeyCode::Right => Action::Right,
            KeyCode::Char('c') => Action::ToggleCursor,
            KeyCode::Char('p') => Action::ToggleRedraw,
            _ => continue,
        };
        ui.apply(action);
    }
}

#[cfg(feature = "windowed")]
fn window_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<Ui>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.any_just_pressed([KeyCode::KeyQ, KeyCode::Escape]) {
        exit.write_default();
    }
    for (key, action) in [
        (KeyCode::ArrowUp, Action::Previous),
        (KeyCode::ArrowDown, Action::Next),
        (KeyCode::ArrowLeft, Action::Left),
        (KeyCode::ArrowRight, Action::Right),
        (KeyCode::KeyC, Action::ToggleCursor),
        (KeyCode::KeyP, Action::ToggleRedraw),
    ] {
        if keys.just_pressed(key) {
            ui.apply(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings, ScheduleLabel};
    use ratatui::backend::TestBackend;

    use super::*;

    /// Text only the sidebar draws, and text only the editor draws.
    const SIDEBAR: &str = " Notes ";
    const EDITOR: &str = "Tea, oat milk, two lemons.";
    const FOOTER: &str = "Up/Down note";

    /// One frame of the pattern on any backend: the contributors in the order of
    /// [`DrawSystems`], then [`present`].
    fn frame<B: Backend>(
        terminal: &mut Terminal<B>,
        pass: &mut FramePass,
        ui: &Ui,
    ) -> Result<Rect, B::Error> {
        let panes = Panes::split(begin_frame(terminal, pass)?);
        render_chrome(ui, 0, &panes, terminal.current_buffer_mut());
        let panes = Panes::split(begin_frame(terminal, pass)?);
        render_sidebar(
            ui,
            &mut ListState::default(),
            panes.sidebar,
            terminal.current_buffer_mut(),
        );
        let area = begin_frame(terminal, pass)?;
        let cursor = render_editor(ui, Panes::split(area).editor, terminal.current_buffer_mut());
        if cursor.is_some() {
            pass.cursor = cursor;
        }
        present(terminal, pass)?;
        Ok(area)
    }

    /// What the backend shows, row by row with newlines so failure output stays readable.
    fn screen(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let width = usize::from(buffer.area.width);
        let mut out = String::new();
        for (index, cell) in buffer.content().iter().enumerate() {
            if index > 0 && width > 0 && index % width == 0 {
                out.push('\n');
            }
            out.push_str(cell.symbol());
        }
        out
    }

    #[test]
    fn contributors_compose_one_frame_where_two_draws_overwrite() -> Result {
        let ui = Ui::default();

        // Two `draw` calls: the second starts from a cleared buffer and erases the first.
        let mut twice = Terminal::new(TestBackend::new(60, 8))?;
        twice.draw(|frame| {
            let area = Panes::split(frame.area()).sidebar;
            render_sidebar(&ui, &mut ListState::default(), area, frame.buffer_mut());
        })?;
        assert!(screen(&twice).contains(SIDEBAR));
        twice.draw(|frame| {
            let area = Panes::split(frame.area()).editor;
            render_editor(&ui, area, frame.buffer_mut());
        })?;
        assert!(!screen(&twice).contains(SIDEBAR) && screen(&twice).contains(EDITOR));

        // The pattern: every region is on screen, with the cursor where the editor asked.
        let mut terminal = Terminal::new(TestBackend::new(60, 8))?;
        let mut pass = FramePass::default();
        frame(&mut terminal, &mut pass, &ui)?;
        assert!(!pass.started && pass.cursor.is_none());
        let composed = screen(&terminal);
        for text in ["multi_draw", SIDEBAR, EDITOR, FOOTER] {
            assert!(composed.contains(text), "{text:?} is missing");
        }
        let editor = Panes::split(Rect::new(0, 0, 60, 8)).editor;
        assert!(terminal.backend().cursor_visible());
        assert_eq!(
            terminal.backend().cursor_position(),
            Position::new(editor.x + 1, editor.y + 1)
        );

        // An idle frame: no contributor ran, so presenting changes nothing, cursor included.
        present(&mut terminal, &mut pass)?;
        assert_eq!(screen(&terminal), composed);
        assert!(terminal.backend().cursor_visible());

        // A frame in which nobody asks for the cursor hides it.
        let hidden = Ui {
            cursor: false,
            ..Ui::default()
        };
        frame(&mut terminal, &mut pass, &hidden)?;
        assert!(!terminal.backend().cursor_visible());
        assert!(screen(&terminal).contains(EDITOR));

        // After a resize, `autoresize` waits for the next contributor; once it runs, every
        // region is drawn cleanly into the new dimensions.
        terminal.backend_mut().resize(70, 12);
        assert_eq!(terminal.current_buffer_mut().area, Rect::new(0, 0, 60, 8));
        assert_eq!(
            frame(&mut terminal, &mut pass, &ui)?,
            Rect::new(0, 0, 70, 12)
        );
        let resized_area = Rect::new(0, 0, 70, 12);
        assert_eq!(terminal.backend().buffer().area, resized_area);
        let mut expected = Buffer::empty(resized_area);
        let panes = Panes::split(resized_area);
        render_chrome(&ui, 0, &panes, &mut expected);
        render_sidebar(&ui, &mut ListState::default(), panes.sidebar, &mut expected);
        render_editor(&ui, panes.editor, &mut expected);
        assert_eq!(terminal.backend().buffer(), &expected);
        Ok(())
    }

    /// The example's own systems, on the windowed backend with no window: the grid is what
    /// `WindowedPlugin` would render.
    #[cfg(feature = "windowed")]
    #[test]
    fn an_idle_resize_keeps_the_frame_until_the_next_draw() -> Result {
        fn grid(app: &App) -> &bevy_ratatui::windowed::TerminalGrid {
            app.world().resource::<RatatuiContext>().backend().grid()
        }
        fn resize(app: &mut App, columns: u16, rows: u16) {
            let mut context = app.world_mut().resource_mut::<RatatuiContext>();
            Terminal::backend_mut(&mut context).resize(columns, rows);
        }

        let mut app = App::new();
        app.insert_resource(RatatuiContext::init()?)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins(multi_draw);
        app.update();
        let size = grid(&app).size();
        let composed = grid(&app).to_string();
        for text in ["multi_draw", SIDEBAR, EDITOR, FOOTER] {
            assert!(composed.contains(text), "{text:?} is missing");
        }
        let editor = Panes::split(Rect::new(0, 0, size.width, size.height)).editor;
        assert!(grid(&app).cursor_visible());
        assert_eq!(
            grid(&app).cursor_position(),
            bevy_ratatui::windowed::CellPosition::new(editor.x + 1, editor.y + 1)
        );

        // Redraw on change only. The change itself draws one more frame; the next is idle.
        app.world_mut()
            .resource_mut::<Ui>()
            .apply(Action::ToggleRedraw);
        app.update();
        let drawn = grid(&app).to_string();
        assert!(drawn.contains("redraw: on change"));
        app.update();
        assert_eq!(grid(&app).to_string(), drawn);

        // The window shrinks and grows back while no contributor runs: Ratatui's buffer stays at
        // the old size (lazy `autoresize`), and the grid shows what fits of the last frame, then
        // all of it again, without blanking.
        resize(&mut app, 30, 6);
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<RatatuiContext>()
                .current_buffer_mut()
                .area,
            Rect::new(0, 0, size.width, size.height)
        );
        assert!(grid(&app).row_text(0).starts_with(" multi_draw | frame"));
        assert!(grid(&app).to_string().contains(SIDEBAR));
        resize(&mut app, size.width, size.height);
        app.update();
        assert_eq!(grid(&app).to_string(), drawn);

        // The next change draws at the size the grid has by then, and hiding the cursor clears it.
        resize(&mut app, 60, 10);
        {
            let mut ui = app.world_mut().resource_mut::<Ui>();
            ui.apply(Action::Next);
            ui.apply(Action::ToggleCursor);
        }
        app.update();
        assert_eq!(
            (grid(&app).size().width, grid(&app).size().height),
            (60, 10)
        );
        assert!(!grid(&app).cursor_visible());
        assert!(grid(&app).row_text(9).contains(FOOTER));
        let redrawn = grid(&app).to_string();
        for text in ["multi_draw", SIDEBAR, "A terminal that is also a window."] {
            assert!(redrawn.contains(text), "{text:?} is missing after a resize");
        }
        Ok(())
    }

    #[test]
    fn systems_schedule_without_ambiguities() {
        let mut app = App::new();
        #[cfg(feature = "windowed")]
        {
            use bevy_ratatui::windowed::{TerminalSystems, WindowedPlugin};
            // `bevy_terminal` leaves its own two `Build` systems unordered (one for each kind
            // of terminal it draws); that pair is not this example's to order.
            app.add_plugins(WindowedPlugin).configure_sets(
                PostUpdate,
                TerminalSystems::Build.ambiguous_with(TerminalSystems::Build),
            );
        }
        app.add_plugins(multi_draw);

        for label in [Update.intern(), PostUpdate.intern()] {
            app.edit_schedule(label, |schedule| {
                schedule.set_build_settings(ScheduleBuildSettings {
                    ambiguity_detection: LogLevel::Error,
                    ..default()
                });
            });
            // Building the schedule reports ambiguities as an error, without running systems
            // that need a terminal or a window.
            let built = app.world_mut().schedule_scope(label, |world, schedule| {
                schedule
                    .initialize(world)
                    .map_err(|error| error.to_string(schedule.graph(), world))
            });
            assert!(
                built.is_ok(),
                "{label:?}: {}",
                built.err().unwrap_or_default()
            );
        }
    }
}
