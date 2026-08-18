//! This example demonstrates:
//!
//! - Building Ratatui `Layout`s once and keeping them in a resource. A `Layout` owns its
//!   constraints and is `Send + Sync`, so it lives happily in the ECS. The `Rc<[Rect]>` that
//!   `Layout::split` returns is neither, which is the usual stumbling block when trying to build a
//!   layout in a startup system and use it later.
//! - Using `Layout::areas` rather than `Layout::split`: it returns a plain `[Rect; N]` that can be
//!   destructured into named regions, with no reference counting or lifetimes to work around.
//! - Splitting the frame inside a single draw system, then handing each region to a helper
//!   function or to a widget that reads its data from the ECS. Ratatui is double buffered, so one
//!   frame is drawn by one `draw` call; systems contribute the data for that call rather than each
//!   drawing a frame of their own.
//! - Storing the computed areas in a resource so systems that do not draw (input handling, mouse
//!   hit testing, ...) can use them too. `Rect` is `Copy + Send + Sync`.
//!
//! Keys:
//! - Up & Down: change the selected body
//! - PageUp & PageDown: move by one screen of the list
//! - Q or Esc: quit

use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    prelude::*,
};
use bevy_ratatui::{RatatuiContext, RatatuiPlugins, event::KeyMessage};
use ratatui::{
    Frame,
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, List, ListState, Paragraph, Widget, Wrap},
};

fn main() -> Result<()> {
    color_eyre::install()?;

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
                std::time::Duration::from_secs_f32(1. / 60.),
            )),
            RatatuiPlugins::default(),
        ))
        .init_resource::<AppLayout>()
        .init_resource::<Areas>()
        .init_resource::<Selection>()
        .add_systems(Startup, setup_system)
        .add_systems(PreUpdate, input_system)
        .add_systems(Update, draw_system)
        .run();

    Ok(())
}

/// The layouts used to split the frame, built once and reused every frame.
///
/// Splitting is cached by Ratatui, so calling `areas` on these each frame is cheap.
#[derive(Resource)]
struct AppLayout {
    /// Header, everything else, footer.
    outer: Layout,
    /// A fixed width sidebar beside a column that takes the remaining width.
    columns: Layout,
    /// Layouts nest by splitting one of the areas that a previous split produced.
    column: Layout,
}

impl Default for AppLayout {
    fn default() -> Self {
        Self {
            outer: Layout::vertical([
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
            ]),
            columns: Layout::horizontal([Constraint::Length(24), Constraint::Fill(1)]),
            column: Layout::vertical([Constraint::Fill(1), Constraint::Length(7)]),
        }
    }
}

/// The regions produced by the most recent draw.
///
/// `Layout::split` hands back an `Rc<[Rect]>` that cannot be stored in a resource, but the `Rect`s
/// themselves are `Copy + Send + Sync`, so the areas can be saved for other systems to read.
#[derive(Resource, Default)]
struct Areas {
    header: Rect,
    list: Rect,
    detail: Rect,
    regions: Rect,
    footer: Rect,
}

/// Which entry of the sidebar list is selected, and how far the list is scrolled.
#[derive(Resource, Default, Deref, DerefMut)]
struct Selection(ListState);

impl Selection {
    /// Move the selection by `delta` entries, clamped to the number of bodies.
    fn step(&mut self, delta: isize, count: usize) {
        let Some(last) = count.checked_sub(1) else {
            return;
        };
        let current = self.selected().unwrap_or(0) as isize;
        let next = current.saturating_add(delta).clamp(0, last as isize);
        self.select(Some(next as usize));
    }
}

/// The panes are filled from the ECS, so the bodies listed in the sidebar are entities.
#[derive(Component)]
struct Body {
    name: &'static str,
    kind: &'static str,
    distance_au: f32,
    summary: &'static str,
}

fn setup_system(mut commands: Commands, mut selection: ResMut<Selection>) {
    commands.spawn_batch([
        Body {
            name: "Mercury",
            kind: "terrestrial",
            distance_au: 0.39,
            summary: "The smallest planet and the closest to the Sun, with almost no atmosphere \
                      and a day longer than its year.",
        },
        Body {
            name: "Venus",
            kind: "terrestrial",
            distance_au: 0.72,
            summary: "Wrapped in a thick carbon dioxide atmosphere that makes it the hottest \
                      planet in the solar system.",
        },
        Body {
            name: "Earth",
            kind: "terrestrial",
            distance_au: 1.00,
            summary: "The only planet known to support life, and the only one with liquid water \
                      on its surface.",
        },
        Body {
            name: "Mars",
            kind: "terrestrial",
            distance_au: 1.52,
            summary: "The red planet, home to the tallest volcano and the deepest canyon in the \
                      solar system.",
        },
        Body {
            name: "Jupiter",
            kind: "gas giant",
            distance_au: 5.20,
            summary: "The largest planet, massive enough that the solar system's barycenter sits \
                      outside the Sun itself.",
        },
        Body {
            name: "Saturn",
            kind: "gas giant",
            distance_au: 9.58,
            summary: "Known for its bright ring system, made almost entirely of water ice.",
        },
        Body {
            name: "Uranus",
            kind: "ice giant",
            distance_au: 19.20,
            summary: "Tipped on its side, so each pole spends decades in sunlight and decades in \
                      darkness.",
        },
        Body {
            name: "Neptune",
            kind: "ice giant",
            distance_au: 30.05,
            summary: "The windiest planet, with storms that reach supersonic speeds.",
        },
        Body {
            name: "Pluto",
            kind: "dwarf planet",
            distance_au: 39.48,
            summary: "Reclassified as a dwarf planet in 2006, it orbits in the Kuiper belt with \
                      its large moon Charon.",
        },
    ]);

    selection.select_first();
}

fn input_system(
    mut messages: MessageReader<KeyMessage>,
    mut exit: MessageWriter<AppExit>,
    mut selection: ResMut<Selection>,
    areas: Res<Areas>,
    bodies: Query<&Body>,
) {
    let count = bodies.iter().count();

    // Paging needs to know how tall the list is, which is exactly the kind of question a system
    // that never touches the frame can answer with the saved areas. The list is drawn inside a
    // bordered block, so the top and bottom rows are not available for entries.
    let page = areas.list.height.saturating_sub(2).max(1) as isize;

    for message in messages.read() {
        // Terminals that support the kitty keyboard protocol also report key releases.
        if message.kind == KeyEventKind::Release {
            continue;
        }

        match message.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                exit.write_default();
            }
            KeyCode::Down | KeyCode::Char('j') => selection.step(1, count),
            KeyCode::Up | KeyCode::Char('k') => selection.step(-1, count),
            KeyCode::PageDown => selection.step(page, count),
            KeyCode::PageUp => selection.step(-page, count),
            _ => {}
        }
    }
}

/// The one system that draws. Every region is filled from here, either by a helper function or by
/// a widget, instead of each pane being drawn by a system of its own.
fn draw_system(
    mut context: ResMut<RatatuiContext>,
    layout: Res<AppLayout>,
    mut areas: ResMut<Areas>,
    mut selection: ResMut<Selection>,
    bodies: Query<&Body>,
) -> Result {
    // Query iteration order is not guaranteed, so sort into the order the list is drawn in.
    let mut bodies: Vec<&Body> = bodies.iter().collect();
    bodies.sort_by(|a, b| a.distance_au.total_cmp(&b.distance_au));

    context.draw(|frame| {
        // `areas` returns a fixed size array, so each region can be given a name by destructuring
        // it. Splitting a region again just means calling `areas` on the `Rect` it produced.
        let [header, main, footer] = layout.outer.areas(frame.area());
        let [list, column] = layout.columns.areas(main);
        let [detail, regions] = layout.column.areas(column);

        // Hand the regions to the systems that do not draw.
        *areas = Areas {
            header,
            list,
            detail,
            regions,
            footer,
        };

        render_header(frame, header);
        render_list(frame, list, &bodies, &mut selection);

        // Widgets can be implemented directly on components, which keeps the draw system down to
        // deciding what goes where.
        if let Some(body) = selection.selected().and_then(|index| bodies.get(index)) {
            frame.render_widget(*body, detail);
        }

        render_regions(frame, regions, &areas);
        render_footer(frame, footer, bodies.len());
    })?;

    Ok(())
}

fn render_header(frame: &mut Frame, area: Rect) {
    let title = Line::from("bevy_ratatui layout example".bold()).centered();
    frame.render_widget(title, area);
}

fn render_list(frame: &mut Frame, area: Rect, bodies: &[&Body], selection: &mut ListState) {
    let list = List::new(bodies.iter().map(|body| Line::raw(body.name)))
        .block(Block::bordered().title(" bodies "))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    // The list scrolls itself to keep the selection visible, using the area it is given.
    frame.render_stateful_widget(list, area, selection);
}

impl Widget for &Body {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text = vec![
            Line::from(vec!["type:     ".bold(), self.kind.into()]),
            Line::from(vec![
                "distance: ".bold(),
                format!("{:.2} AU", self.distance_au).into(),
            ]),
            Line::raw(""),
            Line::raw(self.summary),
        ];

        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title(format!(" {} ", self.name)))
            .render(area, buf);
    }
}

/// Prints the regions the layouts produced, so the effect of resizing the terminal is visible.
fn render_regions(frame: &mut Frame, area: Rect, areas: &Areas) {
    let regions = [
        ("header", areas.header),
        ("list", areas.list),
        ("detail", areas.detail),
        ("regions", areas.regions),
        ("footer", areas.footer),
    ];

    let lines: Vec<Line> = regions
        .iter()
        .map(|(name, rect)| {
            Line::raw(format!(
                "{name:<8}{width:>3} x{height:>3} at {x},{y}",
                width = rect.width,
                height = rect.height,
                x = rect.x,
                y = rect.y,
            ))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" regions ")),
        area,
    );
}

/// Two widgets sharing one area: nothing says a region has to hold a single widget.
fn render_footer(frame: &mut Frame, area: Rect, count: usize) {
    frame.render_widget(Line::raw("up/down select  pgup/pgdn page  q quit"), area);
    frame.render_widget(Line::raw(format!("{count} bodies")).right_aligned(), area);
}
