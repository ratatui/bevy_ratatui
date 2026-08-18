//! A layout stress test: layouts nested inside layouts, every constraint and flex mode next to
//! each other, and every widget Ratatui ships positioned by a layout.
//!
//! Where the `layout` example shows the pattern to reach for in an app, this one leans on the
//! layout engine as hard as it can from a Bevy app:
//!
//! - The `nesting` page splits recursively, changing direction and constraint mix at every level,
//!   so one frame can end up holding hundreds of nested regions. The depth is adjustable at
//!   runtime.
//! - The `constraints` and `flex` pages put every `Constraint` variant, every `Flex` mode, both
//!   kinds of `Spacing` and a margin side by side, each region labelled with the size it was
//!   given.
//! - The `widgets` page positions every widget in the Ratatui library with nested layouts, down to
//!   a popup centered by a layout and punched out with `Clear`. Only two are left out: the
//!   calendar, which sits behind a Ratatui feature flag, and `Fill`, which is newer than the
//!   Ratatui this crate asks for.
//!
//! Every page is drawn by the same single draw system, from data that ordinary Bevy systems keep
//! in resources. The footer reports how many regions the page's layouts produced. It opens on the
//! `widgets` page; Tab walks through the rest.
//!
//! Keys:
//! - Tab & BackTab, or Left & Right: change page
//! - Up & Down: move the list, table and scroll positions
//! - + & -: change the nesting depth
//! - P: toggle the popup
//! - Q or Esc: quit

use std::time::Duration;

use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    prelude::*,
};
use bevy_ratatui::{RatatuiContext, RatatuiPlugins, event::KeyMessage};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEventKind},
    layout::{Constraint, Flex, Layout, Margin, Rect, Spacing},
    style::{Color, Modifier, Style, Stylize},
    symbols::Marker,
    text::Line,
    widgets::{
        Axis, Bar, BarChart, Block, BorderType, Chart, Clear, Dataset, Gauge, GraphType, LineGauge,
        List, ListState, Padding, Paragraph, RatatuiLogo, RatatuiMascot, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Sparkline, Table, TableState, Tabs, Wrap,
        canvas::{Canvas, Circle, Rectangle},
    },
};

fn main() -> Result<()> {
    color_eyre::install()?;

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1. / 60.,
            ))),
            RatatuiPlugins::default(),
        ))
        .init_resource::<Pages>()
        .init_resource::<Nesting>()
        .init_resource::<Animation>()
        .init_resource::<Widgets>()
        .add_systems(PreUpdate, input_system)
        .add_systems(Update, (animate_system, draw_system).chain())
        .run();

    Ok(())
}

/// Which page is on screen. The tab bar is a layout region like any other.
#[derive(Resource)]
struct Pages {
    selected: usize,
}

impl Pages {
    const TITLES: [&'static str; 4] = ["nesting", "constraints", "flex", "widgets"];

    const NESTING: usize = 0;
    const CONSTRAINTS: usize = 1;
    const FLEX: usize = 2;
    /// The page the app opens on.
    const WIDGETS: usize = 3;

    fn step(&mut self, delta: isize) {
        let count = Self::TITLES.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(count) as usize;
    }
}

impl Default for Pages {
    fn default() -> Self {
        Self {
            selected: Self::WIDGETS,
        }
    }
}

/// How many times the nesting page splits before it draws a leaf.
#[derive(Resource)]
struct Nesting {
    depth: usize,
}

impl Nesting {
    const MAX_DEPTH: usize = 7;
}

impl Default for Nesting {
    fn default() -> Self {
        Self { depth: 4 }
    }
}

/// Data for the widgets that show something moving, fed by a normal system rather than by the
/// draw system.
#[derive(Resource)]
struct Animation {
    elapsed: f32,
    /// Rolling window used by the sparkline and the bar charts.
    samples: Vec<u64>,
    /// Points for the chart.
    points: Vec<(f64, f64)>,
}

impl Animation {
    const SAMPLE_COUNT: usize = 200;

    /// A value that sweeps between 0 and 1, for the gauges.
    fn ratio(&self) -> f64 {
        (f64::from(self.elapsed).sin() * 0.5 + 0.5).clamp(0.0, 1.0)
    }
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            samples: vec![0; Self::SAMPLE_COUNT],
            points: Vec::new(),
        }
    }
}

/// The state of the stateful widgets lives in the ECS, not in the draw system.
#[derive(Resource)]
struct Widgets {
    list: ListState,
    table: TableState,
    scroll: usize,
    popup: bool,
}

impl Default for Widgets {
    fn default() -> Self {
        Self {
            list: ListState::default().with_selected(Some(0)),
            table: TableState::default().with_selected(Some(0)),
            scroll: 0,
            popup: false,
        }
    }
}

impl Widgets {
    /// Move every selection at once, so one pair of keys drives the whole page.
    fn step(&mut self, delta: isize) {
        self.list.select(Some(step_index(
            self.list.selected(),
            delta,
            LIST_ITEMS.len(),
        )));
        self.table.select(Some(step_index(
            self.table.selected(),
            delta,
            TABLE_ROWS.len(),
        )));
        self.scroll = step_index(Some(self.scroll), delta, PROSE.len());
    }
}

fn step_index(current: Option<usize>, delta: isize, len: usize) -> usize {
    let last = len.saturating_sub(1) as isize;
    (current.unwrap_or(0) as isize + delta).clamp(0, last) as usize
}

fn animate_system(time: Res<Time>, mut animation: ResMut<Animation>) {
    animation.elapsed += time.delta_secs();
    let elapsed = f64::from(animation.elapsed);

    let sample = ((elapsed * 3.0).sin() * 0.5 + 0.5) * 100.0;
    animation.samples.push(sample as u64);
    let overflow = animation.samples.len() - Animation::SAMPLE_COUNT;
    animation.samples.drain(..overflow);

    animation.points = (0..Animation::SAMPLE_COUNT)
        .map(|index| {
            let x = index as f64 / 20.0;
            (x, (x + elapsed).sin())
        })
        .collect();
}

fn input_system(
    mut messages: MessageReader<KeyMessage>,
    mut exit: MessageWriter<AppExit>,
    mut pages: ResMut<Pages>,
    mut nesting: ResMut<Nesting>,
    mut widgets: ResMut<Widgets>,
) {
    for message in messages.read() {
        // Terminals that support the kitty keyboard protocol also report key releases.
        if message.kind == KeyEventKind::Release {
            continue;
        }

        match message.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                exit.write_default();
            }
            KeyCode::Tab | KeyCode::Right => pages.step(1),
            KeyCode::BackTab | KeyCode::Left => pages.step(-1),
            KeyCode::Down | KeyCode::Char('j') => widgets.step(1),
            KeyCode::Up | KeyCode::Char('k') => widgets.step(-1),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                nesting.depth = (nesting.depth + 1).min(Nesting::MAX_DEPTH);
            }
            KeyCode::Char('-') => nesting.depth = nesting.depth.saturating_sub(1).max(1),
            KeyCode::Char('p') => widgets.popup = !widgets.popup,
            _ => {}
        }
    }
}

/// The only system that draws: it splits the frame, dispatches the body to the selected page, and
/// puts the popup on top.
fn draw_system(
    mut context: ResMut<RatatuiContext>,
    pages: Res<Pages>,
    nesting: Res<Nesting>,
    animation: Res<Animation>,
    mut widgets: ResMut<Widgets>,
) -> Result {
    context.draw(|frame| {
        let area = frame.area();
        let [tabs, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        render_tabs(frame, tabs, &pages);

        let mut regions = match pages.selected {
            Pages::CONSTRAINTS => render_constraints(frame, body),
            Pages::FLEX => render_flex(frame, body),
            Pages::NESTING => render_nesting(frame, body, nesting.depth, ""),
            _ => render_widgets(frame, body, &animation, &mut widgets),
        };

        if widgets.popup {
            regions += render_popup(frame, body);
        }

        render_footer(frame, footer, area, &pages, &nesting, regions);
    })?;

    Ok(())
}

fn render_tabs(frame: &mut Frame, area: Rect, pages: &Pages) {
    let tabs = Tabs::new(Pages::TITLES)
        .select(pages.selected)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .divider("│");

    frame.render_widget(tabs, area);
}

fn render_footer(
    frame: &mut Frame,
    area: Rect,
    terminal: Rect,
    pages: &Pages,
    nesting: &Nesting,
    regions: usize,
) {
    let keys = match pages.selected {
        Pages::NESTING => "tab page  +/- depth  q quit",
        Pages::WIDGETS => "tab page  up/down select  p popup  q quit",
        _ => "tab page  q quit",
    };
    frame.render_widget(Line::raw(keys), area);

    let status = if pages.selected == Pages::NESTING {
        format!(
            "{}x{}  depth {}  {regions} regions",
            terminal.width, terminal.height, nesting.depth,
        )
    } else {
        format!("{}x{}  {regions} regions", terminal.width, terminal.height)
    };

    // Two widgets share this row, so check there is room for both before drawing the second one.
    if area.width as usize >= keys.len() + status.len() + 2 {
        frame.render_widget(Line::raw(status).right_aligned(), area);
    }
}

/// Split the area again and again, changing direction and constraint mix at every level, and
/// return the number of leaves drawn.
///
/// This uses `Layout::split` rather than `Layout::areas` because the number of regions is decided
/// at runtime. The `Rc<[Rect]>` it hands back is fine as long as it stays inside the frame; only
/// holding on to it, in a resource or a component, is a problem.
fn render_nesting(frame: &mut Frame, area: Rect, depth: usize, path: &str) -> usize {
    // Stop splitting once a region can no longer hold a border and a label.
    if depth == 0 || area.width < 10 || area.height < 4 {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::raw(path).centered());
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Line::raw(format!("{}x{}", area.width, area.height))
                .centered()
                .dim(),
            inner,
        );

        return 1;
    }

    let layout = match depth % 4 {
        0 => Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]),
        1 => Layout::vertical([Constraint::Percentage(40), Constraint::Fill(1)]),
        2 => Layout::horizontal([Constraint::Ratio(1, 3), Constraint::Fill(2)]),
        _ => Layout::vertical([
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ]),
    };

    let mut leaves = 0;
    for (index, region) in layout.split(area).iter().enumerate() {
        leaves += render_nesting(frame, *region, depth - 1, &format!("{path}{index}"));
    }

    leaves
}

fn render_constraints(frame: &mut Frame, area: Rect) -> usize {
    let rows = [
        (
            "Length",
            Layout::horizontal([
                Constraint::Length(12),
                Constraint::Length(20),
                Constraint::Length(28),
            ]),
        ),
        (
            "Percentage",
            Layout::horizontal([
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Percentage(50),
            ]),
        ),
        (
            "Ratio",
            Layout::horizontal([
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 2),
            ]),
        ),
        (
            "Min",
            Layout::horizontal([
                Constraint::Min(10),
                Constraint::Min(20),
                Constraint::Min(30),
            ]),
        ),
        (
            "Max",
            Layout::horizontal([
                Constraint::Max(10),
                Constraint::Max(20),
                Constraint::Max(30),
            ]),
        ),
        (
            "Fill",
            Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Fill(2),
                Constraint::Fill(3),
            ]),
        ),
        (
            "mixed",
            Layout::horizontal([
                Constraint::Length(10),
                Constraint::Fill(1),
                Constraint::Max(24),
                Constraint::Percentage(20),
            ]),
        ),
    ];

    render_demo_rows(frame, area, &rows)
}

fn render_flex(frame: &mut Frame, area: Rect) -> usize {
    let cells = [
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ];
    let rows = [
        ("Legacy", Layout::horizontal(cells).flex(Flex::Legacy)),
        ("Start", Layout::horizontal(cells).flex(Flex::Start)),
        ("Center", Layout::horizontal(cells).flex(Flex::Center)),
        ("End", Layout::horizontal(cells).flex(Flex::End)),
        (
            "SpaceBetween",
            Layout::horizontal(cells).flex(Flex::SpaceBetween),
        ),
        (
            "SpaceAround",
            Layout::horizontal(cells).flex(Flex::SpaceAround),
        ),
        (
            "SpaceEvenly",
            Layout::horizontal(cells).flex(Flex::SpaceEvenly),
        ),
        (
            "Space(4)",
            Layout::horizontal(cells)
                .flex(Flex::Center)
                .spacing(Spacing::Space(4)),
        ),
        (
            "Overlap(1)",
            Layout::horizontal(cells)
                .flex(Flex::Center)
                .spacing(Spacing::Overlap(1)),
        ),
        (
            "margin(4)",
            Layout::horizontal(cells)
                .flex(Flex::SpaceBetween)
                .horizontal_margin(4),
        ),
    ];

    // Two levels before the demos even start: the page is halved, then each half holds its rows.
    let [left, right] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
        .spacing(Spacing::Space(1))
        .areas(area);
    let (first, second) = rows.split_at(rows.len() / 2);

    render_demo_rows(frame, left, first) + render_demo_rows(frame, right, second)
}

/// Draw one row per layout: a label on the left, then the regions that layout produces, each
/// showing the width it was given.
fn render_demo_rows(frame: &mut Frame, area: Rect, rows: &[(&str, Layout)]) -> usize {
    // Rows are three high when the page can afford it and two high when it cannot, so the demo
    // adapts to the terminal instead of dropping rows off the bottom.
    let row_height = if area.height >= rows.len() as u16 * 3 {
        3
    } else {
        2
    };
    let row_areas = Layout::vertical(rows.iter().map(|_| Constraint::Length(row_height)))
        .flex(Flex::SpaceAround)
        .split(area);

    let mut regions = 0;
    for (row, (label, layout)) in row_areas.iter().zip(rows) {
        let [label_area, demo_area] =
            Layout::horizontal([Constraint::Length(13), Constraint::Fill(1)]).areas(*row);
        frame.render_widget(Line::raw(*label).bold(), label_area);

        for region in layout.split(demo_area).iter() {
            let block = Block::bordered().title(Line::raw(region.width.to_string()).centered());
            frame.render_widget(block, *region);
            regions += 1;
        }
    }

    regions
}

const LIST_ITEMS: [&str; 8] = [
    "Length",
    "Percentage",
    "Ratio",
    "Min",
    "Max",
    "Fill",
    "Flex",
    "Spacing",
];

const TABLE_ROWS: [[&str; 3]; 6] = [
    ["Block", "no", "borders, titles, padding"],
    ["List", "yes", "ListState"],
    ["Table", "yes", "TableState"],
    ["Scrollbar", "yes", "ScrollbarState"],
    ["Chart", "no", "datasets and axes"],
    ["Canvas", "no", "shapes in braille"],
];

const PROSE: [&str; 13] = [
    "Every widget on this page is positioned by a layout, and several of the cells are split",
    "again on the inside.",
    "",
    "A Layout owns its constraints, so it can be built once and reused, and splitting is cached",
    "by Ratatui.",
    "",
    "Layout::areas returns a fixed size array that destructures into named regions, while",
    "Layout::split returns an Rc<[Rect]> for cases where the count is only known at runtime.",
    "",
    "The widgets missing here are the calendar, behind a Ratatui feature flag, and Fill, which",
    "is newer than the Ratatui this crate asks for.",
    "",
    "Press p for a popup centered by a layout and cleared out of the frame below it.",
];

/// Every widget Ratatui ships, placed by nested layouts.
fn render_widgets(
    frame: &mut Frame,
    area: Rect,
    animation: &Animation,
    widgets: &mut Widgets,
) -> usize {
    let [top, middle, bottom] = Layout::vertical([
        Constraint::Fill(2),
        Constraint::Fill(2),
        Constraint::Fill(3),
    ])
    .areas(area);

    let [prose, list, table] = Layout::horizontal([
        Constraint::Fill(2),
        Constraint::Length(18),
        Constraint::Fill(2),
    ])
    .areas(top);

    let [gauges, sparkline, bars] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ])
    .areas(middle);

    let [chart, canvas, logos] = Layout::horizontal([
        Constraint::Fill(2),
        Constraint::Fill(2),
        Constraint::Length(34),
    ])
    .areas(bottom);

    9 + render_prose(frame, prose, widgets)
        + render_list(frame, list, widgets)
        + render_table(frame, table, widgets)
        + render_gauges(frame, gauges, animation)
        + render_sparkline(frame, sparkline, animation)
        + render_bars(frame, bars, animation)
        + render_chart(frame, chart, animation)
        + render_canvas(frame, canvas, animation)
        + render_logos(frame, logos)
}

/// A paragraph with a scrollbar drawn over the right edge of the same region.
fn render_prose(frame: &mut Frame, area: Rect, widgets: &Widgets) -> usize {
    let paragraph = Paragraph::new(PROSE.map(Line::raw).to_vec())
        .wrap(Wrap { trim: true })
        .scroll((widgets.scroll as u16, 0))
        .block(
            Block::bordered()
                .title(" Paragraph ")
                .padding(Padding::horizontal(1)),
        );
    frame.render_widget(paragraph, area);

    let mut state = ScrollbarState::new(PROSE.len()).position(widgets.scroll);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
        area.inner(Margin::new(0, 1)),
        &mut state,
    );

    1
}

fn render_list(frame: &mut Frame, area: Rect, widgets: &mut Widgets) -> usize {
    let list = List::new(LIST_ITEMS.map(Line::raw))
        .block(Block::bordered().title(" List "))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, area, &mut widgets.list);

    1
}

fn render_table(frame: &mut Frame, area: Rect, widgets: &mut Widgets) -> usize {
    let table = Table::new(
        TABLE_ROWS.map(Row::new),
        [
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Fill(1),
        ],
    )
    .header(Row::new(["widget", "stateful", "notes"]).bold())
    .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
    .block(Block::bordered().title(" Table "));
    frame.render_stateful_widget(table, area, &mut widgets.table);

    1
}

/// Three widgets stacked inside one cell by another layout.
fn render_gauges(frame: &mut Frame, area: Rect, animation: &Animation) -> usize {
    let block = Block::bordered().title(" Gauge · LineGauge ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [gauge, line_gauge, thick] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .spacing(Spacing::Space(1))
    .areas(inner);

    frame.render_widget(
        Gauge::default()
            .ratio(animation.ratio())
            .gauge_style(Color::Cyan),
        gauge,
    );
    frame.render_widget(
        LineGauge::default()
            .ratio(animation.ratio())
            .filled_style(Color::Magenta),
        line_gauge,
    );
    frame.render_widget(
        LineGauge::default()
            .ratio(animation.ratio())
            .filled_symbol("━")
            .unfilled_symbol("─")
            .filled_style(Color::Yellow),
        thick,
    );

    3
}

fn render_sparkline(frame: &mut Frame, area: Rect, animation: &Animation) -> usize {
    let sparkline = Sparkline::default()
        .data(&animation.samples)
        .max(100)
        .style(Color::Green)
        .block(Block::bordered().title(" Sparkline "));
    frame.render_widget(sparkline, area);

    1
}

/// Both bar chart directions, side by side inside one cell.
fn render_bars(frame: &mut Frame, area: Rect, animation: &Animation) -> usize {
    let block = Block::bordered().title(" BarChart ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [vertical, horizontal] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(inner);

    let bars: Vec<Bar> = animation
        .samples
        .iter()
        .rev()
        .step_by(16)
        .take(5)
        .enumerate()
        .map(|(index, value)| {
            Bar::new(*value)
                .label(Line::raw(index.to_string()))
                .style(Color::Blue)
        })
        .collect();

    frame.render_widget(
        BarChart::vertical(bars.clone())
            .max(100)
            .bar_width(3)
            .bar_gap(1),
        vertical,
    );
    frame.render_widget(BarChart::horizontal(bars).max(100).bar_width(1), horizontal);

    2
}

fn render_chart(frame: &mut Frame, area: Rect, animation: &Animation) -> usize {
    let dataset = Dataset::default()
        .name("sin")
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Color::Cyan)
        .data(&animation.points);

    let chart = Chart::new(vec![dataset])
        .block(Block::bordered().title(" Chart "))
        .x_axis(
            Axis::default()
                .bounds([0.0, 10.0])
                .labels(["0", "10"])
                .style(Color::DarkGray),
        )
        .y_axis(
            Axis::default()
                .bounds([-1.0, 1.0])
                .labels(["-1", "1"])
                .style(Color::DarkGray),
        );
    frame.render_widget(chart, area);

    1
}

fn render_canvas(frame: &mut Frame, area: Rect, animation: &Animation) -> usize {
    let angle = f64::from(animation.elapsed);
    let canvas = Canvas::default()
        .block(Block::bordered().title(" Canvas "))
        .marker(Marker::Braille)
        .x_bounds([-1.5, 1.5])
        .y_bounds([-1.5, 1.5])
        .paint(move |ctx| {
            ctx.draw(&Rectangle {
                x: -1.4,
                y: -1.4,
                width: 2.8,
                height: 2.8,
                color: Color::DarkGray,
            });
            ctx.draw(&Circle {
                x: 0.0,
                y: 0.0,
                radius: 1.0,
                color: Color::Blue,
            });
            ctx.draw(&Circle {
                x: angle.cos(),
                y: angle.sin(),
                radius: 0.2,
                color: Color::Yellow,
            });
        });
    frame.render_widget(canvas, area);

    1
}

fn render_logos(frame: &mut Frame, area: Rect) -> usize {
    let block = Block::bordered().title(" Logo · Mascot ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [logo, mascot] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);

    frame.render_widget(RatatuiLogo::small(), logo);
    frame.render_widget(RatatuiMascot::new(), mascot);

    2
}

/// Centering with layouts: one `Flex::Center` split per axis, then `Clear` to punch a hole in
/// whatever the page already drew.
fn render_popup(frame: &mut Frame, area: Rect) -> usize {
    let [area] = Layout::horizontal([Constraint::Percentage(60)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(7)])
        .flex(Flex::Center)
        .areas(area);

    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw("This popup is centered by two layouts and drawn over the page."),
            Line::raw(""),
            Line::raw("Press p to close it.").dim(),
        ])
        .wrap(Wrap { trim: true })
        .block(
            Block::bordered()
                .border_type(BorderType::Double)
                .title(" Clear ")
                .padding(Padding::uniform(1)),
        ),
        area,
    );

    2
}
