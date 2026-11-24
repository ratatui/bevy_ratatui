//! Mouse input in the windowed backend: `WindowedCursor` is the terminal cell under the mouse.
//!
//! Windowed mode has no mouse messages of its own. Read `WindowedCursor` together with Bevy's
//! `ButtonInput<MouseButton>` to know which cell a click is over.
//!
//! Run with:
//! ```sh
//! cargo run --example windowed_mouse --features windowed
//! ```
//!
//! - Move the mouse: the cell under it is highlighted and named in the title
//! - Left button (hold to drag): paint cells; right button: erase them
//! - `C`: clear, `Q` / `Esc`: quit

use std::collections::HashSet;

use bevy::{app::AppExit, prelude::*};
use bevy_ratatui::{RatatuiContext, RatatuiPlugins, windowed::WindowedCursor};
use ratatui::{
    layout::Position,
    style::{Color, Style},
    widgets::Block,
};

/// The cells painted so far.
#[derive(Resource, Default)]
struct Canvas(HashSet<Position>);

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "bevy_ratatui · windowed mouse".into(),
                    ..default()
                }),
                ..default()
            }),
            RatatuiPlugins::default(),
        ))
        .init_resource::<Canvas>()
        .add_systems(Update, (paint, draw).chain())
        .run()
}

fn paint(
    cursor: Res<WindowedCursor>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut canvas: ResMut<Canvas>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.any_just_pressed([KeyCode::KeyQ, KeyCode::Escape]) {
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        canvas.0.clear();
    }
    // `None` while the mouse is outside the terminal.
    let Some(cell) = cursor.0 else {
        return;
    };
    if buttons.pressed(MouseButton::Left) {
        canvas.0.insert(cell);
    } else if buttons.pressed(MouseButton::Right) {
        canvas.0.remove(&cell);
    }
}

fn draw(
    cursor: Res<WindowedCursor>,
    canvas: Res<Canvas>,
    mut context: ResMut<RatatuiContext>,
) -> Result {
    let title = match cursor.0 {
        Some(cell) => format!(" column {}, row {} ", cell.x, cell.y),
        None => " move the mouse over the terminal ".to_owned(),
    };
    context.draw(|frame| {
        // ASCII only: the default font has nothing else.
        let help = " left: paint | right: erase | C: clear | Q: quit ";
        let block = Block::bordered().title(title).title_bottom(help);
        frame.render_widget(block, frame.area());
        let buffer = frame.buffer_mut();
        // Cells outside the frame, after the window shrank, are skipped.
        for cell in &canvas.0 {
            if let Some(cell) = buffer.cell_mut(*cell) {
                cell.set_style(Style::new().bg(Color::LightCyan));
            }
        }
        if let Some(cell) = cursor.0.and_then(|cell| buffer.cell_mut(cell)) {
            cell.set_style(Style::new().bg(Color::Yellow));
        }
    })?;
    Ok(())
}
